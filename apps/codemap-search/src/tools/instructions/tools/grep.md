Search current files for known identifiers, literals or regular expressions. Use search only when wording is unknown. Start with the relevant area, then widen path and relax glob/type filters when a required connection is still missing; a scoped miss is not repository-wide absence.

Content mode returns matching named callable bodies by default. Use expand=none for matching rows and line context, files_with_matches to locate files, or count to size results. Prefer view=source_grouped for source without declaration/relationship context: every file has a heading, with original line numbers and match/context markers beneath it. view=source preserves the older path-per-row format. The input schema specifies expansion, pagination and output-mode restrictions.

At most eight returned files receive indexed context within one shared budget. Source truncation, stale context and unresolved call targets remain explicit.

For a match that only names a relationship, inspect its enclosing body with expand=callable or read the missing lines. For a cross-file investigation, check references to discovered names and shared literals outside the first area; verify alternative callers and selection conditions rather than assuming one caller covers all uses.
