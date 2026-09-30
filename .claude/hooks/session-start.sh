#!/bin/bash
# Relaunch an authorized, interrupted residual campaign when a remote session starts.
#
# A container restart kills the runner but leaves its checkpoint on disk. This
# hook resumes it only when every guard holds, and never starts a new campaign:
#   - the session is remote and outputs/<run>/state.json already exists;
#   - state.json is RUNNING (the runner sets ATTENTION on any YES, UNKNOWN, or
#     error, and COMPLETE at the end, so those are never resumed);
#   - PAUSED.json has paused=false and automatic_resume_authorized=true (the
#     runner sets it false whenever it stops);
#   - no runner is alive (checked under a lock, so a second hook cannot start one).
# Resumption reruns every residual after the checkpoint with fresh evidence, and
# acceptance stays strictly ordered. The script prints nothing on success and
# returns immediately; the runner is detached.
set -euo pipefail

[ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || exit 0

root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
run="${CAMPAIGN_OUTPUT:-$root/outputs/20260929-concurrent}"
binary="${CAMPAIGN_BINARY:-$root/target/release/apa-exact-search}"
candidates="${CAMPAIGN_CANDIDATES:-$root/progress/candidates.txt}"
runner="${CAMPAIGN_RUNNER:-$root/tools/run_events_campaign.py}"
pause_file="${CAMPAIGN_PAUSE_FILE:-$root/PAUSED.json}"
start_after="${CAMPAIGN_START_AFTER:-370039}"
jobs="${CAMPAIGN_JOBS:-3}"
threads="${CAMPAIGN_THREADS_PER_CANDIDATE:-2}"

state="$run/state.json"
[ -f "$state" ] || exit 0

log() { echo "$(date -u +%FT%TZ) session-start: $*" >> "$run/campaign.log"; }

eligible=$(python3 - "$state" "$pause_file" <<'PY'
import json, sys
try:
    state = json.load(open(sys.argv[1]))
    pause = json.load(open(sys.argv[2]))
except (OSError, ValueError):
    print("no")
    raise SystemExit
ok = (state.get("state") == "RUNNING" and not state.get("attention")
      and pause.get("paused") is False
      and pause.get("automatic_resume_authorized") is True)
print("yes" if ok else "no")
PY
)
[ "$eligible" = "yes" ] || exit 0

exec 9> "$run/.autoresume.lock"
flock -n 9 || exit 0

# A live runner is a python process whose command starts with the runner script;
# a shell that merely mentions the script's name does not count.
if ps -eo args | grep -Eq "^(/[^ ]*/)?python3?[0-9.]* +[^ ]*$(basename "$runner" | sed 's/\./\\./g')( |$)"; then
  exit 0
fi

if [ ! -x "$binary" ]; then
  (cd "$root" && cargo build --release >> "$run/campaign.log" 2>&1) || { log "build failed; not resuming"; exit 0; }
fi

log "runner not alive; resuming from checkpoint"
cd "$root"
setsid nohup python3 "$runner" --binary "$binary" --candidates "$candidates" \
  --output "$run" --start-after "$start_after" --jobs "$jobs" \
  --threads-per-candidate "$threads" >> "$run/campaign.log" 2>&1 9>&- < /dev/null &
exit 0
