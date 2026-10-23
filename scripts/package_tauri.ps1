$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Packaging is restricted to GitHub Actions.' }
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$output = Join-Path $root 'release-assets'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$portable = Join-Path $root 'qa\portable'
New-Item -ItemType Directory -Path $portable -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\fdx-atelier-tauri.exe') -Destination $portable
Copy-Item -LiteralPath (Join-Path $root 'target\release\fdx-cli.exe') -Destination $portable
Copy-Item -LiteralPath (Join-Path $root 'docs\GUIDE-TAURI.md') -Destination $portable
Compress-Archive -LiteralPath (Get-ChildItem -LiteralPath $portable).FullName -DestinationPath (Join-Path $output 'FDX-Atelier-Tauri-2.0.0-Windows-x64-portable.zip')
$installer = Get-ChildItem -LiteralPath (Join-Path $root 'target\release\bundle\nsis') -Filter '*.exe' | Select-Object -First 1
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $output 'FDX-Atelier-Tauri-2.0.0-Windows-x64-installation.exe')
$manifest = @{version='2.0.0';commit=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;rust=(rustc --version);node=(node --version);verification=@('core','parity','native-webdriver','playwright','installation')}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'BUILD.json') -Encoding utf8
$hashes = Get-ChildItem -LiteralPath $output -File | Sort-Object Name | ForEach-Object { $hash=Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256; $hash.Hash.ToLowerInvariant() + '  ' + $_.Name }
$hashes | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
