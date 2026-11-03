# NORTEX Code – UI-Framework-Evaluierung

**Status:** egui/eframe ist für den ersten Windows-Desktop-Prototyp ausgewählt.
Die Auswahl ist eine Implementierungsentscheidung, keine abgeschlossene
Produktionsfreigabe; die unten aufgeführten Messungen stehen noch aus.

Der Ablaufplan verlangt einen Prototypvergleich vor der Festlegung. Bis dieser Vergleich abgeschlossen ist, dürfen Compiler, Editor-Core und Projektmodell keine Abhängigkeit von einem konkreten UI-Framework erhalten.

## Kandidaten

| Kandidat | Zu untersuchender Ansatz | Noch zu verifizieren |
|---|---|---|
| egui/eframe | schneller Rust-Desktop-Prototyp, eigenes Editor-Widget | Leerlauf-CPU, große Textmengen, IME, Screenreader, DPI |
| Slint | deklarative Desktop-Oberfläche | Lizenzbedingungen des konkreten Einsatzes, IME, Editor-Widget, Barrierefreiheit |
| iced | Elm-artiger Zustands- und Nachrichtenfluss | IME, Textlayout, große Dateien, Screenreader, DPI |

## Gemeinsame Messaufgaben

Jeder Kandidat erhält denselben kleinen Prototyp: Fenster, Menüleiste, Dateiliste, Tabs, Zeilennummern und ein eigenes Text-Widget, das nur sichtbare Zeilen aus dem UI-unabhängigen `EditorBuffer` abfragt.

Zu messen und zu dokumentieren sind:

1. Scrollen und Eingabe in einer 10-MB-Datei.
2. Eingabeverzögerung nach einem Edit und nach einer Syntax-Neuberechnung.
3. Leerlauf-CPU und Speicherverbrauch.
4. DPI-Wechsel zwischen Monitoren und Skalierung.
5. Deutsche Tastatur, Tottasten, Clipboard und IME-Komposition.
6. Tastaturbedienung, Hochkontrast und Screenreader.
7. Lizenzmodell, Lizenzpflichten, Wartbarkeit und Windows-Paketierung.

## Entscheidungsregel

Der erste Prototyp verwendet egui/eframe, um die visuelle Architektur und
Compiler-Anbindung früh zu erproben. Produktionsfreigabe und Zusage für große
Dateien, IME, Barrierefreiheit, DPI und Leerlauf-CPU erfolgen erst nach den
Messungen auf Windows 10/11. Das Editor-Widget verwendet gegenwärtig noch
eguis `TextEdit`; ein viewport-basiertes Widget direkt auf dem Piece-Table ist
weiterhin erforderlich.
