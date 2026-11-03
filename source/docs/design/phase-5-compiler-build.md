# NORTEX Phase 5 – Compilerarchitektur und Build-Toolchain

**Spezifikationsstatus:** beschlossen für den Zielentwurf 0.3.5-dev  
**Implementierungsstatus:** Lexer, Parser, semantische Prüfung, MIR, Optimierer, Cranelift-Codegenerator und MSVC-Linker sind für ein skalares Subset vorhanden (`nortexc build` erzeugt EXE/OBJ, siehe [Compiler-Doku](../compiler/architecture.md)). Offen: vollständiges Backend, ARM64, Debug-Symbole, Unwind-Daten, CLI jenseits von `nortexc`.

## 5.1 Compiler-Pipeline

1. Projektmanifest und Quelldateien laden.
2. Lexing mit Quelltextspannen.
3. Parsing und AST-Erzeugung.
4. Namens-, Namespace- und Modulauflösung.
5. Typprüfung, Typinferenz und Overload Resolution.
6. Ownership-, Borrow- und Definite-Assignment-Analyse.
7. HIR-Erzeugung mit aufgelösten Symbolen und Typen.
8. MIR-Erzeugung mit explizitem Kontrollfluss und Speicheroperationen.
9. Datenflussanalyse und Optimierung.
10. Target-spezifische Codegenerierung.
11. COFF-Objekterzeugung.
12. Linken zur PE-EXE oder DLL.
13. Debugsymbole, Unwind-Daten und Build-Metadaten schreiben.

Jede Stufe erhält eine klar typisierte Eingabe und Ausgabe. Fehlerhafte Eingaben liefern Diagnosen und dürfen keine stillschweigend ungültigen Zwischenrepräsentationen erzeugen.

## 5.2 Frontend

### Lexer

Der Lexer bleibt case-sensitive. Bezeichner bestehen in Version 1.0 aus ASCII-Buchstaben, Ziffern und Unterstrich; das erste Zeichen ist Buchstabe oder Unterstrich. Unicode bleibt in Kommentaren und Strings zulässig. Der Lexer erkennt Schlüsselwörter, Identifier, Integer-/Float-/Decimal-Literale, String-/Character-Literale, Kommentare, Operatoren und Delimiter. Mehrzeichenoperatoren werden nach dem Longest-Match-Prinzip erkannt.

Jedes Token trägt Dateiquelle, Byteoffset, Zeile und Spalte. Fehlerhafte Escape-Sequenzen, nicht abgeschlossene Literale und ungültige Zeichen werden mit eigenständigen Diagnosecodes gemeldet.

### Parser

Der Parser wird als Recursive-Descent-Parser umgesetzt; Ausdrücke verwenden Pratt-Parsing mit dokumentierter Präzedenz und Assoziativität. Fehler-Recovery synchronisiert an Semikolon, schließenden Klammern und Deklarationsanfängen. Der Parser darf bei einem einzelnen Syntaxfehler nicht ohne Diagnose abbrechen.

Die C#-ähnliche Deklarationsform ist die einzige normative Deklarationsform. Eine zusätzliche `fn`-/`let`-Syntax gehört nicht zur Sprache und darf nicht parallel unterstützt oder dokumentiert werden.

### Semantic Analyzer

Der Analyzer prüft mindestens:

- Namen, Scopes, Sichtbarkeit und doppelte Deklarationen.
- Typinferenz, Typkompatibilität und erlaubte Konversionen.
- Rückgabe- und Kontrollflussvollständigkeit.
- Generics, Constraints und Monomorphisierung.
- Overload Resolution und Operatorregeln.
- Mutabilität, definite assignment, Move und Borrowing.
- Interface-/Trait-Implementierungen und Vererbung.
- `must_use` für `Result` und `Option`.
- `unsafe`-Grenzen und FFI-Anforderungen.

## 5.3 HIR und MIR

HIR ist semantisch aufgelöst, erhält sprechende Sprachkonstrukte und Quelltextspannen. MIR ist targetneutral und enthält Kontrollflussblöcke, Terminatoren, temporäre Werte, typisierte Operationen, explizite Speicherzugriffe, Move-/Borrow-Informationen sowie Drop- und Unwind-Kanten.

MIR-Grundregeln:

- Jeder Basic Block besitzt genau einen Terminator.
- Jeder verwendete temporäre Wert ist auf allen erreichbaren Pfaden initialisiert.
- Jeder Drop-Punkt ist explizit modelliert.
- Kontrollflussgraphen sind vollständig und für Datenflussanalysen verwendbar.
- Vor der Codegenerierung müssen Ownership- und Typinvarianten erneut geprüft sein.

## 5.4 Backend-Entscheidung

