Search known identifiers, literals, config keys, errors or comments with a regular expression over current files. Use for exhaustive text matches and live edits; constrain path/glob/type to the relevant area. For unknown wording or implementation concepts, use search.

Content mode returns matching named callable bodies by default. Use expand=none for matching rows and line context, files_with_matches to locate files, or count to size results. Prefer view=source_grouped for source without declaration/relationship context: every file has a heading, with original line numbers and match/context markers beneath it. view=source preserves the older path-per-row format. The input schema specifies expansion, pagination and output-mode restrictions.

At most eight returned files receive indexed context within one shared budget. Source truncation, stale context and unresolved call targets remain explicit.
