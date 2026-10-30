$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Packaging is restricted to GitHub Actions.' }
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = & python (Join-Path $PSScriptRoot 'promote_tauri.py') --check-manifests --source-root $root
if ($LASTEXITCODE -ne 0) { throw 'Release manifests do not agree.' }
$commit = (& git -C $root rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $commit -cnotmatch '^[0-9a-f]{40}$' -or $commit -cne $env:GITHUB_SHA) { throw 'Checkout does not match the verified Actions commit.' }
$output = Join-Path $root 'release-assets'
if (Test-Path -LiteralPath $output) { throw 'Release output must not already exist; refusing stale assets.' }
$installers = @(Get-ChildItem -LiteralPath (Join-Path $root 'target\release\bundle\nsis') -Filter '*.exe' -File)
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer.' }
$installer = $installers[0]
$installerHash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$installation = Get-Content -LiteralPath (Join-Path $root 'qa\installation.json') -Raw | ConvertFrom-Json
if ($installation.ok -ne $true -or $installation.installed -ne $true -or $installation.uninstalled -ne $true -or $installation.binary_verified -ne $true -or
    $installation.licenses_verified -ne $true -or $installation.upgrade_registration_verified -ne $true -or $installation.version -cne $version -or
    $installation.commit -cne $commit -or $installation.installer_sha256 -cne $installerHash) { throw 'Installer does not match successful installation and license verification.' }
foreach ($report in @('qa\parity.json', 'qa\native\verification.json')) {
    $result = Get-Content -LiteralPath (Join-Path $root $report) -Raw | ConvertFrom-Json
    if ($result.ok -ne $true) { throw "Verification report failed: $report" }
}
New-Item -ItemType Directory -Path $output | Out-Null
$installerName = "FDX-Atelier-$version-Windows-x64-installation.exe"
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $output $installerName)
$rust = & rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Cannot record Rust version.' }
$node = & node --version
if ($LASTEXITCODE -ne 0) { throw 'Cannot record Node version.' }
$manifest = [ordered]@{
    schema_version = 1
    product = 'FDX Atelier'
    version = $version
    commit = $commit
    repository = $env:GITHUB_REPOSITORY
    run = $env:GITHUB_RUN_ID
    run_attempt = $env:GITHUB_RUN_ATTEMPT
    installer = $installerName
    installer_sha256 = $installerHash
    rust = $rust
    node = $node
    verification = @('frontend', 'core', 'parity', 'native-webdriver', 'playwright', 'installation')
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'BUILD.json') -Encoding utf8
$hashes = Get-ChildItem -LiteralPath $output -File | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
    $hash.Hash.ToLowerInvariant() + '  ' + $_.Name
}
$hashes | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
