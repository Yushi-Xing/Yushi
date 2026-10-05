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
    quality = ROOT / 'target/input-quality-ci'
    quality_summary = json.loads((quality / 'summary.json').read_text(encoding='utf-8'))
    if quality_summary['count'] != 1444 or quality_summary['clean']['count'] != 92 or quality_summary['mutations']['count'] != 1104:
        raise ValueError('pinyin combination evaluation is incomplete')
    with zipfile.ZipFile(output / 'input-quality-evaluation.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in ['cases.json', 'inputs.tsv', 'details.jsonl', 'summary.json', 'results.jsonl', 'eval.log', 'build-info.json', 'config.toml']:
            archive.write(quality / name, f'ci/{name}')
        archive.write(ROOT / 'assets/eval/input-quality/seeds.json', 'seeds.json')
        archive.write(ROOT / 'assets/eval/input-quality/README.md', 'README.md')
        for path in sorted((ROOT / 'docs/notes/windows-refactor-round3-eval').glob('*')):
            if path.suffix in ['.json', '.jsonl']:
                archive.write(path, f'local/{path.name}')
    daily = {}
    lexicon = json.loads((ROOT / 'target/lexicon-ci/summary.json').read_text(encoding='utf-8'))
    if lexicon['count'] != 237 or lexicon['coverage']['count'] != 220 or lexicon['coverage']['entries'] != 220:
        raise ValueError('lexicon coverage evaluation is incomplete')
    with zipfile.ZipFile(output / 'daily-reading-evaluation.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for profile in ['static', 'model']:
            directory = ROOT / f'target/daily-{profile}-ci'
            report = json.loads((directory / 'summary.json').read_text(encoding='utf-8'))
            if report['profile'] != 'full' or report['count'] != 1123 or report['clean']['count'] != 64:
                raise ValueError('daily reading evaluation is incomplete')
            daily[profile] = report
            for name in ['cases.json', 'inputs.tsv', 'details.jsonl', 'summary.json', 'results.jsonl', 'failures.jsonl', 'eval.log', 'config.toml']:
                archive.write(directory / name, f'ci/{profile}/{name}')
        for name in ['cases.json', 'inputs.tsv', 'summary.json', 'results.jsonl', 'failures.jsonl', 'eval.log', 'config.toml']:
            archive.write(ROOT / 'target/lexicon-ci' / name, f'ci/lexicon/{name}')
        for name in ['seeds.json', 'sources.json', 'README.md']:
            archive.write(ROOT / 'assets/eval/daily-reading' / name, f'corpus/{name}')
        for name in ['manifest.json', 'README.md', 'JIEBA-LICENSE.txt', 'UNICODE-LICENSE.txt']:
            archive.write(ROOT / 'assets/lexicon/supplement' / name, f'lexicon/{name}')
        archive.write(ROOT / 'assets/lexicon/00_meta/THUOCL_LICENSE.txt', 'lexicon/THUOCL_LICENSE.txt')
        for path in sorted((ROOT / 'docs/notes/windows-refactor-round4-eval').glob('*.json')):
            archive.write(path, f'local/{path.name}')
    supplement_hash = sha(ROOT / 'assets/lexicon/supplement/dict.tsv')
    for report in [summary, quality_summary, *daily.values()]:
        if report['binary_sha256'] != quality_summary['binary_sha256'] or report['builtin_supplement_sha256'] != supplement_hash:
            raise ValueError('evaluation executable or supplement fingerprint mismatch')
    if lexicon['binary_sha256'] != quality_summary['binary_sha256'] or lexicon['supplement_sha256'] != supplement_hash:
        raise ValueError('lexicon evaluation fingerprint mismatch')
    info = dict(version=release, tag=os.environ['PREVIEW_TAG'], commit=os.environ['GITHUB_SHA'],
                workflow_run=os.environ['GITHUB_RUN_ID'], rust=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                architectures=['x86_64-server-settings-tsf', 'i686-tsf'], uiaccess=False, code_signed=False,
                data_lock=(ROOT / 'tools/release/data.lock').read_text(encoding='utf-8'),
                evaluation=summary, input_quality=quality_summary, daily_reading=daily, lexicon=lexicon, gui_acceptance='pending user tests on Win10 and Win11')
    (output / 'build-info.json').write_text(json.dumps(info, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    files = sorted(output.iterdir())
    (output / 'SHA256SUMS').write_text(''.join(f'{sha(path)}  {path.name}\n' for path in files), encoding='utf-8')
    print('staged', ', '.join(path.name for path in output.iterdir()))


if __name__ == '__main__':
    {'validate': validate, 'stage': stage}[sys.argv[1]]()
