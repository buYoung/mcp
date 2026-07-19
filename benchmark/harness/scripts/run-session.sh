#!/bin/bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1

OWNER_TOKEN_PREFIX="--baseline-owner-token="
if [[ "$#" -eq 5 ]]; then
  OWNER_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
  exec /usr/bin/env BASELINE_OWNER_TOKEN="$OWNER_TOKEN" /bin/bash "$0" "$@" "$OWNER_TOKEN_PREFIX$OWNER_TOKEN"
fi
if [[ "$#" -ne 6 || "$6" != "$OWNER_TOKEN_PREFIX"* ]]; then
  echo "usage: run-session.sh GENERATION TASK_ID r1|r2|r3 B1|B2 initial|replacement" >&2
  exit 64
fi
OWNER_TOKEN="${6#"$OWNER_TOKEN_PREFIX"}"
[[ "$OWNER_TOKEN" =~ ^[0-9a-f]{64}$ && "${BASELINE_OWNER_TOKEN:-}" == "$OWNER_TOKEN" ]] || { echo "invalid owner process identity" >&2; exit 77; }
set -- "${@:1:5}"

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
QUALITY_ROOT="$(cd "$ROOT/.." && pwd -P)"
BENCHMARK="$QUALITY_ROOT/benchmark"
CORPUS="$QUALITY_ROOT/corpus/directus"
GOLDEN="$QUALITY_ROOT/corpus/directus-index-golden/.codemap"
OPENCODE="$QUALITY_ROOT/runtime/opencode"
B2_BINARY="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["binary_path"])' "$ROOT/config/b2-runtime.json")"
PRODUCT_ROOT="$(cd "$QUALITY_ROOT/.." && pwd -P)"
LEGACY_ROOT="/private/tmp/codemap-search-quality.7f4a91c2"
HOST_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
HOST_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/opencode"
AUTH_FILE="$HOST_DATA/auth.json"
MODEL="ollama-cloud/deepseek-v4-flash"

