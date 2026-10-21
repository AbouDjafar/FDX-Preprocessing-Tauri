param([switch]$TestInstaller)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$qa = Join-Path $projectRoot 'qa\windows'
New-Item -ItemType Directory -Path $qa -Force | Out-Null
$env:PYTHONPATH = ''
$env:PYTHONHOME = ''
$env:PATH = (Join-Path $env:WINDIR 'System32') + ';' + $env:WINDIR

function Test-Runtime([string]$Binary, [string]$Folder) {
    New-Item -ItemType Directory -Path $Folder -Force | Out-Null
    $report = Join-Path $Folder 'verification.json'
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report }
    $process = Start-Process -FilePath $Binary -ArgumentList @('selftest', '--output', ('"' + $report + '"')) -WindowStyle Hidden -PassThru
    if (-not $process.WaitForExit(180000)) {
        Stop-Process -Id $process.Id -Force
        throw 'Verification du runtime trop longue'
    }
    $process.Refresh()
    if ($process.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $report)) {
        throw ('Verification du runtime en echec : ' + $Binary + ', code ' + $process.ExitCode)
    }
    $result = Get-Content -LiteralPath $report -Raw -Encoding UTF8 | ConvertFrom-Json
    if (-not $result.ok -or $result.gui_steps -ne 6) { throw 'Rapport de verification incomplet' }
    Write-Output ('Runtime verifie : ' + $Binary)
}

$binary = Join-Path $projectRoot 'dist\FDXAtelier.dist\FDXAtelier.exe'
Test-Runtime $binary (Join-Path $qa 'portable')
if ($TestInstaller) {
    $installer = Join-Path $projectRoot 'dist\FDXAtelier-Installation-1.0.0.exe'
    $installation = Join-Path $qa 'installation-test'
    if (Test-Path -LiteralPath $installation) { throw 'Dossier de test deja present : ne pas reutiliser une installation inconnue.' }
    $process = Start-Process -FilePath $installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/NOICONS', ('/DIR="' + $installation + '"')) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Installation de recette en echec' }
    try {
        Test-Runtime (Join-Path $installation 'FDXAtelier.exe') (Join-Path $qa 'installed')
    } finally {
        $uninstaller = Join-Path $installation 'unins000.exe'
        if (Test-Path -LiteralPath $uninstaller) {
            $process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -WindowStyle Hidden -Wait -PassThru
            if ($process.ExitCode -ne 0) { throw 'Desinstallation de recette en echec' }
        }
    }
    Write-Output 'Installation, runtime installe et desinstallation verifies.'
}
