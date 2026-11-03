# NORTEX Code – Public Preview 0.1.0

**Vorabversion / Pre-release** – zum Testen und Evaluieren, nicht für den Produktiveinsatz.
Der Quellcode steht unter MIT-Lizenz, siehe `LICENSE`.

## Downloads

- `NORTEX-Setup-x64.exe` – Windows-Installer (pro Benutzer)
- `NORTEX-Portable-x64.zip` – portable Paketversion
- `nortex-0.1.0-src.zip` – reproduzierbare, geprüfte Quellcode-Distribution
- `SHA256SUMS.txt` – Prüfsummen für die Download-Dateien

Der inspectierbare Quellcode liegt zusätzlich unter `source/`. Build- und Testanleitung:
`source/README.md`.

Beide Anwendungs-Pakete enthalten den Offline-Extension-Katalog mit 8 gebündelten und 17
optionalen Paketen. Das ist keine Live-Verbindung zu einem Online-Marktplatz; Funktionsumfang und
Integrationsgrad der einzelnen Pakete unterscheiden sich.

## Änderungen

- Nativer Windows-x64-Compiler: Heap-Klassen mit Feldern, Methoden, Konstruktoren, Objektaliasierung, Einzelvererbung und `virtual`/`override`.
- Geschlossene generische Klassen und generische Instanzmethoden mit konkreten Typargumenten.
- Gehostete Extensions können dauerhaft laufen und bestätigte Menüaktionen bereitstellen.
- Plugin-API 1.2: echtes Deflate-ZIP (`.nxpack`) und Unicode-PDF-Export mit NXPDF-Formatierung.
- Die Windows-IDE deklariert Windows 10/11-Kompatibilität im Manifest.

## Plattform und Tests

Windows 11 x64 wurde getestet. Windows 10 22H2 ist als Kompatibilitätsziel eingetragen, wurde
jedoch mangels Testsystem nicht ausgeführt und ist daher noch nicht als verifiziert unterstützt
zu betrachten. AppContainer-Smoke- und Berechtigungstests bestanden auf Windows 11; ein Test auf
einer sauberen Windows-Installation wurde nicht durchgeführt.

## Bekannte Einschränkungen

Die Runtime-Arena gibt Klassenobjekte bei normaler Rückkehr aus `main` frei, nicht während der
Ausführung. Es gibt keinen Tracing-GC; Speicher kann bei langen Programmen wachsen. Generische
freie Funktionen, implizite Methoden-Typargument-Inferenz, Bounds, Interfaces/Traits,
Mehrfachvererbung und parametrisierte Basiskonstruktoren fehlen. NXPDF unterstützt keine Bilder
oder freie Positionierung; nicht in den eingebetteten Schriften enthaltene Zeichen erscheinen
als fehlende Glyphen.

Die Programme sind nicht digital signiert (SmartScreen-Warnung möglich). Ein Debugger, eine
Live-Marketplace-Authentifizierung, Bewertungen und automatische Updates fehlen. Terminal,
Builds und gestartete Nutzerprogramme laufen ohne Sandbox.

## Feedback

GitHub Issues. Sicherheitslücken: siehe `SECURITY.md`.