[[ $# -eq 5 ]] || { echo "usage: run-session.sh GENERATION TASK_ID r1|r2|r3 B1|B2 initial|replacement" >&2; exit 64; }
GENERATION="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
TASK_ID="$2"
TRIAL_ID="$3"
ARM="$4"
MODE="$5"
[[ "$ARM" == B1 || "$ARM" == B2 ]] || { echo "invalid arm" >&2; exit 64; }
[[ "$TRIAL_ID" == r1 || "$TRIAL_ID" == r2 || "$TRIAL_ID" == r3 ]] || { echo "invalid trial" >&2; exit 64; }
[[ "$MODE" == initial || "$MODE" == replacement ]] || { echo "invalid attempt mode" >&2; exit 64; }
[[ "${BASELINE_3X_EXTERNAL_APPROVED:-}" == 1 && "${BASELINE_3X_AUTH_READY:-}" == 1 ]] || { echo "external-model/auth gate is closed" >&2; exit 77; }
[[ -z "${CODEMAP_TASTE_CANDIDATE:-}" && -z "${CODEMAP_TASTE_READ_ONLY:-}" && -z "${CODEMAP_TASTE_METRICS_PATH:-}" ]] || { echo "candidate environment contamination" >&2; exit 78; }

python3 "$ROOT/scripts/generation.py" verify-execution "$GENERATION"
python3 "$ROOT/scripts/auth_runtime.py" validate "$AUTH_FILE" "ollama-cloud"
RESOLVED="$(python3 "$ROOT/scripts/scheduler.py" resolve "$TASK_ID" "$TRIAL_ID" "$ARM")"
GENERATION_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["generation_id"])' "$GENERATION")"
RUNS_ROOT="$ROOT/runs/$GENERATION_ID"
LEDGER="$RUNS_ROOT/ledger.json"
WORK_ID="prep-$(python3 -c 'import uuid; print(uuid.uuid4().hex)')"
WORK_DIR="$ROOT/work/$WORK_ID"
AUTH_PARENT="$ROOT/runtime-auth"
AUTH_RUNTIME=""
RUN_DIR=""
RUN_ID=""
CLAIM_RECEIPT="$WORK_DIR/claim-receipt.json"
CLAIMED=0
TERMINAL_RECORDED=0
PUBLISHED=0
SEALED=0
METRICS_FINALIZED=0
METRICS_SEALED=0
STAGE="pre-claim-setup"
mkdir -p "$WORK_DIR" "$RUNS_ROOT" "$AUTH_PARENT"

cleanup_unclaimed() {
  local status="$?"
  trap - EXIT
  set +e
  if [[ -f "$CLAIM_RECEIPT" ]]; then
    python3 "$ROOT/scripts/protocol.py" cancel-claim "$LEDGER" "$GENERATION" "$CLAIM_RECEIPT" "shell exited before the paid process started (status=$status)"
  fi
  if [[ -n "$AUTH_RUNTIME" ]]; then python3 "$ROOT/scripts/auth_runtime.py" remove "$AUTH_RUNTIME" "$AUTH_PARENT" "$WORK_ID-"; fi
  if [[ -d "$WORK_DIR" ]]; then chmod -R u+w "$WORK_DIR" 2>/dev/null; rm -rf "$WORK_DIR"; fi
  exit "$status"
}
trap cleanup_unclaimed EXIT

finalize_claimed_on_exit() {
  local original_status="$?" auth_status=0 target ledger_status
  trap - EXIT
  set +e
  if [[ "$CLAIMED" -eq 1 && -n "$RUN_ID" ]]; then
    ledger_status="$(python3 "$ROOT/scripts/protocol.py" status "$LEDGER" "$GENERATION" "$RUN_ID" 2>/dev/null)"
    if [[ -n "$ledger_status" ]]; then
      TERMINAL_RECORDED="$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["terminal_recorded"]))' "$ledger_status")"
      PUBLISHED="$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["published"]))' "$ledger_status")"
      SEALED="$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["artifacts_sealed"]))' "$ledger_status")"
      [[ "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("metrics_status"))' "$ledger_status")" == sealed ]] && METRICS_SEALED=1
    fi
  fi
  if [[ -n "$RUN_DIR" && "$RUN_DIR" == "$ROOT/work/"* ]]; then
    target="$RUNS_ROOT/$RUN_ID"
    RUN_DIR="$(python3 "$ROOT/scripts/recover_run_directory.py" "$RUN_DIR" "$target")"
    CLAIM_RECEIPT="$RUN_DIR/claim-receipt.json"
  fi
  if [[ -n "$AUTH_RUNTIME" ]]; then
    python3 "$ROOT/scripts/auth_runtime.py" remove "$AUTH_RUNTIME" "$AUTH_PARENT" "$WORK_ID-"
    auth_status=$?
    [[ "$auth_status" -eq 0 ]] && AUTH_RUNTIME=""
  fi
  if [[ "$PUBLISHED" -eq 0 && -n "$RUN_DIR" && -d "$RUN_DIR" ]]; then
    python3 - "$RUN_DIR/auth-cleanup.json" "$auth_status" <<'PY'
import json,pathlib,sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({"status":int(sys.argv[2]),"exit_trap":True},indent=2,sort_keys=True)+"\n")
PY
  fi
  if [[ "$CLAIMED" -eq 1 && "$TERMINAL_RECORDED" -eq 0 && -n "$RUN_DIR" ]]; then
    python3 "$ROOT/scripts/abort_claimed.py" "$RUN_DIR" "$STAGE" "$original_status"
    python3 "$ROOT/scripts/protocol.py" terminal "$LEDGER" "$GENERATION" "$RUN_ID" "$RUN_DIR/attempt-classification.json"
    [[ "$?" -eq 0 ]] && TERMINAL_RECORDED=1
  fi
  if [[ "$CLAIMED" -eq 1 && "$SEALED" -eq 0 && -n "$RUN_DIR" && -d "$RUN_DIR" ]]; then
    python3 "$ROOT/scripts/seal_artifacts.py" "$RUN_DIR"
    [[ "$?" -eq 0 ]] && SEALED=1
  fi
  if [[ "$TERMINAL_RECORDED" -eq 1 && "$SEALED" -eq 1 && "$PUBLISHED" -eq 0 ]]; then
    python3 "$ROOT/scripts/protocol.py" publish "$LEDGER" "$GENERATION" "$RUN_ID" "$RUN_DIR"
    [[ "$?" -eq 0 ]] && PUBLISHED=1
  fi
  if [[ "$PUBLISHED" -eq 1 && "$METRICS_FINALIZED" -eq 0 ]]; then
    STAGE="exit-trap-automatic-metrics"
    python3 "$ROOT/scripts/finalize_metrics.py" "$LEDGER" "$GENERATION" "$RUN_DIR" \
      "$QUALITY_ROOT/analysis-tools/extract_run_metrics.py" "$ROOT/schemas/automatic-run-metrics.schema.json"
    [[ "$?" -eq 0 ]] && METRICS_SEALED=1
    METRICS_FINALIZED=1
  fi
  [[ -n "$RUN_DIR" ]] && echo "$RUN_DIR"
  exit 79
}

