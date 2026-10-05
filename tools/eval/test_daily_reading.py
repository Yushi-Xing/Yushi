#!/usr/bin/env python3
"""检查文章固定集的人工音节、来源边界和多处错拼，避免评测口径随结果变化。"""
import json
import unittest

import daily_reading
import journal


class DailyReadingTests(unittest.TestCase):
    def test_frozen_sources_splits_and_duplicates(self):
        rows = daily_reading.cases()
        self.assertEqual(rows, daily_reading.cases())
        self.assertEqual(len(rows), 1123)
        self.assertEqual(len({(r['text'],r['pinyin'],r['context']) for r in rows}), len(rows))
        seeds = json.loads(daily_reading.SEEDS.read_text(encoding='utf-8'))
        sources = json.loads(daily_reading.SOURCES.read_text(encoding='utf-8'))
        self.assertEqual(len(sources),4)
        self.assertEqual(len(seeds),64)
        self.assertEqual(sum(s['split']=='holdout' for s in seeds),16)
        self.assertEqual(sum(s['provenance']=='short_excerpt' for s in seeds),4)
        self.assertEqual({r['source'] for r in rows if r['split']=='user'}, {'user'})
        self.assertEqual(sum(r['group']=='clean' for r in rows),64)

    def test_mutations_really_cross_boundaries_and_have_two_edits(self):
        rows = daily_reading.cases()
        seeds = {s['id']:s for s in json.loads(daily_reading.SEEDS.read_text(encoding='utf-8'))}
        for row in rows:
            if row['source']=='user':
                continue
            seed = seeds[row['id'].split('/')[0]]
            keys = ''.join(seed['syllables'])
            if row['group']=='double_delete':
                self.assertEqual(journal.distance(keys,row['pinyin']),2)
            if row['group']=='double_neighbor':
                self.assertEqual(sum(a!=b for a,b in zip(keys,row['pinyin'])),2)
            if row['group'].startswith('boundary_transpose'):
                boundaries = [sum(map(len,seed['syllables'][:i])) for i in range(1,len(seed['syllables']))]
                self.assertTrue(any(row['pinyin']==keys[:at-1]+keys[at]+keys[at-1]+keys[at+1:] for at in boundaries))
        self.assertIn(('词库收纳了多少词','cekushounaleduoshaoci'), [(r['text'],r['pinyin']) for r in rows])


if __name__=='__main__':
    unittest.main()
