"""Download the matching Microsoft driver on a GitHub-hosted Windows runner only."""
import json
import os
import subprocess
import sys
import urllib.request
import zipfile
from io import BytesIO
from pathlib import Path

if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('This helper is restricted to GitHub Actions.')
edge = Path(os.environ['PROGRAMFILES(X86)']) / 'Microsoft/Edge/Application/msedge.exe'
version = subprocess.check_output(['powershell', '-NoProfile', '-Command', f"(Get-Item -LiteralPath '{edge}').VersionInfo.ProductVersion"], text=True).strip()
if not all(part.isdigit() for part in version.split('.')) or len(version.split('.')) != 4:
    raise SystemExit('Unexpected Edge version')
url = f'https://msedgedriver.microsoft.com/{version}/edgedriver_win64.zip'
with urllib.request.urlopen(url, timeout=90) as response:
    content = response.read(50_000_000)
with zipfile.ZipFile(BytesIO(content)) as archive:
    binary = archive.read('msedgedriver.exe')
destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
(destination / 'msedgedriver.exe').write_bytes(binary)
print(json.dumps({'edge': version, 'driver': 'msedgedriver.exe'}))
