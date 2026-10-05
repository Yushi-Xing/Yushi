#!/usr/bin/env python3
"""中文期刊主题评测：固定人工读音，确定性错拼，严格核对每一条输出。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SEEDS = ROOT / 'assets/eval/journal/seeds.json'
NEIGHBOR = dict(a='s', b='v', c='x', d='f', e='r', f='g', g='h', h='j', i='o', j='k', k='l', l='k', m='n', n='m', o='p', p='o', q='w', r='t', s='d', t='y', u='i', v='b', w='e', x='c', y='u', z='x')


def cases():
    rows = []
    for seed in json.loads(SEEDS.read_text(encoding='utf-8')):
        pinyin = ''.join(seed['syllables'])
        assert len(seed['text']) == len(seed['syllables']), seed['id']
        variants = [('clean', pinyin), ('context', pinyin)]
        for label, index in [('middle', len(pinyin) // 2), ('end', len(pinyin) - 3)]:
            for kind in ['transpose', 'delete', 'insert', 'substitute']:
                for offset in range(len(pinyin) - 1):
                    at = (index + offset) % (len(pinyin) - 1)
                    if kind == 'transpose':
                        typed = pinyin[:at] + pinyin[at + 1] + pinyin[at] + pinyin[at + 2:]
                    elif kind == 'delete':
                        typed = pinyin[:at] + pinyin[at + 1:]
                    elif kind == 'insert':
                        typed = pinyin[:at] + pinyin[at] + pinyin[at:]
                    else:
                        typed = pinyin[:at] + NEIGHBOR[pinyin[at]] + pinyin[at + 1:]
                    if typed not in [v for _, v in variants]:
                        variants.append((f'{kind}_{label}', typed))
                        break
                else:
                    raise AssertionError((seed['id'], kind, label))
        variants.append(('double_error', pinyin[:2] + NEIGHBOR[pinyin[2]] + pinyin[3:-3] + NEIGHBOR[pinyin[-3]] + pinyin[-2:]))
        for group, typed in variants:
            context = '研究人员正在讨论相关问题。' if group == 'context' else seed['context']
            rows.append(dict(id=f"{seed['id']}/{group}", source=seed['source'], group=group,
                             text=seed['text'], pinyin=typed, context=context))
    # 模糊音必须按当前配置处理，不能被泛化拼写纠错抢走。
    for i, (text, typed) in enumerate([('听我说', 'tinwosuo'), ('听说', 'tinsuo'),
                                     ('明天', 'mintian'), ('工程', 'gongcen'),
                                     ('双手', 'suangsou'), ('程式', 'censi')]):
        rows.append(dict(id=f'fuzzy-{i}', source='control', group='fuzzy', text=text, pinyin=typed, context=''))
    assert len({(r['text'], r['pinyin'], r['context']) for r in rows}) == len(rows)
    return rows


def distance(expected, actual):
    row = list(range(len(actual) + 1))
    for i, a in enumerate(expected, 1):
        diagonal, row[0] = row[0], i
        for j, b in enumerate(actual, 1):
            above = row[j]
            row[j] = min(diagonal + (a != b), above + 1, row[j - 1] + 1)
            diagonal = above
    return row[-1]


def summarize(rows):
    count = len(rows)
    errors = sum(r['char_errors'] for r in rows)
    chars = sum(len(r['text']) for r in rows)
    return dict(count=count, top1=sum(r['top'] == r['text'] for r in rows),
                top3=sum(r.get('position') is not None and r['position'] < 3 for r in rows),
                top5=sum(r.get('position') is not None and r['position'] < 5 for r in rows),
                parse_failures=sum('error' in r for r in rows),
                corrected=sum(r.get('corrected') is not None for r in rows),
                char_errors=errors, chars=chars,
                top1_percent=round(100 * sum(r['top'] == r['text'] for r in rows) / count, 3),
                cer_percent=round(100 * errors / chars, 3))


def run(binary, output, model=None):
    # 不读取用户配置与个人词库，不启用云服务，不将标准答案加入词库。
    output.mkdir(parents=True, exist_ok=False)
    config = output / 'config.toml'
    config.write_text('[predict]\nenabled = false\n[general]\nshuangpin = "off"\n', encoding='utf-8')
    all_cases = cases()
    (output / 'cases.json').write_text(json.dumps(all_cases, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    fingerprint = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    results = []
    for profile in ['normal', 'fuzzy']:
        selected = [r for r in all_cases if (r['group'] == 'fuzzy') == (profile == 'fuzzy')]
        frozen = output / f'{profile}.tsv'
        frozen.write_text(''.join(f"{r['text']}\t{r['pinyin']}\t{r['context']}\n" for r in selected), encoding='utf-8')
        details = output / f'{profile}.jsonl'
        command = [str(Path(binary).resolve()), '--config', str(config), '--shuangpin', 'off',
                   '--eval-text', str(frozen), '--eval-details', str(details), '--misses', '30']
        if profile == 'fuzzy':
            command += ['--fuzzy', 'in-ing,en-eng,s-sh,c-ch,an-ang']
        if model:
            command += ['--eval-p2c', str(Path(model).resolve())]
        env = dict(os.environ)
        # 环境密钥不传入评测进程；CLI 的配置也禁用了网络。
        for name in ['QINGJIAN_API_KEY', 'OPENAI_API_KEY']:
            env.pop(name, None)
        with (output / f'{profile}.log').open('w', encoding='utf-8') as log:
            subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)
        assert hashlib.sha256(Path(binary).read_bytes()).hexdigest() == fingerprint, 'evaluation binary changed during the run'
        detail_rows = [json.loads(line) for line in details.read_text(encoding='utf-8').splitlines()]
        assert len(detail_rows) == len(selected), (profile, len(detail_rows), len(selected))
        for case, result in zip(selected, detail_rows):
            assert (case['text'], case['pinyin'], case['context']) == (result['text'], result['pinyin'], result['context'])
            assert result['char_errors'] == distance(case['text'], result['top'] or ''), case['id']
            assert result.get('position') == next((i for i, text in enumerate(result['candidates']) if text == case['text']), None)
            result.update(case)
            results.append(result)
    groups = sorted({r['group'] for r in results})
    sources = sorted({r['source'] for r in results})
    report = dict(schema=1, seed_sha256=hashlib.sha256(SEEDS.read_bytes()).hexdigest(),
                  dictionary_sha256=hashlib.sha256((ROOT / 'data/generated/dict.qj').read_bytes()).hexdigest(),
                  model_sha256=hashlib.sha256(Path(model).read_bytes()).hexdigest() if model else None,
                  binary_sha256=fingerprint,
                  builtin_patch_sha256=hashlib.sha256((ROOT / 'assets/lexicon/patches.tsv').read_bytes()).hexdigest(),
                  builtin_supplement_sha256=hashlib.sha256((ROOT / 'assets/lexicon/supplement/dict.tsv').read_bytes()).hexdigest(),
                  corpus_kind='original_paraphrases_based_on_journal_topics',
                  groups={g: summarize([r for r in results if r['group'] == g]) for g in groups},
                  sources={s: summarize([r for r in results if r['source'] == s and r['group'] == 'clean']) for s in sources},
                  mutations=summarize([r for r in results if r['group'] not in ['clean', 'context', 'fuzzy']]),
                  clean=summarize([r for r in results if r['group'] == 'clean']),
                  clean_changed=sum(r.get('corrected') is not None for r in results if r['group'] == 'clean'))
    (output / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (output / 'results.jsonl').write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in results), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--model', type=Path)
    args = parser.parse_args()
    run(args.binary, args.output, args.model)
