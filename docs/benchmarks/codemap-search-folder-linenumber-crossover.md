# folder overview 라인번호 — 효율 crossover 측정 (2026-06-09)

> 질문: "어느 index 크기부터 folder overview에 라인번호(`[L..]`)를 두는 게 비효율로 뒤집히는가?"
> 방법: folder overview를 **반드시 거치게** 강제한 동일 과제로, 티어별 R1(folder 범위 있음) vs R2(folder 범위 없음) 각 3회 병렬 A/B. 토큰이 주 효율지표(동시성 무관), tool_uses·locate 도구는 트레이스에서 직접 집계.

## 결정적 스윕 — 범위 오버헤드는 folder의 ~¼~⅓ 고정 비율

| corpus | index | root | R1 folder | R2 folder | Δ bytes | Δ% |
|---|--:|--:|--:|--:|--:|--:|
| fd | 0.16 MB | 1 KB | 10,180 | 7,294 | 2,886 | 28.3% |
| ripgrep | 0.73 MB | 6 KB | 20,238 | 13,896 | 6,342 | 31.3% |
| scrapy | 1.24 MB | 29 KB | 16,752 | 12,529 | 4,223 | 25.2% |
| vue-core | 1.40 MB | 43 KB | 23,698 | 17,755 | 5,943 | 25.1% |

→ 라인번호 제거는 티어 무관 folder overview의 **~25–31%**를 절감. 절대 절감량은 folder 크기에 비례.

## 에이전트 A/B — 토큰 효율의 crossover (강제형 과제, n=3)

| tier | index | R1 토큰(평균) | R2 토큰(평균) | Δ% (R2−R1) | R1 locate | R2 locate |
|---|--:|--:|--:|--:|--:|--:|
| fd | 0.16 MB | 30,388 | 30,335 | **−0.2%** | 0 | 2 |
| ripgrep | 0.73 MB | 37,776 | 36,616 | **−3.1%** | 0 | 3 |
| vue-core | 1.40 MB | 37,370 | 34,582 | **−7.5%** | 0 | 5 |

원자료(토큰): fd-R1 30135/31048/29982 · fd-R2 32713/28777/29515 · rg-R1 37081/40026/36221 · rg-R2 39543/36334/33970 · vue-R1 37225/37490/37394 · vue-R2 34780/35268/33698

### 메커니즘 (트레이스로 확정)
- **R1(범위 있음)**: folder overview가 라인번호를 줘서 에이전트가 곧바로 read로 점프 → **locate(grep/search) 0회** (전 티어). folder overview가 locator로 동작.
- **R2(범위 없음)**: folder overview가 위치를 안 줘서 에이전트가 grep으로 라인번호 복구 → locate 2–5회. 라운드트립↑, 토큰↓(folder overview가 가벼움 + grep 응답이 작음).

## crossover 해석

토큰 효율(Δ%)이 index 크기에 대해 **단조 증가**(R2가 점점 유리):

```
Δ%(R2 vs R1) ≈ -5.9 × index_MB + 0.74     (3점 선형근사)
  break-even(R1=R2):  index ≈ 0.13 MB
  ranges 토큰 패널티 2%:  index ≈ 0.46 MB
  ranges 토큰 패널티 3%:  index ≈ 0.64 MB
```

- **index ≲ 0.13 MB(≈ fd, ~7k LOC)**: 라인번호가 토큰상 **break-even** 이고 라운드트립을 줄임 → **표시가 순이득**(win-win).
- **index ≳ 0.13 MB**: 라인번호가 토큰을 잡아먹기 시작, 크기에 비례해 악화(−3% @0.73MB, −7.5% @1.4MB). 절감하는 라운드트립(~1–2회 grep, warm 서버에선 수~십 ms)은 거의 일정 → **숨기는 게 유리**.

### 결론
folder 라인번호가 토큰상 "공짜로 라운드트립을 아끼는" 구간은 **매우 작은 코드베이스(index ≲ 0.15 MB ≈ ~7–10k LOC)** 뿐이다. 그 위로는 표시할수록 토큰 손해가 커진다. 따라서 하이브리드 임계값(index 크기)은:

| 정책 성향 | 임계 index | 근거 |
|---|--:|---|
| 토큰 순수주의(범위는 진짜 공짜일 때만) | **~0.3 MB** (~15k LOC) | 패널티 ≤1% 구간 |
| 균형(사용자 초기 직감 25k LOC) | **~0.5 MB** (~25–30k LOC) | 패널티 ≤2% 구간, 소형 repo에 라운드트립 절감 제공 |

**권장: ~0.3–0.5 MB.** 그 미만이면 folder에 라인번호 표시(소형 repo 편의·라운드트립 절감, 토큰 손해 미미), 이상이면 숨김(토큰 절감 + overview≠search 원칙). config bool=false면 크기 무관 항상 숨김.

## 최종 결정 (2026-06-09): 하이브리드 폐기 → R2 고정

하이브리드(index 0.5MB 임계 + config bool)를 검토했으나 **폐기**하고 **folder 라인번호를 항상 숨김(R2 고정)**으로 확정.

- 이유: literal index 크기가 **overview 시점에 신뢰 불가**. MCP 서버는 시작 시 인덱싱하지 않고 tantivy 인덱스는 첫 `search` 때 lazy 빌드(`mcp.rs:369`)된다. `search` 없이 `overview`만 호출하면 인덱스가 비어 대형 repo도 라인번호가 켜지고, overview 동작이 "search 이력"에 의존해 **결정성(working-tree fingerprint 캐시 계약)이 깨진다.**
- 결정적 프록시(총 심볼/총 LOC)로 우회 가능하나, "index 크기" 본래 의도와 어긋나고 임계 복잡도만 늘어 가치가 낮다고 판단.
- 측정이 뒷받침: folder 라인번호가 토큰상 뚜렷이 이득인 구간이 없음(최소 fd에서도 break-even). 항상 숨김은 토큰이 항상 같거나 이득이고, "overview ≠ search" 원칙도 일관된다. 라운드트립 약간 증가는 warm 서버에선 미미.

## 측정 한계
- n=3, 에이전트 분산 존재(강제형 과제로 타이트해졌으나 정밀 crossover는 근사).
- index 크기는 전체 코드베이스 기준 — 실제 folder overview 크기는 folder 선택에 따라 다르므로 임계값은 프록시.
- A/B의 시간(latency)은 cms 래퍼가 호출마다 codemap 재빌드(~500ms)해 과장됨. warm MCP 서버에선 R2의 추가 grep 라운드트립 지연이 작음. → 토큰축이 더 신뢰할 신호.
</content>
