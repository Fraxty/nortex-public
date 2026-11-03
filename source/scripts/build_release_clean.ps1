#requires -Version 5.1
<#
.SYNOPSIS
Release-Build ohne lokale Pfade in den Binaries (--remap-path-prefix) und mit Verifikation.
#>
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
$env:RUSTFLAGS = "--remap-path-prefix=$root=nortex --remap-path-prefix=$cargoHome=cargo --remap-path-prefix=$rustupHome=rustup --remap-path-prefix=$env:USERPROFILE=~"
cargo build --workspace --release --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo build fehlgeschlagen.' }

$needles = @($root, $root.Replace('\', '/'), $env:USERPROFILE, $env:USERPROFILE.Replace('\', '/')) |
    Where-Object { $_ } |
    Select-Object -Unique
foreach ($name in 'nortex-code.exe', 'nortexc.exe') {
    $text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes((Join-Path $root "target\release\$name")))
    foreach ($needle in $needles) {
        $count = ([regex]::Matches($text, [regex]::Escape($needle))).Count
        if ($count -gt 0) { throw "$name enthält den lokalen absoluten Pfad '$needle' $count x." }
    }
}
Write-Host '>>> Release-Build fertig.'
