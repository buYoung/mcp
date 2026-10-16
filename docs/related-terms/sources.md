# 관련어 PoC 원자료와 근거 저장소

관련어 PoC 데이터셋을 만들 때 쓴 원자료와, 단어 수집과 코드 근거 확인에 쓴 12개 저장소를 정리한다. 데이터셋의 출처를 확인하거나 같은 입력을 다시 구하려는 개발자가 쓴다.

- **외부 자료:** 아래 고정 판으로 다시 받을 수 있다. 저장소에는 원자료 파일을 두지 않는다.
- **LLM 생성 자료:** 다시 받을 곳이 없고 보존하지도 않는다. 데이터셋 자체를 기준으로 쓴다.
- **라이선스와 저작자 표시:** `apps/codemap-search/experiments/related-terms-poc/ATTRIBUTION.md`에 있다.

## 외부 사전 자료

관련어 데이터셋 `rtr1-930791c83bea6132`에 들어 있는 외부 자료다. 1라운드 원본 사전(`rt-dataset/1`)과 4라운드 v2 정제 사전을 거쳐 들어왔으며, 두 사전은 보존하지 않는다. 관계 수는 관련어 데이터셋의 빌드 기록에서 옮겼다.

| 자료(출처 ID) | 주소와 고정 판 | 라이선스 | 가져온 항목 | `rtr1`의 관계 수 |
| --- | --- | --- | --- | ---: |
| Open English WordNet 2025 (`oewn-2025`) | https://github.com/globalwordnet/english-wordnet — 태그 `2025-edition`, 커밋 `dc343f2683279ecbb13fab4e2fd778d7b162d287`, 파일 `english-wordnet-2025-json.zip` | CC BY 4.0. Princeton WordNet License도 함께 적용 | synset 구성원, sense key, 품사, lexicographer file, 첫 정의, `domain_topic`, `similar`. 반의어는 관련어 데이터셋에서 뺐다 | 331,532 |
| GitHub Explore (`github-explore-c8d7bc5`) | https://github.com/github/explore — 커밋 `c8d7bc516928a4f17845b767884a80cc53affd57`, `topics/*/index.md` | CC BY 4.0 | topic slug, `display_name`, `aliases`, `related`, `short_description` | 37,813 |
| Wikidata (`wikidata-sel1`) | https://www.wikidata.org — GitHub Explore의 `wikipedia_url`로 고른 개체. 개체마다 `lastrevid`로 고정했다 | CC0 1.0 | 영어 label, aliases, description | 9,184 |
| abbreviations-in-code (`abbrcode-bd33b1c`) | https://github.com/abbrcode/abbreviations-in-code — 커밋 `bd33b1c7bbfea159275c88d223482f5da9a01e58`, `data/abbrs/*.json` | MIT | `word`, `abbr`, `degree`, `context` | 612 |
| software-engineering-abbreviations-acronyms (`se-abbr-8e1d77e`) | https://github.com/wuyichen24/software-engineering-abbreviations-acronyms — 커밋 `8e1d77e99b184106d50a180ba52cb3557273af6d`, `dictionary/**/*.md` | CC0 1.0 | 약어, 전체 이름, 개요 첫 줄 | 716 |

- Wikidata에서 고른 개체 목록과 각 개체의 `lastrevid`는 1라운드 사전과 함께 보존하지 않는다.
- Open English WordNet은 복사본에 `LICENSE.md`와 `WNDB_License.txt`를 함께 두도록 요구한다. 관련어 데이터셋을 배포하려면 위 태그의 두 파일을 함께 넣어야 한다. 지금 로컬 데이터셋 폴더에는 라이선스 파일이 없다.

## LLM 생성 자료

| 자료 | 생성 지침 | 생성 주체 | `rtr1`의 관계 수 |
| --- | --- | --- | ---: |
| 52개 분야별 단어 | `generation/vocab.md` | LLM | — |
| 표제어별 관련어 | `generation/relations.md` | Claude Sonnet(표제어 11,041개), Codex `gpt-6.1-sol` xhigh(표제어 806개) | 519,202, 17,659 |
| 함께 쓰는 단어(`co_use`) | `generation/co-use.md` | LLM | — (뜻·카테고리 데이터셋에만 쓰임) |