**Backend-Strategie:** NORTEX erhält ein eigenes Codegenerierungs-Backend. Das Frontend, die semantische Analyse, HIR und MIR sind vollständig NORTEX-eigen. Die erste Implementierung erzeugt Windows-x64-COFF-Objektdateien für `x86_64-pc-windows-msvc`; ARM64 folgt als zweites Ziel.

**Linker:** Der Entwurf nannte `lld-link` als Standard. **Ist-Stand:** `nortexc build` ruft den MSVC-`link.exe` als externen Prozess auf; das Backend ist Cranelift. Eine LLVM-Anbindung ist keine verbindliche Abhängigkeit.

Vor dem Backend-Aufruf müssen Typ-, Kontrollfluss-, Ownership-, Drop- und Unwind-Invarianten validiert sein.

**Ist-Stand:** MIR, Optimierer, Codegenerator und Linker erzeugen für das skalare Subset native EXEs bzw. COFF-Objekte (`--emit-obj`). Alles darüber hinaus ist nicht implementiert.

## 5.5 Projektmanifest

Das Projektmanifest heißt `nortex.toml` (der Entwurf nannte `.nortex`) und ist UTF-8-kodiertes TOML. Das verbindliche Basisschema ist in [`../build-system/manifest.md`](../build-system/manifest.md) definiert. Der Einstiegspunkt ist relativ zum Manifest und standardmäßig `src/main.nx`. Targets werden über `target.platform` benannt, zum Beispiel `windows-x86_64` oder `windows-aarch64`.

Unbekannte Schlüssel im reservierten Kernschema sind Fehler; projektspezifische Erweiterungen gehören unter `[metadata]`. Loader und Validierung existieren im Compiler (siehe Manifest-Referenz); die Zielplattformliste ist nicht ausgewertet.

## 5.6 CLI-Vertrag (Entwurf)

**Ist-Stand:** Es gibt nur `nortexc` mit `build <datei.nx> [-o <datei.exe|datei.obj>] [--emit-obj] [--overflow-checks[=true|false]]`, `check <datei.nx>`, `mir <datei.nx>`, `--version` und `--help`. Die folgende `nortex`-Tabelle ist reiner Entwurf; keiner dieser Befehle (`new`, `run`, `clean`, `test`, `package`, `format`, `doc`) existiert.

| Befehl | Verhalten | Voraussetzung |
|---|---|---|
| `nortex new NAME` | Projektgerüst erstellen | implementierbares Projekt-Template |
| `nortex check` | Frontend und Semantik prüfen | Compiler-Frontend |
| `nortex build` | natives Artefakt erzeugen | Backend und Linker |
| `nortex run` | Build ausführen | erfolgreicher Build |
| `nortex clean` | Build-Ausgabe löschen | Projektverzeichnis |
| `nortex test` | Tests bauen und ausführen | Test-Runner |
| `nortex package` | `.ntx`-Paket erstellen | Paketformat |
| `nortex format` | Quelltext formatieren | Formatter |
| `nortex doc` | API-Dokumentation generieren | Dokumentationsgenerator |
| `nortex version` | Toolchain-Versionen ausgeben | CLI |

Ein Befehl darf nicht als erfolgreich gemeldet werden, wenn sein erforderlicher Schritt übersprungen wurde. Nicht implementierte Befehle geben einen eigenen Fehlercode und einen klaren Statushinweis zurück.

## 5.7 Artefakte und `.ntx`

| Endung | Bedeutung |
|---|---|
| `.nx` | NORTEX-Quelldatei |
| `nortex.toml` | TOML-Projektmanifest (Entwurf: `.nortex`) |
| `.ntx` | eigener binärer NORTEX-Paketcontainer |
| `.obj` | Windows-COFF-Objektdatei |
| `.exe` | Windows-Programm |
| `.dll` | Windows-Dynamic-Link-Library |
| `.pdb` | Debug-Symboldatei, sofern erzeugt |

Das normative Containerformat einschließlich Header, Index, Pfadregeln, SHA-256-Prüfung und deterministischer Serialisierung steht in [Sprachreferenz Kapitel 12](../language/12-compiler-architecture.md#paketformat-ntx-geplant). `.ntx` ist ausdrücklich weder Klartext-Quelldatei noch ZIP-Archiv. Der Reader und Writer sind noch nicht implementiert.

## 5.8 Build-Reproduzierbarkeit

Ein Build protokolliert Compiler-Version, Target, Optimierungsprofil, Abhängigkeitsversionen und relevante Umgebungsvariablen. Release-Builds sollen bei identischer Eingabe reproduzierbare Artefakte erzeugen. Absolute lokale Pfade und Zeitstempel werden nicht in deterministische Artefaktinhalte geschrieben, sofern dies nicht ausdrücklich für Debuginformationen konfiguriert ist.
