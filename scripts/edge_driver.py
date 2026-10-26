"""Download the matching Microsoft driver on a GitHub-hosted Windows runner only."""
import json
import os
import re
import sys
import urllib.request
import urllib.error
import zipfile
from io import BytesIO
from pathlib import Path

if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('This helper is restricted to GitHub Actions.')
roots = [Path(os.environ[key]) / 'Microsoft/EdgeWebView/Application'
         for key in ('PROGRAMFILES(X86)', 'PROGRAMFILES', 'LOCALAPPDATA') if os.environ.get(key)]
runtimes = [binary for root in roots for binary in root.glob('*/msedgewebview2.exe')
            if re.fullmatch(r'\d+\.\d+\.\d+\.\d+', binary.parent.name)]
if not runtimes:
    raise SystemExit('No installed WebView2 Runtime was found on this runner')
runtime = max(runtimes, key=lambda binary: tuple(map(int, binary.parent.name.split('.'))))
version = runtime.parent.name

def download(driver_version):
    if not re.fullmatch(r'\d+\.\d+\.\d+\.\d+', driver_version) or driver_version.split('.')[:3] != version.split('.')[:3]:
        raise RuntimeError('The driver must match the actual WebView2 Runtime build')
    url = f'https://msedgedriver.microsoft.com/{driver_version}/edgedriver_win64.zip'
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read(50_000_000)

try:
    driver_version = version
    content = download(driver_version)
except urllib.error.HTTPError as failure:
    if failure.code != 404:
        raise
    url = f'https://msedgedriver.microsoft.com/LATEST_RELEASE_{version.split(".")[0]}_WINDOWS'
    with urllib.request.urlopen(url, timeout=30) as response:
        raw = response.read(100)
        driver_version = raw.decode('utf-16' if raw.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig').strip()
    content = download(driver_version)
with zipfile.ZipFile(BytesIO(content)) as archive:
    binary = archive.read('msedgedriver.exe')
destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
(destination / 'msedgedriver.exe').write_bytes(binary)
(destination / 'runtime.json').write_text(json.dumps({'runtime': version, 'driver': driver_version}), encoding='utf-8')
with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as environment:
    environment.write(f'WEBVIEW2_BROWSER_EXECUTABLE_FOLDER={runtime.parent}\n')
print(json.dumps({'webview2': version, 'driver': driver_version}))
