# codemap-search 검색 품질 벤치마크

이 폴더는 2026년 7월 검색 품질 측정에 사용한 코드 저장소, 14개 개발 과제, 실행·채점 하네스 소스를 제품 저장소 안에 보존한다. 이전 `benchmark/`의 180회 공개 비교 결과와 Node.js 하네스는 이 자료로 대체했다.

## 현재 기준 자료

- 대상 저장소: [Directus](https://github.com/directus/directus)
- 고정 커밋: `9f2f73aee7d8647d3f187dac43f724fe617763f5`
- 고정 Git 트리: `0beb7bd5187e9131aba4a582effb3630d378eb4c`
- 모델 설정: `ollama-cloud/deepseek-v4-flash`
- 개발 과제: 14개
- 기준 조건: OpenCode 기본 도구만 사용하는 B1, `codemap-search`만 추가한 B2

Directus의 중첩 `.git`, 로컬 `.codemap` 색인, 의존성 설치 결과는 포함하지 않았다. `corpus/directus`는 위 커밋의 Git 트리에서 직접 만든 소스 스냅샷이다.

## 폴더 구조

```text
benchmark/
├── README.md
├── PREVIOUS_BENCHMARK_REPOSITORIES.md
├── analysis-tools/       # 토큰·도구 호출·점수 추출 및 집계
├── benchmark/            # 14개 질문·정답·고정 목록
├── corpus/directus/      # 측정 대상 소스 스냅샷
└── harness/              # 실행·격리·채점 코드와 계약
```

## 포함하지 않은 자료

다음은 크기가 크거나 인증 정보를 포함하거나 다시 만들 수 있는 실행 산출물이므로 커밋하지 않는다.

- OpenCode 실행 파일과 인증 자료
- `codemap-search` 빌드 결과
- Tantivy 색인과 `.codemap` 캐시
- 실행별 대화·점수·임시 작업 폴더
- Python `__pycache__`

하네스의 고정 해시와 실행 제한은 보존했다. 실제 외부 모델 실행 전에는 [하네스 안내](harness/README.md)에 적힌 런타임 입력을 별도로 준비하고 사전 점검을 통과해야 한다.

이전 벤치마크가 사용한 코드 저장소 정보만 [이전 저장소 기록](PREVIOUS_BENCHMARK_REPOSITORIES.md)에 남겼다.