- 생성 지침은 `apps/codemap-search/experiments/related-terms-poc/generation/`에 있다.
- 생성 결과는 보존하지 않는다. 같은 지침으로 다시 생성해도 결과는 달라진다.
- 관련어 데이터셋은 LLM 생성 관계의 라이선스를 `NOASSERTION`으로 표시한다.

## 임베딩 모델

| 항목 | 값 |
| --- | --- |
| 모델 | `jinaai/jina-embeddings-v2-base-code` — https://huggingface.co/jinaai/jina-embeddings-v2-base-code |
| 사용한 판 | 로컬 모델 캐시의 `refs/main`: `516f4baf13dec4ddddda8631e019b5737c8bc250` |
| 실행 환경 | `fastembed` 0.9.0, `numpy` 2.5.3 |
| 저장 형식 | 768차원 단위 벡터를 `int8`로 양자화해 관련어 데이터셋에 저장한다. 조회할 때는 모델을 실행하지 않는다 |

## 코드 저장소 12개

단어 평가용 식별자 수집과, 데이터셋 보강·보정에서 코드 근거를 확인하는 데 쓴 고정 체크아웃이다. 이 저장소들은 데이터셋의 원래 관계 출처가 아니다. 표본 언어는 단어 평가용 표본 파일의 언어별 수다.

| 저장소 | 주소 | 태그 | 커밋 | 표본 언어(파일 수) |
| --- | --- | --- | --- | --- |
| bitwarden-server | https://github.com/bitwarden/server | `v2026.9.2` | `9ee4e0ebf502fd1c8bf5c1bbcbc2942c3b66bbcc` | C# 168 |
| express | https://github.com/expressjs/express | `v5.2.1` | `dbac741a49a5a64336b70c06e85c2e2706e36336` | JavaScript 142 |
| flask | https://github.com/pallets/flask | `3.1.3` | `22d924701a6ae2e4cd01e9a15bbaf3946094af65` | Python 83 |
| gin | https://github.com/gin-gonic/gin | `v1.12.0` | `73726dc606796a025971fe451f0aa6f1b9b847f6` | Go(전체) |
| gutenberg | https://github.com/WordPress/gutenberg | `v24.1.0` | `954fe7f9e04631c41aec4552acd627a7203a2aad` | JavaScript 87, TypeScript 47, TSX 31, PHP 14 |
| keycloak | https://github.com/keycloak/keycloak | `26.8.0` | `4246609cf2024c85016d3fb1254c3d2533367c31` | Java 138, TSX 15, TypeScript 9 |
| redis | https://github.com/redis/redis | `8.10.2` | `498ecd0d6d007db11ddb3aea9428552598a78622` | C 22, Python 3, Ruby 1, JavaScript 1 |
| sentry | https://github.com/getsentry/sentry | `26.9.0` | `91e940e6fe4df840c91d865b9d29df9a1b044faf` | TSX 77, Python 66, TypeScript 9, JavaScript 1 |
| terminal | https://github.com/microsoft/terminal | `v1.25.2733.0` | `dc8ae365847d41f62d1ebca93815bb00959bc12f` | C++ 84, C# 2, Python 1 |
| tokio | https://github.com/tokio-rs/tokio | `tokio-1.53.2` | `ff0c4068a4ba5f3af0fe3c5657ba42879b6ea599` | Rust 89 |
| vector | https://github.com/vectordotdev/vector | `v0.59.0` | `a3231c76bdea5078bc54c205cf89f5a1ceddbb73` | Rust 90, Python 1 |
| woocommerce | https://github.com/woocommerce/woocommerce | `11.2.0` | `1699191d5074a22e7f6c43ff2da8fa9942905435` | PHP 75, TypeScript 33, TSX 27, JavaScript 19 |

같은 목록이 뜻 데이터셋 폴더의 `sources.json`에도 있다. 체크아웃 위치는 `.codemap/poc/related-terms/workspaces/checkouts/<저장소>`다. 예를 들어 다음처럼 받을 수 있다.

```sh
git clone --depth 1 --branch v1.12.0 https://github.com/gin-gonic/gin.git \
  .codemap/poc/related-terms/workspaces/checkouts/gin
git -C .codemap/poc/related-terms/workspaces/checkouts/gin rev-parse HEAD
# 73726dc606796a025971fe451f0aa6f1b9b847f6
```
