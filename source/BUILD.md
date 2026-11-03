# Build

Voraussetzungen: Windows 11 x64, Rust ≥ 1.88 (MSVC-Toolchain), Visual Studio Build Tools (Linker), Python 3.11+ (nur für Skripte), optional Inno Setup 6/7 (Installer).

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

Release-Artefakte (alle UNSIGNIERT):

```powershell
scripts\stage_release.ps1        # dist\staging
ISCC installer\nortex_setup.iss  # dist\NORTEX-Setup-x64.exe
scripts\build_portable.ps1       # dist\NORTEX-Portable-x64.zip + dist\SHA256SUMS.txt
```

Weitere Skripte: `scripts\bench_compile.ps1` (Benchmark), `scripts\generate_sbom.py` (SBOM + `THIRD_PARTY_NOTICES.md`), `scripts\build_manual_pdf.py` (Handbuch- und Sprachreferenz-PDFs, DE/EN).

Hinweis: Läuft `nortex-code.exe` noch, scheitert `cargo build --release` mit „Zugriff verweigert“.
