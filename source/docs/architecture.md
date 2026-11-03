# NORTEX – Architektur

Dieses Dokument gibt den Überblick über Aufbau und Zielplattformen. Den tatsächlichen Implementierungsstand führt [BUILD_STATUS.md](../BUILD_STATUS.md); Details zur Pipeline stehen in [language/12-compiler-architecture.md](language/12-compiler-architecture.md).

## Workspace

| Verzeichnis | Inhalt |
|---|---|
| `compiler/` | Compiler `nortexc`: Lexer, Parser, Semantik, HIR, MIR, Optimizer, Cranelift-Codegen, COFF, MSVC-Linkeraufruf, Manifest |
| `runtime/` | Runtime-Crate (`nortex_runtime.lib`): Speicher, Strings, Konsole, Threadpool, Future-Executor |
| `ide/` | NORTEX Code, egui/eframe-Desktop-IDE (Crate `nortex-ide`) |
| `tools/` | Hilfsprogramme (Crate `nortex-tools`) |
| `graphics/` | Platzhalter-Struktur für das Grafiksystem (nicht implementiert) |
| `standard-library/` | NORTEX-Quellen der Standardbibliothek (Entwurf) |
| `examples/`, `tests/` | Beispiele und Fixtures |
| `scripts/` | Build-, Verify-, Package-Skripte |
| `docs/` | Dokumentation (siehe [README.md](README.md)) |

## Compiler-Pipeline

Quelltext `.nx` → AST → HIR → MIR → Optimizer → Cranelift → COFF-Objekt → `link.exe` (MSVC) → `.exe`.
Native Codegenerierung deckt eine Teilmenge der Sprache ab (siehe [language/README.md](language/README.md)).

## Zielplattformen

| Reihenfolge | Plattform | Status |
|---|---|---|
| 1 | Windows x86-64 (`x86_64-pc-windows-msvc`) | einziges implementiertes Ziel |
| 2 | Windows ARM64 | geplant |
| 3 | Linux, macOS | später, nicht geplant im Detail |

## Dateitypen

- `.nx` – Quelltext
- `nortex.toml` – Projektmanifest ([build-system/manifest.md](build-system/manifest.md))
- `.ntx` – geplanter binärer Paketcontainer (Entwurf, nicht implementiert)
- `.obj` / `.exe` – COFF-Objekt / ausführbares Windows-Programm

## Entwurfsdokumente

Die Phasen 3–9 (Fehlerbehandlung/Async, ABI/Runtime, Toolchain, Standardbibliothek, Grafik, IDE, Qualität) sind Zielentwürfe in [design/](design/README.md). Sie beschreiben Pläne, nicht den Ist-Stand.
