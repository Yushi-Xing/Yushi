#!/usr/bin/env python3
"""发布打包门禁：核对证据归档，并拒绝来源指纹混用。"""
import importlib.util
import json
import os
import re
import shutil
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location('preview', Path(__file__).with_name('windows-preview.py'))
preview = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(preview)
REPO = preview.ROOT
RELEASE = re.search(r'^version = "([^"]+)"$', (REPO/'apps/windows/server/Cargo.toml').read_text(encoding='utf-8'), re.MULTILINE)[1]


class PreviewTests(unittest.TestCase):
    def prepare(self, root):
        (root/'docs').mkdir()
        for app in ['server','tsf','settings']:
            target=root/f'apps/windows/{app}/Cargo.toml'
            target.parent.mkdir(parents=True)
            shutil.copy2(REPO/f'apps/windows/{app}/Cargo.toml',target)
        for name in ['assets/eval/journal','assets/eval/input-quality','assets/eval/daily-reading','assets/lexicon/supplement']:
            shutil.copytree(REPO/name,root/name)
        license_path=root/'assets/lexicon/00_meta/THUOCL_LICENSE.txt'
        license_path.parent.mkdir(parents=True)
        shutil.copy2(REPO/'assets/lexicon/00_meta/THUOCL_LICENSE.txt',license_path)
        lock = root/'tools/release/data.lock'
        lock.parent.mkdir(parents=True)
        lock.write_text((REPO/'tools/release/data.lock').read_text())
        installer = root/f'target/installer/qingjian-{RELEASE}-windows-x86_64-setup.exe'
        installer.parent.mkdir(parents=True)
        installer.write_bytes(b'MZ'+b'\0'*1_000_000)
        fingerprint = preview.sha(REPO/'assets/lexicon/supplement/dict.tsv')
        base = dict(binary_sha256='test-executable', builtin_supplement_sha256=fingerprint)
        profiles = {
            'journal-ci':dict(base, clean=dict(count=30),mutations=dict(count=270),groups=dict(fuzzy=dict(count=6))),
            'input-quality-ci':dict(base, count=1444,clean=dict(count=92),mutations=dict(count=1104)),
            'daily-static-ci':dict(base, profile='full',count=1123,clean=dict(count=64)),
            'daily-model-ci':dict(base, profile='full',count=1123,clean=dict(count=64)),
            'lexicon-ci':dict(binary_sha256='test-executable',supplement_sha256=fingerprint,count=237,coverage=dict(count=220,entries=220)),
        }
        for name,report in profiles.items():
            directory=root/'target'/name
            directory.mkdir(parents=True)
            for file in ['cases.json','normal.tsv','fuzzy.tsv','normal.jsonl','fuzzy.jsonl','results.jsonl','normal.log','fuzzy.log','inputs.tsv','details.jsonl','failures.jsonl','eval.log','build-info.json','config.toml']:
                (directory/file).write_text('{}\n')
            (directory/'summary.json').write_text(json.dumps(report))

    def run_stage(self, root):
        env=dict(PREVIEW_TAG=f'yushi-windows-v{RELEASE}',GITHUB_SHA='test-commit',GITHUB_RUN_ID='test-run')
        with patch.object(preview,'ROOT',root),patch.dict(os.environ,env):
            preview.stage()

    def test_archives_contain_daily_states_lexicon_and_licenses(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            self.prepare(root)
            self.run_stage(root)
            output=root/'target/windows-preview'
            with zipfile.ZipFile(output/'daily-reading-evaluation.zip') as archive:
                self.assertIsNone(archive.testzip())
                for name in ['ci/static/results.jsonl','ci/model/results.jsonl','ci/lexicon/failures.jsonl','lexicon/manifest.json','lexicon/THUOCL_LICENSE.txt','lexicon/JIEBA-LICENSE.txt','lexicon/UNICODE-LICENSE.txt']:
                    self.assertIn(name,archive.namelist())
            info=json.loads((output/'build-info.json').read_text())
            self.assertEqual(info['daily_reading']['static']['count'],1123)
            self.assertEqual(info['lexicon']['coverage']['entries'],220)
            for line in (output/'SHA256SUMS').read_text().splitlines():
                expected,name=line.split('  ',1)
                self.assertEqual(expected,preview.sha(output/name))

    def test_mismatched_library_fingerprint_rejects_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            self.prepare(root)
            path=root/'target/daily-model-ci/summary.json'
            report=json.loads(path.read_text())
            report['builtin_supplement_sha256']='wrong-library'
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError,'fingerprint mismatch'):
                self.run_stage(root)
            self.assertFalse((root/'target/windows-preview/SHA256SUMS').exists())


if __name__=='__main__':
    unittest.main()
