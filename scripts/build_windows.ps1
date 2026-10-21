param([switch]$SkipTests, [string]$ISCC = "", [ValidateSet('PyInstaller', 'Nuitka')][string]$Packager = 'PyInstaller')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$pythonExe = Join-Path $projectRoot '.venv\Scripts\python.exe'
if (-not (Test-Path -LiteralPath $pythonExe)) { throw 'Environnement .venv absent. Voir README.md.' }
if (-not $SkipTests) {
    $env:QT_QPA_PLATFORM = 'offscreen'
    & $pythonExe -m pytest -q
    if ($LASTEXITCODE -ne 0) { throw 'Tests en echec' }
}
New-Item -ItemType Directory -Path (Join-Path $projectRoot 'build'), (Join-Path $projectRoot 'dist') -Force | Out-Null
$env:QT_QPA_PLATFORM = 'offscreen'
& $pythonExe (Join-Path $PSScriptRoot 'make_icon.py')
if ($LASTEXITCODE -ne 0) { throw 'Creation icone en echec' }
$env:PYTHONPATH = Join-Path $projectRoot 'src'
if ($Packager -eq 'Nuitka') {
    $deployScript = Join-Path $projectRoot '.venv\Lib\site-packages\PySide6\scripts\deploy.py'
    & $pythonExe $deployScript -c (Join-Path $projectRoot 'pysidedeploy.spec') --no-install -f --keep-deployment-files --extra-ignore-dirs=.venv,tests,qa,build,dist,.build-tools
} else {
    & $pythonExe -m PyInstaller --noconfirm --distpath dist --workpath build\pyinstaller packaging\FDXAtelier.spec
}
if ($LASTEXITCODE -ne 0) { throw ('Conditionnement en echec : ' + $Packager) }
$binary = Get-ChildItem -LiteralPath (Join-Path $projectRoot 'dist') -Filter FDXAtelier.exe -Recurse | Select-Object -First 1
if (-not $binary) { throw 'Executable absent apres compilation' }
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $binary.DirectoryName
Copy-Item -LiteralPath (Join-Path $projectRoot 'assets\fdx.ico') -Destination $binary.DirectoryName
& $pythonExe (Join-Path $PSScriptRoot 'package_notices.py') (Join-Path $binary.DirectoryName 'licenses')
if (-not $ISCC) {
    $candidate = Join-Path $projectRoot '.build-tools\InnoSetup7\ISCC.exe'
    if (Test-Path -LiteralPath $candidate) { $ISCC = $candidate }
}
if ($ISCC) {
    & $ISCC ('/DAppSource=' + $binary.DirectoryName) (Join-Path $projectRoot 'packaging\installer.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Creation installateur en echec' }
}
Write-Output ('Executable: ' + $binary.FullName)

