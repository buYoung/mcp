Five read-only navigation tools, available after the initial_instructions preflight. Pick by intent, not order. A known file, identifier or source line is enough to start; root overview is not a prerequisite.

* grep: exact identifiers, literals, config keys/defaults, errors, regexes, comments, exhaustive matches or live edits. Scope path/glob/type. After zero matches check filters and regex syntax; do not repeat unchanged or expand guessed synonyms. Use search when wording is unknown.
* search: meaning, ambiguous wording and cross-file relationships. Use sufficient source snippets directly. Read only missing read_suggestion ranges; partial, ambiguous and list-only results are not complete evidence. caller_context=false skips call/value context. Do not re-search returned facts.
* read: a known file window. Use offset/limit from grep, search or overview directly. No intermediate overview is needed when a usable line is known. view="source" skips context work when only source evidence is needed; full remains the default with automatic relevant relationships.
* overview: orient only when the repository area is unknown, or obtain ranges missing from other results. Root maps can be large. A file overview lists significant indexed declarations; unexported function-local callables may be omitted; absence there is not absence of code.
* find: file-path discovery by glob, not content search.

Stop when the requested facts have sufficient source evidence. Static calls, event routes and value candidates retain their own certainty labels; they do not establish runtime delivery/order. Scope, exclusions, freshness and analysis/output limits apply. A partial analysis notice does not require rereading source already shown.
