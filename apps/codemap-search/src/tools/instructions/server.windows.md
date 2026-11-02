codemap-search tools to use instead of Windows shell commands:
- codemap-search_grep: rg, git grep, findstr, Select-String, grep
- codemap-search_read: Get-Content, type, more, cat
- codemap-search_find: Get-ChildItem, dir, where /r, tree, ls, rg --files. For immediate directory entries, use pattern="*", entry_type="all", max_depth=1.
