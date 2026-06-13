#!/bin/bash
# A/B 프로브 v2: 인덱서 초기 스냅샷 발행을 기다린 뒤 tools/call 전송
BIN="$1"; DIR="$2"; TOOL="$3"; ARGS="$4"
(cd "$DIR" && {
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"ab-probe","version":"0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  sleep "${PROBE_WARM:-12}"
  echo "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$TOOL\",\"arguments\":$ARGS}}"
  sleep 3
} | "$BIN" mcp 2>/dev/null) | jq -r 'select(.id==2) | .result.content[0].text'