mkdir -p "$WORK_DIR"/{home,config,data,cache,sessions,raw}
python3 "$ROOT/scripts/preflight.py" "$GENERATION" --session-lane "$WORK_DIR/preflight.report.json" >/dev/null
python3 "$ROOT/scripts/clone_source.py" "$CORPUS" "$WORK_DIR/source" "$WORK_DIR/source-clone.json"
QUESTION_PATH="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["question_path"])' "$RESOLVED")"
QUESTION_SHA="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["question_sha256"])' "$RESOLVED")"
python3 "$ROOT/scripts/render_prompt.py" "$QUESTION_PATH" "$QUESTION_SHA" "$WORK_DIR/prompt.txt"
python3 "$ROOT/scripts/materialize_config.py" pair-evidence "$WORK_DIR/b1.opencode.json" "$WORK_DIR/b2.opencode.json" "$WORK_DIR/pair-config.json"
cp "$WORK_DIR/$([[ "$ARM" == B1 ]] && echo b1 || echo b2).opencode.json" "$WORK_DIR/opencode.json"
python3 "$ROOT/scripts/index_manifest.py" "$WORK_DIR/source/.codemap" "$WORK_DIR/index.before.json"
AUTH_RUNTIME="$(python3 "$ROOT/scripts/auth_runtime.py" create "$AUTH_FILE" "ollama-cloud" "$AUTH_PARENT" "$WORK_ID-")"

BASELINE_CLAIM_OWNER_PID="$$" BASELINE_CLAIM_WORK_ID="$WORK_ID" BASELINE_CLAIM_AUTH_RUNTIME="$AUTH_RUNTIME" \
  python3 "$ROOT/scripts/protocol.py" claim "$LEDGER" "$GENERATION" "$TASK_ID" "$TRIAL_ID" "$ARM" "$MODE" "$CLAIM_RECEIPT" >"$WORK_DIR/claim-result.json"
RUN_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run_id"])' "$CLAIM_RECEIPT")"
ATTEMPT_NUMBER="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["attempt_number"])' "$CLAIM_RECEIPT")"
CLAIMED=1
RUN_DIR="$WORK_DIR"
trap finalize_claimed_on_exit EXIT
STAGE="publish-run-directory"
TARGET_RUN_DIR="$RUNS_ROOT/$RUN_ID"
RUN_DIR="$(python3 "$ROOT/scripts/recover_run_directory.py" "$WORK_DIR" "$TARGET_RUN_DIR")"
CLAIM_RECEIPT="$RUN_DIR/claim-receipt.json"
WORK_DIR=""

python3 "$ROOT/scripts/materialize_sandbox.py" "$RUN_DIR/sandbox.sb" \
  "$BENCHMARK" "$LEGACY_ROOT" "$PRODUCT_ROOT" "$QUALITY_ROOT/b2/source" \
  "$RUN_DIR/source" "$RUN_DIR/source/.codemap" "$HOST_CONFIG" "$HOST_DATA" \
  "$QUALITY_ROOT" "$RUN_DIR" "$OPENCODE" "$B2_BINARY" "$AUTH_RUNTIME"

PROMPT_SHA="$(shasum -a 256 "$RUN_DIR/prompt.txt" | awk '{print $1}')"
LIMITS_SHA="$(shasum -a 256 "$ROOT/config/limits.json" | awk '{print $1}')"
CONFIG_SHA="$(shasum -a 256 "$RUN_DIR/opencode.json" | awk '{print $1}')"
python3 - "$RUN_DIR/run.manifest.json" "$RESOLVED" "$GENERATION_ID" "$RUN_ID" "$ATTEMPT_NUMBER" "$PROMPT_SHA" "$LIMITS_SHA" "$CONFIG_SHA" <<'PY'
import json,pathlib,sys,time
pathlib.Path(sys.argv[1]).write_text(json.dumps({
  "schema_version":1,"schedule":json.loads(sys.argv[2]),"generation_id":sys.argv[3],"run_id":sys.argv[4],
  "attempt_number":int(sys.argv[5]),"prompt_sha256":sys.argv[6],"limits_sha256":sys.argv[7],
  "arm_config_sha256":sys.argv[8],"source_tree_sha256":"e87bbfe43002f4b68c7ff9dd6218096d222daa01b0ab87f8a85525eb5becb1c0",
  "common_environment":{"CODEMAP_BASELINE_READ_ONLY":"1"},"started_at_ns":time.time_ns()
},indent=2,sort_keys=True)+"\n")
PY

