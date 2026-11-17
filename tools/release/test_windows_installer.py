#!/usr/bin/env python3
"""用真实 PowerShell 执行打包脚本，以隔离的产物和编译器替身验证两种包及缺文件门禁。"""

import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
POWERSHELL = os.environ.get("QINGJIAN_TEST_POWERSHELL") or shutil.which("pwsh") or shutil.which("powershell")


@unittest.skipUnless(POWERSHELL, "PowerShell is required for installer script integration tests")
class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.installer = self.root / "apps/windows/installer"
        self.installer.mkdir(parents=True)
        for name in ["build.ps1", "qingjian.iss", "settings-runtime.txt"]:
            shutil.copy2(ROOT / "apps/windows/installer" / name, self.installer / name)
        self.write("apps/windows/server/Cargo.toml", '[package]\nversion = "0.3.3"\n')
        for name in ["models-fetch.py", "dicts-fetch.py"]:
            self.write("tools/release/" + name, "# 数据已由测试准备\n")
        shutil.copy2(ROOT / "tools/release/windows-size.py", self.root / "tools/release/windows-size.py")
        self.write("apps/windows/installer/fetch-onnxruntime.ps1", "$global:LASTEXITCODE = 0\n")
        self.write("fake-iscc.ps1", "$args | ConvertTo-Json | Set-Content -Encoding utf8 $env:QINGJIAN_INSTALLER_ARGS\n$global:LASTEXITCODE = 0\n")
        for name in ["dict.qj", "lm.qj", "english.qj", "english-ipa.qj", "vietnamese.qj", "vietnamese-chinese.qj",
                     "dicts/idioms.qj", "dicts/general.qj", "schemes/wubi86.qj", "codes/stroke.qj", "glossary-en.qj", "glossary-zh.qj"]:
            self.write("data/generated/" + name, "fixture")
        for name in ["qingjian_tsf.dll", "qingjian-server.exe", "qingjian-settings.exe", "qingjian-clipboard.exe"]:
            self.write("target/release/" + name, "fixture")
        self.write("target/i686-pc-windows-msvc/release/qingjian_tsf.dll", "fixture")
        self.write("target/installer/qingjian-0.3.3-windows-x86_64-setup.exe", "MZfixture")
        for line in (self.installer / "qingjian.iss").read_text(encoding="utf-8-sig").splitlines():
            match = re.match(r'Source:\s*"\{#Repo\}\\([^"]+)";', line)
            if match and "settings-runtime" not in line and "settings-legacy" not in line:
                self.write(match[1].replace("\\", "/").replace("*", "fixture"), "fixture")

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def package(self, legacy=False, iscc=None):
        args_file = self.root / "iscc-args.json"
        env = dict(os.environ, QINGJIAN_ISCC=iscc or str(self.root / "fake-iscc.ps1"),
                   QINGJIAN_INSTALLER_ARGS=str(args_file), PYTHONUTF8="1", PYTHON=sys.executable)
        args = [POWERSHELL, "-NoProfile", "-File", str(self.installer / "build.ps1"), "-SkipBuild"]
        if legacy:
            args.append("-IncludeLegacySettings")
        result = subprocess.run(args, cwd=self.root, env=env, capture_output=True, text=True, encoding="utf-8", timeout=60)
        return result, args_file

    def test_standard_package_needs_no_legacy_executable_or_runtime(self):
        result, args_file = self.package()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("/DIncludeLegacySettings=0", json.loads(args_file.read_text(encoding="utf-8-sig")))
        self.assertFalse((self.root / "target/installer/settings-runtime").exists())
        size = json.loads((self.root / "target/installer/package-size.json").read_text())
        self.assertNotIn("legacy_settings", size["groups"])
        self.assertEqual(size["installed_file_bytes"], sum(row["bytes"] for row in size["files"]))

    @unittest.skipUnless(os.environ.get("QINGJIAN_TEST_ISCC"), "native Inno Setup is required")
    def test_native_inno_excludes_missing_legacy_files_when_flag_is_zero(self):
        # 命令行的字符串 "0" 直接用在 #if 中会意外启用旧设置，必须经过真实预处理器。
        shutil.copy2(ROOT / "apps/windows/tsf/resources/qingjian.ico",
                     self.root / "apps/windows/tsf/resources/qingjian.ico")
        result, _ = self.package(iscc=os.environ["QINGJIAN_TEST_ISCC"])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        package = self.root / "target/installer/qingjian-0.3.3-windows-x86_64-setup.exe"
        self.assertGreater(package.stat().st_size, 100000)

    def test_compatibility_package_requires_and_stages_the_complete_runtime(self):
        self.write("target/release/qingjian-settings-legacy.exe", "legacy")
        result, args_file = self.package(legacy=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Windows App Runtime", result.stdout + result.stderr)
        self.assertFalse(args_file.exists())
        names = [line.strip() for line in (self.installer / "settings-runtime.txt").read_text(encoding="utf-8").splitlines()
                 if line.strip() and not line.startswith("#")]
        for name in names:
            if "." in name:
                self.write("target/release/" + name, "runtime")
            else:
                self.write("target/release/" + name + "/resources.pri", "locale")
        result, args_file = self.package(legacy=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("/DIncludeLegacySettings=1", json.loads(args_file.read_text(encoding="utf-8-sig")))
        for name in names:
            self.assertTrue((self.root / "target/installer/settings-runtime" / name).exists(), name)
        size = json.loads((self.root / "target/installer/package-size.json").read_text())
        self.assertGreater(size["groups"]["legacy_settings"], 0)

    def test_standard_package_still_requires_32_bit_tsf_and_language_data(self):
        for name in ["target/i686-pc-windows-msvc/release/qingjian_tsf.dll", "data/generated/glossary-en.qj", "data/generated/dicts/general.qj"]:
            path = self.root / name
            previous = path.read_bytes()
            path.unlink()
            result, args_file = self.package()
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(path.name, result.stdout + result.stderr)
            self.assertFalse(args_file.exists())
            path.write_bytes(previous)


if __name__ == "__main__":
    unittest.main()
