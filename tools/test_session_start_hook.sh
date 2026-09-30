#!/bin/bash
# Exercises .claude/hooks/session-start.sh against fake campaigns in a temporary
# directory; a renamed runner copy and a fake solver are used, so it never
# touches or is confused by a real campaign. Usage: tools/test_session_start_hook.sh
set -u
REPO=$(cd "$(dirname "$0")/.." && pwd)
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cp "$REPO/tools/run_events_campaign.py" "$T/fake_campaign_runner.py"
cat > "$T/fake_solver.py" <<'PYEOF'
#!/usr/bin/env python3
import json, sys, time
a = sys.argv[1:]
if a[0] == 'verify-events':
    print('VERIFIED_NO'); raise SystemExit
v = lambda n: a[a.index(n) + 1]
m = int(v('--maximum'))
time.sleep(2)  # keeps the fake runner alive long enough to observe it
open(v('--output'), 'w').write(json.dumps({'status': 'NO', 'evidence': 'VERIFIED_NO',
    'scope': 'complete_original_problem', 'config': {'maximum': m}}))
open(v('--proof'), 'w').write(json.dumps({'maximum': m}))
PYEOF
chmod +x "$T/fake_solver.py"
seq 1000 2 1006 > $T/candidates.txt
new_case() {  # name state paused authorized
  local d=$T/$1; rm -rf $d; mkdir -p $d/outputs/run
  python3 - "$d" "$2" "$3" "$4" "$REPO" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[5] + '/tools')
import run_events_campaign as c
d, st, paused, auth = sys.argv[1:5]
json.dump({'state': st, 'start_after': 1000, 'last_verified_no': 1000, 'through': 1000000,
  'verified_no_count': 0, 'attention': None if st != 'ATTENTION' else {'maximum': 1002, 'status': 'YES'},
  'options': c.OPTIONS, 'candidate_timeout_seconds': 600.0}, open(d + '/outputs/run/state.json', 'w'))
json.dump({'paused': paused == 'true', 'automatic_resume_authorized': auth == 'true'}, open(d + '/PAUSED.json', 'w'))
PY
}
run_hook() {  # name [env...]
  local d=$T/$1; shift
  env CLAUDE_CODE_REMOTE=true CAMPAIGN_OUTPUT=$d/outputs/run CAMPAIGN_BINARY=$T/fake_solver.py \
    CAMPAIGN_CANDIDATES=$T/candidates.txt CAMPAIGN_RUNNER=$T/fake_campaign_runner.py \
    CAMPAIGN_PAUSE_FILE=$d/PAUSED.json CAMPAIGN_START_AFTER=1000 CAMPAIGN_JOBS=2 CAMPAIGN_THREADS_PER_CANDIDATE=1 \
    CLAUDE_PROJECT_DIR=$REPO "$@" $REPO/.claude/hooks/session-start.sh
}
runners() { ps -eo args | grep -Ec "^python3 +[^ ]*fake_campaign_runner\.py"; }
final() { python3 -c "import json;s=json.load(open('$T/$1/outputs/run/state.json'));print(s['state'],s['last_verified_no'],s['verified_no_count'])"; }
fail=0; check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: expected [$3] got [$2]"; fail=1; fi; }

new_case resume RUNNING false true; start=$(date +%s.%N); run_hook resume; elapsed=$(echo "$(date +%s.%N) - $start" | bc)
check "hook returns immediately (<2s)" "$(echo "$elapsed < 2" | bc)" 1
sleep 1; check "resumes a dead RUNNING campaign (runner started)" "$(runners)" 1
for i in $(seq 1 30); do [ "$(runners)" = 0 ] && break; sleep 1; done
check "fake campaign completes and verifies in order" "$(final resume)" "COMPLETE 1006 3"
check "hook logged the resume" "$(grep -c 'session-start: runner not alive' $T/resume/outputs/run/campaign.log)" 1

new_case again RUNNING false true; run_hook again; run_hook again; run_hook again; sleep 1
check "repeated hooks start only one runner" "$(runners)" 1
for i in $(seq 1 30); do [ "$(runners)" = 0 ] && break; sleep 1; done

new_case race RUNNING false true; (run_hook race & run_hook race & run_hook race & wait); sleep 1
check "simultaneous hooks start only one runner" "$(runners)" 1
for i in $(seq 1 30); do [ "$(runners)" = 0 ] && break; sleep 1; done

for c in "attention ATTENTION false true" "paused RUNNING true true" "unauthorized RUNNING false false" "complete COMPLETE false true"; do
  set -- $c; new_case $1 $2 $3 $4; run_hook $1; sleep 1
  check "does not resume: $1" "$(runners)" 0
  check "state untouched: $1" "$(final $1 | cut -d' ' -f1)" "$2"
done
new_case local RUNNING false true; run_hook local CLAUDE_CODE_REMOTE=false; sleep 1
check "does nothing outside a remote session" "$(runners)" 0
rm -rf $T/nostate; mkdir -p $T/nostate/outputs/run; echo '{"paused":false,"automatic_resume_authorized":true}' > $T/nostate/PAUSED.json
run_hook nostate; sleep 1; check "never starts a campaign without a state file" "$(runners)" 0
exit $fail
