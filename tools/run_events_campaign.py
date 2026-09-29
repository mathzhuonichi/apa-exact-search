"""Run the event solver over a contiguous residual prefix with proof replay.

Several residual maxima run concurrently, one solver process each, but results
are accepted strictly in residual order. The checkpoint advances only through
consecutive independently replayed VERIFIED_NO results, and the campaign stops
at the first residual, in that order, with any other outcome. The campaign
state is written after each accepted maximum and can be resumed.
"""

import argparse
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
import json
import math
import os
from pathlib import Path
import subprocess
import threading
import time


CANDIDATE_TIMEOUT_SECONDS = 600
POLL_SECONDS = 0.2
LOOKAHEAD = 1000

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


def available_processors():
    try:
        return len(os.sched_getaffinity(0))
    except AttributeError:
        return os.cpu_count() or 1


def solver_options(threads):
    """Campaign options with the solver thread count, a resource setting only."""
    options = list(OPTIONS)
    options[options.index('--threads') + 1] = str(threads)
    return options


def run_process(command, stdout, stderr, timeout_seconds, cancel=None):
    """Return the exit code, or None if cancelled; raise TimeoutExpired on timeout.

    The child is killed and reaped on timeout, cancellation, or any exception.
    """
    process = subprocess.Popen(command, stdout=stdout, stderr=stderr)
    deadline = time.monotonic() + timeout_seconds
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, timeout_seconds)
            try:
                return process.wait(timeout=min(POLL_SECONDS, remaining))
            except subprocess.TimeoutExpired:
                if cancel is not None and cancel.is_set():
                    return None
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def save(path, record):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    os.replace(temporary, path)


