# NORTEX Code – Public Preview 0.1.0 test record

Validation performed for the Windows x64 release package:

- `cargo test --workspace -- --test-threads=1` — passed, including native-class compiler tests, marketplace archive tests, and the interactive plugin-extension end-to-end test.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- Release build, Inno Setup 7 installer build, portable ZIP contents, and SHA-256 sums are checked for this package.

The interactive extension test exercises `.nxpack` creation and PDF export through the hosted-plugin IPC path. It does not exercise the AppContainer sandbox on a clean Windows installation. Release binaries are unsigned; Windows 10 and clean-machine installation are not tested.
