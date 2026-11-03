$ErrorActionPreference = "Stop"

$checks = @(
    @{ Name = "Format source"; Args = @("fmt", "--all") },
    @{ Name = "Formatting check"; Args = @("fmt", "--all", "--", "--check") },
    @{ Name = "Workspace versions"; Tool = "python"; Args = @("scripts/check_versions.py") },
    @{ Name = "Workspace check"; Args = @("check", "--workspace", "--all-targets", "--locked") },
    @{ Name = "Workspace build"; Args = @("build", "--workspace", "--all-targets", "--locked") },
    @{ Name = "Workspace tests"; Args = @("test", "--workspace", "--all-targets", "--locked") },
    @{ Name = "Clippy"; Args = @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings") }
)

foreach ($check in $checks) {
    Write-Host "== $($check.Name) ==" -ForegroundColor Cyan
    $arguments = $check.Args
    if ($check.Tool -eq "python") {
        & python @arguments
    } else {
        & cargo @arguments
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$($check.Name) failed with exit code $LASTEXITCODE."
    }
}

Write-Host "All configured NORTEX verification checks passed." -ForegroundColor Green
