codemap-search: six read-only code-navigation tools — `initial_instructions` plus five navigation tools (`search`, `grep`, `overview`, `read`, `find`). Call `initial_instructions` once with no arguments before the navigation tools to load the usage rules and navigation flow. Some clients hide or compress these server instructions, so do not rely on them alone.

Filesystem permissions for `read`, `find`, and `grep`:
1. `workspace`: only paths inside the current workspace.
2. `allowed_roots`: `workspace` plus the additional paths shown in the tool description.
3. `anywhere`: any filesystem path reachable by the process.
