#!/usr/bin/env python3
"""检查补库来源、独立性与多音字边界；不以评测答案驱动导入。"""
import json
import shutil
import subprocess
from pathlib import Path
import tempfile
import unittest

import supplement


class SupplementTests(unittest.TestCase):
    def test_reading_resolution_rejects_ambiguous_and_invalid_words(self):
        chars = dict(重={'zhong','chong'}, 庆={'qing'}, 双={'shuang'}, 汇={'hui'})
        self.assertIsNone(supplement.resolve('重庆', {}, chars))
        self.assertEqual(supplement.resolve('重庆', {'重庆':{'chong qing'}}, chars), ('chong qing','existing_word'))
        self.assertIsNone(supplement.resolve('重庆', {'重庆':{'zhong qing','chong qing'}}, chars))
        self.assertIsNone(supplement.resolve('重庆', {'重庆':{'chong xing'}}, chars))
        self.assertIsNone(supplement.resolve('重庆', {'重庆':{'chong'}}, chars))
        self.assertEqual(supplement.resolve('双汇', {}, chars), ('shuang hui','unihan_single_reading'))
        self.assertIsNone(supplement.resolve('未知', {}, chars))
        self.assertEqual(supplement.normalize('lǜ'), 'lv')
        self.assertEqual(supplement.normalize('nüè'), 'nve')

    def test_char_readings_include_minor_readings(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)/'readings.txt'
            path.write_text('# fixture\nU+91CD\tkMandarin\tzhòng\nU+91CD\tkHanyuPinlu\tchóng(30) zhòng(70)\nU+91CD\tkXHC1983\t0442.080:chóng\nU+5E86\tkMandarin\tqìng\n',encoding='utf-8')
            readings = supplement.char_readings(path)
            self.assertEqual(readings['重'], {'zhong','chong'})
            self.assertEqual(readings['庆'], {'qing'})

    def test_frozen_shipped_library_provenance_and_no_base_duplicates(self):
        manifest = json.loads((supplement.OUTPUT/'manifest.json').read_text(encoding='utf-8'))
        rows = [json.loads(s) for s in (supplement.OUTPUT/'provenance.jsonl').read_text(encoding='utf-8').splitlines()]
        self.assertEqual(len(rows), manifest['count'])
        self.assertEqual(len({r['word'] for r in rows}), len(rows))
        self.assertEqual(supplement.digest(supplement.OUTPUT/'dict.tsv'), manifest['dictionary_sha256'])
        self.assertEqual(supplement.digest(supplement.OUTPUT/'provenance.jsonl'), manifest['provenance_sha256'])
        base, _, _ = supplement.existing()
        self.assertFalse(base & {r['word'] for r in rows})
        for name, expected in manifest['inputs'].items():
            self.assertEqual(supplement.digest(supplement.ROOT/name), expected, name)
        for row in rows:
            self.assertTrue(2 <= len(row['word']) <= 8)
            self.assertEqual(len(row['word']), len(row['pinyin'].split()))
            self.assertTrue(20 <= row['weight'] <= 500)
        lines = [l for l in (supplement.OUTPUT/'dict.tsv').read_text(encoding='utf-8').splitlines() if l and not l.startswith('#')]
        self.assertEqual(lines, [f"{r['word']}\t{r['pinyin']}\t{r['weight']}" for r in rows])
        # 文章标准句保持独立，整句预测不能靠把答案当词条加入。
        seeds = json.loads((supplement.ROOT/'assets/eval/daily-reading/seeds.json').read_text(encoding='utf-8'))
        words = {r['word'] for r in rows}
        self.assertFalse(words & {s['text'] for s in seeds if s['kind']=='sentence'})

    @unittest.skipUnless(shutil.which('git'), 'Git is required for checkout normalization')
    def test_windows_style_checkout_preserves_frozen_source_bytes(self):
        manifest = json.loads((supplement.OUTPUT/'manifest.json').read_text(encoding='utf-8'))
        expected = dict(manifest['inputs'])
        expected['assets/lexicon/supplement/dict.tsv'] = manifest['dictionary_sha256']
        expected['assets/lexicon/supplement/provenance.jsonl'] = manifest['provenance_sha256']
        with tempfile.TemporaryDirectory() as temporary:
            subprocess.run(['git','-c','core.autocrlf=true','checkout-index',
                            '--prefix='+Path(temporary).as_posix()+'/', '--stdin'],
                           input='\n'.join(expected)+'\n', cwd=supplement.ROOT,
                           encoding='utf-8', capture_output=True, check=True, timeout=30)
            for name, fingerprint in expected.items():
                self.assertEqual(supplement.digest(Path(temporary)/name), fingerprint, name)

    def test_untrusted_source_is_rejected_before_writing(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)/'bad.txt'
            source.write_text('双汇 68 n\n', encoding='utf-8')
            output = Path(temporary)/'output'
            with self.assertRaises(ValueError):
                supplement.generate(source, source, output)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
