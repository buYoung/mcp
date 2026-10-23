# 관련어 PoC 문서

관련어 PoC는 코딩 에이전트의 코드 검색어에 관련어를 붙여 주는 사전을 시험한 실험이다. 이 폴더에는 데이터셋을 만든 과정, 단어 평가, 원자료, 알려진 결함을 정리했다. 코드와 데이터셋은 `apps/codemap-search/experiments/related-terms-poc/`에 있다.

| 문서 | 다루는 것 |
| --- | --- |
| [데이터셋 생성 과정](pipeline.md) | 세 데이터셋(뜻, 카테고리, 관련어)의 입력, 생성 단계, 조회 도구, 검증 스크립트 |
| [코드 단어 수집과 단어 평가](word-evaluation.md) | tree-sitter 식별자 수집, LLM 판정 절차, 데이터셋 보강, 당시 평가 결과 |
| [원자료와 근거 저장소](sources.md) | 외부 사전 자료와 고정 판, LLM 생성 자료, 임베딩 모델, 코드 저장소 12개 |
| [알려진 결함](known-defects.md) | 현재 기본 데이터셋에서 확인한 결함, 빠졌을 수 있는 뜻, 확인하지 못한 것 |
| [에이전트의 조회 도구 사용 평가](agent-usage-eval.md) | 코딩 에이전트가 사용법만 받고 식별자를 단어로 나눠 `sense_lookup.py`에 넣는지 보는 블라인드 평가 스크립트와 결과 |
| [codemap-search 도입 설계](codemap-integration.md) | 데이터셋 배포, Rust 조회 구현, MCP 도구, 코드베이스 맞춤과 검증 계획 |
| [뜻 데이터셋 구조 정리와 분야 매핑 확장](sense-mapping-log/README.md) | 평가 세트, `rt-sense/4` 구조, 분야가 하나뿐인 단어의 주제 매핑, 조회 검증의 단계별 기록 |
| [관련어 사전 조회하기](../../apps/codemap-search/experiments/related-terms-poc/README.related.ko.md) | `rtpoc3 related` 사용법과 관련어 데이터셋 평가 결과 |

## 바로 조회하기

저장소 루트에서 실행한다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py token retry
python3 apps/codemap-search/experiments/related-terms-poc/category_lookup.py token
```

두 도구는 입력 단어의 표기를 해석할 때 관련어 데이터셋 `rtr1-930791c83bea6132`와 `rtpoc3` 실행 파일을 쓴다. 이 데이터셋은 저장소에 없고 `.codemap/poc/related-terms/thesaurus-v1/datasets/`에만 있으므로, 새로 받은 저장소에서는 동작하지 않는다.
