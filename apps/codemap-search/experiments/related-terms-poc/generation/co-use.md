# Task: list the words code uses together with each head word

Your input is a list of developer-vocabulary head words, one per line. For each head word, list 10-25 words that
source code dealing with that word typically uses **together with it in the same function, class or file**:
companion API names, parameters, fields, types, operations, error names, configuration keys and closely tied
concepts, written as code or documentation would spell them (`maxRetries`, `AbortController`, `Retry-After`,
`exponential backoff`).

- Cover the word's common meanings in programming; for an ambiguous word, include companions for each meaning.
- Do not list spelling variants or inflections of the head word itself (`tokens`, `tokenize` for `token`).
- Do not list very generic words that every file uses (`function`, `return`, `data`, `value`, `error`, `string`).
- Use only your own knowledge; do not search the file system or read other files.

Write one JSON object per head word, in input order, JSON Lines, UTF-8:
{"word": "<head word exactly as given>", "co_used": ["...", "..."]}
Check that every head word appears exactly once. Reply with only the number of head words written.
