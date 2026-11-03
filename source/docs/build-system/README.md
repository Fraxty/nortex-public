# Build-System-Dokumentation

- `.nx` – NORTEX-Quellcodedatei
- `.nortex` – TOML-basiertes Projektmanifest
- `.ntx` – deterministischer binärer NORTEX-Paketcontainer
- `.obj` – COFF-Objektdatei unter Windows
- `.exe` – ausführbares Windows-Programm
- `.dll` – dynamische Windows-Bibliothek
- `.pdb` – Debugsymboldatei, soweit vom Toolchain-Profil erzeugt

Die Manifest-, CLI-, Artefakt- und Paketformatregeln stehen in [Phase 5 (Zielentwurf)](../design/phase-5-compiler-build.md); das tatsächlich implementierte Manifest `nortex.toml` steht in [manifest.md](manifest.md). `nortexc check` prüft Quellcode, `nortexc build` erzeugt für den unterstützten Sprachsubset native Windows-x64-EXEs (Cranelift, COFF, MSVC `link.exe`); DLLs werden nicht erzeugt.

- [`.nortex`-Manifest-Schema](manifest.md)
