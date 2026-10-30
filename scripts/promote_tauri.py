"""Validate release metadata and promote verified Actions assets without rebuilding."""
import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCT = 'FDX Atelier'
VERSION_PATTERN = r'(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)'
VERIFICATIONS = {'frontend', 'core', 'parity', 'native-webdriver', 'playwright', 'installation'}
WINDOWS_WORKFLOW = '.github/workflows/windows-tauri.yml'
CHECKS_WORKFLOW = '.github/workflows/checks.yml'


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def release_version(root):
    """The web package supplies the version; all shipped manifests must agree."""
    package = read_json(root / 'web/package.json')
    version = package['version']
    if not isinstance(version, str) or not re.fullmatch(VERSION_PATTERN, version):
        raise ValueError('Release version must be a stable major.minor.patch version.')
    workspace = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))
    cargo_lock = tomllib.loads((root / 'Cargo.lock').read_text(encoding='utf-8'))
    web_lock = read_json(root / 'web/package-lock.json')
    config = read_json(root / 'src-tauri/tauri.conf.json')
    versions = [workspace['workspace']['package']['version'], config['version'],
                web_lock['version'], web_lock['packages']['']['version']]
    members = {'fdx-core', 'fdx-cli', 'fdx-atelier-tauri'}
    locked = [item for item in cargo_lock['package'] if item['name'] in members and not item.get('source')]
    if {item['name'] for item in locked} != members or len(locked) != len(members):
        raise ValueError('Workspace lockfile entries are incomplete.')
    versions.extend(item['version'] for item in locked)
    if any(item != version for item in versions):
        raise ValueError('Web, Cargo, lockfile and native application versions must match.')
    if config['productName'] != PRODUCT or config['identifier'] != 'com.fdx.atelier.tauri':
        raise ValueError('Product name or stable application identifier mismatch.')
    return version


def installer_name(version):
    return f'FDX-Atelier-{version}-Windows-x64-installation.exe'


def gh(*args, json_output=False):
    result = subprocess.run(['gh', *args], check=True, capture_output=True, text=True)
    return json.loads(result.stdout) if json_output else result.stdout.strip()


def successful_run(run, repository, workflow, commit=None):
    return (run.get('status') == 'completed' and run.get('conclusion') == 'success'
            and run.get('path') == workflow
            and (run.get('head_repository') or {}).get('full_name') == repository
            and (commit is None or run.get('head_sha') == commit))


def verified_run(repository, run_id, tag, version):
    if not re.fullmatch(r'[1-9]\d*', run_id) or tag != f'v{version}':
        raise ValueError('Run ID or tag does not match the release manifests.')
    run = gh('api', f'repos/{repository}/actions/runs/{run_id}', json_output=True)
    if (not successful_run(run, repository, WINDOWS_WORKFLOW)
            or run.get('event') != 'workflow_dispatch' or str(run.get('id')) != run_id):
        raise ValueError('Only a successful native Windows verification run from this repository can be promoted.')
    commit = run['head_sha']
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Invalid verified commit SHA.')
    if gh('api', f'repos/{repository}/commits/{tag}', '--jq', '.sha') != commit:
        raise ValueError('The tag must point to the exact verified commit, not a newer source tree.')
    page = 1
    while True:
        checks = gh('api', f'repos/{repository}/actions/workflows/checks.yml/runs'
                    f'?head_sha={commit}&status=success&per_page=100&page={page}', json_output=True)
        runs = checks['workflow_runs']
        if any(successful_run(item, repository, CHECKS_WORKFLOW, commit) for item in runs):
            return run
        if len(runs) < 100:
            raise ValueError('Rust/React/parity checks must also pass for this exact commit and repository.')
        page += 1


def verified_assets(folder, repository, run, version):
    """Only the installer and its two provenance files may cross the release boundary."""
    installer = installer_name(version)
    expected_files = {installer, 'BUILD.json', 'SHA256SUMS.txt'}
    paths = list(folder.iterdir())
    if ({path.name for path in paths} != expected_files
            or any(path.is_symlink() or not path.is_file() for path in paths)):
        raise ValueError('Expected exactly one installer, BUILD.json and SHA256SUMS.txt; no portable or extra assets.')
    manifest = read_json(folder / 'BUILD.json')
    expected = {'schema_version': 1, 'product': PRODUCT, 'version': version,
                'commit': run['head_sha'], 'repository': repository,
                'run': str(run['id']), 'run_attempt': str(run['run_attempt']), 'installer': installer}
    if any(manifest.get(key) != value for key, value in expected.items()):
        raise ValueError('Build manifest provenance mismatch.')
    verification = manifest.get('verification', [])
    if (not isinstance(verification, list) or any(not isinstance(item, str) for item in verification)
            or sorted(verification) != sorted(VERIFICATIONS)):
        raise ValueError('Build manifest verification gates are incomplete.')
    checksums = {}
    for line in (folder / 'SHA256SUMS.txt').read_text(encoding='ascii').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  ([A-Za-z0-9._-]+)', line)
        if not match or match[2] in checksums:
            raise ValueError('Malformed or duplicate asset checksum.')
        checksums[match[2]] = match[1]
    if checksums.keys() != expected_files - {'SHA256SUMS.txt'}:
        raise ValueError('Checksums must cover the installer and BUILD.json exactly.')
    for name, expected_hash in checksums.items():
        with (folder / name).open('rb') as stream:
            actual = hashlib.file_digest(stream, 'sha256').hexdigest()
        if actual != expected_hash:
            raise ValueError(f'Asset hash verification failed: {name}')
    if manifest.get('installer_sha256') != checksums[installer]:
        raise ValueError('Installer differs from the installation-verified hash.')
    return sorted(str(folder / name) for name in expected_files)


def promote(source_root):
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('Release promotion must run on GitHub Actions.')
    repository = os.environ['GITHUB_REPOSITORY']
    run_id, tag = os.environ['RUN_ID'], os.environ['TAG']
    version = release_version(source_root)
    run = verified_run(repository, run_id, tag, version)
    commit = run['head_sha']
    checkout = subprocess.check_output(['git', '-C', str(source_root), 'rev-parse', 'HEAD'], text=True).strip()
    if checkout != commit:
        raise ValueError('Release manifests must be read from the exact verified source commit.')
    with tempfile.TemporaryDirectory() as temporary:
        folder = Path(temporary)
        gh('run', 'download', run_id, '--repo', repository, '--name', f'windows-verified-{commit}', '--dir', str(folder))
        assets = verified_assets(folder, repository, run, version)
        if gh('api', f'repos/{repository}/commits/{tag}', '--jq', '.sha') != commit:
            raise ValueError('Release tag moved during verification.')
        notes = (f'Application Windows x64 locale, React et moteur Rust.\n\n'
                 f'Commit verifie : `{commit}`.\nBuild et tests : {run["html_url"]}\n\n'
                 'Distribution : installateur uniquement, avec licences tierces incluses. '
                 'Telechargement manuel. WebView2 est verifie par l installateur ; '
                 'son installation peut necessiter Internet. Binaires non signes : '
                 'Windows peut afficher un avertissement de provenance. Aucun moteur Python embarque.')
        # gh release create fails for an existing release; never upload with --clobber.
        gh('release', 'create', tag, *assets, '--repo', repository, '--verify-tag',
           '--title', f'{PRODUCT} {version}', '--notes', notes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check-manifests', action='store_true')
    parser.add_argument('--source-root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        if args.check_manifests:
            print(release_version(args.source_root))
        else:
            promote(args.source_root)
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit(str(error)) from error


if __name__ == '__main__':
    main()
