# tree-sitter-c-sharp 0.24.0의 알려진 파싱 문제

codemap-search는 C# 파일을 tree-sitter-c-sharp 0.24.0으로 파싱합니다. 0.24.0은 0.23.5보다 오류가 나는 파일이 적지만, `&&` 조건 중간에 `#if` 블록이 있는 한 가지 형태에서는 파일 전체를 ERROR 노드 하나로 읽습니다. 그러면 그 파일의 선언 대부분이 색인에서 빠집니다. 상위 저장소의 `dev` 브랜치는 이 형태를 오류 없이 읽지만, 2026-11-27에 확인한 시점까지 그 수정이 든 릴리스는 없습니다.

## 요약

- **문제가 되는 형태**: `&&` 조건 중간의 `#if … #endif` 블록 안 피연산자에 `typeof(...)`가 있고, `#endif` 다음 피연산자가 `!`로 시작합니다. 두 조건 중 하나만 맞으면 0.24.0도 오류를 그 자리에 가둡니다.
- **빈도**: 확인한 C# 파일 1,366개 중 이 형태로 파일 전체가 오류가 된 파일은 Newtonsoft.Json의 `JsonSerializerInternalReader.cs` 하나입니다. 이 파일의 선언은 63개에서 20개로, 호출은 599개에서 341개로 줄었습니다.
- **0.24.0을 쓰는 이유**: 같은 파일 묶음에서 파일 전체가 오류인 파일은 0.23.5에서 2개, 0.24.0에서 1개입니다. 국소 오류가 있는 파일도 91개에서 74개로 줄었습니다. godot의 `InteropStructs.cs`는 0.23.5에서 파일 전체 오류였지만 0.24.0에서 오류 없이 파싱되어 선언이 127개에서 281개로 늘었습니다.
- **상위 상태**: `dev` 브랜치의 #483(2026-11-20 머지)이 이 형태를 고쳤습니다. 2026-11-27 기준 crates.io의 최신 버전은 0.24.0입니다.

## 확인 범위와 방법

비교한 문법은 네 가지입니다.

| 문법 | 출처 |
| --- | --- |
| 0.23.5 | crates.io |
| 0.24.0 | crates.io, 2026-11-18 게시 |
| `master` e3971fc | 상위 저장소, 2026-11-18 |
| `dev` f802dfb | 상위 저장소, 2026-11-26 |

파일은 다음 1,366개입니다.

| 출처 | 커밋 | C# 파일 수 |
| --- | --- | --- |
| Newtonsoft.Json | 52fa3ae | 951 |
| godot | 7c97c297 | 314 |
| Pester | 98e56d6 | 69 |
| 이 저장소의 테스트 픽스처와 실험 코드 | — | 32 |

각 파일을 각 문법으로 파싱해서, 루트 노드가 파일 전체를 덮는 ERROR이면 파일 전체 오류로, 루트는 정상이고 ERROR 노드가 있으면 국소 오류로 셌습니다. 선언 수와 호출 수는 codemap-search `parse` 출력의 `symbols`와 `navigation.calls` 개수입니다.

## 발견

### 조건식 중간의 `#if` 블록

다음 19줄이 최소 재현입니다. `JsonSerializerInternalReader.cs` 875~879행과 같은 모양입니다.

```csharp
namespace N
{
    class C
    {
        void M(Type objectType, Type specifiedType)
        {
            if (objectType != null
#if HAVE_DYNAMIC
                && objectType != typeof(IDynamicMetaObjectProvider)
#endif
                && !objectType.IsAssignableFrom(specifiedType))
            {
                throw new Exception();
            }
        }

        void After() { }
    }
}
```

0.24.0으로 파싱하면 `#endif` 줄이 `binary_expression` 안의 ERROR로 남고, 그 뒤부터 파일 끝까지 복구하지 못해 루트가 ERROR가 됩니다. 0.23.5는 같은 파일에서 국소 ERROR 하나만 남기고 `After`까지 정상으로 읽습니다.

