# Round 3 Validation Summary

Candidate: `clickhouse-unused-fallback-slots-observable-v1`

- run 1: `run-1/validation_transcript.md` -> score `0.6`
- run 2: `run-2/validation_transcript.md` -> score `0.45`

Both runs are valid target-root no-MCP executions:

- cwd: `/Users/buyong/workspace/private/buyong-mcp/.agents/benchmark-data/ClickHouse-master`
- MCP servers: `0`

Run 1 recovered the whole-tuple throw path and the LowCardinality type-visibility mismatch, but missed the non-constant tuple side-effect guard. Run 2 stayed even shallower and missed both analyzer-side guard contracts, which kept it comfortably below threshold.
