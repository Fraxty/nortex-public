# NORTEX – Versionsregeln

**Gültiger Repository-Stand:** `0.3.7-dev`
**Sprachentwurf:** `0.3.5-dev` (keine normative Sprachänderung in 0.3.7-dev)

## Getrennte Versionsräume

- **Repository und Rust-Crates:** Alle Workspace-Crates übernehmen die gemeinsame Cargo-Version aus `[workspace.package]`. `Cargo.lock` muss dieselbe Version für alle internen Workspace-Pakete enthalten.
- **NORTEX-Sprachentwurf:** Die Sprachversion wird nur bei normativen Änderungen an Syntax oder Semantik erhöht. Eine reine IDE-/Tooling-Korrekturrunde darf die Sprachversion unverändert lassen. Der aktuelle Sprachentwurf ist `0.3.5-dev`; der Repository-Code ist `0.3.7-dev`. Ein Sprachrelease ist erst dann stabil, wenn die Release-Gates der Phase 9 erfüllt sind.
- **Projektversion in `.nortex`:** `project.version` bezeichnet das jeweilige Anwenderprojekt und ist unabhängig von der NORTEX-Toolchain-Version.
- **`.ntx`-Containerformat:** Die Containerformat-Version wird im Binärheader separat geführt. Sie darf nicht aus der Repository-Version abgeleitet werden.
- **Runtime-ABI:** Die ABI-Version wird unabhängig über `nortex_runtime_abi_version()` ausgewiesen. Inkompatible ABI-Änderungen müssen diese ABI-Hauptversion erhöhen.

## Entwicklungs- und Release-Regeln

- `0.3.7-dev` ist der aktuelle Repository-Entwicklungsstand und kein stabiler Release. Jede Audit-Korrekturrunde erhält eine eigene Patch-/Dev-Kennung. Die Sprachversion wird nur bei normativen Sprachänderungen aktualisiert.
- Vor einem stabilen Release müssen Parser, Typchecker, Ownership-/Borrow-Checker, HIR/MIR, Backend, Linkerpfad, Runtime und Release-Tests die in Phase 9 definierten Gates erfüllen.
- Sprachänderungen, die bestehende Programme inkompatibel machen, erfordern eine dokumentierte Sprachversionserhöhung und Migrationserläuterung.
- Jede Repository-Version muss in Workspace-Manifesten, Lockfile, Versionskonstante (über Cargo), README, Changelog und Repository-Status übereinstimmen. CI prüft die Workspace-Paketversionen gegen das Workspace-Manifest und Lockfile. Sprachspezifikationen tragen ihre separate Sprachversion; historische und unveränderte normative Dokumente behalten diese Angabe.
- Historische Prüfberichte und unveränderte Ausgangsunterlagen behalten ihre ursprüngliche Versionsangabe. Sie sind als historische Quelle gekennzeichnet und nicht Teil der aktuellen Produktversion.

## Automatische Konsistenzprüfung

Die CI prüft, dass alle internen Cargo-Pakete dieselbe Workspace-Version verwenden und dass die im Lockfile gespeicherten Versionen dazu passen. Zusätzlich führt sie Build, Tests, Formatierung und Clippy aus.