0.24.0에는 식 안에 놓인 `#if` 블록을 식의 일부로 읽는 `preproc_if_in_expression` 규칙이 새로 들어갔습니다(상위 커밋 59cde90, PR #464). `#endif`가 식 노드 안에서 ERROR가 되는 것으로 보아, 파서는 이 규칙으로 `#if` 블록을 읽기 시작했다가 `#endif`에서 실패합니다. 이 해석은 출력 트리에서 추론한 것이며, 규칙을 빼고 문법을 다시 생성해 확인하지는 않았습니다.

재현 파일의 피연산자를 바꾸면 결과가 다음과 같이 갈립니다.

| 형태 | `#if` 안 피연산자 | `#endif` 뒤 피연산자 | 0.23.5 | 0.24.0 | `dev` |
| --- | --- | --- | --- | --- | --- |
| 재현 파일 | `objectType != typeof(IDynamicMetaObjectProvider)` | `!objectType.IsAssignableFrom(specifiedType)` | 국소 오류 | 파일 전체 오류 | 오류 없음 |
| `typeof` 없음 | `objectType != specifiedType` | `!objectType.IsAssignableFrom(specifiedType)` | 국소 오류 | 국소 오류 | 오류 없음 |
| `!` 없음 | `objectType != typeof(IDynamicMetaObjectProvider)` | `objectType.IsAssignableFrom(specifiedType)` | 국소 오류 | 국소 오류 | 오류 없음 |
| 둘 다 없음 | `objectType != specifiedType` | `ok` | 국소 오류 | 국소 오류 | 오류 없음 |
| `&&`가 줄 끝에 있음 | `a != b &&` | `b != null` | 파일 전체 오류 | 파일 전체 오류 | 오류 없음 |

마지막 형태는 0.23.5에서도 파일 전체 오류이므로 0.24.0의 회귀가 아닙니다. 0.23.5와 0.24.0은 모든 형태에서 적어도 국소 오류를 내므로, 조건식 안의 `#if`를 오류 없이 읽는 것은 `dev`뿐입니다. `master` e3971fc의 결과는 모든 형태에서 0.24.0과 같습니다.

### 오류 파일 수

같은 1,366개 파일에서 센 결과입니다.

| 문법 | 파일 전체 오류 | 국소 오류가 있는 파일 |
| --- | --- | --- |
| 0.23.5 | 2 | 91 |
| 0.24.0 | 1 | 74 |
| `dev` f802dfb | 0 | 21 |

0.23.5에서 파일 전체 오류였던 두 파일은 godot의 `InteropStructs.cs`와 `MustBeVariant.GD0301.cs`입니다. `InteropStructs.cs`는 0.24.0에서 오류 없이 파싱됩니다. `MustBeVariant.GD0301.cs`는 분석기 테스트용 표식 `{|GD0301:object|}`이 든, 올바르지 않은 C#입니다. 0.24.0에서는 루트 노드가 정상이지만 클래스 전체(6~672행)가 여전히 ERROR입니다. 0.24.0에서 파일 전체 오류인 파일은 `JsonSerializerInternalReader.cs` 하나입니다.

### 포인터 캐스트 뒤의 호출

0.24.0은 `*(movable*)CustomUnsafe.AsPointer(ref value)`를 캐스트 식 전체를 호출하는 식, 곧 `(*(movable*)CustomUnsafe.AsPointer)(ref value)`로 읽습니다. 오류 없이 파싱되지만 호출 대상이 이름이 아니어서, codemap-search는 이 `AsPointer` 호출을 찾지 못합니다. 0.23.5는 이 줄에서 국소 오류를 냈고, `dev`는 캐스트 안의 호출로 바르게 읽습니다.

확인한 파일에서 이 형태는 godot `InteropStructs.cs`의 10줄에만 있습니다. 이 파일은 0.23.5에서 파일 전체 오류였기 때문에, 0.24.0으로 바꾸면서 선언은 127개에서 281개로 늘고, 호출은 131개에서 77개로 줄었습니다. 줄어든 호출 73개 중 61개는 `[StructLayout(...)]`, `[MethodImpl(...)]`, `[FieldOffset(...)]` 같은 특성(attribute)을 0.23.5의 ERROR 노드 안에서 호출로 잘못 센 것이고, 2개는 `if`를 호출로 센 것이며, 10개가 이 형태의 `AsPointer` 호출입니다.

## codemap-search에 주는 영향

- 파일 전체 오류가 된 파일은 선언, 호출, 구현 관계의 대부분을 잃습니다. 다른 파일에서 그 파일의 빠진 선언으로 가는 호출과 구현 관계도 연결되지 않습니다.
- 다른 파일의 파싱 결과는 달라지지 않습니다. 파일은 하나씩 따로 파싱됩니다.
- 포인터 캐스트 뒤의 호출은 그 호출 하나만 빠집니다.

## 상위 저장소 상태

2026-11-27에 tree-sitter/tree-sitter-c-sharp에서 확인했습니다.

- **#483** "perf!: parse every directive line through one extra and skip inactive branches"(8d7021e)가 2026-11-20에 `dev`로 머지되었습니다. `dev` f802dfb는 위 표의 모든 형태와 `JsonSerializerInternalReader.cs`를 오류 없이 읽습니다. 제목의 `perf!:`는 호환성을 깨는 변경이라는 표시입니다.
- 이 형태를 다루는 열린 이슈는 없습니다.
- **#377** "C# Parser Fails (Root ERROR Node) with Nested Preprocessor Directives Conditionally Defining Method Parameters"는 중첩된 지시문 때문에 루트가 ERROR가 되던 문제이고, 2026-11-20에 닫혔습니다.
- **PR #453** "fix: let an `#if` block hold whole elements of an initializer or collection"은 머지되지 않고 2026-11-18에 닫혔습니다.
- **#495** "dev: incremental reparse differs from a fresh parse after deleting the newline of an `#if` line (since #483)"은 열려 있습니다. #483 이후 증분 재파싱 결과가 처음부터 파싱한 결과와 달라지는 문제로, 이 문서의 문제와는 다릅니다. codemap-search는 이전 트리 없이 처음부터 파싱하므로(`src/parser/bounded.rs`) 직접 영향은 없습니다.

## 다음 버전으로 올릴 때

#483이 든 버전이 crates.io에 나오면 올리는 것을 권장합니다. 올릴 때 다음을 확인합니다.

1. 이 문서의 재현 파일과 `JsonSerializerInternalReader.cs`가 오류 없이 파싱되는지 확인합니다.
2. `InteropStructs.cs`에서 `AsPointer` 호출 10개가 다시 나오는지 확인합니다.
3. #483은 호환성을 깨는 변경으로 표시되어 있으므로, 노드 이름이 바뀌었는지 `node-types.json`을 비교하고 `queries/csharp`와 C# 추출 코드를 맞춥니다.
4. 1,366개 파일 전체에서 이전 바이너리와 파싱 결과를 비교합니다.

## 한계

- 조건식은 `&&`로 이은 `if`·`while`·대입만 확인했습니다. `||`, 삼항 연산자, 메서드 인자 안의 `#if`는 확인하지 않았습니다.
- 확인한 파일은 네 묶음뿐입니다. 여러 대상 프레임워크를 `#if`로 나눠 지원하는 라이브러리에서는 이 형태가 더 자주 나올 수 있습니다(추정).
- `dev`는 아직 릴리스되지 않았으므로 릴리스 전에 동작이 바뀔 수 있습니다.
