# Development curation v2 rubric (WordNet synsets)

Frozen before any v2 judgment. Calibration examples deliberately avoid the 92 development inputs in `cases.json`.

## Task

Each record is one WordNet synset: `id`, part of speech, lexicographer file, definition, examples, and member words `m`.
Decide which members to KEEP for a code-search related-terms dictionary. Kept members of one synset become mutual
synonyms suggested to a coding agent that searches source code. Every other member is dropped.

## Keep a member only when all four hold

1. **Code sense.** The definition itself describes what a program, data, a system, or a software artifact does or is,
   or a direct numeric/mathematical operation. Human, social, emotional, bodily, physical-material, artistic, legal,
   commercial, sports, and person-to-person communication senses fail, even when the same word is common in code
   with a different sense.
2. **No invented bridge.** If keeping it requires reasoning such as "this could describe documentation, a design
   discussion, team process, or a developer's own thinking", it fails.
3. **Member used in code with this sense.** A developer would plausibly write this member, meaning this definition,
   in an identifier, API or function name, CLI flag, config key, log or error message, or code comment. Drop literary,
   rare, regional, archaic, or idiomatic members, and phrasal verbs that are not established in code. Established
   code phrases such as `set up`, `tear down`, `look up`, `log in`, `shut down`, `roll back` can pass.
4. **Unsure means drop.** Precision matters more than coverage here.

Keeping a single member is allowed; it simply produces no synonym pair.

## Calibration

| Synset | Members | Keep | Why |
|---|---|---|---|
| 02569242-v carry out a process or program, as on a computer or a machine | run, execute | run, execute | program execution sense |
| 01930264-v move fast by using one's feet | run | — | bodily motion |
| 02448714-v direct or control; projects, businesses, etc. | operate, run | — | business management, not program behavior |
| 01512805-v cause to go on or to be engaged or set in operation | throw, flip, switch | flip, switch | `throw` is not used in code with this sense |
| 00456484-v rinse, clean, or empty with a liquid | flush, scour, purge | — | physical liquid sense, although `flush`/`purge` are code words in other senses |

## Output

Write one JSON object per input record, in input order, to the output path you are given:

```json
{"id": "02569242-v", "keep": ["run", "execute"], "reason": "프로그램 실행 뜻"}
```

- `keep` uses member strings exactly as listed in `m`; use `[]` when nothing passes.
- `reason` is one short Korean phrase (20 words or fewer) naming the deciding rule.
- Cover every input `id` exactly once. Do not add members, ids, or fields.
