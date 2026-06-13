# codemap-search 스케일링 벤치마크 — 설계 & 메트릭 아이디에이션 (2026-06-09)

> 코드베이스 크기(~10k → ~100k 지원언어 LOC)에 따라 codemap-search가 어떻게 버티는지를
> **사이즈의 함수**로 특성화한다. 1차 초점은 **축 A(스케일링/성능)**, 2차로 한 티어에서만
> **축 B(검색 품질)**를 얇게 샘플한다.

---

## 1. 코퍼스 (Corpora)

코퍼스는 레포 밖 별도 디렉터리에 **고정 SHA**로 핀해 사용했다 — git 트리를 오염시키지
않기 위함이다. 측정 종료 후 클론 디렉터리는 정리됐으며, 아래 SHA로 재클론하면 누구나
동일 코퍼스를 복원한다.
티어 단위는 **codemap-search가 실제 심볼-인덱싱하는 지원언어(Rust/Python/TS·JS) LOC**다
(tokei 14.0, `.gitignore` 준수). 비지원 언어/문서/벤더 파일은 티어 정의에서 제외하되,
`read`/`find`/`grep` 대상에는 동일하게 포함되므로 공정성에 영향 없다.

| 티어 | 레포 | SHA | 지원 LOC | 지원 파일 | 주 언어 |
|---|---|---|--:|--:|---|
| ~10k | `sharkdp/fd` | `25461e5` | 6,813 | 23 | Rust |
| ~30k | `BurntSushi/ripgrep` | `82313cf` | 39,070 | 102 | Rust |
| ~50k | `scrapy/scrapy` | `4e956bd` | 63,513 | 439 | Python |
| ~100k | `vuejs/core` | `48ad452` | 128,285 | 519 | TypeScript |

> 티어 라벨은 명목 버킷이고 **실측 LOC를 그대로 표기**한다(예: "100k"가 아니라 128,285).
> 스프레드는 Rust(경량 추출) → Python → TypeScript(중량 추출)로 언어별 거동도 함께 관찰한다.

**대안 코퍼스**(재슬라이스용 — SHA 핀 기록, 필요 시 재클론):

| 레포 | SHA | 지원 LOC | 주 언어 |
|---|---|--:|---|
| `pallets/flask` | `36e4a82` | 13,993 | Python |
| `denoland/rusty_v8` | `7e2d4a2` | 36,423 | Rust |
| `tiangolo/fastapi` | `5cdf820` | 94,530 | Python |
| `vitejs/vite` | `689a066` | 79,183 | TypeScript |
| `prettier/prettier` | `15f1320` | 127,839 | JavaScript |
| (폐기·과대) `python/mypy` `e15a6d5` 214,871 Py · `rust-lang/cargo` `0140b9b` 285,414 Rust | | | |

---

## 2. 측정 환경 (기록 필수)

매 측정 산출물에 박는다: CPU/RAM/OS, **측정 시 loadavg**, codemap-search 빌드 SHA·`--release`,
tokei 14.0, 각 코퍼스 SHA, `config.toml`(특히 `max_file_size`·`result_threshold`).
지연은 평균이 아니라 분포(p50/p90/p95)로, 콜드/웜을 분리해 표기한다.

---

## 3. 축 A — 스케일링/성능 (메인, 4티어 전부, ground-truth 불필요)

> ⚠️ 내장 `benchmark` 서브커맨드는 **빌드시간·메모리·인덱스크기를 만들지 않는다**(`index_files()`에
> 계측 없음). 또한 그 baseline은 rg가 아니라 *쿼리마다 전체 파일을 재파싱하는 선형스캔 strawman*이다.
> 따라서 **성능 스케일링은 전부 외부 계측**으로 잡고, 서브커맨드는 §5의 BM25 divergence 한정으로만 쓴다.

### 3.1 인덱싱 (코퍼스당)
| 지표 | 정의 | 수집 방법 |
|---|---|---|
| Cold index time | 콜드 전체 색인 소요(median, N≥3) | `/usr/bin/time -l codemap-search index .` 의 wall |
| Peak RSS | 색인 중 최대 상주 메모리 | `/usr/bin/time -l` 의 `maximum resident set size` |
| Index size | 디스크상 tantivy 인덱스 크기 | `du -sh .codemap/index` |
| Throughput | 지원 LOC/s · 파일/s | LOC ÷ cold time |
| Incremental | 1파일 touch 후 재색인 소요 | (증분 경로가 있으면) 재실행 wall |

