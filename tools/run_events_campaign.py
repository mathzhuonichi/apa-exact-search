"""Run the event solver over a contiguous residual prefix with proof replay.

Stops at the first outcome other than independently replayed VERIFIED_NO.
The campaign state is written after each completed maximum and can be resumed.
"""

import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import time


CANDIDATE_TIMEOUT_SECONDS = 600

OPTIONS = [
    '--threads',
    '0',
    '--root-strengthen',
    '--probes',
    '--root-seconds',
    '0',
    '--seconds',
    '0',
    '--node-limit',
    '0',
    '--prime-chain-steps',
    '18446744073709551615',
    '--probe-case-events',
    '18446744073709551615',
    '--probe-members',
    '0',
    '--root-probe-width',
    '8',
    '--probe-events',
    '18446744073709551615',
]


def save(path, record):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    os.replace(temporary, path)


def run_one(binary, output, maximum, timeout_seconds):
    folder = output / 'candidates' / str(maximum)
    folder.mkdir(parents=True, exist_ok=True)
    proof = folder / 'proof.json'
    result = folder / 'result.json'
    with (folder / 'stdout.log').open('w', encoding='utf-8') as stdout, \
            (folder / 'stderr.log').open('w', encoding='utf-8') as stderr:
        try:
            completed = subprocess.run(
                [str(binary), 'solve-events', '--maximum', str(maximum), *OPTIONS,
                 '--proof', str(proof), '--output', str(result)],
                stdout=stdout, stderr=stderr, check=False,
                timeout=timeout_seconds,
            )
        except subprocess.TimeoutExpired:
            reason = f'candidate exceeded {timeout_seconds:g}s wall timeout; solver result was not returned'
            save(folder / 'timeout.json', {
                'maximum': maximum,
                'status': 'UNKNOWN',
                'kind': 'external_wall_timeout',
                'wall_timeout_seconds': timeout_seconds,
                'solver_returned_result': False,
                'reason': reason,
            })
            return 'UNKNOWN', reason
    if completed.returncode or not result.exists():
        return 'ERROR', f'solver exit {completed.returncode}; inspect {folder}'
    try:
        report = json.loads(result.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return 'ERROR', f'invalid result for {maximum}: {error}'
    status = report.get('status')
    evidence = report.get('evidence')
    if status != 'NO' or evidence != 'VERIFIED_NO' or report.get('scope') != 'complete_original_problem':
        return status or 'ERROR', f'{evidence}: {report.get("reason")}'
    if not proof.exists():
        return 'ERROR', f'missing proof for {maximum}'
    with (folder / 'verify.log').open('w', encoding='utf-8') as verify, \
            (folder / 'verify-stderr.log').open('w', encoding='utf-8') as verify_error:
        replay = subprocess.run(
            [str(binary), 'verify-events', '--proof', str(proof)],
            stdout=verify, stderr=verify_error, check=False,
        )
    if replay.returncode or (folder / 'verify.log').read_text(encoding='utf-8').strip() != 'VERIFIED_NO':
        return 'ERROR', f'proof replay failed for {maximum}'
    return 'NO', None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--candidates', type=Path, default=Path('progress/candidates.txt'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--start-after', type=int, required=True)
    parser.add_argument('--through', type=int, default=1_000_000)
    parser.add_argument('--candidate-timeout-seconds', type=float,
                        default=CANDIDATE_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if not math.isfinite(args.candidate_timeout_seconds) or args.candidate_timeout_seconds <= 0:
        parser.error('candidate-timeout-seconds must be finite and positive')
    binary = args.binary.resolve()
    candidates = [int(line) for line in args.candidates.read_text(encoding='utf-8').splitlines() if line.strip()]
    if candidates != sorted(set(candidates)) or candidates[-1] > 1_000_000:
        parser.error('candidate sequence must be sorted, unique, and within one million')
    args.output.mkdir(parents=True, exist_ok=True)
    state_path = args.output / 'state.json'
    if state_path.exists():
        state = json.loads(state_path.read_text(encoding='utf-8'))
        if state['start_after'] != args.start_after or state['through'] != args.through:
            parser.error('resume options differ from the existing campaign')
        if (state.get('candidate_timeout_seconds') is not None and
                state['candidate_timeout_seconds'] != args.candidate_timeout_seconds):
            parser.error('candidate timeout differs from the recorded campaign configuration')
        state['candidate_timeout_seconds'] = args.candidate_timeout_seconds
        if state['state'] == 'PAUSED':
            # Launching this runner is an explicit resume action. Continue from
            # the last independently replayed checkpoint; an interrupted
            # candidate is automatically retried by the ordered loop below.
            state.update(state='RUNNING', attention=None)
            save(state_path, state)
            pause_path = state_path.parent.parent.parent / 'PAUSED.json'
            if pause_path.exists():
                pause = json.loads(pause_path.read_text(encoding='utf-8'))
                pause.update(paused=False, automatic_resume_authorized=True)
                pause_path.write_text(json.dumps(pause, indent=2) + '\n', encoding='utf-8')
        elif state['state'] not in ('RUNNING', 'COMPLETE'):
            parser.error(f'campaign stopped at {state["attention"]}; inspect before resuming')
    else:
        if args.start_after not in candidates:
            parser.error('start-after must be a residual maximum')
        state = {
            'state': 'RUNNING', 'start_after': args.start_after,
            'last_verified_no': args.start_after, 'through': args.through,
            'verified_no_count': 0, 'attention': None, 'options': OPTIONS,
            'candidate_timeout_seconds': args.candidate_timeout_seconds,
        }
        save(state_path, state)
    if state['options'] != OPTIONS:
        parser.error('campaign options differ from the recorded configuration')
    for maximum in candidates:
        if maximum <= state['last_verified_no'] or maximum > args.through:
            continue
        started = time.monotonic()
        status, reason = run_one(
            binary, args.output, maximum, args.candidate_timeout_seconds)
        if status != 'NO':
            state.update(state='ATTENTION', attention={'maximum': maximum, 'status': status, 'reason': reason})
            save(state_path, state)
            pause_path = state_path.parent.parent.parent / 'PAUSED.json'
            pause = json.loads(pause_path.read_text(encoding='utf-8')) if pause_path.exists() else {}
            pause.update(
                paused=True,
                automatic_resume_authorized=False,
                reason=f'Campaign paused at {maximum} after {status}: {reason}',
                resume_from=maximum,
            )
            save(pause_path, pause)
            print(json.dumps(state['attention']), flush=True)
            return 1
        state['last_verified_no'] = maximum
        state['verified_no_count'] += 1
        save(state_path, state)
        print(json.dumps({'maximum': maximum, 'status': 'VERIFIED_NO',
                          'seconds': round(time.monotonic() - started, 3)}), flush=True)
    state['state'] = 'COMPLETE'
    save(state_path, state)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
