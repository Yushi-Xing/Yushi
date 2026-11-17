#!/usr/bin/env python3
"""在独立 Windows CI 上验安装包，再归档准确度、测试结果与源码指纹。"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]
CHECKS = ROOT / 'target/package-checks'


def prepare(root=ROOT):
    """Rust 缓存可能恢复整个 target；一次性报告与安装器必须重新生成。"""
    for name in ['package-checks', 'package-eval', 'windows-package', 'installer']:
        path = root / 'target' / name
        if path.exists():
            shutil.rmtree(path)
    (root / 'target/package-checks').mkdir(parents=True, exist_ok=True)


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def installer():
    matches = list((ROOT / 'target/installer').glob('qingjian-*-windows-x86_64-setup.exe'))
    if len(matches) != 1:
        raise ValueError(f'expected one installer, found {len(matches)}')
    return matches[0]


def machine(path):
    with path.open('rb') as source:
        assert source.read(2) == b'MZ', path
        source.seek(0x3c)
        offset, = struct.unpack('<I', source.read(4))
        source.seek(offset)
        assert source.read(4) == b'PE\0\0', path
        return struct.unpack('<H', source.read(2))[0]


def seed_preferences(folder):
    """只供一次性 runner：保留非默认设置、手写注释和真实格式的学习文件。"""
    samples = {
        'config.toml': '# Reinstall preservation probe\n[general]\npage_size = 7\nlayout = "horizontal"\n'
                       'theme = "system-blue"\nlearning = true\n[model]\nenabled = false\n[predict]\nenabled = false\n',
        'user.tsv': '吗\t11\n',
        'user-choices.tsv': 'm\t吗\t7\n',
        'user-preferences.tsv': 'm\t吗\n',
        'user-words.tsv': '测试人名\tce shi ren ming\t100\n',
        'user-ngram.tsv': '<s>\t吗\t7\n',
        'user-english.tsv': 'MyProject\t3\n',
        'user-typos.tsv': 'tain\ttian\t4\n',
        '.env': '# Synthetic marker, no credentials\nQINGJIAN_REINSTALL_PROBE=preserved\n',
        'themes/reinstall-probe/keep.txt': 'custom theme assets\n',
        'models/reinstall-probe/keep.txt': 'downloaded model marker\n',
    }
    for name, text in samples.items():
        path = folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            raise RuntimeError(f'refusing to overwrite preexisting runner preferences: {path}')
        path.write_bytes(text.encode('utf-8'))
    return {name: sha(folder / name) for name in samples}


def assert_preferences(folder, expected, local=None):
    """同时接受旧布局与启动后迁移的布局，内容必须逐字节保留。"""
    local = local or folder
    for name, fingerprint in expected.items():
        if name.startswith('user') and '.tsv' in name:
            migrated = folder / 'learning' / name
        elif name.startswith('themes/'):
            migrated = folder / 'custom' / name
        elif name.startswith('models/'):
            migrated = local / 'downloads' / name
        else:
            migrated = folder / name
        path = migrated if migrated.is_file() else folder / name
        assert path.is_file() and sha(path) == fingerprint, f'user preference changed or missing: {name}'


def smoke():
    # 安装和注册只允许在一次性的 Windows runner，避免误改开发者的输入法。
    if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true':
        raise RuntimeError('native installation smoke test requires an isolated Windows Actions runner')
    import winreg
    destination = Path(os.environ['RUNNER_TEMP']) / 'qingjian-package-smoke'
    CHECKS.mkdir(parents=True, exist_ok=True)
    source = installer()
    clsid_source = (ROOT / 'apps/windows/tsf/src/com/mod.rs').read_text(encoding='utf-8')
    clsid = re.search(r'CLSID_QINGJIAN_STR: &str = "([^"]+)"', clsid_source)[1]
    key = 'SOFTWARE\\Classes\\CLSID\\' + clsid + '\\InprocServer32'
    processes = []
    preferences = Path(os.environ['APPDATA']) / 'Qingjian'
    local_preferences = Path(os.environ['LOCALAPPDATA']) / 'Qingjian'
    expected_preferences = seed_preferences(preferences)
    report = dict(installer=source.name, installer_sha256=sha(source), architectures={}, registrations={})
    try:
        subprocess.run([str(source), '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-',
                        '/DIR=' + str(destination), '/LOG=' + str(CHECKS / 'install.log')], check=True, timeout=180)
        assert_preferences(preferences, expected_preferences, local_preferences)
        version = source.name.removeprefix('qingjian-').removesuffix('-windows-x86_64-setup.exe')
        binaries = [(name, 'release/' + name, 0x8664) for name in
                    ['qingjian-server.exe', 'qingjian-settings.exe', 'qingjian-clipboard.exe']]
        binaries += [(f'qingjian_tsf-{version}.dll', 'release/qingjian_tsf.dll', 0x8664),
                     (f'qingjian_tsf-{version}-x86.dll', 'i686-pc-windows-msvc/release/qingjian_tsf.dll', 0x14c),
                     ('onnxruntime.dll', 'installer/onnxruntime/onnxruntime.dll', 0x8664)]
        for name, original, expected in binaries:
            path = destination / name
            assert machine(path) == expected, f'wrong architecture: {name}'
            assert sha(path) == sha(ROOT / 'target' / original), f'installed bytes differ: {name}'
            report['architectures'][name] = 'x86_64' if expected == 0x8664 else 'x86'
        for label, view, suffix in [('x64', winreg.KEY_WOW64_64KEY, ''), ('x86', winreg.KEY_WOW64_32KEY, '-x86')]:
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key, 0, winreg.KEY_READ | view) as opened:
                registered = Path(winreg.QueryValueEx(opened, '')[0])
            assert registered == destination / f'qingjian_tsf-{version}{suffix}.dll', registered
            report['registrations'][label] = registered.name
        assert not (destination / 'qingjian-settings-legacy.exe').exists()
        assert not (destination / 'Microsoft.UI.Xaml.dll').exists()
        inventory = json.loads((ROOT / 'target/installer/package-size.json').read_text(encoding='utf-8'))
        included = {row['path'] for row in inventory['files']}
        for path in (ROOT / 'data/generated').rglob('*'):
            if path.is_file() and path.suffix in ['.qj', '.tsv']:
                installed = destination / 'data/generated' / path.relative_to(ROOT / 'data/generated')
                # 数据目录也含开发中间产物，只核对实际打包清单里的文件。
                if path.relative_to(ROOT).as_posix() in included:
                    assert sha(installed) == sha(path), installed
        for name in ['qingjian-server.exe', 'qingjian-settings.exe', 'qingjian-clipboard.exe']:
            processes.append(subprocess.Popen([str(destination / name)], cwd=destination))
        time.sleep(5)
        assert all(p.poll() is None for p in processes), [(p.args, p.returncode) for p in processes]
        report['startup'] = 'server, settings and clipboard alive after five seconds'
        config = CHECKS / 'isolated-settings.toml'
        config.write_text('[predict]\nenabled = false\n', encoding='utf-8')
        snapshots = CHECKS / 'settings-snapshots'
        subprocess.run([str(destination / 'qingjian-settings.exe'), '--config', str(config), '--png', str(snapshots)],
                       check=True, timeout=120)
        assert list(snapshots.rglob('*.png')), 'no settings render output'
        report['settings_render'] = 'passed'
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=30)
        # 直接覆盖重装，以及卸载后再安装，都必须逐字节保留原用户文件。
        for round_name, remove_first in [('overinstall', False), ('reinstall', True)]:
            if remove_first:
                subprocess.run([str(destination / 'unins000.exe'), '/VERYSILENT', '/SUPPRESSMSGBOXES',
                                '/NORESTART'], check=True, timeout=180)
                assert_preferences(preferences, expected_preferences, local_preferences)
            subprocess.run([str(source), '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-',
                            '/DIR=' + str(destination), '/LOG=' + str(CHECKS / f'{round_name}.log')],
                           check=True, timeout=180)
            assert_preferences(preferences, expected_preferences, local_preferences)
        report['preferences'] = dict(status='passed', files=expected_preferences,
                                     operations=['install', 'overinstall', 'uninstall', 'reinstall'])
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=30)
        uninstaller = destination / 'unins000.exe'
        if uninstaller.exists():
            subprocess.run([str(uninstaller), '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART',
                            '/LOG=' + str(CHECKS / 'uninstall.log')], check=True, timeout=180)
    for view in [winreg.KEY_WOW64_64KEY, winreg.KEY_WOW64_32KEY]:
        try:
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key, 0, winreg.KEY_READ | view):
                raise AssertionError('COM registration remains after uninstall')
        except FileNotFoundError:
            pass
    assert not (destination / 'qingjian-server.exe').exists(), 'server remains after uninstall'
    assert_preferences(preferences, expected_preferences, local_preferences)
    report['uninstall'] = 'passed'
    report['status'] = 'passed'
    (CHECKS / 'installer-smoke.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))


def stage():
    source = installer()
    smoke_report = json.loads((CHECKS / 'installer-smoke.json').read_text(encoding='utf-8'))
    assert smoke_report['status'] == 'passed' and smoke_report['installer_sha256'] == sha(source)
    output = ROOT / 'target/windows-package'
    output.mkdir(exist_ok=False)
    shutil.copy2(source, output / source.name)
    shutil.copy2(source.with_name('package-size.json'), output / 'package-size.json')
    summaries = {path.parent.name: json.loads(path.read_text(encoding='utf-8'))
                 for path in (ROOT / 'target/package-eval').glob('*/summary.json')}
    assert len(summaries) == 8, 'incomplete evaluation'
    assert summaries['experience']['status'] == 'passed' and summaries['experience']['regressions'] == []
    assert summaries['names-static']['count'] == 290 and summaries['names-static']['regressions'] == []
    info = dict(commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                workflow_url=f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}",
                platform='x86_64-pc-windows-msvc', x86_tsf=True, code_signed=False, uiaccess=False,
                rust=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                data_locks={name: (ROOT / 'tools/release' / name).read_text(encoding='utf-8')
                            for name in ['dicts.lock', 'dicts-extra.lock', 'models.lock']},
                evaluation=summaries, installer_smoke=smoke_report,
                gui_acceptance='Automated startup/render/install/uninstall only; interactive Win11 search occlusion unverified')
    (output / 'build-info.json').write_text(json.dumps(info, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    with zipfile.ZipFile(output / 'validation-evidence.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for folder in [CHECKS, ROOT / 'target/package-eval']:
            for path in sorted(folder.rglob('*')):
                if path.is_file():
                    archive.write(path, path.relative_to(ROOT / 'target'))
        for path in sorted((ROOT / 'assets/eval/names-sentences').iterdir()):
            archive.write(path, 'names-sentences-corpus/' + path.name)
    files = sorted(output.iterdir())
    (output / 'SHA256SUMS').write_text(''.join(f'{sha(path)}  {path.name}\n' for path in files), encoding='utf-8')
    print('Packaged', source.name)


if __name__ == '__main__':
    {'prepare': prepare, 'smoke': smoke, 'stage': stage}[sys.argv[1]]()
