#requires -Version 5.1
<#
.SYNOPSIS
Startet NORTEX Code: baut bei Bedarf release und kopiert die exe nach bin\nortex-code.exe.
#>
[CmdletBinding()]
param([switch]$NoBuild)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
Set-Location $root

if (-not $NoBuild) {
    cargo build -p nortex-ide --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build fehlgeschlagen (Exitcode $LASTEXITCODE)." }
}

$built = Join-Path $root 'target\release\nortex-code.exe'
if (-not (Test-Path -LiteralPath $built -PathType Leaf)) {
    throw "Release-Binary fehlt: $built (ohne -NoBuild erneut ausfuehren)."
}

$bin = Join-Path $root 'bin'
New-Item -ItemType Directory -Force -Path $bin | Out-Null
$exe = Join-Path $bin 'nortex-code.exe'
Copy-Item -LiteralPath $built -Destination $exe -Force

Start-Process -FilePath $exe -WorkingDirectory $root
Write-Host "Gestartet: $exe"
