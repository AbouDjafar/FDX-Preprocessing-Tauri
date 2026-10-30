$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Verification reserved for a GitHub-hosted runner.' }
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = & python (Join-Path $PSScriptRoot 'promote_tauri.py') --check-manifests --source-root $root
if ($LASTEXITCODE -ne 0) { throw 'Release manifests do not agree.' }
$commit = (& git -C $root rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $commit -cne $env:GITHUB_SHA) { throw 'Checkout does not match the Actions commit.' }
$destination = Join-Path $root 'qa\installation'
if (Test-Path -LiteralPath $destination) { throw 'Installation test directory already exists.' }
$installers = @(Get-ChildItem -LiteralPath (Join-Path $root 'target\release\bundle\nsis') -Filter '*.exe' -File)
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer.' }
$installer = $installers[0]
$installerHash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$licenses = Join-Path $root 'qa\licenses'
$notices = Get-Content -LiteralPath (Join-Path $licenses 'DEPENDENCIES.json') -Raw | ConvertFrom-Json
if (@($notices).Count -eq 0 -or @($notices | Where-Object { $_.texts -gt 0 }).Count -eq 0) { throw 'Dependency license texts missing.' }
$oldPath = $env:PATH
$running = $null
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
    $builtHash = (Get-FileHash -LiteralPath (Join-Path $root 'target\release\fdx-atelier-tauri.exe') -Algorithm SHA256).Hash
    if ((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -cne $builtHash) { throw 'Installed executable differs from the verified build.' }
    foreach ($source in Get-ChildItem -LiteralPath $licenses -File -Recurse) {
        $relative = [IO.Path]::GetRelativePath($licenses, $source.FullName)
        $installed = Join-Path (Join-Path $destination 'licenses') $relative
        if (!(Test-Path -LiteralPath $installed -PathType Leaf)) { throw "Installed license missing: $relative" }
        if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash) { throw "Installed license differs: $relative" }
    }
    if (!(Test-Path -LiteralPath (Join-Path $destination 'GUIDE-TAURI.md') -PathType Leaf)) { throw 'Installed user guide missing.' }
    $running = Start-Process -FilePath $binary -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 6
    $running.Refresh()
    if ($running.HasExited) { throw 'Installed application exited before startup completed.' }
    Stop-Process -Id $running.Id
    $running = $null
    $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FDX Atelier'
    $legacyRegistration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FDX Atelier Tauri'
    $installedProduct = Get-ItemProperty -LiteralPath $registration
    if ($installedProduct.DisplayName -cne 'FDX Atelier' -or $installedProduct.DisplayVersion -cne $version) { throw 'Installed product name or version mismatch.' }
    if (Test-Path -LiteralPath $legacyRegistration) { throw 'Unexpected existing legacy installation on the runner.' }
    # Reuse the just-built installer with synthetic legacy registration; never fetch an old release.
    Copy-Item -LiteralPath $registration -Destination $legacyRegistration
    Set-ItemProperty -LiteralPath $legacyRegistration -Name DisplayName -Value 'FDX Atelier Tauri'
    Set-ItemProperty -LiteralPath $legacyRegistration -Name DisplayVersion -Value '2.0.0'
    Remove-Item -LiteralPath $registration
    $process = Start-Process -FilePath $installer.FullName -ArgumentList @('/S',('/D=' + $destination)) -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $binary)) { throw 'Legacy installation registration migration failed.' }
    if (Test-Path -LiteralPath $legacyRegistration) { throw 'Legacy uninstall registration was not migrated.' }
    $installedProduct = Get-ItemProperty -LiteralPath $registration
    if ($installedProduct.DisplayName -cne 'FDX Atelier' -or $installedProduct.DisplayVersion -cne $version) { throw 'Migrated product name or version mismatch.' }
    $uninstaller = Join-Path $destination 'uninstall.exe'
    if (!(Test-Path -LiteralPath $uninstaller)) { throw 'Uninstaller missing.' }
    $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw 'Uninstaller failed.' }
    $deadline = (Get-Date).AddSeconds(15)
    while ((Test-Path -LiteralPath $binary) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
    if (Test-Path -LiteralPath $binary) { throw 'Uninstaller left the application executable installed.' }
    if (Test-Path -LiteralPath $registration) { throw 'Uninstaller left the product registered.' }
    @{ok=$true; version=$version; commit=$commit; installer_sha256=$installerHash; licenses_verified=$true; upgrade_registration_verified=$true; no_developer_path=$true; installed=$true; uninstalled=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'qa\installation.json') -Encoding utf8
} finally {
    if ($running -and !$running.HasExited) { Stop-Process -Id $running.Id -ErrorAction SilentlyContinue }
    $env:PATH = $oldPath
}
