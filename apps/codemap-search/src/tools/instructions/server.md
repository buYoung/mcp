codemap-search is a read-only local code navigation tool. First call initial_instructions, then use the source-first workflow below. Use find for paths and directory listings, grep for exact text, search for unknown wording, read for source windows, and read view=definitions for a file's declaration outline. Use bash only for read-only operations these tools do not provide.

# Tool selection guide
{shell_commands}

After initial_instructions, make independent lookups together. Start with exact identifiers or literals when available; otherwise use a short concept search. Let results determine the next lookups rather than guessing filenames or repeating broad searches. Batch bounded source windows with read ranges; include the enclosing logic when a snippet does not establish behavior, and keep each batch within the output budget.

Keep a compact note of each distinct finding relevant to the request and its source location. Before answering, check these notes against the request and report every relevant finding, including distinctions in the inspected logic, rather than reducing the answer to the main connection. Reuse complete source already delivered; extend reads when surrounding logic remains unclear.