def run_one(binary, output, maximum, timeout_seconds, threads=0, cancel=None):
    folder = output / 'candidates' / str(maximum)
    folder.mkdir(parents=True, exist_ok=True)
    # A successful exit must not make an earlier attempt's result or proof
    # eligible for acceptance. Preserve old evidence before creating this run.
    previous = [folder / name for name in (
        'proof.json', 'result.json', 'stdout.log', 'stderr.log',
        'verify.log', 'verify-stderr.log', 'timeout.json', 'cancelled.json',
    ) if (folder / name).exists()]
    if previous:
        archive = folder / 'attempts' / str(time.time_ns())
        archive.mkdir(parents=True, exist_ok=False)
        for path in previous:
            path.rename(archive / path.name)
    proof = folder / 'proof.json'
    result = folder / 'result.json'
    with (folder / 'stdout.log').open('w', encoding='utf-8') as stdout, \
            (folder / 'stderr.log').open('w', encoding='utf-8') as stderr:
        try:
            returncode = run_process(
                [str(binary), 'solve-events', '--maximum', str(maximum),
                 *solver_options(threads), '--proof', str(proof), '--output', str(result)],
                stdout, stderr, timeout_seconds, cancel,
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
    if returncode is None:
        reason = 'cancelled after an earlier residual stopped the ordered campaign'
        save(folder / 'cancelled.json', {
            'maximum': maximum,
            'status': 'CANCELLED',
            'solver_returned_result': False,
            'reason': reason,
        })
        return 'CANCELLED', reason
    if returncode or not result.exists():
        return 'ERROR', f'solver exit {returncode}; inspect {folder}'
    try:
        report = json.loads(result.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return 'ERROR', f'invalid result for {maximum}: {error}'
    if not isinstance(report, dict):
        return 'ERROR', f'invalid result object for {maximum}'
    config = report.get('config')
    if not isinstance(config, dict) or config.get('maximum') != maximum:
        return 'ERROR', f'result does not identify requested maximum {maximum}'
    status = report.get('status')
    evidence = report.get('evidence')
    if status != 'NO':
        return status or 'ERROR', f'{evidence}: {report.get("reason")}'
    if evidence != 'VERIFIED_NO' or report.get('scope') != 'complete_original_problem':
        return 'ERROR', f'NO lacks unconditional complete-root evidence: {evidence}, scope={report.get("scope")}'
    if not proof.exists():
        return 'ERROR', f'missing proof for {maximum}'
    try:
        certificate = json.loads(proof.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return 'ERROR', f'invalid proof for {maximum}: {error}'
    if not isinstance(certificate, dict) or certificate.get('maximum') != maximum:
        return 'ERROR', f'proof does not identify requested maximum {maximum}'
    del certificate
    with (folder / 'verify.log').open('w', encoding='utf-8') as verify, \
            (folder / 'verify-stderr.log').open('w', encoding='utf-8') as verify_error:
        replay = subprocess.run(
            [str(binary), 'verify-events', '--proof', str(proof)],
            stdout=verify, stderr=verify_error, check=False,
        )
    if replay.returncode or (folder / 'verify.log').read_text(encoding='utf-8').strip() != 'VERIFIED_NO':
        return 'ERROR', f'proof replay failed for {maximum}'
    return 'NO', None


def run_ordered(run, sequence, jobs, lookahead, accept):
    """Run `sequence` with up to `jobs` concurrent candidates; accept in order.

    `run(maximum, cancel)` returns `(status, reason)`. `accept(maximum,
    seconds)` is called for each VERIFIED_NO in sequence order, only after every
    earlier residual was accepted. Candidates are launched in sequence order and
    at most `lookahead` beyond the first unaccepted one. When a candidate returns
    anything other than NO, later candidates are cancelled, earlier running
    ones finish, and the first non-NO in sequence order is returned as
    `(maximum, status, reason)`. None means every residual was accepted.
    """
    running = {}
    finished = {}
    launched = accepted = 0
    stop = len(sequence)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        try:
            while accepted < len(sequence):
                while (len(running) < jobs and launched < stop
                       and launched - accepted < lookahead):
                    cancel = threading.Event()
                    future = pool.submit(run, sequence[launched], cancel)
                    running[future] = (launched, time.monotonic(), cancel)
                    launched += 1
                if not running:
                    raise RuntimeError('ordered campaign has no running candidate')
                done, _ = wait(running, return_when=FIRST_COMPLETED)
                for future in done:
                    index, started, cancel = running.pop(future)
                    try:
                        status, reason = future.result()
                    except Exception as error:  # a runner failure is never a result
                        status, reason = 'ERROR', f'runner failure: {error!r}'
                    if status == 'CANCELLED' and cancel.is_set():
                        continue
                    finished[index] = (status, reason, time.monotonic() - started)
                    if status != 'NO' and index < stop:
                        stop = index
                        for later, _, other in running.values():
                            if later > index:
                                other.set()
                while accepted in finished:
                    status, reason, seconds = finished.pop(accepted)
                    if status != 'NO':
                        return sequence[accepted], status, reason
                    accept(sequence[accepted], seconds)
                    accepted += 1
        finally:
            for _, _, cancel in running.values():
                cancel.set()
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--candidates', type=Path, default=Path('progress/candidates.txt'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--start-after', type=int, required=True)
    parser.add_argument('--through', type=int, default=1_000_000)
    parser.add_argument('--candidate-timeout-seconds', type=float,
                        default=CANDIDATE_TIMEOUT_SECONDS)
    parser.add_argument('--jobs', type=int, default=0,
                        help='concurrent residual maxima; 0 uses every available processor')
    parser.add_argument('--threads-per-candidate', type=int, default=0,
                        help='solver threads per maximum; 0 divides the processors among jobs')
    parser.add_argument('--lookahead', type=int, default=LOOKAHEAD,
                        help='candidates that may start beyond the first unaccepted one')
    args = parser.parse_args()
    if not math.isfinite(args.candidate_timeout_seconds) or args.candidate_timeout_seconds <= 0:
        parser.error('candidate-timeout-seconds must be finite and positive')
    if args.jobs < 0 or args.threads_per_candidate < 0 or args.lookahead < 1:
        parser.error('jobs and threads-per-candidate must be non-negative; lookahead positive')
    processors = available_processors()
    jobs = args.jobs or processors
    # One job keeps the previous all-processor solver; several jobs split them.
    threads = args.threads_per_candidate or (0 if jobs == 1 else max(1, processors // jobs))
    execution = {'jobs': jobs, 'threads_per_candidate': threads, 'processors': processors}
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
    # Concurrency and solver threads change resource use and search order, not
    # the evidence required for acceptance; record every change of them.
    if state.get('execution') != execution:
        if state.get('execution') is not None:
            state.setdefault('execution_history', []).append(
                {**state['execution'], 'through': state['last_verified_no']})
        state['execution'] = execution
        save(state_path, state)
    sequence = [maximum for maximum in candidates
                if state['last_verified_no'] < maximum <= args.through]

    def accept(maximum, seconds):
        state['last_verified_no'] = maximum
        state['verified_no_count'] += 1
        save(state_path, state)
        print(json.dumps({'maximum': maximum, 'status': 'VERIFIED_NO',
                          'seconds': round(seconds, 3)}), flush=True)

    stopped = run_ordered(
        lambda maximum, cancel: run_one(
            binary, args.output, maximum, args.candidate_timeout_seconds, threads, cancel),
        sequence, jobs, args.lookahead, accept)
    if stopped is not None:
        maximum, status, reason = stopped
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
    state['state'] = 'COMPLETE'
    save(state_path, state)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
