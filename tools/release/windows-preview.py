#!/usr/bin/env python3
"""Windows fork 测试版门禁与可核对的发布产物，不写入上游官网索引。"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def version():
    tag = os.environ['PREVIEW_TAG']
    match = re.fullmatch(r'yushi-windows-v(\d+\.\d+\.\d+-(?:alpha|beta|rc)\.\d+)', tag)
    if not match:
        raise ValueError('preview tag must contain an explicit prerelease version')
    release = match[1]
    for app in ['server', 'tsf', 'settings']:
        manifest = (ROOT / f'apps/windows/{app}/Cargo.toml').read_text(encoding='utf-8')
        actual = re.search(r'^version = "([^"]+)"$', manifest, re.MULTILINE)
        if not actual or actual[1] != release:
            raise ValueError(f'{app} version does not match {tag}')
    return release


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def validate():
    release = version()
    subprocess.run(['git', 'merge-base', '--is-ancestor', 'HEAD', 'origin/main'], cwd=ROOT, check=True)
    print('validated preview', release)


def stage():
    release = version()
    installer = ROOT / f'target/installer/qingjian-{release}-windows-x86_64-setup.exe'
    if not installer.is_file() or installer.stat().st_size < 1_000_000:
        raise ValueError('expected installer is missing or unexpectedly small')
    with installer.open('rb') as source:
        if source.read(2) != b'MZ':
            raise ValueError('installer is not a Windows executable')
    output = ROOT / 'target/windows-preview'
    output.mkdir(exist_ok=False)
    shutil.copy2(installer, output / installer.name)
    evaluation = ROOT / 'target/journal-ci'
    summary = json.loads((evaluation / 'summary.json').read_text(encoding='utf-8'))
    if summary['clean']['count'] != 30 or summary['mutations']['count'] != 270 or summary['groups']['fuzzy']['count'] != 6:
        raise ValueError('journal evaluation is incomplete')
    with zipfile.ZipFile(output / 'journal-evaluation.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in ['cases.json', 'normal.tsv', 'fuzzy.tsv', 'normal.jsonl', 'fuzzy.jsonl', 'summary.json', 'results.jsonl', 'normal.log', 'fuzzy.log']:
            archive.write(evaluation / name, f'ci/{name}')
        archive.write(ROOT / 'assets/eval/journal/seeds.json', 'seeds.json')
        archive.write(ROOT / 'assets/eval/journal/README.md', 'README.md')
        for path in sorted((ROOT / 'docs/notes/windows-refactor-round2-eval').glob('*')):
            if path.suffix in ['.json', '.jsonl']:
                archive.write(path, f'local/{path.name}')
    info = dict(version=release, tag=os.environ['PREVIEW_TAG'], commit=os.environ['GITHUB_SHA'],
                workflow_run=os.environ['GITHUB_RUN_ID'], rust=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                architectures=['x86_64-server-settings-tsf', 'i686-tsf'], uiaccess=False, code_signed=False,
                data_lock=(ROOT / 'tools/release/data.lock').read_text(encoding='utf-8'),
                evaluation=summary, gui_acceptance='pending user tests on Win10 and Win11')
    (output / 'build-info.json').write_text(json.dumps(info, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    files = sorted(output.iterdir())
    (output / 'SHA256SUMS').write_text(''.join(f'{sha(path)}  {path.name}\n' for path in files), encoding='utf-8')
    print('staged', ', '.join(path.name for path in output.iterdir()))


if __name__ == '__main__':
    {'validate': validate, 'stage': stage}[sys.argv[1]]()
