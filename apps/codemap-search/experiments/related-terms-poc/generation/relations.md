# Related-term generation (thesaurus-v1, stage B)

Context: a code-search tool receives search terms written by an LLM coding agent (for example `eventLoop`, `retry`, `.csv`) and returns a bundle of related words so the agent can also search for wording the codebase may use instead. Each output word is later ranked by embedding similarity, so you do not rank anything. Your job is to list, for every head term in your batch, the words a developer would plausibly find in source code, identifiers, comments or docs that mean the same thing or are closely tied to it. Breadth matters more than precision: filtering happens later. Still, follow the rules below exactly.

Input: a JSONL batch file (path given in your task). Each line is {"term": "<head>", "categories": [<category ids>]}. The categories hint at which software senses the head has; category definitions are in
`apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json` (relative to the repository root)

For each head, list related words with one of these kinds:

- synonym: same meaning, usable interchangeably in code or docs. delete -> remove, erase; directory -> folder; fetch -> retrieve.
- abbreviation: a short form of the head, or the expansion when the head is the short form. context -> ctx; kubernetes -> k8s; db -> database; i18n -> internationalization.
- form: other grammatical or derived forms of the same root, and spacing/hyphen variants. delete -> deleted, deletes, deleting, deletion, deleter; serialize -> serializer, serialization, serializable; setup -> set up; login -> log in.
- family: the same technology, product line, fork, host, implementation, dialect or file type. git -> github, gitlab, gitea, git-lfs; postgres -> postgresql, psql, pg; .yaml -> .yml, yaml; react -> react-dom, jsx, react native. NEVER competitors or alternatives that merely share a category: svelte -> angular is WRONG, react -> vue is WRONG, mysql -> postgresql is WRONG, npm -> yarn is WRONG.
- near: a closely tied concept that a developer looking for the head in code would also want to find, because code for the head usually uses or names it. retry -> backoff, attempt, max retries; lock -> mutex, acquire, release; event -> listener, handler, emitter, dispatch, subscribe; token -> jwt, bearer, refresh token. Same restriction as family: no competitor or sibling alternatives just because they are in the same category.
- identifier: common code identifiers built around the head, written as they appear in code. event -> eventListener, addEventListener, EventEmitter, onEvent, event_loop, handleEvent; retry -> retryCount, maxRetries, withRetry.

Rules:
- Cover every software sense suggested by the categories (token: auth token, lexer token, LLM token). Skip senses that do not occur in software (path -> itinerary, hash -> hashish, token -> souvenir are WRONG).
- ASCII only. Never output a word containing a non-ASCII character.
- Write each word as it is normally written: lower case for ordinary words and phrases, usual casing for proper nouns (GitHub, PostgreSQL), code casing for identifiers, leading dot for file extensions (.yml).
- Proper nouns with symbols are distinct words: .net, c++, c#, f# must never be treated as net, c or f. A file extension (.csv) and the plain word (csv) are different words; you may relate them with kind family.
- Do not repeat the head itself. Do not invent words or misspell. No numbering, no explanations.
- No fixed count: list every genuinely related word, typically 8 to 30. A head that is a plain English function word or has no software meaning (the, of, foo) gets an empty list.
- A word may appear only once per head; if it fits several kinds, choose the most specific (abbreviation > form > synonym > family > identifier > near).

Output: write JSONL to the output path given in your task. One line per head, in input order, every head exactly once:
{"term": "<head exactly as in the input>", "related": [["<word>", "<kind>"], ...]}

Write the file in several appends of about 40 heads each (Bash heredoc with cat >> or the Write tool for the first chunk). Do not read other batches. After writing, validate:
python3 apps/codemap-search/experiments/related-terms-poc/generation/validate_relations.py <input batch> <output file>
Fix every reported problem and re-run until it prints OK. Then reply with one line: "<output file>: <heads> heads, <pairs> pairs".