COMMAND=(/usr/bin/env -i PATH="/usr/bin:/bin:/usr/sbin:/sbin" HOME="$RUN_DIR/home" XDG_CONFIG_HOME="$RUN_DIR/config" XDG_DATA_HOME="$AUTH_RUNTIME" XDG_CACHE_HOME="$RUN_DIR/cache" OPENCODE_CONFIG="$RUN_DIR/opencode.json" OPENCODE_SESSION_DIR="$RUN_DIR/sessions" OPENCODE_DISABLE_MODELS_FETCH=1 OPENCODE_DISABLE_AUTOUPDATE=1 OPENCODE_DISABLE_PROJECT_CONFIG=1 CODEMAP_BASELINE_READ_ONLY=1 TMPDIR="$RUN_DIR" NO_COLOR=1 PYTHONDONTWRITEBYTECODE=1 sandbox-exec -f "$RUN_DIR/sandbox.sb" "$OPENCODE" run --pure --model "$MODEL" --dir "$RUN_DIR/source" --format json --auto "$(<"$RUN_DIR/prompt.txt")")
python3 - "$RUN_DIR/command.json" "${COMMAND[@]}" <<'PY'
import json,pathlib,sys
pathlib.Path(sys.argv[1]).write_text(json.dumps(sys.argv[2:],indent=2)+"\n")
PY
STAGE="session-supervisor"
set +e
BASELINE_RECOVERY_LEDGER="$LEDGER" BASELINE_RECOVERY_GENERATION="$GENERATION" \
BASELINE_RECOVERY_RUN_ID="$RUN_ID" BASELINE_RECOVERY_RECEIPT="$CLAIM_RECEIPT" \
  python3 "$ROOT/scripts/session_supervisor.py" "$RUN_DIR" "$RUN_DIR/raw/events.jsonl" "$RUN_DIR/raw/stderr.log" "${COMMAND[@]}"
SUPERVISOR_STATUS=$?
set -e
STAGE="index-after-manifest"
python3 "$ROOT/scripts/index_manifest.py" "$RUN_DIR/source/.codemap" "$RUN_DIR/index.after.json"
STAGE="event-parser"
set +e
python3 "$ROOT/scripts/parse_events.py" "$RUN_DIR/raw/events.jsonl" "$RUN_DIR/wrapper.json" "$RUN_DIR/normalized.json"
PARSER_STATUS=$?
set -e
STAGE="postprocess-record"
python3 "$ROOT/scripts/record_postprocess.py" "$RUN_DIR" "$SUPERVISOR_STATUS" "$PARSER_STATUS"
STAGE="generation-and-source-invariants"
python3 "$ROOT/scripts/finalize_attempt.py" "$RUN_DIR" "$GENERATION"
STAGE="auth-runtime-cleanup"
set +e
python3 "$ROOT/scripts/auth_runtime.py" remove "$AUTH_RUNTIME" "$AUTH_PARENT" "$WORK_ID-"
AUTH_CLEANUP_STATUS=$?
set -e
python3 - "$RUN_DIR/auth-cleanup.json" "$AUTH_CLEANUP_STATUS" <<'PY'
import json,pathlib,sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({"status":int(sys.argv[2]),"exit_trap":False},indent=2,sort_keys=True)+"\n")
PY
[[ "$AUTH_CLEANUP_STATUS" -eq 0 ]] && AUTH_RUNTIME=""
STAGE="attempt-classification"
python3 "$ROOT/scripts/classify_attempt.py" "$RUN_DIR" "$RUN_DIR/attempt-classification.json" >/dev/null
STAGE="ledger-terminal-record"
python3 "$ROOT/scripts/protocol.py" terminal "$LEDGER" "$GENERATION" "$RUN_ID" "$RUN_DIR/attempt-classification.json"
TERMINAL_RECORDED=1
if [[ -n "$AUTH_RUNTIME" ]]; then
  set +e
  python3 "$ROOT/scripts/auth_runtime.py" remove "$AUTH_RUNTIME" "$AUTH_PARENT" "$WORK_ID-"
  [[ "$?" -eq 0 ]] && AUTH_RUNTIME=""
  set -e
fi
STAGE="artifact-seal"
python3 "$ROOT/scripts/seal_artifacts.py" "$RUN_DIR"
SEALED=1
STAGE="ledger-publish"
python3 "$ROOT/scripts/protocol.py" publish "$LEDGER" "$GENERATION" "$RUN_ID" "$RUN_DIR"
PUBLISHED=1
STAGE="automatic-metrics-extraction"
set +e
python3 "$ROOT/scripts/finalize_metrics.py" "$LEDGER" "$GENERATION" "$RUN_DIR" \
  "$QUALITY_ROOT/analysis-tools/extract_run_metrics.py" "$ROOT/schemas/automatic-run-metrics.schema.json"
METRICS_STATUS=$?
set -e
METRICS_FINALIZED=1
if [[ "$METRICS_STATUS" -ne 0 ]]; then
  exit 79
fi
METRICS_SEALED=1
CLASSIFICATION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["measurement_status"], json.load(open(sys.argv[1]))["generation_invalid"])' "$RUN_DIR/attempt-classification.json")"
echo "$RUN_DIR"
trap - EXIT
if [[ "$CLASSIFICATION" == "valid False" ]]; then exit 0; fi
if [[ "$CLASSIFICATION" == "infrastructure_invalid False" ]]; then exit 75; fi
exit 79
