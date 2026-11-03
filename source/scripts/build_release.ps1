#requires -Version 5.1
<#
.SYNOPSIS
Baut den Release-Stand und legt Compiler, IDE, Runtime und ein Beispielprogramm in build\ ab.

.PARAMETER Example
Quelldatei, die zu build\app.exe kompiliert wird (Standard: examples\native-sum\main.nx).
#>
[CmdletBinding()]
param(
    [string]$Example = 'examples\native-sum\main.nx'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

function Invoke-Checked {
    param([string]$Description, [scriptblock]$Command)
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description fehlgeschlagen (Exitcode $LASTEXITCODE)."
    }
}

$source = Join-Path $root $Example
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
    throw "Beispieldatei nicht gefunden: $Example"
}

$build = Join-Path $root 'build'
New-Item -ItemType Directory -Force -Path $build | Out-Null

Invoke-Checked 'cargo build --release' { cargo build --release --workspace --locked }

$release = Join-Path $root 'target\release'
foreach ($file in 'nortexc.exe', 'nortex-code.exe', 'nortex_runtime.lib') {
    $path = Join-Path $release $file
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Release-Artefakt fehlt: $path"
    }
    Copy-Item -LiteralPath $path -Destination $build -Force
}

$bin = Join-Path $root 'bin'
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item -LiteralPath (Join-Path $release 'nortex-code.exe') -Destination $bin -Force
Write-Host "IDE: $bin\nortex-code.exe"

# Der Linker braucht die Runtime-Bibliothek für Konsolenausgabe.
$env:NORTEX_RUNTIME_LIB = Join-Path $build 'nortex_runtime.lib'
$app = Join-Path $build 'app.exe'
Invoke-Checked 'nortexc build' { & (Join-Path $build 'nortexc.exe') build $source -o $app }

if (-not (Test-Path -LiteralPath $app -PathType Leaf)) {
    throw "build\app.exe wurde nicht erzeugt."
}
Write-Host "Erfolg: $app"
