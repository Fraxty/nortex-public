$ErrorActionPreference = "Stop"
cargo build --workspace
if ($LASTEXITCODE -ne 0) {
    throw "NORTEX build failed."
}
