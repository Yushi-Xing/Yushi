#!/usr/bin/env python3
"""补库能力抽测：来源清单稳定散列抽样 + 全部人工核验词，报告覆盖与首选分开。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import journal

ROOT = Path(__file__).resolve().parents[2]
SUPPLEMENT = ROOT/'assets/lexicon/supplement'


def cases():
    entries = [json.loads(s) for s in (SUPPLEMENT/'provenance.jsonl').read_text(encoding='utf-8').splitlines()]
    automatic = [r for r in entries if r['reading']!='reviewed']
    automatic.sort(key=lambda r:hashlib.sha256(r['word'].encode()).hexdigest())
    selected = automatic[:192]+[r for r in entries if r['reading']=='reviewed']
    rows = [dict(text=r['word'],pinyin=''.join(r['pinyin'].split()),context='', group='reviewed' if r['reading']=='reviewed' else 'sample') for r in selected]
    rows += [dict(text=w,pinyin=p,context='',group='control') for w,p in [('暑假','shujia'),('职业','zhiye'),('换成','huancheng'),('左翼','zuoyi'),('统一','tongyi'),('充电','chongdian'),('我的','wode'),('我们','women'),('你好','nihao'),('数据库','shujuku'),('hello','hello'),('database','database'),('环太平洋','huantaipingyang'),('火腿肠','huotuichang')]]
    rows += [dict(text='双汇火腿肠',pinyin=keys,context='',group='user_typo') for keys in ['shuanghuihuotuic','shuanghuihuotuchang','shuanghuihuotuichnag']]
    assert len({(r['text'],r['pinyin']) for r in rows}) == len(rows)
    return rows


def run(binary,output):
    output.mkdir(parents=True,exist_ok=False)
    rows = cases()
    (output/'cases.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    inputs = output/'inputs.tsv'
    inputs.write_text(''.join(f"{r['text']}\t{r['pinyin']}\t\n" for r in rows),encoding='utf-8')
    config = output/'config.toml'
    config.write_text('[predict]\nenabled = false\n[general]\nshuangpin = "off"\n',encoding='utf-8')
    details = output/'details.jsonl'
    fingerprint = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    env = dict(os.environ)
    for key in ['QINGJIAN_API_KEY','OPENAI_API_KEY']:
        env.pop(key,None)
    with (output/'eval.log').open('w',encoding='utf-8') as log:
        subprocess.run([str(Path(binary).resolve()),'--config',str(config),'--shuangpin','off','--eval-text',str(inputs),'--eval-details',str(details),'--misses','30'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=1800)
    assert fingerprint == hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    results = [json.loads(s) for s in details.read_text(encoding='utf-8').splitlines()]
    assert len(results)==len(rows)
    for case,result in zip(rows,results):
        assert (case['text'],case['pinyin'],case['context'])==(result['text'],result['pinyin'],result['context'])
        assert result['char_errors']==journal.distance(case['text'],result['top'] or '')
        assert result.get('position')==next((i for i,s in enumerate(result['candidates']) if s==case['text']),None)
        result.update(case)
    report = dict(schema=1,count=len(rows),corpus_kind='imported_lexicon_capability_sample_not_independent_accuracy',binary_sha256=fingerprint,
                  dictionary_sha256=hashlib.sha256((ROOT/'data/generated/dict.qj').read_bytes()).hexdigest(),
                  supplement_sha256=hashlib.sha256((SUPPLEMENT/'dict.tsv').read_bytes()).hexdigest(),
                  groups={g:journal.summarize([r for r in results if r['group']==g]) for g in ['sample','reviewed','control','user_typo']},
                  coverage=dict(count=sum(r['group'] in ['sample','reviewed'] for r in results),entries=sum(r.get('dictionary_entry',False) for r in results if r['group'] in ['sample','reviewed'])))
    for name,selected in [('results.jsonl',results),('failures.jsonl',[r for r in results if r['top']!=r['text']])]:
        (output/name).write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in selected),encoding='utf-8')
    (output/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',required=True)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    run(args.binary,args.output)
