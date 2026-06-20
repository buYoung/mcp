# 추출 스냅샷 픽스처 (extract snapshots)

이 디렉터리는 `TreeSitterExtractor::extract`의 언어별 출력을 고정(pin)하는 스냅샷 테스트의
입력 픽스처와 정답(golden) 파일을 담는다. 스냅샷은 `lang/` 마이그레이션(브리프셋 children
08/09)이 언어별 분기 체인을 레지스트리 훅으로 바꿀 때, 특정 언어의 플래그가 조용히 뒤집히는
회귀를 잡아내기 위한 동작 보존(behavior-preserving) 안전망이다.

테스트 하니스는 `apps/codemap-search/tests/extract_snapshots.rs`에 있다.

## 디렉터리 구성

- `sample.<ext>`: 지원 확장자별 입력 픽스처 한 개.
- `golden/sample.<ext>.json`: 해당 픽스처에 대한 정답 JSON(`serde_json::to_string_pretty`
  결과 + 끝줄 개행 한 개).

`extract`의 `file_path` 인자로는 디스크 경로가 아니라 픽스처와 동일한 짧은 이름(`sample.rs`
등)을 그대로 넘긴다. `extract`는 `file_path`를 출력의 `filePath` 필드에 그대로 복사하고
이외에는 확장자만 사용하므로, 절대 경로를 넘기면 정답이 머신 종속이 되어 깨진다.

## 정답 재생성 흐름

스냅샷은 현재 동작을 **기록**할 뿐 교정이 아니다. 의도된 변경(예: 명시적으로 승인된 동작
변경)이 있을 때만 다음으로 재생성한 뒤 바뀐 정답을 커밋한다.

```sh
UPDATE_SNAPSHOTS=1 cargo test --manifest-path apps/codemap-search/Cargo.toml \
    --test extract_snapshots
```

환경 변수 없이 실행하면 비교만 수행하며, 차이가 있으면 첫 불일치 줄을 출력하고 실패한다.

```sh
cargo test --manifest-path apps/codemap-search/Cargo.toml --test extract_snapshots
```

`every_fixture_and_golden_is_present` 테스트가 픽스처 목록 자체를 지키므로, 정답 파일을 하나
지우면(또는 픽스처를 빠뜨리면) 스킵되지 않고 실패한다.

## 픽스처가 의도적으로 자극하는 분기

각 픽스처는 해당 언어가 지원하는 한도 안에서 다음 분기를 모두 자극한다: test 표시 심볼,
export된 심볼과 export되지 않은 심볼, deprecated 심볼, 소유자(owner: impl/리시버/클래스/
C++ out-of-line 멤버), docstring, 문자열 리터럴.

## 언어별 누락(omissions)

지원하지 않는 구문은 픽스처가 자극할 수 없으므로 정답에도 나타나지 않는다. 이는 현재
`extract` 동작 그대로이며 버그가 아니다.

- `sample.c`: 소유자(owner) 없음 — C 자유 함수에는 소유 타입이 없다. test 표시는 경로 기반
  탐지만 적용되는데(`path_indicates_test`), 픽스처 이름 `sample.c`는 테스트 경로 패턴에
  해당하지 않으므로 `isTest`는 모두 false다. deprecated는 docstring의 `@deprecated`
  표식으로만 잡힌다.
- `sample.cpp` / `sample.hpp`: test 표시는 C와 동일하게 경로 기반만 적용되어 `isTest`는 모두
  false다(gtest 등 매크로 탐지는 범위 밖). deprecated는 docstring `@deprecated`만 사용한다.
- `sample.s` (GAS 어셈블리): docstring, test 표시, deprecation을 모두 지원하지 않는다.
  어셈블리 주석은 docstring으로 승격되지 않으므로 `docstring`은 항상 null이고, deprecated는
  docstring `@deprecated`에만 의존하므로 항상 false이며, test 표시도 경로 기반만 적용되어 항상
  false다. 문자열 리터럴 또한 쿼리가 캡처하지 않으므로 `literals`는 빈 배열이다. 자극하는
  분기는 export 여부(`.globl`로 export된 라벨 대 일반 라벨)와 심볼 종류(`.macro` 정의와
  라벨)뿐이다.
