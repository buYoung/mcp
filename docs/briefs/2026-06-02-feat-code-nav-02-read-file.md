# [feat] Add read_file tool (Claude Code Read mirror)

## Work Type
feat

## Current State (As-Is)
- `apps/code-nav` has no file-read tool; an agent cannot fetch a file or a line range to confirm a search hit.
- The C4 pipeline's "read" confirmation step — the quality multiplier that fixes M3-L enum false positives to precision 1.00 — has no implementation here.
- Verified this session against `/Users/buyonglee/Downloads/claude-code-main/src/tools/FileReadTool/`: the exact `FILE_UNCHANGED_STUB` string (prompt.ts:7-8), the empty-file and offset-exceeds `<system-reminder>` strings (FileReadTool.ts:706-707), the 256KB (262144) byte cap, and the runtime defaults `offset=1` / `limit=undefined`.

## Desired Outcome (To-Be)
- A `read_file` tool returns a file (or an `offset`/`limit` range) in cat -n format with line numbers, mirroring Claude Code Read semantics.
- Host/UI/telemetry concerns are dropped; image, PDF, and Jupyter reading are unsupported with explicit errors (decision 7).
- A re-read of the same range on an unchanged file returns the `FILE_UNCHANGED_STUB` instead of resending content.

## Scope
### In Scope
- New `ReadProvider` `read-file`, a `line-numbering` serializer, and a `read-state-store` for dedup.
- Input fields: `file_path` (required), `offset`, `limit`.
- 256KB byte cap applied only when `limit` is unset (throw past it; explicit `limit` bypasses the byte cap — preserve this asymmetry).
- `file_unchanged` dedup keyed on (path, offset, limit) with timestamp `Math.floor(mtimeMs)`.
- Exact empty-file and offset-exceeds `<system-reminder>` strings; directory (EISDIR), binary-extension, blocked-device, and UNC rejection; `expandPath` normalization.
- cat -n line format: compact `<number><TAB><content>` joined by `\n`, numbers starting from the raw `offset`.
### Out of Scope
- [hard] image reading (png/jpg/jpeg/gif/webp) — return an explicit "not supported in v1" error.
- [hard] PDF / Jupyter notebook reading — explicit error; drop the `pages` parameter from the schema.
- [deferred] a local token-count cap to replace the dropped 25000-token cap.
- [deferred] large-file streaming optimization (>10MB fast path).

## Constraints
- Reproduce these exact strings verbatim (inlined here so this brief is self-sufficient; they were extracted from the Claude Code source this session):
  - file_unchanged stub: `File unchanged since last read. The content from the earlier Read tool_result in this conversation is still current — refer to that instead of re-reading.`
  - empty file: `<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>`
  - offset beyond EOF: `<system-reminder>Warning: the file exists but is shorter than the provided offset (N). The file has M lines.</system-reminder>` where N is the requested offset and M is the file's line count.
- The dedup timestamp must use `Math.floor(mtimeMs)` — comparing raw float mtimes breaks equality.
- Binary-extension policy: images (png/jpg/jpeg/gif/webp) and PDF/Jupyter return the explicit unsupported error; SVG is read as text; other unambiguously binary extensions are rejected. Reuse Claude Code's image-extension set as the image list.
- Reuse `path-guard`, `tools/arguments.ts`, and the `textResult` helper.

## Related Files / Entry Points
- `apps/code-nav/src/tools/index.ts` — register `read_file` and dispatch here (shared conflict hotspot).
- `apps/code-nav/src/security/path-guard.ts` — `expandPath` and boundary check; extend with blocked-device and binary-extension rejection.
- `apps/code-nav/src/config/defaults.ts` — add the 256KB cap, blocked-device list, binary extensions, and the `FILE_UNCHANGED_STUB` string.
- `apps/code-nav/DESIGN.md` — §4.1 carries the per-item reproduce/drop decisions and the exact constants.
- `apps/code-nav/src/providers/read/read-file.ts` (proposed) — read body and reminder branches.
- `apps/code-nav/src/providers/read/line-numbering.ts` (proposed) — compact cat -n serializer.
- `apps/code-nav/src/providers/read/read-state-store.ts` (proposed) — dedup map.

## Side Effect Checkpoints
- [ ] Extending `path-guard` does not break `search_text`'s existing `path` validation.
- [ ] Reading a known file returns exact cat -n format with line numbers starting from the raw offset.
- [ ] Binary/image/PDF paths return the explicit unsupported error rather than garbled bytes.

## Acceptance Criteria
- [ ] `read_file` of a known file returns numbered lines; an `offset`/`limit` subset is correct and numbered from the raw offset.
- [ ] An empty file returns exactly `<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>`.
- [ ] An `offset` beyond EOF returns the exact offset-exceeds reminder with the file's line count.
- [ ] The same (path, offset, limit) re-read with unchanged mtime returns `FILE_UNCHANGED_STUB`; after an mtime change it returns real content.
- [ ] An image or PDF path returns the explicit "not supported" error.

## Open Questions
- Should the dedup `read-state-store` be shared with `lookup_symbol`'s fingerprint cache or kept separate? Decide during implementation.
