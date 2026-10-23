# 02 — 데이터 배포 준비

2026-10-17에 rts5의 생성 경로를 확인하고 배포용 압축 파일을 두 번 만들었다. 두 파일의 SHA-256이 같으며 압축을 풀면 고정한 원본과 같은 `dataset.sqlite3` 하나만 나온다. **사용자가 2026-10-17에 게시 보류를 선택했다. 태그 push·공개 게시·게시 후 다운로드 검증은 하지 않았다. 로컬 MCP 통합은 계속한다.**

## 데이터 출처 확인

원본은 `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3`다. 크기는 31,715,328 bytes이고 SHA-256은 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`다. 읽기 전용 SQLite 연결로 모든 테이블의 열과 메타데이터 9개를 확인했다.

`pipeline.md`의 생성 경로와 `build_category_dataset.py`의 `canonical_terms`·`relation_rows`, `build_sense_dataset.py`의 `build`·입력 뜻 파생, 보정·매핑 도구, `compact_sense_dataset.py`를 대조했다. 초기 어휘는 `vocab-merged.json`, 관계는 생성 `relations/*.jsonl`과 co-use 배치에서 읽는다. 뜻 빌드 경로는 외부 사전을 병합하는 `build_related_dataset.py`의 rtr1 경로를 사용하지 않는다.

| 테이블 | 행 수 | 생성 단계와 자료 종류 |
| --- | ---: | --- |
| `terms` | 18,566 | 생성 어휘의 표기·종류를 canonical key로 합치고 검토한 추가 항목을 반영한다 |
| `fields` | 39 | LLM이 지정한 분야의 id·이름·범위다 |
| `topics` | 2,545 | 생성 그래프의 주제와 검토된 이름·범위·재분류다 |
| `topic_fields` | 3,225 | 검토된 주제→분야 연결과 순서다 |
| `concepts` | 5,148 | 생성 그래프의 개념과 이후 검토·구조 정리 결과다 |
| `personas` | 44,424 | 생성 관계의 뜻 분할, 병합·주제 매핑 결과다 |
| `relations` | 592,793 | 생성 관련어·co-use 및 검토된 관계다. 압축 단계는 종류를 번호로, 표기 중복을 null로 바꾼다 |
| `relation_kinds` | 7 | 바로 앞 판의 관계 종류를 중복 제거한 표다 |
| `input_senses` | 334,187 | 생성 관계를 통한 입력 뜻과 검토된 추가 매핑이다 |
| `input_relations` | 9 | 검토 보정 계획에서 지정한 입력별 관계다 |
| `sense_meta` | 9 | 아래 도구·경로·규칙과 형식 식별 정보다 |

메타데이터도 모두 다음 경로에 연결된다.

| key | 출처 |
| --- | --- |
| `graph` | 생성 관계를 묶고 검토한 `sense-graph-2-split-review2` 경로다 |
| `input_sense_rule` | head·head_topics 파생 규칙, 최소 head 수와 관계 종류다 |
| `input_senses` | 그 파생 규칙을 설명하는 빌더 문자열이다 |
| `key_rule` | `term_canonical.word_key`다 |
| `migration` | `migrate_sense_dataset.py`의 구조 정리 설명이다 |
| `schema_version` | `rt-sense/5` 상수다 |
| `dataset_id` | `rts5-75e02862daa29308`다 |
| `base_dataset` | `rts4-301d57d419380d08`다 |
| `compaction` | `compact_sense_dataset.py`의 저장 방식 설명이다 |

바로 앞 rts4 판과 변경하지 않는 8개 테이블을 양방향 `EXCEPT`로 전수 비교했고 차이가 0행이었다. rts5 `relation_texts`와 rts4 `relations`의 6개 열도 양방향 차이가 0행이었다. 모든 열·메타 값과 비교 결과는 외부 작업 폴더의 `provenance.json`에 보존했다. 이 확인에서는 외부 사전에서 행을 가져오는 경로를 발견하지 않았다. LLM의 초기 원출력은 보존되지 않으므로 각 생성 문구를 원출력까지 재대조하거나 데이터셋을 재생성한 검증은 아니다.

## 제거한 고지와 보존한 내용

PoC의 기존 고지 파일을 삭제하고 `sources.md`의 고지 안내 bullet, 표의 고지 열, 복사본에 파일을 동봉하라는 문단, 생성 관계의 고지 표시 문장을 제거했다. 통합 설계 문서의 삭제 대상 참조도 현재 설명으로 바꿨다. 외부 자료 주소·고정 판·가져온 항목, 12개 저장소 목록, local rtr1의 `license_spdx` 열·reader, 프로젝트 자체 `LICENSE`와 Cargo의 `license`는 바꾸지 않았다.

## 재현 가능한 패키징

패키징 경로는 `apps/codemap-search/experiments/related-terms-poc/package_release_dataset.py`다. 원본의 크기·SHA-256·schema·id를 먼저 검사하고 USTAR 형식으로 일반 파일 하나를 넣는다. 파일명은 `dataset.sqlite3`, mtime·uid·gid는 0, mode는 0644이며 owner/group 이름은 빈 문자열이다. zstd 1.5.7의 `-19 -T1`을 사용했다. 패키징 후 스트림을 다시 해제해 entry·내용·hash를 검사하고 원본 hash도 다시 확인한다.

작업 디렉터리는 저장소 루트다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/package_release_dataset.py \
  --output-dir /Users/buyong/tmp/rterms-data-release-20261017/build-a
```

`build-b`로 같은 명령을 다시 실행했고 압축 바이트가 같았다. 재현은 같은 zstd 판·수준·스레드 수를 사용한 경우에 확인했다.

| 항목 | 결과 |
| --- | --- |
| asset | `rts5-75e02862daa29308.tar.zst` |
| 압축 크기 | 10,570,357 bytes |
| 압축 SHA-256 | `40d1a95f5bd45670f323835eea62e1f2ff3c5125164ddb30a642866b3abe4102` |
| 추출 크기 | 31,715,328 bytes |
| 추출 SHA-256 | `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced` |
| 압축 내 항목 | 일반 파일 `dataset.sqlite3` 하나다 |

산출물은 `/Users/buyong/tmp/rterms-data-release-20261017/`에 있다. `build-a`에는 archive, `.sha256`, `release-notes.md`, `package-report.json`이 있다. archive에는 보고서·벡터·다른 데이터셋·고지 파일을 넣지 않았다.

## 게시 상태와 승인 대상

확인 시 해당 데이터 릴리스는 `release not found`였고 `releases/latest`는 `codemap-v0.11.0`이었다. 원격 데이터 태그도 없다. 데이터셋을 포함한 커밋 `e473b5f48ed916f771e239e97c4fa5267e368e67`을 태그 대상으로 제안한다. 다만 GitHub commit API는 이 커밋에 HTTP 422를 반환했다. 원격에 없는 커밋이므로 게시하려면 태그와 연결된 이력을 먼저 push해야 한다. 로컬 origin 추적 ref 기준 미전송 이력은 12개다. 이 push 역시 사용자 승인 전에는 실행하지 않는다. 태그는 `related-terms-data-rts5-75e02862daa29308`, 제목은 `관련어 데이터셋 rts5-75e02862daa29308`이며 `--latest=false`를 명시해야 한다.

예정 URL은 `https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst`다. 아직 공개 URL의 다운로드 성공이나 게시 후 latest 상태를 확인한 것으로 취급해서는 안 된다. 승인 뒤 두 asset을 게시하고 공개 다운로드의 압축 checksum·추출 hash·latest 태그를 확인해야 이 단계가 완료된다.
