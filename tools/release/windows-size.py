#!/usr/bin/env python3
"""按 Inno 的实际文件清单统计标准包或兼容包的展开字节，另记压缩安装包大小。"""

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]


def inventory(root, legacy):
    inside, enabled = False, True
    rows = []
    for line in (root / "apps/windows/installer/qingjian.iss").read_text(encoding="utf-8-sig").splitlines():
        if line.startswith("["):
            inside = line == "[Files]"
        if not inside:
            continue
        if line == "#if Int(IncludeLegacySettings)":
            enabled = legacy
            continue
        if line == "#endif":
            enabled = True
            continue
        if line.startswith("#"):
            raise ValueError("unsupported installer condition: " + line)
        if not enabled or not line.startswith("Source:"):
            continue
        match = re.match(r'Source:\s*"\{#Repo\}\\([^"]+)";', line)
        if not match:
            raise ValueError("unsupported installer source: " + line)
        pattern = root / match[1].replace("\\", "/")
        files = []
        for path in sorted(pattern.parent.glob(pattern.name)):
            if path.is_file():
                files.append(path)
            elif path.is_dir() and "recursesubdirs" in line:
                files.extend(sorted(p for p in path.rglob("*") if p.is_file()))
        if not files:
            raise ValueError("missing installer source: " + match[1])
        for path in files:
            relative = path.relative_to(root).as_posix()
            group = ("legacy_settings" if "settings-runtime/" in relative or path.name == "qingjian-settings-legacy.exe"
                     else "onnx_runtime" if "/onnxruntime/" in relative
                     else "data" if relative.startswith("data/")
                     else "binaries" if relative.startswith("target/") else "assets_and_licenses")
            rows.append(dict(path=relative, bytes=path.stat().st_size, group=group))
    return rows


def run(installer, output, legacy):
    files = inventory(ROOT, legacy)
    digest = hashlib.sha256()
    with installer.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    result = dict(schema=1, include_legacy_settings=legacy,
                  installer_bytes=installer.stat().st_size,
                  installer_sha256=digest.hexdigest(),
                  installed_file_bytes=sum(row["bytes"] for row in files),
                  groups={group: sum(row["bytes"] for row in files if row["group"] == group)
                          for group in sorted({row["group"] for row in files})}, files=files)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Installer: {result['installer_bytes']} bytes; installed files: {result['installed_file_bytes']} bytes")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--installer", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--include-legacy-settings", action="store_true")
    args = parser.parse_args()
    run(args.installer, args.output, args.include_legacy_settings)
