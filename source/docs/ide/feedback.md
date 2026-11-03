# Feedback und Support / Feedback and support

## Deutsch

Das Hilfe-Menü der IDE enthält: Hilfe und Dokumentation, Fehler melden, Verbesserung vorschlagen, Dokumentationsfehler melden, Allgemeines Feedback, Support und Kontakt. Alle Texte gibt es auf Deutsch und Englisch (Spracheinstellung der IDE).

**Formular.** Titel, Beschreibung, optionale Kontakt-E-Mail, Kategorie (Bug, Verbesserung, Funktionswunsch, Dokumentation, Oberfläche, Performance, Sicherheit, Sonstiges). Bei Fehlerberichten zusätzlich Schritte, erwartetes und tatsächliches Verhalten, Häufigkeit, Fehlermeldung und Komponente. Pflichtfelder werden validiert.

**Datenschutz.**
- NORTEX überträgt nichts. Es gibt keinen Server, kein Konto und keine Telemetrie.
- Technische Angaben (Version, Betriebssystem, Architektur, Compiler-Version) kommen nur mit ausdrücklicher Zustimmung ins PDF; der Inhalt ist vorher sichtbar.
- Vor dem Export werden alle Angaben auf mögliche Passwörter, Tokens, Schlüssel, E-Mail-Adressen und Benutzernamen in Pfaden geprüft. Treffer werden angezeigt; der Export wird erst nach „Automatisch schwärzen“ oder ausdrücklicher Bestätigung freigegeben.
- Ein Entwurf wird nur auf Knopfdruck gespeichert (`feedback-draft.json` neben der Einstellungsdatei) und lässt sich löschen. Zurücksetzen löscht ihn nicht.

**PDF.** „Als PDF exportieren“ speichert das ausgefüllte Formular, „PDF-Formular herunterladen“ ein leeres. Das PDF (`NORTEX_Feedback_Form.pdf`, 4 Seiten) enthält echte AcroForm-Felder (Text, Auswahl, Checkbox). Der Writer (`ide/src/feedback/pdf.rs`) benötigt keine zusätzliche Bibliothek. Leere Muster: `docs/feedback/de/` und `docs/feedback/en/`, erzeugt mit `cargo run -p nortex-ide --example write_feedback_form`.

**E-Mail.** „Per E-Mail senden“ öffnet das Standard-Mailprogramm per `mailto:` mit Betreff und Text. Anhänge unterstützt `mailto:` nicht: Die gespeicherte PDF-Datei muss selbst angehängt und der Versand dort bestätigt werden. Ohne Mailprogramm erscheint eine Meldung mit Alternativen (Adresse kopieren, Webmail oder anderes Gerät).

**Kontaktadresse.** `CONTACT_EMAIL` in `ide/src/feedback/mod.rs` ist `NorisTec@protonmail.com` (vom Projektinhaber angegeben). Steht dort `None`, sind die E-Mail-Schaltflächen deaktiviert; PDF-Export und Entwurf funktionieren immer. Die Adresse steht auch im Fußbereich der PDF-Seite 1 als Sendehinweis.

## English

The Help menu offers: Help and documentation, Report a bug, Suggest an improvement, Report a documentation error, General feedback, Support and contact, fully in German and English.

The in-app form validates required fields, never sends anything, and includes technical details only with explicit consent. Entries are scanned for passwords, tokens, keys, email addresses and user names in paths; export is blocked until they are redacted or explicitly kept. Drafts are stored locally only on request and can be deleted.

“Export as PDF” saves the filled form; “Download PDF form” saves an empty one. `NORTEX_Feedback_Form.pdf` has four pages with real AcroForm fields, written without any extra library (`ide/src/feedback/pdf.rs`). “Send by email” opens the default mail client via `mailto:` with subject and body; `mailto:` cannot attach files, so attach the saved PDF yourself and confirm sending in the mail client. If no client is available, an error with alternatives is shown.

The contact address is `NorisTec@protonmail.com`, set in `CONTACT_EMAIL` in `ide/src/feedback/mod.rs`; if it is `None`, the email buttons are disabled.
