# NORTEX Code – Public Preview 0.1.0 test record

## Source distribution

`python scripts/package.py` passed all configured gates:

- `cargo fmt --all -- --check`
- `cargo check --workspace --locked --all-targets`
- `cargo clippy --workspace --locked --all-targets -- -D warnings`
- `cargo test --workspace --locked --all-targets`
- Clean extraction followed by `cargo build --workspace --locked`

The test suite includes native class/inheritance/generic dispatch, plugin IPC, extension
end-to-end, archive handling, and PDF generation tests.

## Windows 11 release smoke tests

- Release binaries built with local paths remapped.
- Inno Setup 7.1.1 compiled the installer.
- Installer installed into an isolated temporary directory, the IDE launched, and the app
  uninstalled successfully. The uninstaller left an empty target directory, which was removed
  after verification.
- Portable ZIP extracted and the IDE launched.
- SHA-256 checksums were generated for the installer, portable package, and source archive.
- `cargo test -p nortex-ide windows_smoke -- --ignored --test-threads=1` — all five
  AppContainer smoke/policy tests passed on Windows 11, including timeout, cancellation, and
  permission-boundary checks.
- Formatted Unicode PDF opened in the integrated PDF viewer. Two-page output, headings, bold,
  italic, Greek and German text, page break, and header were visible.

## Not validated

Windows 10 22H2 and a clean Windows installation were not tested because no separate test
machine or VM is available. The AppContainer and package tests ran on the existing Windows 11
host. Release binaries are unsigned; SmartScreen may show a warning.
