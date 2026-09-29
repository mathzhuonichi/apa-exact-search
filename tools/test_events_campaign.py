"""Campaign acceptance regressions using fake subprocesses, never a search."""

import json
import contextlib
import io
import math
from pathlib import Path
from subprocess import CompletedProcess, TimeoutExpired
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

import run_events_campaign as campaign


def argument(command, name):
    return command[command.index(name) + 1]


class FakePopen:
    """A solver process finishing after `delay` seconds; `finish` writes files."""

    def __init__(self, command, delay=0.0, finish=None, returncode=0, tracker=None):
        self.command = command
        self.done_at = time.monotonic() + delay
        self.finish = finish
        self.code = returncode
        self.returncode = None
        self.tracker = tracker
        if tracker is not None:
            tracker.start()

    def _complete(self, code):
        if self.returncode is None:
            self.returncode = code
            if self.tracker is not None:
                self.tracker.stop()

    def poll(self):
        if self.returncode is None and time.monotonic() >= self.done_at:
            if self.finish is not None:
                self.finish(self.command)
            self._complete(self.code)
        return self.returncode

    def wait(self, timeout=None):
        if self.returncode is not None:
            return self.returncode
        remaining = self.done_at - time.monotonic()
        if timeout is not None and remaining > timeout:
            time.sleep(timeout)
            raise TimeoutExpired(self.command, timeout)
        time.sleep(max(0.0, remaining))
        return self.poll()

    def kill(self):
        if self.tracker is not None and self.returncode is None:
            self.tracker.killed.append(int(argument(self.command, '--maximum')))
        self._complete(-9)


class Tracker:
    def __init__(self):
        self.lock = threading.Lock()
        self.active = self.peak = 0
        self.killed = []

    def start(self):
        with self.lock:
            self.active += 1
            self.peak = max(self.peak, self.active)

    def stop(self):
        with self.lock:
            self.active -= 1


class CampaignAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.output = Path(self.temporary.name) / 'outputs' / 'run'
        self.maximum = 113
        self.folder = self.output / 'candidates' / str(self.maximum)
        self.report = {
            'status': 'NO', 'evidence': 'VERIFIED_NO',
            'scope': 'complete_original_problem',
            'config': {'maximum': self.maximum},
        }
        self.proof = {'maximum': self.maximum}
        self.write_result = True
        self.write_proof = True
        self.replay_output = 'VERIFIED_NO\n'
        self.replays = 0

    def fake_process(self, command, **kwargs):
        if command[1] == 'solve-events':
            if self.write_result:
                Path(argument(command, '--output')).write_text(json.dumps(self.report))
            if self.write_proof:
                Path(argument(command, '--proof')).write_text(json.dumps(self.proof))
        else:
            self.assertEqual(command[1], 'verify-events')
            self.replays += 1
            kwargs['stdout'].write(self.replay_output)
        return CompletedProcess(command, 0)

    def fake_popen(self, command, stdout, stderr):
        return FakePopen(command, finish=self.fake_process)

    def run_one(self):
        with patch.object(campaign.subprocess, 'run', side_effect=self.fake_process), \
                patch.object(campaign.subprocess, 'Popen', side_effect=self.fake_popen):
            return campaign.run_one(Path('/fake/solver'), self.output, self.maximum, 1)

    def test_complete_no_requires_successful_replay(self):
        self.assertEqual(self.run_one(), ('NO', None))
        self.assertEqual(self.replays, 1)

    def test_external_lemma_no_cannot_advance(self):
        self.report['evidence'] = 'SOLVER_NO_EXTERNAL_LEMMAS'
        self.assertEqual(self.run_one()[0], 'ERROR')
        self.assertEqual(self.replays, 0)

    def test_unverified_no_cannot_advance(self):
        self.report['evidence'] = 'NONE'
        self.assertEqual(self.run_one()[0], 'ERROR')

    def test_conditional_no_cannot_advance(self):
        self.report['scope'] = 'conditional_branch'
        self.assertEqual(self.run_one()[0], 'ERROR')

    def test_result_must_name_requested_maximum(self):
        self.report['config']['maximum'] = 257
        self.assertEqual(self.run_one()[0], 'ERROR')

    def test_missing_result_maximum_is_rejected(self):
        del self.report['config']
        self.assertEqual(self.run_one()[0], 'ERROR')

    def test_proof_must_name_requested_maximum(self):
        self.proof['maximum'] = 257
        self.assertEqual(self.run_one()[0], 'ERROR')
        self.assertEqual(self.replays, 0)

    def test_stale_success_is_not_reused(self):
        self.folder.mkdir(parents=True)
        self.folder.joinpath('result.json').write_text(json.dumps(self.report))
        self.folder.joinpath('proof.json').write_text(json.dumps(self.proof))
        self.write_result = self.write_proof = False
        self.assertEqual(self.run_one()[0], 'ERROR')
        self.assertEqual(self.replays, 0)
        self.assertEqual(len(list(self.folder.glob('attempts/*/result.json'))), 1)

    def test_stale_proof_is_not_reused(self):
        self.folder.mkdir(parents=True)
        self.folder.joinpath('proof.json').write_text(json.dumps(self.proof))
        self.write_proof = False
        self.assertEqual(self.run_one()[0], 'ERROR')
        self.assertEqual(self.replays, 0)

    def test_non_no_stops_with_original_status(self):
        for status in ('UNKNOWN', 'YES', 'ERROR'):
            with self.subTest(status=status):
                self.report['status'] = status
                self.assertEqual(self.run_one()[0], status)
        self.assertEqual(self.replays, 0)

    def test_replay_failure_cannot_advance(self):
        self.replay_output = 'VALID_DERIVATION_WITH_EXTERNAL_LEMMAS\n'
        self.assertEqual(self.run_one()[0], 'ERROR')

    def test_timeout_remains_unknown(self):
        tracker = Tracker()
        with patch.object(campaign.subprocess, 'Popen',
                          side_effect=lambda command, stdout, stderr: FakePopen(
                              command, delay=math.inf, tracker=tracker)):
            self.assertEqual(
                campaign.run_one(Path('/fake/solver'), self.output, 113, 0.05)[0], 'UNKNOWN')
        self.assertEqual(tracker.killed, [113])
        self.assertTrue(self.folder.joinpath('timeout.json').exists())

    def test_invalid_no_does_not_advance_checkpoint(self):
        candidates = Path(self.temporary.name) / 'candidates.txt'
        candidates.write_text('112\n113\n114\n')
        self.report['evidence'] = 'SOLVER_NO_EXTERNAL_LEMMAS'
        args = ['campaign', '--binary', '/fake/solver', '--candidates', str(candidates),
                '--output', str(self.output), '--start-after', '112']
        with patch('sys.argv', args), \
                patch.object(campaign.subprocess, 'run', side_effect=self.fake_process), \
                patch.object(campaign.subprocess, 'Popen', side_effect=self.fake_popen), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(campaign.main(), 1)
        state = json.loads((self.output / 'state.json').read_text())
        self.assertEqual(state['state'], 'ATTENTION')
        self.assertEqual(state['last_verified_no'], 112)
        self.assertEqual(state['verified_no_count'], 0)
        self.assertEqual(state['attention']['maximum'], 113)
        self.assertEqual(self.replays, 0)


if __name__ == '__main__':
    unittest.main()


