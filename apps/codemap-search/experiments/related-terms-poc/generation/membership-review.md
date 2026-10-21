# Task: review which concept each word meaning belongs to

A developer-vocabulary graph splits every word into meanings ("personas"), each shown by the words it was found
related to (`evidence`). Meanings are grouped into small **concepts** (median about 10 words) and concepts into named
**topics**. A lookup tool answers an LLM coding agent's code-search query with the query word's concept-mates as its
closest related words, so a wrong member shows up as a wrong suggestion.

## Input (JSON Lines, three record types)
- `review`: a concept to review — `concept` id, `name`, `topic` id, `members` (its words) and `items`. Each item is one
  membership: `item` id, `word`, `evidence` (the meaning's related words), `alternatives` (other concept ids that hold
  many of those related words) and `topics` (topic ids for a new concept).
- `concept`: an alternative concept referenced by items — id, `name`, `topic`, `members`.
- `topic`: a referenced topic — id, `label`, `scope`.

## Question for every item
**An LLM searching code with a word of this concept gets this word as a close related word. In the meaning its
evidence shows, would code relevant to this concept likely also use this word?**
("이 검색어로 코드를 찾는 LLM에게, 관련 코드가 이 단어를 함께 쓰고 있을 만한가")

## Answer with one action
- `keep`: yes, or the fit is acceptable and no listed alternative is clearly better. Most items should be `keep`.
- `move` + `"to": <concept id>`: this meaning clearly belongs in one of the item's `alternatives` instead.
- `split` + `"parts": [{"evidence": [...], "to": <id>}, ...]`: the evidence mixes different meanings. Put shown
  evidence words into parts; one part must stay with the current concept id, the others go to listed alternatives;
  each evidence word at most once; unlisted evidence words stay with the current concept.
- `merge` + `"with": <concept id>`: the current concept and a listed alternative are, as wholes, the same subject and
  should be one concept. Not for a single misplaced word (use `move`).
- `detach` + `"topic": <topic id>` + `"name": "<short concept name>"`: the meaning fits neither the current concept nor
  any listed alternative; start a new concept under the best of the item's listed `topics`.
- Optional on any answer: `"evidence_wrong": true` when the evidence words are not really related to the word (bad
  source data). Still choose an action; nothing in the source is edited.

Change only when the change is clearly better; judge from your knowledge of real code. Do not open any file other than
this brief and your input batch.

## Output
Write `decisions-NN.jsonl` (same NN as the batch) next to the input batch: one JSON object per item, in input order.
```json
{"item": "M000083", "action": "keep"}
{"item": "M016454", "action": "move", "to": 1409, "note": "FFI 바인딩 개념에 더 가깝다"}
```
`note` is a short reason (Korean is fine) for every action other than `keep`.
Run the check command given in your task until it prints `ok`. Reply with only the action counts.
