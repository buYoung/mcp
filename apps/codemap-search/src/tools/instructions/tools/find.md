Locate file or directory paths by glob or basename regex. Use when the filename or path shape is known; this does not search file contents. Use grep for exact content and search for implementation concepts.

Defaults select regular files with glob matching. A slash-less pattern such as '*rpc*' matches entry basenames, not ancestor directories; use '**/rpc/**' for entries under a directory component. Narrow with path, entry_type and max_depth. Results are mtime-sorted and capped; directories have a trailing slash.
