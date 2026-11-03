# NORTEX – Dokumentation

**Quellenhierarchie:** Maßgeblich für den Ist-Stand sind (1) der Code, (2) [BUILD_STATUS.md](../BUILD_STATUS.md), (3) die verifizierte [Sprachreferenz](language/README.md). Dokumente unter [design/](design/README.md) sind Zielentwürfe und keine Zusage.

| Bereich | Dokument |
|---|---|
| Einstieg | [Projekt-README](../README.md), [Installation](getting-started/installation.md), [Beispiele](../examples) |
| Sprache | [Sprachreferenz](language/README.md) (Kapitel 01–15), [Ist-Spezifikation](language/SPECIFICATION_IMPLEMENTED.md), [Ownership](language/ownership.md), [Grammatik](language/grammar.ebnf) |
| Compiler | [Architektur](compiler/architecture.md), [Diagnosecodes](compiler/diagnostic-codes.md), [Syntax-Matrix](compiler/syntax-support-matrix.md), [Typsystem](compiler/type-system.md), Stufen: [Lexer](compiler/lexer.md), [Parser](compiler/parser.md), [Semantik](compiler/semantic-analysis.md), [HIR](compiler/hir.md), [MIR](compiler/mir.md), [Codegen](compiler/codegen.md), [ABI](compiler/abi.md) |
| IDE | [IDE-Überblick](ide/README.md), [Editor-Core](ide/editor-core.md), [Plugins](ide/plugins.md), [UI-Framework](ide/ui-framework-evaluation.md), [Benutzerhandbuch](user-manual/README.md) (in der IDE über Hilfe → NORTEX Code-Handbuch) |
| Build-System | [Build-System](build-system/README.md), [Manifest `nortex.toml`](build-system/manifest.md) |
| Runtime / Bibliothek / Grafik | [Runtime](runtime/README.md), [Standardbibliothek](standard-library/README.md), [Grafik](graphics/README.md) |
| Architektur | [Architektur-Überblick](architecture.md) |
| Entwicklung | [Developer-Handbuch](development/developer-handbook.md), [Roadmap](development/roadmap.md), [Lückenanalyse](development/gap-analysis.md), [Versionsregeln](development/versioning.md), [CONTRIBUTING](../CONTRIBUTING.md) |
| Sicherheit & Qualität | [security/README.md](security/README.md) |
| Zielentwürfe | [Phasen 3–8](design/README.md) |
| Änderungen | [CHANGELOG](../CHANGELOG.md) |

`docs/archive/source-snapshots/` enthält ein historisches Quell-ZIP und wird nicht gepflegt.
