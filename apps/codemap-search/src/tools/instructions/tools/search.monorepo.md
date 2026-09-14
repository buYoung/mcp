Find behavior, ambiguous concepts or cross-file relationships when exact wording is unknown. Use scoped grep for known exact text, config keys, errors or exhaustive enumeration. Search uses the active overview scope; workspace_scope overrides it. With no chosen scope, use workspace_scope="all" for discovery, then narrow to actual implementation paths and languages.

Every scope listed by root overview is accepted, including conventional workspaces and top-level source roots; a basename must be unique. Subdirectories remain narrow, a file selects its parent, and all/전체 resets repo-wide search. The tool never widens an explicit or active scope automatically.

Results include snippets, match reasons, symbol ranges and read_suggestion. Exact-name queries preserve exact ranking; composite queries favor coverage. Answer from sufficient source evidence. Read only missing ranges; no intermediate overview or reread of visible evidence is required. Partial, ambiguous or list-only results are not complete evidence.

Matched functions add bounded call/value relationships; caller_context=false skips them and callsite implementation lookup. Declaration/implementation links and static collection relationships remain independent. Eligible indexed events are automatic; include_events=false suppresses them, and event_key selects an exact map with query still required. Explicit event_navigation.is_enabled=false disables event analysis. Relationships respect scope, exclusions and freshness. Source/model/candidate and precise/unresolved labels distinguish evidence; static relationships do not guarantee runtime delivery or callback execution.

Supplementary sections use only displayed matching evidence, share the bytes left after ranked details/tail, and never shorten search results. Named declaration sections and file-grouped results are limited to grep/read. [analysis limit] marks unavailable/bounded value summaries; [unresolved] marks unproven value/call semantics.

Unsupported arguments are errors: path and per-request limit are not supported. Use workspace_scope (alias: scope) and documented keys query, caller_context, language_hint and extension_hint.
