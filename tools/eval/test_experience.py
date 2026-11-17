#!/usr/bin/env python3
"""验证体验评分的失败惩罚与逐任务回归门槛。"""

import copy
import unittest

import experience


def row(**changes):
    item = dict(id='one', text='你好', keys='nihao', position=0, completed=True,
                actions=6, stable=True, page_turns=0, query_ms=[1, 2, 3],
                repeat_query_ms=[.01, .01, .01], event_ms=[1, 2, 3], interval_ms=50,
                prefixes=[dict(event_ms=t) for t in [1, 2, 3]])
    item.update(changes)
    return item


class ExperienceTests(unittest.TestCase):
    def test_perfect_typing_scores_one_hundred(self):
        report = experience.summarize([row()], [dict(passed=True)])
        self.assertEqual(report['score'], 100)

    def test_failed_tasks_reduce_accuracy_completion_and_efficiency(self):
        failed = row(position=None, completed=False, actions=1)
        report = experience.summarize([row(), failed], [dict(passed=True)])
        self.assertEqual(report['completion'], .5)
        self.assertEqual(report['efficiency'], .5)
        self.assertLess(report['score'], 75)

    def test_pages_and_selection_actions_cost_points(self):
        report = experience.summarize([row(actions=16, page_turns=10)], [dict(passed=True)])
        self.assertLess(report['score_parts']['efficiency'], 10)
        self.assertEqual(report['total_page_turns'], 10)

    def test_failed_learning_cannot_be_offset_by_accuracy(self):
        report = experience.summarize([row()], [dict(passed=False)])
        self.assertEqual(report['score_parts']['interactions'], 0)
        self.assertEqual(report['score'], 80)

    def test_top1_top5_and_completion_are_independent_gates(self):
        previous = dict(results=[row()])
        after = copy.deepcopy(previous['results'])
        after[0].update(position=8, completed=False)
        failures = experience.compare(after, previous)
        self.assertEqual({r['metric'] for r in failures}, {'top1', 'top5', 'completion'})

    def test_previously_failed_tasks_can_improve_without_changing_baseline(self):
        previous = dict(results=[row(position=None, completed=False)])
        self.assertEqual(experience.compare([row()], previous), [])

    def test_fast_repeats_cannot_hide_slow_key_processing(self):
        slow = row(query_ms=[25], repeat_query_ms=[.001] * 100, event_ms=[40])
        report = experience.summarize([slow], [dict(passed=True)])
        self.assertEqual(report['query_p95_ms'], 25)
        self.assertEqual(report['event_p95_ms'], 40)
        self.assertEqual(report['score_parts']['response'], 1.25)

    def test_typing_queue_accumulates_then_recovers_after_a_slow_key(self):
        self.assertEqual(experience.queued_latencies([120, 20, 5, 5], 50), [120, 90, 45, 5])
        self.assertEqual(experience.queued_latencies([5, 10, 15], 50), [5, 10, 15])

    def test_session_keystrokes_and_phrase_lengths_are_frozen(self):
        import json
        rows = json.loads(experience.EXTENDED.read_text(encoding='utf-8'))
        phrases = [r for r in rows if r['id'].startswith('phrase-')]
        self.assertEqual(len({r['text'] for r in phrases}), 24)
        self.assertEqual({len(r['text']) for r in phrases}, {3, 4})
        for item in phrases:
            keys = ''
            for key in item.get('keystrokes', item['keys']):
                keys = keys[:-1] if key == '\b' else keys + key
            self.assertEqual(keys, item['keys'])
        self.assertEqual(len({r['session'] for r in rows if 'session' in r}), 3)

    def test_v2_keeps_all_original_baseline_rows_and_adds_no_dictionary_answers(self):
        import json
        old = json.loads((experience.ROOT / 'assets/eval/experience/reference.json').read_text())
        current = json.loads(experience.REFERENCE.read_text())
        self.assertEqual(current['results'][:len(old['results'])], old['results'])
        self.assertEqual(current['product_data_sha256'], old['product_data_sha256'])

    def test_full_spelling_rescue_is_charged_and_does_not_improve_initial_accuracy(self):
        rescued = row(position=None, actions=22, retry=row())
        report = experience.summarize([rescued], [dict(passed=True)])
        self.assertEqual(report['completion'], 1)
        self.assertEqual(report['top1'], 0)
        self.assertEqual(report['top5'], 0)
        self.assertEqual(report['spelling_retries'], 1)
        self.assertLess(report['score_parts']['efficiency'], 3)


if __name__ == '__main__':
    unittest.main()
