$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Verification reserved for a GitHub-hosted runner.' }
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$destination = Join-Path $root 'qa\installation'
if (Test-Path -LiteralPath $destination) { throw 'Installation test directory already exists.' }
$installer = Get-ChildItem -LiteralPath (Join-Path $root 'target\release\bundle\nsis') -Filter '*.exe' | Select-Object -First 1
if (!$installer) { throw 'NSIS installer missing.' }
$oldPath = $env:PATH
try {
    $env:PATH = "$env:WINDIR\System32;$env:WINDIR"
    $cli = Join-Path $root 'target\release\fdx-cli.exe'
    $report = Join-Path $root 'qa\standalone.json'
    & $cli selftest --output $report
    if ($LASTEXITCODE -ne 0) { throw 'CLI requires undeclared developer dependencies.' }
    $process = Start-Process -FilePath $installer.FullName -ArgumentList @('/S',('/D=' + $destination)) -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw 'Installer failed.' }
    $binary = Join-Path $destination 'fdx-atelier-tauri.exe'
    if (!(Test-Path -LiteralPath $binary)) { throw 'Installed executable missing.' }
    $running = Start-Process -FilePath $binary -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 6
    $running.Refresh()
    if ($running.HasExited) { throw 'Installed application exited before startup completed.' }
    Stop-Process -Id $running.Id
    $uninstaller = Join-Path $destination 'uninstall.exe'
    if (!(Test-Path -LiteralPath $uninstaller)) { throw 'Uninstaller missing.' }
    $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw 'Uninstaller failed.' }
    @{ok=$true; no_developer_path=$true; installed=$true; uninstalled=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'qa\installation.json') -Encoding utf8
} finally { $env:PATH = $oldPath }
