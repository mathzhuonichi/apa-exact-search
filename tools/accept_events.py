"""Required v2 acceptance: regenerate both NO certificates, then replay each.

Only the two known regression maxima are run; this is not a candidate campaign.
UNKNOWN, conditional evidence, and invalid/incomplete certificates fail the run.
No historical proof is an input. The exact same configuration is used for both.
"""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    records = []
    # Shared-root work is single-threaded, so resource use is identical. The
    # one-node DFS limit makes accidental reliance on an expensive DFS visible.
    options = ['--threads', '1', '--root-strengthen', '--probes',
               '--root-seconds', '120', '--seconds', '1', '--node-limit', '1',
               '--prime-chain-steps', '50000000', '--probe-case-events', '1000000',
               '--probe-members', '2000', '--root-probe-width', '8',
               '--probe-events', '200000000']
    for n in [218303, 221969]:
        certificate = args.output / f'{n}.proof.json'
        result = args.output / f'{n}.json'
        command = [str(binary), 'solve-events', '--maximum', str(n), *options,
                   '--proof', str(certificate), '--output', str(result)]
        completed = subprocess.run(command, text=True, capture_output=True, timeout=180, check=True)
        report = json.loads(completed.stdout)
        if report['status'] != 'NO' or report['evidence'] != 'VERIFIED_NO':
            raise RuntimeError(f'{n} failed: {report["status"]}, {report["evidence"]}')
        if report['scope'] != 'complete_original_problem':
            raise RuntimeError(f'{n} returned a conditional refutation')
        replay = subprocess.run([str(binary), 'verify-events', '--proof', str(certificate)],
                                text=True, capture_output=True, timeout=60, check=True)
        if replay.stdout.strip() != 'VERIFIED_NO':
            raise RuntimeError(f'{n} was not independently replayed')
        records.append({'maximum': n, 'status': report['status'], 'evidence': report['evidence'],
                        'shared_root_time': report['shared_root_time'],
                        'verification_time': report['verification_time'],
                        'proof_nodes': report['compressed_proof_events'],
                        'root_rounds': report['root_metrics']['root_strengthen_rounds'],
                        'replay': replay.stdout.strip()})
        print(json.dumps(records[-1]), flush=True)
    (args.output / 'acceptance.json').write_text(
        json.dumps({'status': 'PASS', 'same_options': options, 'results': records}, indent=2)+'\n',
        encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
