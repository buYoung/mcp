Find behavior, ambiguous concepts or cross-file relationships when exact wording is unknown. BM25 searches indexed symbols, docs and literals with identifier splitting. For known exact text, exhaustive matches or just-edited files, use grep; ranked index results are not an exhaustive live-text scan.

Results include snippets, match reasons, symbol ranges and read_suggestion. Exact-name queries preserve exact ranking; composite queries favor coverage and may show a later body window.

Supplementary relationships use displayed matching evidence and only the bytes left after ranked details and compact tail rows; they never shorten ranked results. Exact event-key maps use a dedicated layout. Unsupported arguments are errors, including path and per-request limit.

Use ranked results to choose source locations, not to decide that all relevant code has been found. Once useful names or literals appear, use grep to enumerate references and read enough enclosing source to resolve unclear behavior. In monorepos, set workspace_scope explicitly on every scoped call; use 'all' for discovery across areas. Check every requested part, not just the claims already selected for the answer. Cite source at both ends of each connection and the link between them. Partial, ambiguous or list-only output leaves evidence missing; report unresolved parts explicitly.
