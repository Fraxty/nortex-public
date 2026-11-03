# Installation

**Plattform:** Windows 11 x64 (Windows 10 nicht getestet, kein Supportstatus). Alle Dateien sind derzeit **nicht signiert**; Windows SmartScreen kann warnen. Prüfe die SHA-256-Summe aus `SHA256SUMS.txt`:

```powershell
Get-FileHash .\NORTEX-Setup-x64.exe -Algorithm SHA256
```

## Installer (`NORTEX-Setup-x64.exe`)
Standardmäßig pro Benutzer ohne Administratorrechte (Ziel: `%LOCALAPPDATA%\Programs\NORTEX Code`); ein Dialog erlaubt die Installation für alle Benutzer. Optional: Desktopverknüpfung, `.nx`-Dateizuordnung, PATH-Eintrag (nur für den Benutzer bzw. systemweit je nach Modus). Deinstallation über „Apps“ oder `unins000.exe`. Benutzerdaten (`%APPDATA%\NORTEX Code\settings.json`) und Projekte bleiben erhalten.

## Portable (`NORTEX-Portable-x64.zip`)
Entpacken und `bin\nortex-code.exe` starten. Keine Systemänderung. Einstellungen liegen auch hier in `%APPDATA%\NORTEX Code\settings.json`; ein rein portabler Datenordner ist nicht implementiert. Absturzprotokoll `ide_crash.log` neben der EXE oder im Temp-Ordner.

## Handbücher
In der IDE: Hilfe → Handbuch (PDF) öffnen. PDFs: `docs/user-manual/`, Sprachreferenz: `docs/language/` (Deutsch und Englisch).
