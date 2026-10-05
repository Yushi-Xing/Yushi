#!/usr/bin/env python3
"""离线补库：固定结巴词表 + THUOCL 高 DF 词，复用整词读音或 Unihan 单音字。"""
import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import json
from pathlib import Path
import re
import unicodedata

ROOT = Path(__file__).resolve().parents[2]
LEXICON = ROOT / 'assets/lexicon'
OUTPUT = LEXICON / 'supplement'
JIEBA_SHA = '7197c3211ddd98962b036cdf40324d1ea2bfaa12bd028e68faa70111a88e12a8'
UNIHAN_SHA = '575e69c9ad85a4737a889a4f94cbd987042a90a1a6cc16dd3f4ed995c715b17c'
DOMAIN_MIN = dict(food=1000, finance=1000, it_computing=1000, automotive=1000, places=10000)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalize(text):
    text = ''.join('v' if c in 'üǖǘǚǜ' else c for c in text.lower())
    plain = ''.join(c for c in unicodedata.normalize('NFD', text) if not unicodedata.combining(c))
    return {'lue':'lve', 'nue':'nve'}.get(plain, plain)


def char_readings(path):
    readings = defaultdict(set)
    for line in path.read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('#'):
            continue
        code, field, value = line.split('\t')
        if field not in ['kMandarin', 'kHanyuPinlu', 'kXHC1983']:
            continue
        for token in value.split():
            if field == 'kHanyuPinlu':
                token = token.split('(')[0]
            elif field == 'kXHC1983':
                token = token.split(':')[-1]
            reading = normalize(token)
            if not re.fullmatch('[a-z]+', reading):
                raise ValueError(f'invalid Unihan reading: {token}')
            readings[chr(int(code[2:], 16))].add(reading)
    return readings


def existing():
    base, readings = set(), defaultdict(set)
    paths = [LEXICON/'dict.tsv', *sorted((LEXICON/'dicts').glob('*.tsv'))]
    for path in paths:
        for line in path.read_text(encoding='utf-8').splitlines():
            if not line or line.startswith('#'):
                continue
            word, pinyin, _ = line.split('\t')
            readings[word].add(pinyin)
            if path.name == 'dict.tsv':
                base.add(word)
    for line in (LEXICON/'patches.tsv').read_text(encoding='utf-8').splitlines():
        if line and not line.startswith('#'):
            base.add(line.split('\t')[0])
    return base, readings, paths


def resolve(word, known, chars):
    whole = known.get(word, set())
    if len(whole) == 1:
        pinyin = next(iter(whole)).split()
        if len(pinyin) == len(word) and all(p in chars.get(c, set()) for c, p in zip(word, pinyin)):
            return ' '.join(pinyin), 'existing_word'
        return None
    if whole or any(len(chars.get(c, set())) != 1 for c in word):
        return None
    return ' '.join(next(iter(chars[c])) for c in word), 'unihan_single_reading'


def generate(jieba, unihan, output):
    if digest(jieba) != JIEBA_SHA or digest(unihan) != UNIHAN_SHA:
        raise ValueError('source SHA256 does not match the frozen upstream data')
    base, known, paths = existing()
    chars = char_readings(unihan)
    standard = {r['word'] for r in csv.DictReader((LEXICON/'01_characters/standard_8105.tsv').open(encoding='utf-8'), delimiter='\t')}
    selected, rejected = {}, Counter()

    def add(word, count, source, proper=False, weight=None):
        if word in base:
            rejected['already_in_base'] += 1
            return
        if not 2 <= len(word) <= 8 or any(c not in standard for c in word):
            rejected['length_or_nonstandard'] += 1
            return
        if proper and word not in known:
            rejected['unverified_proper_name'] += 1
            return
        reading = resolve(word, known, chars)
        if reading is None:
            rejected['ambiguous_or_unverified_reading'] += 1
            return
        pinyin, method = reading
        if word not in selected:
            selected[word] = dict(pinyin=pinyin, weight=max(20, min(500, count if weight is None else weight)), evidence=[], reading=method)
        selected[word]['evidence'].append([source, count])

    for line in jieba.read_text(encoding='utf-8').splitlines():
        word, count, pos = line.split()
        if int(count) >= 20:
            add(word, int(count), 'jieba', pos.startswith(('nr', 'ns')))
    for domain, minimum in DOMAIN_MIN.items():
        path = LEXICON/'03_domains'/f'{domain}.tsv'
        paths.append(path)
        for row in csv.DictReader(path.open(encoding='utf-8'), delimiter='\t'):
            if row['doc_freq'] and int(row['doc_freq']) >= minimum:
                # DF 仅在同领域筛选；低权重补充，不能冒充本项目语料出现次数。
                add(row['word'], int(row['doc_freq']), f'thuocl_{domain}', weight=20)
    reviewed = OUTPUT/'reviewed.tsv'
    for row in csv.DictReader(reviewed.open(encoding='utf-8'), delimiter='\t'):
        word, pinyin = row['word'], row['pinyin']
        syllables = pinyin.split()
        if len(word) != len(syllables) or any(p not in chars.get(c, set()) for c,p in zip(word, syllables)):
            raise ValueError(f'invalid reviewed reading: {word}')
        if word not in base:
            selected[word] = dict(pinyin=pinyin, weight=int(row['weight']), evidence=[['project_reviewed', 0]], reading='reviewed')
    output.mkdir(parents=True, exist_ok=True)
    lines = ['# 常用词补充：jieba（MIT）、THUOCL（MIT）、Unihan（Unicode-3.0）；许可证与来源见同目录。',
             '# 第三列是保守排序权重，不是统一语料频次；多音字只使用核验过的整词读音。']
    lines += [f"{word}\t{row['pinyin']}\t{row['weight']}" for word,row in sorted(selected.items())]
    dictionary = output/'dict.tsv'
    dictionary.write_text('\n'.join(lines)+'\n', encoding='utf-8', newline='\n')
    provenance = output/'provenance.jsonl'
    provenance.write_text(''.join(json.dumps(dict(word=word, **row),ensure_ascii=False,separators=(',',':'))+'\n' for word,row in sorted(selected.items())), encoding='utf-8', newline='\n')
    report = dict(schema=1, count=len(selected), dictionary_sha256=digest(dictionary), dictionary_bytes=dictionary.stat().st_size, provenance_sha256=digest(provenance),
                  jieba_revision='67fa2e36e72f69d9134b8a1037b83fbb070b9775', jieba_sha256=digest(jieba),
                  unihan_version='17.0.0', unihan_readings_sha256=digest(unihan),
                  thresholds=dict(jieba_min=20, min_chars=2, max_chars=8, domain_df=DOMAIN_MIN),
                  reading_methods=dict(Counter(r['reading'] for r in selected.values())), rejected=dict(rejected),
                  inputs={str(p.relative_to(ROOT)):digest(p) for p in [*paths,LEXICON/'01_characters/standard_8105.tsv',reviewed,LEXICON/'patches.tsv']})
    (output/'manifest.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8', newline='\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jieba', required=True, type=Path)
    parser.add_argument('--unihan', required=True, type=Path)
    parser.add_argument('--output', default=OUTPUT, type=Path)
    args = parser.parse_args()
    print(json.dumps(generate(args.jieba,args.unihan,args.output),ensure_ascii=False,indent=2))
