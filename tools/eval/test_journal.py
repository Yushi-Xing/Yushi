#!/usr/bin/env python3
"""确定性评测集和统计口径的独立回归，不加载产品引擎或网络。"""
import unittest
import journal


class JournalTests(unittest.TestCase):
    def test_deterministic_corpus_preserves_every_mutation_and_context(self):
        rows = journal.cases()
        self.assertEqual(rows, journal.cases())
        self.assertEqual(len(rows), 336)
        self.assertEqual(len({(r['text'], r['pinyin'], r['context']) for r in rows}), 336)
        clean = {r['text']: r for r in rows if r['group'] == 'clean'}
        self.assertEqual(len(clean), 30)
        for row in rows:
            if row['group'] in ['clean', 'context', 'fuzzy']:
                continue
            expected_edits = 2 if row['group'].startswith(('transpose', 'double')) else 1
            self.assertEqual(journal.distance(clean[row['text']]['pinyin'], row['pinyin']), expected_edits, row['id'])

    def test_metrics_include_failures_insertions_and_missing_gold(self):
        rows = [dict(text='你好', top='你好', position=0, char_errors=0),
                dict(text='你好', top='你好吗', position=2, char_errors=1),
                dict(text='你好', top=None, char_errors=2, error='parse')]
        report = journal.summarize(rows)
        self.assertEqual((report['count'], report['top1'], report['top3'], report['top5']), (3, 1, 2, 2))
        self.assertEqual((report['char_errors'], report['chars'], report['parse_failures']), (3, 6, 1))
        self.assertEqual(report['cer_percent'], 50)
        self.assertAlmostEqual(report['top1_percent'], 33.333)


if __name__ == '__main__':
    unittest.main()
