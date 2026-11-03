#requires -Version 5.1
<#
.SYNOPSIS
Baut dist\NORTEX-Portable-x64.zip aus dist\staging (zuvor scripts\stage_release.ps1 ausführen)
und schreibt SHA-256-Prüfsummen für alle Release-Dateien nach dist\SHA256SUMS.txt.
Es wird nichts signiert; Signaturen müssen separat mit einem echten Zertifikat erzeugt werden.
#>
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$staging = Join-Path $root 'dist\staging'
$zip = Join-Path $root 'dist\NORTEX-Portable-x64.zip'
if (-not (Test-Path -LiteralPath (Join-Path $staging 'bin\nortex-code.exe'))) {
    throw 'dist\staging fehlt oder ist unvollständig (zuerst scripts\stage_release.ps1).'
}
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip -CompressionLevel Optimal

$sums = foreach ($name in 'NORTEX-Setup-x64.exe', 'NORTEX-Portable-x64.zip', 'nortex-0.1.0-src.zip') {
    $file = Join-Path $root "dist\$name"
    if (-not (Test-Path -LiteralPath $file)) { throw "Release-Artefakt fehlt: $file" }
    $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash *$name"
}
[IO.File]::WriteAllLines((Join-Path $root 'dist\SHA256SUMS.txt'), [string[]]$sums)
Write-Host ">>> $zip"
$sums
