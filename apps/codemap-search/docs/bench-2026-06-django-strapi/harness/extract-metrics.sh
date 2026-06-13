#!/bin/bash
# 에피소드 jsonl에서 기계 추출 가능한 메트릭만 산출 (score는 null — 채점은 에이전트 몫, playbook §4-3)
# 사용: extract-metrics.sh <claude-sonnet|codex-gpt55|claude-sonnet-base|codex-gpt55-base> <jsonl> <expected_file>
# ToolSearch는 하니스 메커니즘이므로 turns/first_answer_turn에서 제외 (runbook §0).
# baseline arm: tool_calls 키가 빌트인 도구명으로 바뀌고, mcp_response_bytes_total은
# "도구 결과 바이트 총합" 의미로 동일 필드명을 유지한다. denied_builtin_attempts는
# 허용 목록 밖 도구 시도(mcp__* 포함 — purity 위반 감지)를 센다.
set -u
ARM="$1"; JSONL="$2"; EXPECTED_FILE="$3"

if [ "$ARM" = "claude-sonnet-base" ]; then
  jq -s --arg ef "$EXPECTED_FILE" '
    . as $lines
    | [ $lines[] | select(.type=="assistant") | .message.content[]? | select(.type=="tool_use") | select(.name != "ToolSearch") ] as $uses
    | ([ $lines[] | select(.type=="user") | .message.content[]? | select(.type=="tool_result")
        | {key: (.tool_use_id // "?"),
           value: ((.content // "") | if type=="string" then . else (map(.text // "") | join("")) end)} ]
       | from_entries) as $res
    | {
        turns: ($uses | length),
        tool_calls: (reduce ([ $uses[] | select(.name == "Bash" or .name == "Read" or .name == "Glob" or .name == "Grep") | .name | ascii_downcase ][]) as $n
          ({bash:0, read:0, glob:0, grep:0}; .[$n] = ((.[$n] // 0) + 1))),
        shell_bypass_calls: 0,
        denied_builtin_attempts: ([ $uses[] | select(.name == "Bash" or .name == "Read" or .name == "Glob" or .name == "Grep" | not) ] | length),
        duplicate_calls: (($uses | group_by(.name + (.input | tostring)) | map(length - 1) | add) // 0),
        mcp_response_bytes_total: (([ $uses[] | ($res[.id] // "") | utf8bytelength ] | add) // 0),
        first_answer_turn: (([ range(0; ($uses | length)) as $i | select(($res[$uses[$i].id] // "") | contains($ef)) | ($i + 1) ] | first) // null),
        answer_text: (([ $lines[] | select(.type=="result") | (.result // "") ] | last) // ""),
        contamination_found: ([ $lines[] | tostring ] | any(contains("Serena MCP Tool Policy"))),
        tokens: (([ $lines[] | select(.type=="result") | .usage | select(. != null) ] | last) as $u
          | if $u == null then null else {
              input_tokens: ($u.input_tokens // 0),
              output_tokens: ($u.output_tokens // 0),
              cache_read_input_tokens: ($u.cache_read_input_tokens // 0),
              cache_creation_input_tokens: ($u.cache_creation_input_tokens // 0)
            } end)
      }' "$JSONL"
elif [ "$ARM" = "codex-gpt55-base" ]; then
  jq -s --arg ef "$EXPECTED_FILE" '
    [ .[] | select(.type=="item.completed") | .item ] as $items
    | [ $items[] | select(.type=="command_execution") ] as $calls
    | {
        turns: ($calls | length),
        tool_calls: { shell: ($calls | length) },
        shell_bypass_calls: 0,
        denied_builtin_attempts: ([ $items[] | select(.type=="mcp_tool_call") ] | length),
        duplicate_calls: (($calls | group_by(.command // "") | map(length - 1) | add) // 0),
        mcp_response_bytes_total: (([ $calls[] | ((.aggregated_output // .output // "") | utf8bytelength) ] | add) // 0),
        first_answer_turn: (([ range(0; ($calls | length)) as $i | select((($calls[$i].aggregated_output // $calls[$i].output // "")) | contains($ef)) | ($i + 1) ] | first) // null),
        answer_text: (([ $items[] | select(.type=="agent_message") | .text ] | last) // ""),
        contamination_found: false,
        tokens: (([ .[] | select(.type=="turn.completed") | .usage | select(. != null) ]) as $us
          | if ($us | length) == 0 then null else {
              input_tokens: ([ $us[].input_tokens // 0 ] | add),
              cached_input_tokens: ([ $us[].cached_input_tokens // 0 ] | add),
              output_tokens: ([ $us[].output_tokens // 0 ] | add),
              reasoning_output_tokens: ([ $us[].reasoning_output_tokens // 0 ] | add)
            } end)
      }' "$JSONL"
elif [ "$ARM" = "claude-sonnet" ]; then
  jq -s --arg ef "$EXPECTED_FILE" '
    . as $lines
    | [ $lines[] | select(.type=="assistant") | .message.content[]? | select(.type=="tool_use") | select(.name != "ToolSearch") ] as $uses
    | ([ $lines[] | select(.type=="user") | .message.content[]? | select(.type=="tool_result")
        | {key: (.tool_use_id // "?"),
           value: ((.content // "") | if type=="string" then . else (map(.text // "") | join("")) end)} ]
       | from_entries) as $res
    | {
        turns: ($uses | length),
        tool_calls: (reduce ([ $uses[] | select(.name | startswith("mcp__codemap-search__")) | .name | sub("mcp__codemap-search__"; "") ][]) as $n
          ({search:0, grep:0, read:0, overview:0, find:0}; .[$n] = ((.[$n] // 0) + 1))),
        shell_bypass_calls: 0,
        denied_builtin_attempts: ([ $uses[] | select((.name | startswith("mcp__codemap-search__")) | not) ] | length),
        duplicate_calls: (($uses | group_by(.name + (.input | tostring)) | map(length - 1) | add) // 0),
        mcp_response_bytes_total: (([ $uses[] | select(.name | startswith("mcp__codemap-search__")) | ($res[.id] // "") | utf8bytelength ] | add) // 0),
        first_answer_turn: (([ range(0; ($uses | length)) as $i | select(($res[$uses[$i].id] // "") | contains($ef)) | ($i + 1) ] | first) // null),
        answer_text: (([ $lines[] | select(.type=="result") | (.result // "") ] | last) // ""),
        contamination_found: ([ $lines[] | tostring ] | any(contains("Serena MCP Tool Policy"))),
        tokens: (([ $lines[] | select(.type=="result") | .usage | select(. != null) ] | last) as $u
          | if $u == null then null else {
              input_tokens: ($u.input_tokens // 0),
              output_tokens: ($u.output_tokens // 0),
              cache_read_input_tokens: ($u.cache_read_input_tokens // 0),
              cache_creation_input_tokens: ($u.cache_creation_input_tokens // 0)
            } end)
      }' "$JSONL"
else
  jq -s --arg ef "$EXPECTED_FILE" '
    [ .[] | select(.type=="item.completed") | .item ] as $items
    | [ $items[] | select(.type=="mcp_tool_call" or .type=="command_execution") ] as $calls
    | {
        turns: ($calls | length),
        tool_calls: (reduce ([ $items[] | select(.type=="mcp_tool_call") | .tool ][]) as $n
          ({search:0, grep:0, read:0, overview:0, find:0}; .[$n] = ((.[$n] // 0) + 1))),
        shell_bypass_calls: ([ $items[] | select(.type=="command_execution") ] | length),
        denied_builtin_attempts: 0,
        duplicate_calls: (([ $items[] | select(.type=="mcp_tool_call") ] | group_by(.tool + (.arguments | tostring)) | map(length - 1) | add) // 0),
        mcp_response_bytes_total: (([ $items[] | select(.type=="mcp_tool_call") | ((.result.content // []) | map(.text // "") | join("") | utf8bytelength) ] | add) // 0),
        first_answer_turn: (([ range(0; ($calls | length)) as $i | $calls[$i] | select(.type=="mcp_tool_call")
          | select(((.result.content // []) | map(.text // "") | join("")) | contains($ef)) | ($i + 1) ] | first) // null),
        answer_text: (([ $items[] | select(.type=="agent_message") | .text ] | last) // ""),
        contamination_found: false,
        tokens: (([ .[] | select(.type=="turn.completed") | .usage | select(. != null) ]) as $us
          | if ($us | length) == 0 then null else {
              input_tokens: ([ $us[].input_tokens // 0 ] | add),
              cached_input_tokens: ([ $us[].cached_input_tokens // 0 ] | add),
              output_tokens: ([ $us[].output_tokens // 0 ] | add),
              reasoning_output_tokens: ([ $us[].reasoning_output_tokens // 0 ] | add)
            } end)
      }' "$JSONL"
fi
