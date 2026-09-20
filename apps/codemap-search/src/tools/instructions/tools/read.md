Read a known file and line range from the working tree, including recent edits. Use locations returned by grep/search directly. To locate unknown text, use grep; to discover an implementation with unknown wording, use search.

The default full view combines source with declaration and relationship context. Choose view=source for file content alone, or expand=callable for the enclosing named callable. The input schema describes how expansion changes the requested line window.

Unbounded large reads and windows exceeding the output cap are refused. For an oversized callable, use expand=none with smaller offset/limit windows. Missing or stale indexed context is reported separately from the live source.
