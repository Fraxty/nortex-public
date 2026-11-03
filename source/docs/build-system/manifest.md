# NORTEX-Projektmanifest (`nortex.toml`)

Ein Projekt wird durch eine `nortex.toml` (UTF-8-TOML) im Projektordner beschrieben. Implementiert in `compiler/src/manifest.rs` (Tests: `compiler/tests/manifest_test.rs`). `nortexc build|check <Ordner|nortex.toml>` und die IDE (Build & Run) verwenden sie.

```toml
[package]
name = "Demo"            # Pflicht
version = "0.1.0"
authors = ["Name"]
description = "Beispiel"

[build]
main = "src/main.nx"     # Einstiegsdatei, relativ zum Manifest, darf das Projektverzeichnis nicht verlassen
output_name = "demo"     # Standard: Paketname
target = "x86_64-pc-windows-msvc"   # einziges unterstütztes Ziel
overflow_checks = true
opt_level = 0            # 0–3
linker_flags = []

[dependencies]
# je Eintrag genau eines von: path | lib
```

Ausgabe: `<Manifest-Ordner>\build\<output_name>.exe`. `[dependencies]` mit `path` (Quellen) oder `lib` (Linker-Bibliothek) werden übernommen; eine Paketauflösung gibt es nicht.

Verbindliche Details (Defaults, Validierung, Fehlermeldungen) sind im Quelltext und in [SPECIFICATION_IMPLEMENTED.md](../language/SPECIFICATION_IMPLEMENTED.md) §8 festgehalten.

## Zielentwurf (nicht implementiert)

Der ursprüngliche Entwurf sah eine `.nortex`-Datei mit `[project]`, `[target]`, `[build]` (`optimization`, `debug_symbols`) und `[metadata]` vor sowie Ausgabearten `dynamic-library`/`static-library`. Das ist **nicht** umgesetzt; siehe [Phase 5](../design/phase-5-compiler-build.md).
