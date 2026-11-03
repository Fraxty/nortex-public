# NORTEX Code – Public Preview 0.1.0 (Release Notes)

**Vorabversion / Pre-release** – zum Testen und Evaluieren, nicht für den Produktiveinsatz. MIT-Lizenz, siehe `LICENSE.txt`.

## Dateien
- `NORTEX-Setup-x64.exe` – Installer (pro Benutzer, ohne Administratorrechte)
- `NORTEX-Portable-x64.zip` – portable Version
- `SHA256SUMS.txt` – Prüfsummen

Beide Pakete enthalten den Offline-Extension-Katalog mit 8 gebündelten und 17 optionalen Paketen.
Der Katalog ist keine Live-Verbindung zu einem Online-Marktplatz; Funktionsumfang und
Integrationsgrad der einzelnen Pakete unterscheiden sich.

## Änderungen
- NORTEX-Compiler: native Klassen mit skalaren Feldern, Konstruktoren sowie Instanz- und statischen Methoden.
- Extension-Host: gehostete Plugins können laufen bleiben und bestätigte Aktionen im Menü „Erweiterungen“ bereitstellen.
- Plugin-API 1.2: `.nxpack` ist ein echtes Deflate-ZIP; `pdf-tools` exportiert aktive Textdokumente als einfache PDF-Dateien.

## Systemanforderungen
Windows 11 x64; Windows 10 nicht getestet/nicht unterstützt. Zum Bauen nativer Programme mit `nortexc` werden die Microsoft Visual Studio Build Tools (MSVC-Linker) benötigt. Grafikfähiger Desktop (OpenGL).

## Bekannte Einschränkungen
Nicht digital signiert (SmartScreen-Warnung möglich); kein Quellcode in diesem Repository; kein
Debugger; Klassen unterstützen keine Vererbung, Generics oder Heap-Objekte; erzeugte PDFs
enthalten einfachen Text (Latin-1), ohne Bilder oder Formatierung; Terminal, Build und gestartete
Programme laufen ohne Sandbox; keine Live-Marketplace-Konten, Bewertungen oder automatischen
Updates; nur lokal getestet, nicht auf einer sauberen Windows-11-Installation; Windows 10 ungetestet.

## Feedback
GitHub Issues. Sicherheitslücken: siehe `SECURITY.md`.
