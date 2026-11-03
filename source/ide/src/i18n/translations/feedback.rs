//! Englische Texte des Feedback- und Support-Systems (IDE-Fenster und PDF-Formular).

pub(super) const TABLE: &[(&str, &str)] = &[
    ("Hilfe und Support", "Help and support"),
    ("Hilfe und Dokumentation", "Help and documentation"),
    ("Support und Kontakt", "Support and contact"),
    ("Fehler melden", "Report a bug"),
    ("Verbesserung vorschlagen", "Suggest an improvement"),
    (
        "Dokumentationsfehler melden",
        "Report a documentation error",
    ),
    ("Allgemeines Feedback", "General feedback"),
    ("Bug / Fehler", "Bug / error"),
    ("Verbesserungsvorschlag", "Improvement suggestion"),
    ("Funktionswunsch", "Feature request"),
    ("Dokumentationsfehler", "Documentation error"),
    ("Benutzeroberfläche", "User interface"),
    ("Performance", "Performance"),
    ("Sicherheit", "Security"),
    ("Sonstiges", "Other"),
    ("Immer", "Always"),
    ("Häufig", "Often"),
    ("Manchmal", "Sometimes"),
    ("Selten", "Rarely"),
    ("Einmalig", "Once"),
    ("Bitte geben Sie einen Titel an.", "Please enter a title."),
    (
        "Der Titel ist zu lang (höchstens 120 Zeichen).",
        "The title is too long (120 characters at most).",
    ),
    (
        "Bitte beschreiben Sie Ihr Anliegen.",
        "Please describe your concern.",
    ),
    (
        "Bitte nennen Sie die Schritte zur Reproduktion.",
        "Please list the steps to reproduce.",
    ),
    (
        "Bitte beschreiben Sie das tatsächliche Verhalten.",
        "Please describe the actual behavior.",
    ),
    (
        "Die Kontakt-E-Mail-Adresse ist ungültig.",
        "The contact email address is invalid.",
    ),
    (
        "mögliches Passwort oder Token",
        "possible password or token",
    ),
    ("E-Mail-Adresse", "Email address"),
    ("Benutzername im Dateipfad", "User name in a file path"),
    ("Titel", "Title"),
    ("Beschreibung", "Description"),
    ("Schritte zur Reproduktion", "Steps to reproduce"),
    ("Erwartetes Verhalten", "Expected behavior"),
    ("Tatsächliches Verhalten", "Actual behavior"),
    ("Fehlermeldung", "Error message"),
    ("Fehlermeldungen", "Error messages"),
    (
        "Betroffene Funktion oder Komponente",
        "Affected feature or component",
    ),
    ("Betroffene Extension", "Affected extension"),
    ("Relevante Fehlermeldungen", "Relevant error messages"),
    ("NORTEX-Version", "NORTEX version"),
    ("Betriebssystem", "Operating system"),
    ("Systemarchitektur", "System architecture"),
    ("Compiler-Version", "Compiler version"),
    (
        "NORTEX – Feedback / Fehlerbericht",
        "NORTEX – Feedback / bug report",
    ),
    (
        "Hallo,\n\nich sende eine Rückmeldung zu NORTEX. Das ausgefüllte Formular hänge ich als PDF an diese E-Mail an.\n\nViele Grüße",
        "Hello,\n\nI am sending feedback about NORTEX. I am attaching the completed form as a PDF to this email.\n\nBest regards",
    ),
    (
        "Im Formular wurden mögliche vertrauliche Angaben gefunden. Schwärzen Sie sie oder bestätigen Sie ausdrücklich, dass sie bleiben sollen.",
        "Possible confidential information was found in the form. Redact it or explicitly confirm that it should stay.",
    ),
    (
        "Es ist keine Kontaktadresse konfiguriert, der E-Mail-Versand ist deshalb nicht verfügbar. Sie können das PDF trotzdem speichern.",
        "No contact address is configured, so sending email is unavailable. You can still save the PDF.",
    ),
    (
        "E-Mail-Programm geöffnet. Hängen Sie die gespeicherte PDF-Datei selbst an und bestätigen Sie den Versand dort. NORTEX sendet nichts.",
        "Email client opened. Attach the saved PDF file yourself and confirm sending there. NORTEX sends nothing.",
    ),
    (
        "PDF gespeichert: {}. Es wurde nichts gesendet.",
        "PDF saved: {}. Nothing was sent.",
    ),
    (
        "PDF konnte nicht gespeichert werden: {}",
        "The PDF could not be saved: {}",
    ),
    (
        "Kein E-Mail-Programm gefunden. Kopieren Sie die Adresse {} und senden Sie die PDF-Datei mit einem Webmail-Dienst oder einem anderen Gerät.",
        "No email client found. Copy the address {} and send the PDF file with a webmail service or another device.",
    ),
    (
        "E-Mail-Programm konnte nicht geöffnet werden: {}",
        "The email client could not be opened: {}",
    ),
    ("Entwurf lokal gespeichert: {}", "Draft saved locally: {}"),
    (
        "Entwurf konnte nicht gespeichert werden: {}",
        "The draft could not be saved: {}",
    ),
    ("Entwurf geladen.", "Draft loaded."),
    (
        "Es ist kein gespeicherter Entwurf vorhanden.",
        "There is no saved draft.",
    ),
    (
        "Entwurf konnte nicht geladen werden: {}",
        "The draft could not be loaded: {}",
    ),
    ("Gespeicherter Entwurf gelöscht.", "Saved draft deleted."),
    (
        "Entwurf konnte nicht gelöscht werden: {}",
        "The draft could not be deleted: {}",
    ),
    (
        "Formular zurückgesetzt. Ein gespeicherter Entwurf bleibt bestehen.",
        "Form reset. A saved draft is kept.",
    ),
    ("Per E-Mail senden", "Send by email"),
    ("E-Mail-Programm öffnen", "Open email client"),
    (
        "Keine Kontaktadresse konfiguriert.",
        "No contact address configured.",
    ),
    (
        "So senden Sie das PDF: 1. PDF speichern. 2. „Per E-Mail senden“ wählen. 3. Die gespeicherte Datei selbst an die E-Mail anhängen. 4. Den Versand im E-Mail-Programm bestätigen.",
        "To send the PDF: 1. Save the PDF. 2. Choose “Send by email”. 3. Attach the saved file to the email yourself. 4. Confirm sending in the email client.",
    ),
    ("Datenschutz", "Privacy"),
    (
        "NORTEX sendet nichts automatisch: keine Telemetrie, keine Fehlerberichte, keine Anhänge.",
        "NORTEX sends nothing automatically: no telemetry, no error reports, no attachments.",
    ),
    (
        "Technische Angaben (Version, Betriebssystem, Architektur, Compiler) werden nur beigefügt, wenn Sie das ausdrücklich ankreuzen.",
        "Technical details (version, operating system, architecture, compiler) are only included if you explicitly tick the box.",
    ),
    (
        "Der Inhalt jeder Angabe ist vor dem Export sichtbar. Mögliche Passwörter, Tokens, E-Mail-Adressen und Benutzernamen in Pfaden werden erkannt und können geschwärzt werden.",
        "The content of every entry is visible before export. Possible passwords, tokens, email addresses and user names in paths are detected and can be redacted.",
    ),
    (
        "Ein Entwurf wird nur lokal gespeichert, wenn Sie „Entwurf lokal speichern“ wählen. Sie können ihn jederzeit löschen.",
        "A draft is only stored locally if you choose “Save draft locally”. You can delete it at any time.",
    ),
    (
        "Mit dem Versand der PDF-Datei übermitteln Sie die Angaben selbst an den Empfänger der E-Mail.",
        "By sending the PDF file you transmit the information to the email recipient yourself.",
    ),
    (
        "Handbuch, Tastenkürzel und Dokumentation finden Sie hier.",
        "You can find the manual, shortcuts and documentation here.",
    ),
    (
        "Fehler, Wünsche oder Fehler in der Dokumentation können Sie über die Bereiche links melden.",
        "You can report bugs, requests or documentation errors in the sections on the left.",
    ),
    ("Kontakt", "Contact"),
    ("Adresse kopieren", "Copy address"),
    (
        "Adresse in die Zwischenablage kopiert.",
        "Address copied to the clipboard.",
    ),
    (
        "Es ist keine Kontaktadresse konfiguriert. Der E-Mail-Versand ist deshalb deaktiviert; das PDF-Formular lässt sich trotzdem speichern.",
        "No contact address is configured. Sending email is therefore disabled; you can still save the PDF form.",
    ),
    ("PDF-Formular herunterladen", "Download PDF form"),
    ("Kategorie", "Category"),
    ("Kontakt-E-Mail (optional)", "Contact email (optional)"),
    ("Häufigkeit des Auftretens", "Frequency of occurrence"),
    (
        "Technische Informationen beifügen (nur mit meiner Zustimmung)",
        "Include technical information (only with my consent)",
    ),
    (
        "Diese Angaben werden exportiert:",
        "This information will be exported:",
    ),
    (
        "Mögliche vertrauliche Angaben gefunden:",
        "Possible confidential information found:",
    ),
    ("Automatisch schwärzen", "Redact automatically"),
    ("Trotzdem unverändert verwenden", "Use unchanged anyway"),
    ("Als PDF exportieren", "Export as PDF"),
    ("Zurücksetzen", "Reset"),
    ("Entwurf lokal speichern", "Save draft locally"),
    ("Entwurf laden", "Load draft"),
    ("Entwurf löschen", "Delete draft"),
    (
        "Ein Entwurf wird nur auf Ihren Wunsch gespeichert: {}",
        "A draft is only stored on request: {}",
    ),
    ("Datenschutzinformationen", "Privacy information"),
    // PDF-Formular
    ("Feedback-Formular", "Feedback form"),
    ("Allgemeine Angaben", "General information"),
    ("Fehlerbericht", "Bug report"),
    ("Verbesserungsvorschläge", "Improvement suggestions"),
    (
        "Datenschutz und zusätzliche Informationen",
        "Privacy and additional information",
    ),
    (
        "Hängen Sie die gespeicherte Datei an eine E-Mail an den Entwickler an.",
        "Attach the saved file to an email to the developer.",
    ),
    (
        "Senden Sie die gespeicherte Datei als E-Mail-Anhang an {}.",
        "Send the saved file as an email attachment to {}.",
    ),
    ("Art der Rückmeldung", "Type of feedback"),
    (
        "Ich füge freiwillig technische Angaben bei.",
        "I voluntarily include technical details.",
    ),
    (
        "Ich bestätige, dass meine Angaben freiwillig erfolgen.",
        "I confirm that my information is provided voluntarily.",
    ),
    ("Name (optional)", "Name (optional)"),
    ("E-Mail-Adresse (optional)", "Email address (optional)"),
    ("Datum", "Date"),
    ("Titel des Fehlers", "Bug title"),
    ("Gewünschte Funktion", "Requested feature"),
    (
        "Welches Problem soll gelöst werden?",
        "Which problem should be solved?",
    ),
    (
        "Warum wäre diese Funktion sinnvoll?",
        "Why would this feature be useful?",
    ),
    (
        "Freiwillige technische Angaben",
        "Voluntary technical details",
    ),
    ("Weitere Informationen", "Further information"),
];
