#requires -Version 5.1
<#
.SYNOPSIS
Release-Build, Staging und Inno-Setup-Kompilierung -> dist\NORTEX-Setup-x64.exe
#>
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

function Invoke-Checked([string]$Step, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "$Step fehlgeschlagen (Exitcode $LASTEXITCODE)." }
}

Write-Host '1. cargo build --workspace --release' -ForegroundColor Cyan
Invoke-Checked 'cargo build' { cargo build --workspace --release }

Write-Host '2. Staging' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'stage_release.ps1')

Write-Host '3. Inno Setup' -ForegroundColor Cyan
$candidates = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue | ForEach-Object { $_.Source }),
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 7\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) }
if (-not $candidates) {
    Write-Warning 'ISCC.exe (Inno Setup 6 oder 7) nicht gefunden. Staging liegt in dist\staging; Installer nicht gebaut.'
    exit 1
}
Invoke-Checked 'ISCC' { & ($candidates | Select-Object -First 1) (Join-Path $root 'installer\nortex_setup.iss') }

$setup = Join-Path $root 'dist\NORTEX-Setup-x64.exe'
if (-not (Test-Path $setup)) { throw "Erwartetes Setup fehlt: $setup" }
Write-Host ">>> Fertig: $setup" -ForegroundColor Green
