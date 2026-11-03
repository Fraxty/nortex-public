# NORTEX Compiler – Diagnosecodes

Diagnosecodes sind dauerhaft stabil. Eine veröffentlichte Nummer darf nicht für eine andere Bedeutung wiederverwendet werden. Der Compiler soll menschenlesbare Texte und strukturierte Diagnosemetadaten bereitstellen.

| Codebereich | Verantwortlichkeit |
|---|---|
| NX0001–NX0099 | Lexer, Quelltextkodierung und allgemeine Warnungen |
| NX0100–NX0199 | Parser und Syntax |
| NX1000–NX1099 | Namen, Scopes und Sichtbarkeit |
| NX1100–NX1199 | Typen, Generics und Konvertierungen |
| NX1200–NX1299 | Ownership, Borrowing und Mutabilität |
| NX1300–NX1399 | Kontrollfluss und `must_use` |
| NX1400–NX1499 | Module, Pakete und Abhängigkeiten |
| NX1500–NX1599 | Unsafe und FFI |
| NX2000–NX2099 | Backend, Target und Codegenerierung |
| NX2100–NX2199 | Linker und Artefakte |
| NX3000–NX3099 | Runtime- und Standardbibliotheksdiagnosen |
| NX4000–NX4099 | Projektmanifest und Toolchain |

## Erste festgelegte Codes

| Code | Bedeutung | Severity |
|---|---|---|
| NX0001 | Ungültiges oder nicht unterstütztes Zeichen | Error |
| NX0002 | Nicht abgeschlossenes String-Literal | Error |
| NX0003 | Ungültige Escape-Sequenz | Error |
| NX0004 | Allgemeine Stil- oder Konventionswarnung | Warning |
| NX0100 | Syntaxelement erwartet | Error |
| NX1004 | Unbekannter Bezeichner | Error |
| NX1005 | Ungültige Memberdeklaration oder fehlende Implementierung | Error |
| NX1001 | Doppelte Deklaration | Error |
| NX1002 | Ungültige oder nicht unterstützte Vererbungsdeklaration | Error |
| NX1100 | Unbekannter Typ | Error |
| NX1101 | Falsche generische Arität | Error |
| NX1102 | Typunverträglichkeit bei Ausdrücken, Initialisierungen, Operatoren oder Rückgaben | Error |
| NX1103 | Fehlender Rückgabewert in einer Funktion mit Nicht-void-Rückgabetyp | Error |
| NX1104 | Ungültige Operatorüberladung oder Operatorvertrag | Error |
| NX1105 | Ungültiger Compile-Time-Konstantenausdruck | Error |
| NX1200 | Zuweisung an unveränderliche Bindung | Error |
| NX1201 | Ungültiger Borrow-/Move-Zugriff | Error |
| NX1300 | `must_use`-Wert ignoriert | Error |
| NX1301 | `break` außerhalb eines zulässigen Kontextes | Error |
| NX1302 | `continue` außerhalb einer Schleife | Error |
| NX1303 | Fehlerhafte `switch`-/`match`-Fallstruktur | Error |
| NX1500 | Unsafe-Operation außerhalb eines `unsafe`-Blocks | Error |
| NX2000 | Target oder Backend nicht verfügbar | Error |
| NX2100 | Linker konnte nicht ausgeführt werden | Error |

## Implementierter Teilstand

NX1005 wird für unzulässige Methodendeklarationen ohne Implementierungsblock und widersprüchliche Interface-Methodenrümpfe verwendet.

Die aktuelle `Diagnostic`-Struktur speichert Code, Meldung, Severity, SourceSpan, optionalen Dateinamen und Hilfetext. Im Frontend-Prototyp werden `NX0004`, `NX1001`, `NX1004`, `NX1100`, `NX1102`, `NX1103`, `NX1104`, `NX1200`, `NX1201`, `NX1301` und `NX1302` gezielt verwendet. Die übrigen Einträge sind reservierte Zielcodes und noch nicht flächendeckend implementiert.

Die menschenlesbaren Compilerdiagnosen sind in dieser Entwicklungsrevision deutsch. Die stabilen Diagnosecodes sind unabhängig von der Meldungssprache. Eine vollständige Lokalisierungsinfrastruktur und maschinenlesbare JSON-Diagnoseausgabe fehlen.
