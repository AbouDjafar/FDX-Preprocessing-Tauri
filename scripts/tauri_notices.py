"""Prepare installer hooks and third-party licenses from CI dependencies, never personal paths."""
import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('License collection is restricted to GitHub Actions.')
OUTPUT = ROOT / 'qa' / 'licenses'
OUTPUT.mkdir(parents=True, exist_ok=True)
entries = []

# NSIS uses productName for uninstall registration, independently of the stable app identifier.
INSTALLER_HOOKS = r'''
!macro NSIS_HOOK_PREINSTALL
  Push $R7
  Push $R8
  Push $R9
  ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FDX Atelier Tauri" "UninstallString"
  ${If} $R8 != ""
    ReadRegStr $R9 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FDX Atelier Tauri" "InstallLocation"
    StrCpy $R7 $R9 1
    ${If} $R7 == '$\"'
      StrCpy $R9 $R9 -1 1
    ${EndIf}
    ${If} $R9 == ""
      SetErrorLevel 1
      Abort "Impossible de verifier l ancienne installation."
    ${EndIf}
    IfFileExists "$R9\uninstall.exe" 0 fdx_upgrade_failed
    ClearErrors
    ExecWait '$R8 /S /UPDATE _?=$R9' $R7
    IfErrors fdx_upgrade_failed
    ${If} $R7 != 0
      Goto fdx_upgrade_failed
    ${EndIf}
    IfFileExists "$R9\fdx-atelier-tauri.exe" fdx_upgrade_failed 0
    Delete "$R9\uninstall.exe"
    RMDir "$R9"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FDX Atelier Tauri"
  ${EndIf}
  Goto fdx_upgrade_done
  fdx_upgrade_failed:
    SetErrorLevel 1
    Abort "La mise a jour de l ancienne installation a echoue."
  fdx_upgrade_done:
  Pop $R9
  Pop $R8
  Pop $R7
  SetOutPath $INSTDIR
!macroend
'''


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
(ROOT / 'qa' / 'installer-hooks.nsh').write_text(INSTALLER_HOOKS, encoding='utf-8')
print(f'{len(entries)} dependency notices collected')
