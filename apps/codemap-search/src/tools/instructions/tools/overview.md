Inspect an unfamiliar repository area or locate declaration ranges missing from other results. Use grep for a known identifier and read for a known file window. In monorepos, choosing a path also sets the scope of subsequent search calls.

A repository root lists selectable workspace scopes with file/symbol counts and leading languages in monorepos, or a bounded directory map otherwise. Folders list significant declarations, source-backed signatures and nested members without line ranges. Files list significant indexed declarations with exact ranges; unexported function-local declarations may be omitted.

Repository and workspace roots also include language statistics unless tool_output.is_overview_stats_enabled=false. Counts cover indexed physical files within that root, use tokei with total = code + comments + blanks, fold embedded language blobs once into their physical file's language, and report unavailable files. Other paths omit statistics. Pending collection is explicit; retry to reuse completed counts.

If grep/search already supplied the needed file and line, use read directly.
