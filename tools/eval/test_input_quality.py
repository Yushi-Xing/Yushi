#!/usr/bin/env python3
"""组合集去重、编辑强度、真实用户输入与词库审计口径回归。"""
import unittest
import input_quality
import journal


class InputQualityTests(unittest.TestCase):
    def test_frozen_cases_keep_all_edit_positions_and_user_keys(self):
        rows = input_quality.cases()
        self.assertEqual(rows, input_quality.cases())
        self.assertEqual(len(rows), 1444)
        self.assertEqual(len({(r['text'], r['pinyin']) for r in rows}), 1444)
        clean = {r['text']: r['pinyin'] for r in rows if r['group'] == 'clean'}
        self.assertEqual(len(clean), 92)
        for row in rows:
            if row['group'].startswith(('transpose_', 'delete_', 'insert_', 'substitute_')):
                edits = 2 if row['group'].startswith('transpose_') else 1
                self.assertEqual(journal.distance(clean[row['text']], row['pinyin']), edits, row)
        self.assertIn(('环太平洋', 'huantaipingy'), {(r['text'], r['pinyin']) for r in rows})
        self.assertIn(('你觉得这个电影怎么样', 'nijuedezhegdiayzenmy'), {(r['text'], r['pinyin']) for r in rows})
        self.assertEqual(sum(r['group'] == 'control' for r in rows), 3)


if __name__ == '__main__':
    unittest.main()