### 3.2 쿼리 지연 (도구별·웜, 에이전트 무관)
`overview`(root/folder/file) · `search`(BM25) · `read` · `find`(glob) · `grep`(literal/regex)
각각 동일 쿼리셋으로 N회 반복, p50/p90/p95. 콜드(첫 호출, 인덱스 open 포함) 별도 1회.

| 지표 | 정의 |
|---|---|
| Tool latency p50/p90/p95 | 도구 1회 호출 소요(웜) |
| Result size | 반환 바이트/라인 수(컨텍스트 비용 프록시) |

### 3.3 자원/부팅
바이너리 startup 시간, MCP `initialize` 왕복 시간, 상주 메모리.

### 3.4 비교군 (strawman 회피)
- `grep`(컴파일된 ripgrep) **vs** 시스템 `rg` **vs** 내장 `benchmark`의 선형스캔 baseline.
- 헤드라인을 "인덱스가 baseline보다 N배"로 적지 말 것 — 그 baseline은 naive 재파싱이다.
  의미 있는 대조는 *에이전트가 실제 쓰는 rg/grep* 대비다.

---

## 4. 사이즈가 커질 때만 드러나는 질문 (이 벤치의 핵심)
1. 인덱스 빌드시간·peak RSS·인덱스크기가 **선형인가 초선형인가** (LOC 대비 회귀).
2. BM25 `search` **품질이 코퍼스 크기에 따라 열화**되는가 (10k에선 top-k 적중, 128k에선 노이즈에 묻힘?).
3. 선형스캔 baseline 대비 인덱스의 **교차점** — 작은 코퍼스에선 인덱스 오버헤드가 손해, 어디서 역전?
4. 중량 추출 언어(TS)가 경량(Rust) 대비 색인시간·심볼수 측면에서 얼마나 비싼가.

---

## 5. 검색 품질 — 두 갈래

### 5.1 BM25 divergence (축 A에 포함, 앵커 불필요)
내장 `benchmark`에서 `expected[]` 없이도 얻는 유일한 실값: `baseline_set`(전수 심볼스캔) vs
`index_set`(BM25) **divergence %**. "코퍼스가 커질 때 BM25가 전수스캔과 얼마나 갈리나"를 사이즈별로.
※ `expected[]`가 없으면 recall은 baseline·index 모두 무조건 100%로 무의미 — divergence만 본다.

### 5.2 축 B — 에이전트 E2E 품질 (얇게, 축 A 이후)
scout 벤치 방법론 차용: 과제=자연어 증상, 정답=편집 핵심 `file:line` 앵커, tol ±3줄.
헤드라인 **F2**, 보조 recall/precision/over-return + 반환 토큰(컨텍스트 비용).
**비용 통제:** 큐레이션된 앵커가 있어야 의미가 있으므로(scout는 순환편향 차단 위해 독립검증까지 함),
**1개 티어 × 소수 과제로만** 샘플한다. 축 A를 먼저 끝내 durable 결과를 확보한 뒤 진행.

---

## 6. 수집 데이터 스키마 (한 줄 = (repo, tier, tool/op, rep))
```
repo, sha, tier_loc, lang, op, rep,
  wall_ms, peak_rss_mb, index_size_bytes,      # 인덱싱
  latency_ms, result_bytes, result_count,      # 쿼리
  baseline_set_size, index_set_size, divergence_pct,  # BM25
  recall, precision, f2, over_return            # 축 B(앵커 있을 때만)
```

## 7. 실행 순서
1. `--release` 빌드 (진행 중).
2. 코퍼스별 `cd <corpus> && /usr/bin/time -l codemap-search index .` ×N → §3.1.
3. 도구별 웜 쿼리 N회 → §3.2 / 3.3.
4. divergence-only 쿼리셋으로 `benchmark` → §5.1.
5. (선택) 1티어 앵커 큐레이션 → 축 B → §5.2.
