#requires -Version 5.1
<#
.SYNOPSIS
Kopiert alle Release-Artefakte nach dist\staging\ (Eingabe für installer\nortex_setup.iss).
Voraussetzung: cargo build --workspace --release
#>
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$staging = Join-Path $root 'dist\staging'
$release = Join-Path $root 'target\release'

$binaries = @('nortex-code.exe', 'nortexc.exe', 'nortex_runtime.lib')
foreach ($name in $binaries) {
    if (-not (Test-Path (Join-Path $release $name))) {
        throw "Fehlt: target\release\$name (zuerst 'cargo build --workspace --release')."
    }
}

Write-Host ">>> Staging: $staging" -ForegroundColor Cyan
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Force -Path (Join-Path $staging 'bin') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $staging 'assets\icons\app') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $staging 'extensions') | Out-Null

# IDE, Compiler und Runtime liegen zusammen in bin\; die IDE sucht sie im eigenen Ordner.
foreach ($name in $binaries) {
    Copy-Item (Join-Path $release $name) (Join-Path $staging "bin\$name") -Force
}
Copy-Item (Join-Path $root 'assets\icons\app\nortex-code.ico') (Join-Path $staging 'assets\icons\app\nortex-code.ico') -Force
# The IDE locates the offline catalog beside the installed `bin` directory.
foreach ($directory in 'bundled', 'available') {
    Copy-Item (Join-Path $root "extensions\$directory") (Join-Path $staging 'extensions') -Recurse -Force
}
# Nur Handbuch-/Referenz-PDFs und Datenschutzentwurf; keine internen Audit- oder Entwicklungsdokumente.
New-Item -ItemType Directory -Force -Path (Join-Path $staging 'docs') | Out-Null
foreach ($pdf in Get-ChildItem (Join-Path $root 'docs\user-manual\*.pdf'), (Join-Path $root 'docs\language\*.pdf')) {
    Copy-Item $pdf.FullName (Join-Path $staging 'docs') -Force
}
Copy-Item (Join-Path $root 'docs\legal\PRIVACY_POLICY.md') (Join-Path $staging 'docs') -Force
Copy-Item (Join-Path $root 'examples') (Join-Path $staging 'examples') -Recurse -Force
foreach ($file in 'README.md', 'LICENSE') {
    Copy-Item (Join-Path $root "public-repo\$file") $staging -Force
}
Copy-Item (Join-Path $root 'THIRD_PARTY_NOTICES.md') $staging -Force
# Probe-Ausgaben nicht ausliefern.
Get-ChildItem $staging -Recurse -Include *.exe, *.obj -Path (Join-Path $staging 'examples') -ErrorAction SilentlyContinue |
    Remove-Item -Force

Write-Host ">>> Staging abgeschlossen." -ForegroundColor Green
