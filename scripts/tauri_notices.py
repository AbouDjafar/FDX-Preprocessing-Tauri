"""Collect third-party license texts from CI dependency installations, never personal paths."""
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'qa' / 'portable' / 'licenses'
OUTPUT.mkdir(parents=True, exist_ok=True)
entries = []


def record(name, version, license_name, folder, explicit=None):
    safe = ''.join(character if character.isalnum() or character in '._-' else '_' for character in name)
    destination = OUTPUT / f'{safe}-{version}'
    candidates = {p for pattern in ('LICENSE*', 'LICENCE*', 'COPYING*', 'NOTICE*', 'OFL*') for p in folder.glob(pattern) if p.is_file()}
    if explicit:
        target = (folder / explicit).resolve()
        if target.is_relative_to(folder.resolve()) and target.is_file():
            candidates.add(target)
    if candidates:
        destination.mkdir(parents=True, exist_ok=True)
        for source in sorted(candidates):
            shutil.copyfile(source, destination / source.name)
    entries.append({'name': name, 'version': version, 'license': license_name, 'texts': len(candidates)})


metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=ROOT))
for package in metadata['packages']:
    if package['source']:
        record(package['name'], package['version'], package.get('license'), Path(package['manifest_path']).parent, package.get('license_file'))

modules = ROOT / 'web' / 'node_modules'
for folder in modules.iterdir():
    candidates = folder.iterdir() if folder.name.startswith('@') and folder.is_dir() else [folder]
    for candidate in candidates:
        manifest = candidate / 'package.json'
        if manifest.is_file():
            package = json.loads(manifest.read_text(encoding='utf-8'))
            record(package.get('name', candidate.name), package.get('version', 'unknown'), package.get('license'), candidate)

(OUTPUT / 'DEPENDENCIES.json').write_text(json.dumps(entries, ensure_ascii=False, indent=2), encoding='utf-8')
(OUTPUT / 'README.txt').write_text('Third-party component notices. Includes development and verification tools for traceability; not every listed component is included in the executable. Microsoft WebView2 is supplied separately under Microsoft terms. FDX application source licensing is separate from these component licenses.\n', encoding='utf-8')
print(f'{len(entries)} dependency notices collected')
