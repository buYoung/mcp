# Private Answer Key

## Candidate

- id: `clickhouse-unused-fallback-slots-observable-v2`
- fixture: `ClickHouse v25.3.5.42-lts` at `14e08ead34a7900d75e3d378f87cabfba9f8c8d9`
- status: validated

## Correct Answer

The observable behavior comes from the runtime dictionary function, not from the removed historical analyzer pass. `FunctionDictGetNoType` handles the multi-attribute `dictGetOrDefault` form by accurately casting and validating the complete default tuple, splitting every tuple column into defaults, and passing the whole shape into `executeDictionaryRequest`. The multi-attribute branch obtains all requested columns, wraps them as a tuple, and restores short-circuit columns for every result/default element. A bad fallback slot can therefore remain observable even if a downstream consumer uses only one field.

The scalar branch instead calls `getColumn`, so the tuple and scalar paths are not interchangeable merely because one output field is ultimately consumed. `FunctionDictGet`, `FunctionDictGetOrDefault`, and the FunctionFactory registrations are accepted public aliases for this runtime path.

## Canonical Evidence

1. `src/Functions/FunctionsExternalDictionaries.h:297-327` defines `FunctionDictGetNoType` and the function names.
2. `src/Functions/FunctionsExternalDictionaries.h:456-497` accurately casts and validates the full default tuple and extracts all default columns.
3. `src/Functions/FunctionsExternalDictionaries.h:600-602` enters `executeDictionaryRequest`.
4. `src/Functions/FunctionsExternalDictionaries.h:651-710` distinguishes multi-column tuple retrieval/restoration from scalar `getColumn`.
5. `src/Functions/FunctionsExternalDictionaries.h:774-875` defines the accepted aliases.
6. `src/Functions/FunctionsExternalDictionaries.cpp:56-67` registers the public functions.

## Pinned Facts

- The complete default tuple is cast and shape-checked before individual consumption.
- Multi-attribute execution retrieves and restores all tuple elements.
- The scalar path uses a distinct `getColumn` branch.
- The implementation is the runtime dictionary function; the absent historical analyzer pass and LowCardinality-collapse claim are not canonical for this fixture.
