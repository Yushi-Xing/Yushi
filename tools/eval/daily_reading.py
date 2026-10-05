#!/usr/bin/env python3
"""日常文章固定集：正常输入、跨音节与多处错拼；保留独立留出集和全部失败。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import journal

ROOT = Path(__file__).resolve().parents[2]
SEEDS = ROOT / 'assets/eval/daily-reading/seeds.json'
SOURCES = SEEDS.with_name('sources.json')
FAR_KEY = dict(a='o', e='i', i='e', o='a', u='a', v='a')


def cases():
    rows = []
    for seed in json.loads(SEEDS.read_text(encoding='utf-8')):
        syllables = seed['syllables']
        assert len(seed['text']) == len(syllables)
        keys = ''.join(syllables)
        boundaries = [sum(map(len, syllables[:i])) for i in range(1, len(syllables))]
        variants = [('clean', keys)]
        for label, at in [('head', 1), ('middle', len(keys)//2), ('tail', len(keys)-2)]:
            variants += [(f'delete_{label}', keys[:at]+keys[at+1:]),
                         (f'insert_{label}', keys[:at]+keys[at]+keys[at:]),
                         (f'neighbor_{label}', keys[:at]+journal.NEIGHBOR[keys[at]]+keys[at+1:])]
        for label, at in [('head', boundaries[0]), ('middle', boundaries[len(boundaries)//2])]:
            variants.append((f'boundary_transpose_{label}', keys[:at-1]+keys[at]+keys[at-1]+keys[at+1:]))
        first, last = 1, len(keys)-2
        variants += [('double_delete', ''.join(c for i,c in enumerate(keys) if i not in (first,last))),
                     ('double_neighbor', ''.join(journal.NEIGHBOR[c] if i in (first,last) else c for i,c in enumerate(keys))),
                     ('delete_and_insert', keys[:first]+keys[first+1:last]+keys[last]+keys[last:]),
                     ('initials', ''.join(s[0] for s in syllables)),
                     ('mixed_initials', ''.join(s if i%2==0 else s[0] for i,s in enumerate(syllables)))]
        for i,c in enumerate(keys):
            if c in FAR_KEY:
                variants.append(('distant_vowel', keys[:i]+FAR_KEY[c]+keys[i+1:]))
                break
        seen = set()
        for group, typed in variants:
            if typed in seen:
                continue
            seen.add(typed)
            rows.append(dict(id=f"{seed['id']}/{group}", source=seed['source'], kind=seed['kind'],
                             split=seed['split'], group=group, text=seed['text'], pinyin=typed, context=''))
    for i,(text,keys) in enumerate([('词库收纳了多少词','cekushounaleduoshaoci'),
                                   ('词库收纳了多少词','cikushounaleduoshaoci'),
                                   ('你觉得这个电影怎么样','nijuedezhegdiayzenmy'),
                                   ('环太平洋','huantaipingy')]):
        rows.append(dict(id=f'user-{i}',source='user',kind='sentence',split='user',group='user',text=text,pinyin=keys,context=''))
    assert len({(r['text'],r['pinyin'],r['context']) for r in rows}) == len(rows)
    return rows


def run(binary, output, model=None, probe=False):
    output.mkdir(parents=True, exist_ok=False)
    selected = [r for r in cases() if not probe or r['group'] in ['clean','user']
                or (r['kind']=='sentence' and r['group']=='double_delete')]
    (output/'cases.json').write_text(json.dumps(selected,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    config = output/'config.toml'
    config.write_text('[predict]\nenabled = false\n[general]\nshuangpin = "off"\n',encoding='utf-8')
    frozen = output/'inputs.tsv'
    frozen.write_text(''.join(f"{r['text']}\t{r['pinyin']}\t\n" for r in selected),encoding='utf-8')
    details = output/'details.jsonl'
    fingerprint = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    command = [str(Path(binary).resolve()),'--config',str(config),'--shuangpin','off','--eval-text',str(frozen),
               '--eval-details',str(details),'--misses','30']
    if model:
        command += ['--eval-p2c',str(Path(model).resolve())]
    env = dict(os.environ)
    for key in ['QINGJIAN_API_KEY','OPENAI_API_KEY']:
        env.pop(key,None)
    with (output/'eval.log').open('w',encoding='utf-8') as log:
        subprocess.run(command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=3600)
    assert hashlib.sha256(Path(binary).read_bytes()).hexdigest() == fingerprint
    results = [json.loads(s) for s in details.read_text(encoding='utf-8').splitlines()]
    assert len(results) == len(selected)
    for case, result in zip(selected,results):
        assert (case['text'],case['pinyin'],case['context']) == (result['text'],result['pinyin'],result['context'])
        assert result['char_errors'] == journal.distance(case['text'],result['top'] or '')
        assert result.get('position') == next((i for i,s in enumerate(result['candidates']) if s==case['text']),None)
        result.update(case)
    article = [r for r in results if r['source']!='user']
    clean = [r for r in article if r['group']=='clean']
    report = dict(schema=1,count=len(results),profile='probe' if probe else 'full',
                  corpus_kind='four_short_article_excerpts_and_original_daily_paraphrases',
                  seed_sha256=hashlib.sha256(SEEDS.read_bytes()).hexdigest(),source_sha256=hashlib.sha256(SOURCES.read_bytes()).hexdigest(),
                  binary_sha256=fingerprint,dictionary_sha256=hashlib.sha256((ROOT/'data/generated/dict.qj').read_bytes()).hexdigest(),
                  builtin_patch_sha256=hashlib.sha256((ROOT/'assets/lexicon/patches.tsv').read_bytes()).hexdigest(),
                  builtin_supplement_sha256=hashlib.sha256((ROOT/'assets/lexicon/supplement/dict.tsv').read_bytes()).hexdigest(),
                  model_sha256=hashlib.sha256(Path(model).read_bytes()).hexdigest() if model else None,
                  word_coverage=dict(count=sum(r['kind']=='word' for r in clean), entries=sum(r.get('dictionary_entry',False) for r in clean if r['kind']=='word'), missing=[r['text'] for r in clean if r['kind']=='word' and not r.get('dictionary_entry',False)]),
                  clean=journal.summarize(clean),clean_changed=sum(r.get('corrected') is not None for r in clean),
                  clean_by_kind={k:journal.summarize([r for r in clean if r['kind']==k]) for k in ['word','sentence']},
                  clean_by_source={s:journal.summarize([r for r in clean if r['source']==s]) for s in sorted({r['source'] for r in clean})},
                  clean_by_split={s:journal.summarize([r for r in clean if r['split']==s]) for s in ['development','holdout']},
                  groups={g:journal.summarize([r for r in article if r['group']==g]) for g in sorted({r['group'] for r in article})},
                  user=journal.summarize([r for r in results if r['source']=='user']))
    for name,data in [('results.jsonl',results),('failures.jsonl',[r for r in results if r['top']!=r['text']])]:
        (output/name).write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in data),encoding='utf-8')
    (output/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',required=True)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--model',type=Path)
    parser.add_argument('--probe',action='store_true',help='仅正常输入、用户例与句子双漏键；不冒充完整集')
    args = parser.parse_args()
    run(args.binary,args.output,args.model,args.probe)
