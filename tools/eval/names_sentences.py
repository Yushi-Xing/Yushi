#!/usr/bin/env python3
"""冻结姓名与日常语句抽测；报告全部失败，不向词库注入答案。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import journal

ROOT = Path(__file__).resolve().parents[2]
SEEDS = ROOT / 'assets/eval/names-sentences/seeds.json'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cases():
    rows = []
    for seed in json.loads(SEEDS.read_text(encoding='utf-8')):
        syllables = seed['syllables']
        assert len(seed['text']) == len(syllables), seed['id']
        variants = [('clean', ''.join(syllables)),
                    ('last_initial', ''.join(syllables[:-1]) + syllables[-1][0]),
                    ('initials', ''.join(s[0] for s in syllables)),
                    ('mixed_initials', ''.join(s if i % 2 == 0 else s[0] for i, s in enumerate(syllables)))]
        seen = set()
        for group, typed in variants:
            if typed in seen:
                continue
            seen.add(typed)
            rows.append(dict(id=f"{seed['id']}/{group}", category=seed['category'], group=group,
                             text=seed['text'], pinyin=typed, context=''))
    assert len({r['id'] for r in rows}) == len(rows)
    assert len({(r['text'], r['pinyin']) for r in rows}) == len(rows)
    return rows


def compare(results, reference):
    previous = json.loads(reference.read_text(encoding='utf-8'))
    assert previous['seed_sha256'] == sha(SEEDS), 'reference uses different seeds'
    assert previous['data_locks_sha256'] == {name: sha(ROOT / 'tools/release' / name) for name in ['dicts.lock', 'dicts-extra.lock', 'models.lock']}, 'different product data'
    assert previous['supplement_sha256'] == sha(ROOT / 'assets/lexicon/supplement/dict.tsv'), 'different lexicon'
    assert len(previous['results']) == len(results), 'reference is incomplete'
    lost = []
    for before, after in zip(previous['results'], results):
        assert (before['id'], before['text'], before['pinyin']) == (after['id'], after['text'], after['pinyin'])
        for k in [1, 5]:
            if before['position'] is not None and before['position'] < k:
                if after['position'] is None or after['position'] >= k:
                    lost.append(dict(id=after['id'], topk=k, before=before['top'], after=after['top']))
    return lost


def run(binary, output, model=None, reference=None):
    binary = binary.resolve()
    output.mkdir(parents=True, exist_ok=False)
    selected = cases()
    (output / 'cases.json').write_text(json.dumps(selected, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    config = output / 'config.toml'
    config.write_text('[predict]\nenabled = false\n[model]\nenabled = false\n[general]\nshuangpin = "off"\n', encoding='utf-8')
    frozen = output / 'inputs.tsv'
    frozen.write_text(''.join(f"{r['text']}\t{r['pinyin']}\t\n" for r in selected), encoding='utf-8')
    details = output / 'details.jsonl'
    fingerprint = sha(binary)
    command = [str(binary), '--config', str(config.resolve()), '--shuangpin', 'off',
               '--eval-text', str(frozen.resolve()), '--eval-details', str(details.resolve()), '--misses', '80']
    if model:
        command += ['--eval-p2c', str(model.resolve())]
    env = dict(os.environ)
    for key in ['QINGJIAN_API_KEY', 'OPENAI_API_KEY', 'QINGJIAN_LOG_DIR']:
        env.pop(key, None)
    with (output / 'eval.log').open('w', encoding='utf-8') as log:
        subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)
    assert sha(binary) == fingerprint, 'evaluation binary changed'
    results = [json.loads(line) for line in details.read_text(encoding='utf-8').splitlines()]
    assert len(results) == len(selected), 'incomplete evaluation'
    for case, result in zip(selected, results):
        assert (case['text'], case['pinyin'], case['context']) == (result['text'], result['pinyin'], result['context'])
        assert result['char_errors'] == journal.distance(case['text'], result['top'] or '')
        assert result.get('position') == next((i for i, text in enumerate(result['candidates']) if text == case['text']), None)
        result.update(case)
    categories = sorted({r['category'] for r in results})
    groups = sorted({r['group'] for r in results})
    report = dict(schema=1, count=len(results), seed_sha256=sha(SEEDS), binary_sha256=fingerprint,
                  data_locks_sha256={name: sha(ROOT / 'tools/release' / name) for name in ['dicts.lock', 'dicts-extra.lock', 'models.lock']},
                  dictionary_sha256=sha(ROOT / 'data/generated/dict.qj'),
                  supplement_sha256=sha(ROOT / 'assets/lexicon/supplement/dict.tsv'),
                  model_sha256=sha(model) if model else None,
                  categories={c: {g: journal.summarize([r for r in results if r['category'] == c and r['group'] == g])
                                  for g in groups if any(r['category'] == c and r['group'] == g for r in results)}
                              for c in categories})
    lost = compare(results, reference) if reference else []
    report['regressions'] = lost if reference else None
    for name, rows in [('results.jsonl', results), ('failures.jsonl', [r for r in results if r['top'] != r['text']])]:
        (output / name).write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in rows), encoding='utf-8')
    (output / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    compact = {key: report[key] for key in ['seed_sha256', 'data_locks_sha256', 'supplement_sha256']}
    compact['results'] = [{key: r[key] for key in ['id', 'text', 'pinyin', 'top', 'position']} for r in results]
    (output / 'reference.json').write_text(json.dumps(compact, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
    assert not lost, f'{len(lost)} Top-1/Top-5 regressions; see summary.json'
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--model', type=Path)
    parser.add_argument('--reference', type=Path)
    args = parser.parse_args()
    run(args.binary, args.output, args.model, args.reference)
