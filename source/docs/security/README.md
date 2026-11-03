# NORTEX – Qualitätssicherung und Sicherheit

**Spezifikationsstatus:** Mindestanforderungen (Zielentwurf)  
**Implementierungsstatus:** Unit-, Golden- und Integrationstests sowie CI (`.github/workflows/ci.yml`) vorhanden; Fuzzing, Property-Tests, `.ntx`-Reader, Ownership-/Borrow-Prüfung und Unsafe-Semantik sind **nicht** implementiert. Eine Plugin-Sandbox (AppContainer + Job Object, nur Windows, plus IPC-Host) ist in der IDE implementiert, siehe [Plugin-System](../ide/plugins.md); Linux/macOS verweigern den Plugin-Start, und der Compiler für `.nx`-Plugins läuft in einem Job Object mit Low-Integrity-Token, aber ohne Lese- und Netzwerkbeschränkung (kein AppContainer). Die übrigen Sicherheitsabschnitte beschreiben Soll-Anforderungen. Der aktuelle Stand steht in [BUILD_STATUS.md](../../BUILD_STATUS.md).

## 9.1 Testpyramide

| Ebene | Umfang | Ziel |
|---|---|---|
| Unit-Tests | Lexer, Parser, Typen, Runtime, Editor | kleine Regeln isoliert prüfen |
| Golden-/Snapshot-Tests | Diagnosen, AST/HIR/MIR-Ausgabe | stabile Compilerergebnisse vergleichen |
| Integrationstests | vollständige `.nx`-Programme | Phasen gemeinsam prüfen |
| End-to-End-Tests | `nortex new/check/build/run/test/package` | Nutzerworkflow prüfen |
| Property-Tests | Parser, Formatter, Collections, Zahlen | Invarianten über viele Eingaben prüfen |
| Fuzzing | Lexer, Parser, `.ntx`-Reader, Shader-/Asset-Reader | Abstürze und Speicherfehler finden |
| Performance-Tests | Compilezeit, Runtime, Rendergraph, Editor | Regressionen messen |

Jeder Sprachfeature-Änderung werden Positiv- und Negativtests beigelegt. Tests dokumentieren nicht nur, dass ein Programm kompiliert, sondern auch, welche Diagnose bei ungültigem Code erwartet wird.

## 9.2 Diagnoseformat

Jede Compilerdiagnose besitzt einen stabilen Code, Severity, Meldung, Hauptspan, optionale Zusatzspans und optional einen maschinenlesbaren Lösungshinweis.

```text
Fehler[NX1004]: Unbekannter Bezeichner 'playerHealth'
 --> src/player.nx:12:14
  |
12 |     health = playerHealth;
  |              ^^^^^^^^^^^^
  |
  Hilfe: Prüfe die Schreibweise oder deklariere den Wert vor seiner Verwendung.
```

Die Compiler-CLI verwendet standardmäßig deutsche Meldungen. Diagnosecodes, Schweregrad und Quelltextpositionen sind sprachunabhängig; eine spätere IDE-Lokalisierung darf diese Maschinenfelder nicht verändern.

Diagnosecodes werden in `docs/compiler/diagnostic-codes.md` registriert und nach Veröffentlichung nicht für andere Bedeutungen wiederverwendet. Meldungen bleiben für Menschen verständlich und nicht ausschließlich maschinenlesbar.

## 9.3 Sicherheitsmodell

### Sichere Sprache

Sicherer NORTEX-Code muss Use-after-free, Double-free, ungültige Referenzen und Data Races durch Typ-, Ownership- und Borrow-Regeln verhindern. Die Überlaufsemantik muss für Debug- und Release-Builds ausdrücklich festgelegt sein und darf niemals in undefiniertes Verhalten übergehen. Ob Checked-Arithmetik auch im Release-Build Standard ist, ist noch nicht freigegeben. `wrapping_*` und `saturating_*` sind als explizite Alternativen vorgesehen.

### Unsafe

Raw Pointer, Pointer-Arithmetik, manuelle Allokatoren und bestimmte FFI-Aufrufe benötigen einen `unsafe`-Block. Der Block ist eine dokumentierte Sicherheitsgrenze und hebt nur die konkret betroffenen statischen Prüfungen auf. Er deaktiviert weder Parserdiagnosen noch Typprüfung für den übrigen Code.

### Runtime und FFI

- Keine NORTEX-Exception darf unkontrolliert über C-ABI-Grenzen laufen.
- Fremde Pointer benötigen dokumentierte Gültigkeits-, Alignment-, Alias- und Lebensdauerbedingungen.
- FFI-Callbacks dürfen keine ungefangenen NORTEX-Exceptions zurückgeben.
- Destruktoren sind nicht werfend.
- Stackoverflow, OOM und Prozessabbruch werden als nicht wiederherstellbare Runtime-Zustände behandelt, sofern die Plattform keinen sicheren Recovery-Vertrag bietet.

### Pakete und Builds

`.ntx`-Pakete werden vor dem Lesen auf Header-/Indexbereiche, Größenlimits, Integerüberläufe, Pfad-Traversal, doppelte Einträge, SHA-256-Prüfsummen und Versionskompatibilität geprüft. Das Format ist ein eigener unkomprimierter Binärcontainer; ZIP-/Dekompressionslogik ist nicht vorgesehen. Der Package Manager schreibt nicht außerhalb des konfigurierten Paketverzeichnisses. Abhängigkeiten werden mit exakten aufgelösten Versionen und Prüfsummen in einem Lockfile festgehalten. Build-Skripte und Plugins werden nicht automatisch ausgeführt.

### IDE und Plugins

Plugins laufen isoliert mit begrenzten Berechtigungen. Netzwerk- und Dateizugriff sind standardmäßig verweigert, bis sie freigegeben werden. Eine Plugin-Signatur ist eine Herkunftsaussage und keine Garantie für Harmlosigkeit.

## 9.4 Release-Gates

Ein Release wird nur freigegeben, wenn:

1. Workspace formatiert ist und `cargo fmt --all -- --check` besteht.
2. `cargo test --workspace` vollständig besteht.
3. `cargo clippy --workspace --all-targets -- -D warnings` besteht oder jede Ausnahme begründet dokumentiert ist.
4. Compiler-Fixtures und Diagnose-Snapshots aktuell sind.
5. Das Target auf einem sauberen Windows-x64-System baut und startet.
6. ABI-/FFI-Tests und Paket-Reader-Tests bestehen.
7. Sicherheitsrelevante bekannte Fehler dokumentiert und bewertet sind.
8. Release-Artefakte mit Versionen und Prüfsummen veröffentlicht werden.

Ein nicht ausgeführter Test gilt als **nicht verifiziert**, nicht als bestanden.
