#!/usr/bin/env python3
"""日常词、专名、简拼与错拼的冻结评测，逐条核对输出并报告缺词及误纠。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import journal

ROOT = Path(__file__).resolve().parents[2]
SEEDS = ROOT / 'assets/eval/input-quality/seeds.json'


def cases():
    rows = []
    for seed in json.loads(SEEDS.read_text(encoding='utf-8')):
        syllables = seed['syllables']
        raw = ''.join(syllables)
        variants = [('clean', raw), ('last_initial', ''.join(syllables[:-1]) + syllables[-1][0]),
                    ('initials', ''.join(s[0] for s in syllables)),
                    ('mixed_initials', ''.join(s if i % 2 == 0 else s[0] for i, s in enumerate(syllables)))]
        for label, at in [('start', 1), ('middle', len(raw) // 2), ('end', len(raw) - 2)]:
            for kind in ['transpose', 'delete', 'insert', 'substitute']:
                for offset in range(len(raw) - 1):
                    index = (at + offset) % (len(raw) - 1)
                    if kind == 'transpose':
                        typed = raw[:index] + raw[index + 1] + raw[index] + raw[index + 2:]
                    elif kind == 'delete':
                        typed = raw[:index] + raw[index + 1:]
                    elif kind == 'insert':
                        typed = raw[:index] + raw[index] + raw[index:]
                    else:
                        typed = raw[:index] + journal.NEIGHBOR[raw[index]] + raw[index + 1:]
                    if typed != raw and typed not in [s for _, s in variants]:
                        variants.append((kind + '_' + label, typed))
                        break
        seen = set()
        for group, typed in variants:
            if typed in seen:
                continue
            seen.add(typed)
            rows.append(dict(category=seed['category'], group=group, text=seed['text'], pinyin=typed, context=''))
    for expected, typed in [('你觉得这个电影怎么样', 'nijuedezhegdiayzenmy'), ('环太平洋', 'huantaipingy')]:
        if not any(r['text'] == expected and r['pinyin'] == typed for r in rows):
            rows.append(dict(category='user', group='user', text=expected, pinyin=typed, context=''))
    for expected, typed in [('hello', 'hello'), ('database', 'database'), ('我想学习rust', 'woxiangxuexirust')]:
        rows.append(dict(category='control', group='control', text=expected, pinyin=typed, context=''))
    assert len({(r['text'], r['pinyin'], r['context']) for r in rows}) == len(rows)
    return rows


def run(binary, output):
    output.mkdir(parents=True, exist_ok=False)
    rows = cases()
    (output / 'cases.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    frozen = output / 'inputs.tsv'
    frozen.write_text(''.join(f"{r['text']}\t{r['pinyin']}\t\n" for r in rows), encoding='utf-8')
    config = output / 'config.toml'
    config.write_text('[predict]\nenabled = false\n[general]\nshuangpin = "off"\n', encoding='utf-8')
    details = output / 'details.jsonl'
    env = dict(os.environ)
    for key in ['QINGJIAN_API_KEY', 'OPENAI_API_KEY']:
        env.pop(key, None)
    binary_hash = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    metadata = dict(dictionary_sha256=hashlib.sha256((ROOT / "data/generated/dict.qj").read_bytes()).hexdigest(), model_sha256=None, binary_sha256=binary_hash, seed_sha256=hashlib.sha256(SEEDS.read_bytes()).hexdigest(), builtin_patch_sha256=hashlib.sha256((ROOT / 'assets/lexicon/patches.tsv').read_bytes()).hexdigest())
    (output / 'build-info.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    with (output / 'eval.log').open('w', encoding='utf-8') as log:
        subprocess.run([str(Path(binary).resolve()), '--config', str(config), '--shuangpin', 'off',
                        '--eval-text', str(frozen), '--eval-details', str(details), '--misses', '30'],
                       cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=1800)
    assert hashlib.sha256(Path(binary).read_bytes()).hexdigest() == binary_hash
    return complete(output)


def complete(output):
    rows = json.loads((output / 'cases.json').read_text(encoding='utf-8'))
    assert rows == cases(), 'frozen cases changed'
    details = output / 'details.jsonl'
    metadata = json.loads((output / 'build-info.json').read_text(encoding='utf-8'))
    results = [json.loads(s) for s in details.read_text(encoding='utf-8').splitlines()]
    assert len(results) == len(rows)
    for case, result in zip(rows, results):
        result.setdefault("position", None)
        assert (case['text'], case['pinyin'], case['context']) == (result['text'], result['pinyin'], result['context'])
        assert result['char_errors'] == journal.distance(case['text'], result['top'] or '')
        assert result.get('position') == next((i for i, s in enumerate(result['candidates']) if s == case['text']), None)
        result.update(case)
    clean_words = [r for r in results if r['category'] == 'word' and r['group'] == 'clean']
    coverage = None
    if all('dictionary_entry' in r for r in clean_words):
        coverage = dict(count=len(clean_words), entries=sum(r['dictionary_entry'] for r in clean_words),
                        missing=[r['text'] for r in clean_words if not r['dictionary_entry']])
    times = sorted(r['query_ms'] for r in results)
    report = dict(schema=1, count=len(rows), **metadata,
                  groups={g: journal.summarize([r for r in results if r['group'] == g]) for g in sorted({r['group'] for r in results})},
                  clean=journal.summarize([r for r in results if r['group'] == 'clean']),
                  mutations=journal.summarize([r for r in results if any(r['group'].startswith(k + '_') for k in ['transpose', 'delete', 'insert', 'substitute'])]),
                  clean_changed=sum(r.get('corrected') is not None for r in results if r['group'] == 'clean'),
                  word_coverage=coverage,
                  query_ms=dict(p50=times[len(times)//2], p95=times[int(len(times)*.95)], maximum=times[-1]))
    (output / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (output / 'results.jsonl').write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in results), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    run(args.binary, args.output)
