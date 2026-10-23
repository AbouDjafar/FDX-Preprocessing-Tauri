"""Promote an already verified Actions artifact. Never rebuild or download locally."""
import hashlib
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path


def gh(*args, json_output=False):
    result = subprocess.run(['gh', *args], check=True, capture_output=True, text=True)
    return json.loads(result.stdout) if json_output else result.stdout.strip()


if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('Release promotion must run on GitHub Actions.')
repository = os.environ['GITHUB_REPOSITORY']
run_id, tag = os.environ['RUN_ID'], os.environ['TAG']
if not run_id.isdigit() or not re.fullmatch(r'v2\.\d+\.\d+', tag):
    raise SystemExit('Invalid run ID or version tag.')
run = gh('api', f'repos/{repository}/actions/runs/{run_id}', json_output=True)
if run['conclusion'] != 'success' or run['path'] != '.github/workflows/windows-tauri.yml' or run['head_repository']['full_name'] != repository:
    raise SystemExit('Only a successful native Windows verification run from this repository can be promoted.')
commit = run['head_sha']
tag_commit = gh('api', f'repos/{repository}/commits/{tag}', '--jq', '.sha')
if tag_commit != commit:
    raise SystemExit('The tag must point to the exact verified commit, not a newer source tree.')
checks = gh('api', f'repos/{repository}/actions/runs?head_sha={commit}&status=success&per_page=100', json_output=True)
if not any(item['path'] == '.github/workflows/checks.yml' for item in checks['workflow_runs']):
    raise SystemExit('Rust/React/parity checks must also pass for this exact commit.')
with tempfile.TemporaryDirectory() as temporary:
    folder = Path(temporary)
    gh('run', 'download', run_id, '--repo', repository, '--name', f'windows-verified-{commit}', '--dir', str(folder))
    manifest = json.loads((folder / 'BUILD.json').read_text(encoding='utf-8-sig'))
    if manifest['commit'] != commit or manifest['version'] != tag[1:]:
        raise SystemExit('Build manifest mismatch.')
    for line in (folder / 'SHA256SUMS.txt').read_text(encoding='ascii').splitlines():
        expected, name = line.split('  ', 1)
        if Path(name).name != name or hashlib.sha256((folder / name).read_bytes()).hexdigest() != expected:
            raise SystemExit('Asset hash verification failed.')
    assets = sorted(str(path) for path in folder.iterdir() if path.is_file())
    notes = f'Application Windows x64 locale, React et moteur Rust.\n\nCommit verifie : `{commit}`.\nBuild et tests : {run["html_url"]}\n\nTelechargement manuel. Le portable utilise WebView2. L installateur verifie sa disponibilite. Binaires non signes : Windows peut afficher un avertissement de provenance. Aucun moteur Python embarque.'
    # Existing releases are never silently overwritten.
    gh('release', 'create', tag, *assets, '--repo', repository, '--verify-tag', '--title', f'FDX Atelier Tauri {tag[1:]}', '--notes', notes)
