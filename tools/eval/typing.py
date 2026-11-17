#!/usr/bin/env python3
"""用固定输入逐键测 release CLI，保存阶段耗时、首选和 Linux 进程资源；不读取个人数据。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
INPUTS = [
    "nihao", "kaifa", "jintianwanshangwomenquchifan", "woxiangxuehaozhongwen",
    "youcikanlaimeishenmewenti", "cikushounaleduoshaoci", "cekushounaleduoshaoci",
    "nijuedezhegdiayzenmy", "huantaipingy", "shuanghuihuotuichang",
    "wodedatabase", "woxiangxuexirust", "zhonghuarenmingongheguo",
    "sssssssssssssss", "zhzhzhzhzhzhzhzh",
]
PHASES = ["parse", "lookup", "rank", "translate", "total"]
DURATION = r"(\d+(?:\.\d+)?(?:ms|µs))"
ROW = re.compile(r"^\s+(\S+)\s+" + r"\s+".join([DURATION] * 5) + r"\s+(.+)$")


def milliseconds(value):
    return float(value[:-2]) / (1000 if value.endswith("µs") else 1)


def distribution(values):
    ordered = sorted(values)
    return dict(p50=ordered[len(ordered) // 2],
                p95=ordered[min(len(ordered) - 1, int(len(ordered) * .95))],
                maximum=ordered[-1], mean=sum(ordered) / len(ordered))


def run(binary, output, repeats):
    binary = binary.resolve()
    output.mkdir(parents=True, exist_ok=False)
    config = output / "config.toml"
    config.write_text('[predict]\nenabled = false\n[model]\nenabled = false\n', encoding="utf-8")
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    env = dict(os.environ, LC_ALL="C.UTF-8")
    for key in ["QINGJIAN_API_KEY", "OPENAI_API_KEY", "QINGJIAN_LOG_DIR"]:
        env.pop(key, None)
    rows, runs = [], []
    for repetition in range(repeats):
        command = [str(binary), "--config", str(config.resolve()), "--shuangpin", "off", "--typing", *INPUTS]
        resources = output / f"resource-{repetition}.tsv"
        measured = sys.platform.startswith("linux") and Path("/usr/bin/time").is_file()
        if measured:
            command = ["/usr/bin/time", "-f", "%U\t%S\t%M", "-o", str(resources.resolve()), *command]
        started = time.perf_counter()
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True,
                                encoding="utf-8", check=True, timeout=300)
        wall = time.perf_counter() - started
        (output / f"typing-{repetition}.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        current = None
        count = 0
        for line in result.stdout.splitlines():
            if line.startswith("> "):
                current = line[2:]
            match = ROW.match(line)
            if not match:
                continue
            assert current in INPUTS and current.startswith(match[1]), line
            rows.append(dict(repetition=repetition, input=current, prefix=match[1], first=match[7],
                             **{phase: milliseconds(value) for phase, value in zip(PHASES, match.group(2, 3, 4, 5, 6))}))
            count += 1
        assert count == sum(map(len, INPUTS)), f"missing timing rows: {count}"
        run_info = dict(wall_seconds=wall)
        if measured:
            user, system, rss = resources.read_text().strip().split("\t")
            run_info.update(cpu_seconds=float(user) + float(system), peak_rss_kib=int(rss))
        runs.append(run_info)
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == digest
    summary = dict(schema=1, binary_sha256=digest, repeats=repeats, keys=len(rows), inputs=INPUTS,
                   metric="CLI reported query stages plus annotation; all candidates annotated, no desktop rendering",
                   data_sha256=hashlib.sha256((ROOT / "data/generated/dict.qj").read_bytes()).hexdigest(),
                   stages_ms={phase: distribution([row[phase] for row in rows]) for phase in PHASES},
                   by_input={keys: distribution([row["total"] for row in rows if row["input"] == keys]) for keys in INPUTS},
                   process=runs)
    (output / "details.jsonl").write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows), encoding="utf-8")
    (output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    run(args.binary, args.output, args.repeats)