class OrderedPipelineTests(unittest.TestCase):
    """Concurrent execution must accept exactly what sequential execution would."""

    def test_randomized_schedules_match_sequential_acceptance(self):
        import random
        rng = random.Random(20260929)
        for trial in range(150):
            length = rng.randint(1, 25)
            sequence = list(range(100, 100 + 3 * length, 3))
            outcomes = {m: 'NO' for m in sequence}
            for m in rng.sample(sequence, rng.randint(0, min(3, length))):
                outcomes[m] = rng.choice(['UNKNOWN', 'YES', 'ERROR', 'CANCELLED'])
            delays = {m: rng.choice([0, 0, 0.001, 0.003, 0.008]) for m in sequence}
            jobs = rng.randint(1, 6)
            lookahead = rng.choice([1, 2, 5, 1000])
            lock = threading.Lock()
            active = [0, 0]

            def run(maximum, cancel):
                with lock:
                    active[0] += 1
                    active[1] = max(active[1], active[0])
                deadline = time.monotonic() + delays[maximum]
                while time.monotonic() < deadline and not cancel.is_set():
                    time.sleep(0.0005)
                with lock:
                    active[0] -= 1
                if cancel.is_set() and time.monotonic() < deadline:
                    return 'CANCELLED', 'cancelled'
                return outcomes[maximum], None

            accepted = []
            stopped = campaign.run_ordered(
                run, sequence, jobs, lookahead, lambda m, seconds: accepted.append(m))
            first = next((m for m in sequence if outcomes[m] != 'NO'), None)
            expected = sequence if first is None else sequence[:sequence.index(first)]
            self.assertEqual(accepted, expected, (trial, outcomes, jobs, lookahead))
            if first is None:
                self.assertIsNone(stopped)
            else:
                self.assertEqual(stopped[:2], (first, outcomes[first]))
            self.assertLessEqual(active[1], jobs)


class OrderedCampaignTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / 'outputs' / 'run'
        self.candidates = self.root / 'candidates.txt'

    def run_campaign(self, sequence, outcomes=None, delays=None, extra=(), processors=4):
        outcomes = outcomes or {}
        delays = delays or {}
        self.candidates.write_text(''.join(f'{m}\n' for m in sequence))
        tracker = Tracker()
        commands = []
        finished = []

        def finish(command):
            maximum = int(argument(command, '--maximum'))
            finished.append(maximum)
            outcome = outcomes.get(maximum, 'NO')
            report = {'status': outcome, 'evidence': 'VERIFIED_NO' if outcome == 'NO' else 'NONE',
                      'scope': 'complete_original_problem', 'config': {'maximum': maximum}}
            Path(argument(command, '--output')).write_text(json.dumps(report))
            Path(argument(command, '--proof')).write_text(json.dumps({'maximum': maximum}))

        def popen(command, stdout, stderr):
            maximum = int(argument(command, '--maximum'))
            commands.append((command, list(finished)))
            outcome = outcomes.get(maximum, 'NO')
            delay = math.inf if outcome == 'HANG' else delays.get(maximum, 0.01)
            code = 3 if outcome == 'EXIT' else 0
            return FakePopen(command, delay, None if code else finish, code, tracker)

        def replay(command, **kwargs):
            kwargs['stdout'].write('VERIFIED_NO\n')
            return CompletedProcess(command, 0)

        args = ['campaign', '--binary', '/fake/solver', '--candidates', str(self.candidates),
                '--output', str(self.output), '--start-after', str(sequence[0]), *extra]
        out = io.StringIO()
        started = time.monotonic()
        with patch('sys.argv', args), \
                patch.object(campaign.subprocess, 'Popen', side_effect=popen), \
                patch.object(campaign.subprocess, 'run', side_effect=replay), \
                patch.object(campaign, 'available_processors', return_value=processors), \
                contextlib.redirect_stdout(out):
            code = campaign.main()
        self.elapsed = time.monotonic() - started
        state = json.loads((self.output / 'state.json').read_text())
        accepted = [json.loads(line)['maximum'] for line in out.getvalue().splitlines()
                    if '"VERIFIED_NO"' in line]
        return code, state, accepted, tracker, commands

    def test_concurrent_results_are_accepted_in_residual_order(self):
        sequence = list(range(10, 42, 2))
        delays = {m: 0.01 + 0.03 * ((40 - m) % 5) for m in sequence}
        code, state, accepted, tracker, commands = self.run_campaign(sequence, delays=delays)
        self.assertEqual(code, 0)
        self.assertEqual(state['state'], 'COMPLETE')
        self.assertEqual(accepted, sequence[1:])
        self.assertEqual((state['last_verified_no'], state['verified_no_count']), (40, 15))
        self.assertEqual(tracker.peak, 4)
        self.assertEqual(state['execution'], {'jobs': 4, 'threads_per_candidate': 1, 'processors': 4})
        for command, _ in commands:
            self.assertEqual(command[4:4 + len(campaign.OPTIONS)], campaign.solver_options(1))

    def test_first_failure_in_residual_order_is_reported(self):
        sequence = list(range(10, 42, 2))
        outcomes = {20: 'EXIT', 30: 'UNKNOWN'}
        delays = {20: 0.3, 30: 0.01}
        code, state, accepted, _, _ = self.run_campaign(sequence, outcomes, delays)
        self.assertEqual(code, 1)
        self.assertEqual(state['state'], 'ATTENTION')
        self.assertEqual(state['attention']['maximum'], 20)
        self.assertEqual(state['attention']['status'], 'ERROR')
        self.assertEqual(accepted, [12, 14, 16, 18])
        self.assertEqual((state['last_verified_no'], state['verified_no_count']), (18, 4))
        pause = json.loads((self.root / 'PAUSED.json').read_text())
        self.assertEqual((pause['paused'], pause['resume_from']), (True, 20))

    def test_failure_cancels_only_later_candidates(self):
        sequence = list(range(10, 30, 2))
        outcomes = {m: 'HANG' for m in sequence if m > 16}
        outcomes[16] = 'UNKNOWN'
        delays = {12: 0.4, 14: 0.4, 16: 0.05}
        code, state, accepted, tracker, _ = self.run_campaign(sequence, outcomes, delays)
        self.assertEqual(code, 1)
        self.assertEqual(state['attention']['maximum'], 16)
        self.assertEqual(accepted, [12, 14])
        self.assertTrue(tracker.killed and all(m > 16 for m in tracker.killed))
        for maximum in tracker.killed:
            cancelled = self.output / 'candidates' / str(maximum) / 'cancelled.json'
            self.assertEqual(json.loads(cancelled.read_text())['status'], 'CANCELLED')
        self.assertLess(self.elapsed, 5)

    def test_solver_reported_cancellation_still_stops(self):
        code, state, accepted, _, _ = self.run_campaign([10, 12, 14, 16], {14: 'CANCELLED'})
        self.assertEqual(code, 1)
        self.assertEqual(state['attention']['maximum'], 14)
        self.assertEqual(accepted, [12])

    def test_lookahead_bounds_candidates_beyond_the_boundary(self):
        sequence = list(range(10, 30, 2))
        _, state, accepted, _, commands = self.run_campaign(
            sequence, delays={12: 0.3}, extra=['--lookahead', '3'])
        self.assertEqual(accepted, sequence[1:])
        for command, done in commands[3:]:
            self.assertIn(12, done, 'a fourth candidate started before 12 finished')

    def test_single_job_keeps_the_previous_solver_command(self):
        code, state, accepted, tracker, commands = self.run_campaign(
            [10, 12, 14, 16], extra=['--jobs', '1'])
        self.assertEqual((code, accepted, tracker.peak), (0, [12, 14, 16], 1))
        for command, _ in commands:
            self.assertEqual(command[4:4 + len(campaign.OPTIONS)], campaign.OPTIONS)

    def test_resource_changes_are_recorded_but_options_are_enforced(self):
        self.run_campaign([10, 12, 14])
        code, state, _, _, _ = self.run_campaign([10, 12, 14], extra=['--jobs', '1'])
        self.assertEqual(code, 0)
        self.assertEqual(state['execution']['jobs'], 1)
        self.assertEqual(state['execution_history'],
                         [{'jobs': 4, 'threads_per_candidate': 1, 'processors': 4, 'through': 14}])
        state['options'] = ['--threads', '0']
        (self.output / 'state.json').write_text(json.dumps(state))
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            self.run_campaign([10, 12, 14])
