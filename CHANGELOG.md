# Changelog

## 0.1.0 – Public Preview

- Vollständiger Rust-Quellcode mit MIT-Lizenz in `source/` und als geprüfter Source-Download.
- Nativer Windows-x64-Compiler: Heap-Klassen mit skalaren, String- und Klassenreferenzfeldern, Objektaliasierung, Methoden, Konstruktoren und Objektübergabe/-rückgabe.
- Einzelvererbung, `virtual`/`override`-Dispatch, geschlossene generische Klassen sowie generische Instanzmethoden mit expliziten Typargumenten.
- Gehostete Extensions können dauerhaft laufen und bestätigte Aktionen im Menü „Erweiterungen“ bereitstellen.
- Plugin-API 1.2: echte Deflate-ZIP-Archive (`.nxpack`) und Unicode-PDF-Export mit NXPDF-Formatierung.
- Inno-Setup-Installer, Portable-Paket und SHA-256-Prüfsummen wurden neu erstellt.
- AppContainer-Smoke-, Timeout-, Abbruch- und Berechtigungstests bestanden auf Windows 11.

## Bekannte Einschränkungen

- Klassen verwenden eine Runtime-Arena: alle Objekte werden bei normaler Rückkehr aus `main` freigegeben; es gibt keine Einzelobjektfreigabe oder Tracing-GC, daher kann der Speicher während langer Programme wachsen.
- Basiskonstruktoren müssen parameterlos sein; Mehrfachvererbung, Interfaces/Traits, generische freie Funktionen, implizite Methoden-Typargument-Inferenz und Bounds fehlen.
- NXPDF-Markup unterstützt formatierte Texte; Bilder und frei positionierte Elemente fehlen. Zeichen ohne Glyphen in den eingebetteten Schriften werden als fehlende Glyphen angezeigt.
- Windows 10 22H2 ist als Kompatibilitätsziel im App-Manifest eingetragen, wurde mangels Windows-10-Testsystem aber nicht ausgeführt oder als unterstützt verifiziert. Eine saubere Windows-Installation wurde nicht geprüft.
- Nicht signiert; kein Debugger; Terminal, Build und gestartete Programme laufen ohne Sandbox; kein Live-Marketplace mit Konten, Bewertungen oder automatischen Updates.
