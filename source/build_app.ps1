$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $root
try {
    cargo fmt --all
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed." }
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "Formatting check failed." }

    python scripts/check_versions.py
    if ($LASTEXITCODE -ne 0) { throw "Workspace version check failed." }

    cargo check --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw "Workspace check failed." }

    cargo test --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw "Workspace tests failed." }

    cargo clippy --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "Workspace clippy failed." }

    cargo build --workspace --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }

    $compiler = Join-Path $root "target\release\nortex.exe"
    $ide = Join-Path $root "target\release\nortex-code.exe"
    if (-not (Test-Path $compiler -PathType Leaf)) {
        throw "Expected compiler executable was not produced: $compiler"
    }
    if (-not (Test-Path $ide -PathType Leaf)) {
        throw "Expected IDE executable was not produced: $ide"
    }

    Copy-Item -LiteralPath $compiler -Destination (Join-Path $root "nortex.exe") -Force
    Copy-Item -LiteralPath $ide -Destination (Join-Path $root "NORTEX-Code.exe") -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $root "bin") | Out-Null
    Copy-Item -LiteralPath $ide -Destination (Join-Path $root "bin\nortex-code.exe") -Force
    Write-Host "Created $root\nortex.exe"
    Write-Host "Created $root\NORTEX-Code.exe"
}
finally {
    Pop-Location
}
