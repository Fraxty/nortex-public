$ErrorActionPreference = "Stop"
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    throw "NORTEX tests failed."
}
