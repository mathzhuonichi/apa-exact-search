"""Fixed known-outcome controls; never advances the residual campaign.

Run after cargo build --release. Uses only the Python standard library.
Large known cases are opt-in and UNKNOWN is recorded, never converted to NO.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--large', action='store_true')
    parser.add_argument('--root-strengthen', action='store_true',
                        help='enable the v2 shared-root loop in every control')
    args = parser.parse_args()
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    reports = []
    controls = [31, 63, 64, 65, 97, 111, 112, 113, 114, 127, 128, 129, 191, 257]

    def invoke(command):
        result = subprocess.run([str(binary), *command], capture_output=True,
                                text=True, check=True, timeout=90)
        return json.loads(result.stdout)

    for n in controls:
        baseline = invoke(['solve', '--maximum', str(n), '--threads', '1', '--seconds', '5'])
        assert baseline['status'] == ('YES' if n == 113 else 'NO'), baseline
        for policy in ['mrv', 'strict-maximum-row-first']:
            for probes in [False, True]:
                for lemmas in [False, True]:
                    tag = f'{n}-{policy}-probe{int(probes)}-lemma{int(lemmas)}'
                    proof = args.output / f'{tag}.proof.json'
                    command = ['solve-events', '--maximum', str(n), '--threads', '1',
                               '--seconds', '5', '--root-seconds', '5', '--policy', policy,
                               '--proof', str(proof)]
                    if probes:
                        command += ['--probes']
                    if args.root_strengthen:
                        command += ['--root-strengthen']
                    if lemmas:
                        command += ['--universal-members', '--prime-clauses', '--even-bound']
                    if probes and lemmas:
                        command += ['--seed-probe']
                    report = invoke(command)
                    assert report['status'] == baseline['status'], report
                    if report['status'] == 'YES':
                        a = report['example']
                        products = {x*y for x in a for y in a}
                        assert max(a) == n and min(a) >= 1 and all(x+y in products for x in a for y in a)
                    else:
                        check = ['verify-events', '--proof', str(proof)]
                        if report['evidence'] == 'SOLVER_NO_EXTERNAL_LEMMAS':
                            check += ['--allow-external-lemmas']
                        subprocess.run([str(binary), *check], capture_output=True, check=True, timeout=30)
                    reports.append({'case': tag, 'baseline_status': baseline['status'], **report})

    if args.large:
        for n in [218303, 221969]:
            for policy in ['mrv', 'strict-maximum-row-first']:
                for probes in [False, True]:
                    # Same wall time and worker count in every large control.
                    command = ['solve-events', '--maximum', str(n), '--threads', '1',
                               '--root-seconds', '5', '--seconds', '5', '--policy', policy,
                               '--proof', str(args.output / f'{n}-{policy}-{probes}.proof.json')]
                    if probes:
                        command += ['--probes']
                    report = invoke(command)
                    assert report['status'] in ['NO', 'UNKNOWN'], report
                    reports.append({'case': f'large-{n}-{policy}-probe{int(probes)}', **report})
        # Replay the existing independent certificate in a copy so the historical
        # verification record is not overwritten by its original checker.
        source = Path(__file__).resolve().parents[1] / 'evidence' / '221969'
        replay = args.output / 'historical-221969'
        replay.mkdir(exist_ok=True)
        for name in ['verify.py', 'proof.json.gz', 'root-premises.json.gz']:
            shutil.copyfile(source / name, replay / name)
        import sys
        result = subprocess.run([sys.executable, str(replay / 'verify.py')],
                                capture_output=True, text=True, check=True, timeout=90)
        reports.append({'case': 'historical-independent-221969', **json.loads(result.stdout)})

    (args.output / 'summary.json').write_text(json.dumps(reports, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({'records': len(reports), 'summary': str(args.output / 'summary.json')}))


if __name__ == '__main__':
    main()
