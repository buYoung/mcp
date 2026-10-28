Read a known file and line range from the working tree, including recent edits. Use ranges to read several known file windows in one call. Use locations returned by grep/search directly. To locate unknown text, use grep; to discover an implementation with unknown wording, use search.

The default full view combines source with declaration and relationship context. view=source returns the file content and may end with callers from outside the window for the functions it shows; expand=callable reads the enclosing named callable. The input schema describes how expansion changes the requested line window.

Unbounded large reads and windows exceeding the output cap are refused. With ranges, errors stay per item and the shared byte budget defers whole items with a continuation notice. For an oversized callable, use expand=none with smaller offset/limit windows. Missing or stale indexed context is reported separately from the live source.
