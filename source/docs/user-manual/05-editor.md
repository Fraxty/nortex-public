# 5 Arbeiten mit dem Editor

[Zurück](04-projects-and-files.md) · [Inhaltsverzeichnis](README.md) · [Weiter: Kompilieren und Ausführen](06-build-and-run.md)

## Grundfunktionen

- **Syntaxhervorhebung** für Schlüsselwörter, Bezeichner, Zahlen, Strings, Zeichen, Operatoren, Satzzeichen und Kommentare (Farben abhängig vom Theme).
- **Zeilennummern** in der linken Spalte.
- **Rückgängig/Wiederholen**: Strg+Z, Strg+Y oder Strg+Umschalt+Z (auch im Menü *Bearbeiten*).
- Fehler werden direkt im Editor markiert (Inline-Diagnosen, kurz nach dem Tippen aktualisiert) und im Compiler Log gelistet ([Kapitel 8](08-debugging.md)).
- **Klammervervollständigung und Mehrfachcursor gibt es nicht.**

## Autovervollständigung

Ab **2 Zeichen** eines Bezeichners öffnet sich ein Popup mit bis zu **5 Vorschlägen**: erst Schlüsselwörter, dann Bezeichner aus dem aktuellen Dokument.

| Taste | Wirkung |
|---|---|
| Pfeil hoch/runter | Auswahl bewegen (umlaufend) |
| Tab oder Enter | Vorschlag übernehmen |
| Esc | Popup schließen |

## Zeilenbefehle

| Befehl | Taste |
|---|---|
| Kommentar ein/aus | Strg+/ |
| Zeile(n) nach oben/unten | Alt+↑ / Alt+↓ |
| Zeile(n) duplizieren | Umschalt+Alt+↓ |
| Nächstes Vorkommen des markierten Textes auswählen | Strg+D |

## Suchen und Ersetzen

**Strg+F** öffnet ein schwebendes Feld oben rechts im Editor, **Strg+H** zusätzlich mit „Ersetzen durch“. Ist Text markiert, wird er übernommen.

| Bedienelement | Funktion |
|---|---|
| Suchfeld | Treffer werden live gesucht |
| Schalter *Aa* / *W* / *.\** | Groß-/Kleinschreibung · Ganzes Wort · Reguläre Ausdrücke (mit Capture-Gruppen im Ersetzungstext) |
| ▲ / ▼ oder Enter / Umschalt+Enter | vorheriger / nächster Treffer |
| *Ersetzen* / *Alle* | aktuellen / alle Treffer ersetzen |
| × oder Esc | schließen |

Ein ungültiger regulärer Ausdruck wird als Fehler angezeigt, ohne dass die IDE abstürzt. Dateiübergreifende Suche: Seitenleiste **Suche** (Strg+Umschalt+F): Suchbegriff eingeben, *Suchen* oder Enter; Klick auf einen Treffer öffnet die Datei (einfache Textsuche, keine Zeilenmarkierung).

## Tabs und geteilter Editor

Mehrere Dateien liegen in Tabs; **Strg+Tab** wechselt zum nächsten Tab. Über *Ansicht → Editor-Layout* (oder Tastatur) lässt sich der Editor teilen:

| Layout | Taste |
|---|---|
| Zwei Spalten | Strg+\ (auch Strg+Umschalt+2) |
| Zwei Zeilen | Strg+Umschalt+\ (auch Strg+Umschalt+3) |
| 2×2 Raster | Strg+Umschalt+4 |
| Einzeln | *Ansicht → Editor-Layout → Einzeln* |

Fokus: Strg+1 … Strg+4 wählt das Pane; Strg+Alt+Pfeil oder Alt+←/→ wechselt in Richtung; Strg+Umschalt+W schließt das aktive Pane.

## Navigation

Gehe„Gehe zu Definition“- und keine „Gehe zu Zeile“-Funktion. Verwenden: Strg+F, Projektsuche, Tabs, Strg+P (Datei öffnen).


## Umbenennen, Breadcrumbs und Minimap

- **F2** öffnet am Cursor ein Eingabefeld mit dem Bezeichner unter dem Cursor. Enter benennt alle Vorkommen in der aktuellen Datei um, Esc bricht ab. Kommentare, Strings und Schlüsselwörter bleiben unberührt; nur ASCII-Namen, kein projektweites Umbenennen.
- **Breadcrumbs** über dem Editor zeigen Datei und umschließende Scopes (z. B. struct und Funktion). Ein Klick springt zur Startzeile des Scopes.
- **Minimap** am rechten Rand (60 px); Klick scrollt. Ein/Aus mit **Strg+Alt+M**, über das Menü Ansicht oder die Befehlspalette; gespeichert als editor.minimap.enabled.
