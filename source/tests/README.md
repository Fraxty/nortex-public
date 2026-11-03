# NORTEX-Testfälle

Die ausführbaren Rust-Unit- und Integrationstests des Compilers liegen unter `compiler/tests/` und in den jeweiligen Modultests. Der Frontend-Test `standalone_nx_regression_sources_are_wired_into_the_test_suite` lädt die zentralen `.nx`-Fixtures aus diesem Verzeichnis und prüft positive wie negative Erwartungen mit `check_source`.

- `lexer/` – Tokenisierung, Case-Sensitivity und Literale
- `parser/` – Syntax und AST
- `semantic/` – Typen, Deklarationen, Mutabilität und Operatorregeln
- `ownership/` – Move-/Borrow-/Lifetime-Regeln; vollständiger Checker noch offen (derzeit keine Fixtures)
- `codegen/` – native Codegen-Tests liegen in `compiler/tests/` (`codegen_e2e.rs`, `cli_build.rs`)
- `runtime/` – Runtime-Beispiele und Testprogramme; das `.nx`-Fixture wird derzeit nur durch das Frontend geprüft, nicht ausgeführt
- `integration/` – vollständige NORTEX-Programme; Frontend-Prüfung, noch keine native Ausführung

Die Testfixtures sind nicht selbst ein Ersatz für Rust-Regressionstests. Jede Sprachregel benötigt mindestens einen Positiv- und einen Negativtest. Erwartete Diagnosen sollen über stabile Diagnosecodes festgelegt werden. Das Release-Gate steht in `docs/security/README.md`.

Lokale Prüfung:

```powershell
./scripts/verify.ps1
```

Ein nicht ausführbarer Test gilt als nicht verifiziert und darf nicht als bestanden dokumentiert werden.
