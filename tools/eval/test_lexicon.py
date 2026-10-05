#!/usr/bin/env python3
"""检查补库能力抽样及日常词纠错回归的分组和稳定性。"""
import unittest

import lexicon


class LexiconTests(unittest.TestCase):
    def test_frozen_sampling_and_real_brand_variants(self):
        rows = lexicon.cases()
        self.assertEqual(rows, lexicon.cases())
        self.assertEqual(len(rows),237)
        self.assertEqual(sum(r['group']=='sample' for r in rows),192)
        self.assertEqual(sum(r['group']=='reviewed' for r in rows),28)
        self.assertEqual(sum(r['group']=='control' for r in rows),14)
        self.assertEqual(len({(r['text'],r['pinyin']) for r in rows}),len(rows))
        variants = [r for r in rows if r['group']=='user_typo']
        self.assertEqual(len(variants),3)
        self.assertTrue(all(r['text']=='双汇火腿肠' for r in variants))
        self.assertIn('shuanghuihuotuichnag',[r['pinyin'] for r in variants])


if __name__=='__main__':
    unittest.main()
