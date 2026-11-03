#!/usr/bin/env pwsh
$ErrorActionPreference = "Stop"
cargo build --workspace --release
if ($LASTEXITCODE -ne 0) {
    throw "NORTEX release build failed."
}
