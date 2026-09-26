"""Campaign acceptance regressions using fake subprocesses, never a search."""

import json
import contextlib
import io
from pathlib import Path
from subprocess import CompletedProcess, TimeoutExpired
import tempfile
import unittest
from unittest.mock import patch

import run_events_campaign as campaign


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
                self.folder.joinpath('result.json').write_text(json.dumps(self.report))
            if self.write_proof:
                self.folder.joinpath('proof.json').write_text(json.dumps(self.proof))
        else:
            self.assertEqual(command[1], 'verify-events')
            self.replays += 1
            kwargs['stdout'].write(self.replay_output)
        return CompletedProcess(command, 0)

    def run_one(self):
        with patch.object(campaign.subprocess, 'run', side_effect=self.fake_process):
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
        with patch.object(campaign.subprocess, 'run', side_effect=TimeoutExpired('fake', 1)):
            self.assertEqual(campaign.run_one(Path('/fake/solver'), self.output, 113, 1)[0], 'UNKNOWN')

    def test_invalid_no_does_not_advance_checkpoint(self):
        candidates = Path(self.temporary.name) / 'candidates.txt'
        candidates.write_text('112\n113\n114\n')
        self.report['evidence'] = 'SOLVER_NO_EXTERNAL_LEMMAS'
        args = ['campaign', '--binary', '/fake/solver', '--candidates', str(candidates),
                '--output', str(self.output), '--start-after', '112']
        with patch('sys.argv', args), \
                patch.object(campaign.subprocess, 'run', side_effect=self.fake_process), \
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
