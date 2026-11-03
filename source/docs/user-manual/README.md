# NORTEX Code – Benutzerhandbuch

Stand: Paketversion **0.3.7-dev**. Alle Angaben und Screenshots stammen aus der tatsächlichen Anwendung (Windows, x64).

## Kapitel

1. [Willkommen bei Nortex Code](01-welcome.md)
2. [Installation und erster Start](02-installation.md)
3. [Benutzeroberfläche](03-interface-overview.md)
4. [Projekte und Dateien](04-projects-and-files.md)
5. [Arbeiten mit dem Editor](05-editor.md)
6. [Kompilieren und Ausführen](06-build-and-run.md)
7. [Terminal und Konsolen](07-terminal-and-consoles.md)
8. [Fehleranalyse und Debugging](08-debugging.md)
9. [Einstellungen und Anpassungen](09-settings-and-customization.md)
10. [Tastenkombinationen](10-keyboard-shortcuts.md)
11. [Praktischer Einstieg](11-first-project.md)
12. [Effizientes Arbeiten](12-efficient-workflows.md)
13. [FAQ und Fehlerbehebung](13-faq-and-troubleshooting.md)

Die Programmiersprache selbst beschreibt die [Sprachreferenz](../language/README.md). Abbildungen liegen in [images/](images).

## Funktionsstand der IDE

| Stufe | Funktionen |
|---|---|
| **Vollständig** | Menüleiste (Datei, Bearbeiten, Ansicht, Hilfe), Haltepunkt-Verwaltung (F9, ●-Markierung, Haltepunkt-Liste), Einstellungsansicht (Suche, Reset, Export/Import), Activity Bar, Explorer (Dateibaum mit einklappbaren Ordnern), Editor mit Syntaxhervorhebung und Zeilennummern, Tabs, Split-Editor (bis 2×2), Suchen/Ersetzen (Strg+F/H, RegEx), Autovervollständigung, Zeilenbefehle, Build & Run, Compiler-Log mit Diagnosen, Runtime-Ausgabe, Terminal, Befehlspalette, Themes/Dichte/Zoom, `nortex.toml`-Manifest, Statusleiste |
| **Teilweise** | Dashboard (Anzeige; Git-Status nur lesend), Git-Panel (nur Anzeige: Branch und geänderte Dateien), Projektsuche (einfache Textsuche), Sidecar (Panel ohne KI-Anbieter) |
| **Nur lokal / vorbereitet** | Konto (nur lokaler Modus, kein Online-Dienst; Sync-Optionen nur vorbereitet; Quellcode bleibt lokal) |
| **Geplant / Stub** | Debugger-Backend (nicht verfügbar: `nortexc` erzeugt keine Debug-Infos; Panel und F5 zeigen nur Hinweis), Programm abbrechen (Shift+F5), Erweiterungen (Strg+Shift+X), KI-Menü (alle Einträge deaktiviert) |
| **Nicht vorhanden** | Neues-Projekt-Assistent, Datei umbenennen/löschen, Ordner anlegen, Mehrfachcursor, automatische Klammervervollständigung, Konsoleneingabe für laufende Programme, Kontextmenüs |

Bekannte Nutzungsgrenzen des Compilers stehen in der [Sprachreferenz](../language/README.md).


## PDF-Handbuch erzeugen

Aus den Kapiteln `01` bis `13` entsteht ein PDF (Titelseite, Inhaltsverzeichnis, Seitenzahlen). Das Skript nutzt nur die Python-Standardbibliothek; pandoc oder LaTeX sind nicht nötig. Aufruf im Repository-Root:

```powershell
python scripts/build_manual_pdf.py
```

Ausgabe: `docs/user-manual/NORTEX-Code-Handbuch.pdf`. Die Version stammt aus `Cargo.toml` (Workspace). Nach Änderungen an den Kapiteln einfach erneut ausführen. Abbildungen werden als Textverweis `[Abbildung: …]` ausgegeben, Tabellen als einfacher Text.