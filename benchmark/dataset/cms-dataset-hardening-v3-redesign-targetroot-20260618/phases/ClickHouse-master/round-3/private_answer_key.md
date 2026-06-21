# Private Answer Key

## Candidate

- id: `clickhouse-unused-fallback-slots-observable-v1`
- status: draft

## Correct Answer

The discarded fallback fields remain observable because the relevant dictionary-enrichment path is tuple-valued, and the original multi-field form still evaluates and type-checks the whole fallback tuple even when downstream logic keeps only one element.

The specific hidden contracts are:

1. In the multi-attribute `dictGetOrDefault` path, the runtime handles a tuple of result columns and restores defaults for the whole tuple shape, not just the selected field. That means an invalid unselected fallback element can still throw before selection discards it.
2. The analyzer optimization that would collapse a tuple-valued lookup plus field extraction into a single-field lookup must therefore prove exact type equality for every element of the full default tuple. Otherwise collapsing would suppress an exception from an unselected fallback slot.
3. If the fallback is written as `tuple(...)`, every slot must be a constant before collapse is allowed. Otherwise removing the unselected slots would skip their side effects or exceptions.
4. Even when the selected value is the same, the collapsed one-field lookup can surface a different parent-visible type because the scalar form can preserve `LowCardinality` from the key while the tuple-valued form cannot. So the optimizer also requires exact post-rewrite result-type equality and bails out on mismatch.

## Why

1. The source that makes the symptom real is the dictionary function runtime. The multi-attribute branch works with a tuple of result columns and applies default restoration across the tuple columns; the single-attribute branch restores only the selected attribute column. See `src/Functions/FunctionsExternalDictionaries.h:716-775`.
2. The analyzer pass that tries to collapse the multi-field lookup is `DictGetTupleElementPass`. It targets tuple-valued `dictGet` / `dictGetOrDefault` followed by static field extraction. See `src/Analyzer/Passes/DictGetTupleElementPass.cpp:111-155`.
3. The pass explicitly documents the exception-preservation rule: `dictGetOrDefault` casts the entire default tuple to the dictionary attribute tuple type at execution time, so an unselected default element with the wrong type can still throw. The pass bails out unless every default tuple element type exactly equals the corresponding result tuple element type. See `src/Analyzer/Passes/DictGetTupleElementPass.cpp:156-194`.
4. The pass adds a second guard for `tuple(...)` defaults: all tuple arguments must be constants, because dropping the unselected ones can otherwise suppress side effects or exceptions. See `src/Analyzer/Passes/DictGetTupleElementPass.cpp:196-225`.
5. The pass adds a third guard for result-type preservation: after rebuilding the one-field lookup, it checks that the rebuilt scalar result type exactly equals the original extracted-field type. The comment names the concrete mismatch: single-attribute `dictGet` can preserve a `LowCardinality` wrapper from the key, while the tuple-valued form drops it. See `src/Analyzer/Passes/DictGetTupleElementPass.cpp:244-250`.
6. The test file pins each symptom:
   - non-constant tuple fallback must bail out: `tests/queries/0_stateless/04051_optimize_dictget_tuple_element.sql:103-108`
   - low-cardinality type mismatch bailout: `tests/queries/0_stateless/04051_optimize_dictget_tuple_element.sql:130-136`
   - invalid unselected fallback element must still throw: `tests/queries/0_stateless/04051_optimize_dictget_tuple_element.sql:138-145`
   - corresponding reference outputs for the bailout cases: `tests/queries/0_stateless/04051_optimize_dictget_tuple_element.reference:55-63`

## Pinned Facts

- Target mechanism: tuple-valued `dictGetOrDefault` / `dictGet` followed by field extraction.
- Unselected fallback slots remain observable because the original runtime still casts/restores the whole tuple shape.
- Dropped tuple fallback elements can still matter through side effects or exceptions unless they are constant.
- One-field collapse can also change the visible type through `LowCardinality`, so result-type equality is enforced.
