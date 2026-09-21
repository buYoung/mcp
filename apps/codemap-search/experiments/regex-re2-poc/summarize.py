#!/usr/bin/env python3
"""두 번의 독립 측정에서 중앙값, 상대 시간, 변동 폭을 추출한다."""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import random
import statistics


def ratio_interval(baseline, candidate):
    ratios = [c / b for b, c in zip(baseline, candidate)]
    rng = random.Random(173)
    boot = sorted(statistics.median(rng.choices(ratios, k=len(ratios))) for _ in range(2000))
    return boot[50], boot[1949]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('runs', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--metric', choices=('cpu', 'wall'), default='cpu')
    args = parser.parse_args()
    runs = [json.loads(path.read_text()) for path in args.runs]
    if any(run['validate_only'] for run in runs):
        parser.error('검증 전용 결과에는 시간이 없습니다.')
    if len({run['metadata']['input_sha256'] for run in runs}) != 1:
        parser.error('입력 스냅샷이 다릅니다.')
    if len({run['metadata']['cargo_lock_sha256'] for run in runs}) != 1:
        parser.error('의존성 버전이 다릅니다.')
    for name in ('src/main.rs', 'src/engine.rs', 'native/bridge.cc', 'native/CMakeLists.txt', 'Cargo.toml', 'build.rs'):
        if len({run['metadata']['poc_sources_sha256'][name] for run in runs}) != 1:
            parser.error(f'측정 코드가 다릅니다: {name}')
    stats_key = 'cpu_stats' if args.metric == 'cpu' else 'stats'
    samples_key = 'samples_cpu_ns_per_operation' if args.metric == 'cpu' else 'samples_ns_per_operation'
    lines = ['# 정규식 후보 성능 측정 원자료 요약', '',
             '시간 단위는 µs/op. 각 op는 컴파일 1회 또는 해당 코퍼스 전체 1회 처리다. '
             '`request`는 컴파일·검색·해제를 포함한다. 상대 시간은 후보/기준이며 1보다 작을수록 빠르다.', '',
             ('이 표는 스레드 CPU 시간이며, 스케줄러 대기 시간을 제외한다.' if args.metric == 'cpu' else
              '이 표는 경과 시간이며, 시스템의 다른 작업과 자원 경쟁에 영향을 받는다.'), '',
             '`grep` 및 그 `request`의 기준은 `grep_regex`, 그 외 기준은 `regex`다. '
             '각 표의 구간은 같은 반복 번호의 후보/기준 비율 중앙값에 대한 paired bootstrap 95% 구간이다. '
             '독립 프로세스 간 분산이나 시스템 외란까지 보장하는 구간은 아니다.', '']
    aggregate = defaultdict(list)
    for run in runs:
        lines += [f'## 실행 seed={run["seed"]}', '',
                  '| 사례 | 동작 | 기준 µs | RE2 µs | fancy µs | RE2/기준 (95%) | fancy/기준 (95%) | 최대 p90/p10 |',
                  '|---|---|---:|---:|---:|---:|---:|---:|']
        for row in sorted(run['results'], key=lambda r: (r['mode'], r['case'])):
            if 'timings' not in row:
                lines.append(f'| {row["case"]} | {row["mode"]} | {row["status"]} | — | — | — | — | — |')
                continue
            timings = row['timings']
            baseline = 'regex' if 'regex' in timings else 'grep_regex'
            base = timings[baseline][stats_key]['median_ns']
            ratios = []
            for name in ('re2', 'fancy_regex'):
                ratio = timings[name][stats_key]['median_ns'] / base
                low, high = ratio_interval(timings[baseline][samples_key], timings[name][samples_key])
                ratios.append(f'{ratio:.3f} ({low:.3f}–{high:.3f})')
                aggregate[(row['mode'], name)].append(ratio)
            noise = max(t[stats_key]['p90_ns'] / t[stats_key]['p10_ns'] for t in timings.values())
            values = [timings[name][stats_key]['median_ns'] / 1000 for name in (baseline, 're2', 'fancy_regex')]
            lines.append(f'| {row["case"]} | {row["mode"]} | {values[0]:.3f} | {values[1]:.3f} | {values[2]:.3f} | {ratios[0]} | {ratios[1]} | {noise:.2f} |')
        lines.append('')
    lines += ['## 동작별 상대 시간의 기하평균', '',
              '사례와 실행에 동일 가중치를 부여했다. 실제 도구 사용 빈도를 반영한 전체 성능 수치가 아니다.', '',
              '| 동작 | 후보 | 사례×실행 수 | 후보/기준 |', '|---|---|---:|---:|']
    for (mode, name), ratios in sorted(aggregate.items()):
        mean = math.exp(statistics.mean(math.log(r) for r in ratios))
        lines.append(f'| {mode} | {name} | {len(ratios)} | {mean:.3f} |')
    args.output.write_text('\n'.join(lines) + '\n')
    print(args.output)


if __name__ == '__main__':
    main()
