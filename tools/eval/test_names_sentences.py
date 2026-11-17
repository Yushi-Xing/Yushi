#!/usr/bin/env python3
"""保护冻结样例与跨平台回归门禁，目标姓名的失败不能被丢弃或改判。"""

import json
from pathlib import Path
import tempfile
import unittest

import names_sentences


class NamesSentencesTests(unittest.TestCase):
    def test_frozen_inputs_include_ambiguous_names_and_incomplete_syllables(self):
        rows = names_sentences.cases()
        self.assertEqual(len(rows), 290)
        self.assertEqual(sum(r['group'] == 'clean' for r in rows), 80)
        self.assertEqual(rows, names_sentences.cases())
        self.assertIn(('李明', 'liming'), [(r['text'], r['pinyin']) for r in rows])
        self.assertIn(('我们明天见', 'womenmingtianj'), [(r['text'], r['pinyin']) for r in rows])

    def test_regression_gate_detects_lost_top_one_and_top_five(self):
        reference = names_sentences.SEEDS.with_name('reference.json')
        before = json.loads(reference.read_text(encoding='utf-8'))
        rows = before['results']
        self.assertEqual(names_sentences.compare(rows, reference), [])
        row = next(r for r in rows if r['position'] == 0)
        row['position'] = None
        row['top'] = '回归控制'
        self.assertEqual([r['topk'] for r in names_sentences.compare(rows, reference)], [1, 5])
        with tempfile.TemporaryDirectory() as folder:
            wrong = Path(folder) / 'wrong.json'
            before['seed_sha256'] = 'different'
            wrong.write_text(json.dumps(before), encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'different seeds'):
                names_sentences.compare(rows, wrong)


if __name__ == '__main__':
    unittest.main()
