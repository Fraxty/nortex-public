# Datenschutz-Entwurf / Privacy Policy (DRAFT)

> **REVIEW REQUIRED – juristisch ungeprüfter Entwurf.** Er beschreibt nur den technisch belegten Ist-Zustand und ist keine Rechtsberatung. Verantwortlicher, Kontaktdaten und Informationspflichten (DSGVO) müssen vom Inhaber ergänzt werden; sie wurden nicht erfunden.

## Technischer Ist-Zustand (Stand der Prüfung im Quellcode)
- **Telemetrie / Nutzungsdaten:** keine. Im IDE- und Compiler-Code wurde kein Netzwerkcode gefunden (Suche nach `TcpStream`, `reqwest`, `ureq`, `http(s)://`).
- **Update-Abfragen:** keine; die IDE zeigt „offline nicht verfügbar“.
- **Online-Konto:** nicht implementiert; Anmeldung liefert `ServiceUnavailable`. Es werden keine Zugangsdaten übertragen oder gespeichert.
- **Absturzberichte:** nur lokale Datei `ide_crash.log` (neben der EXE oder im Temp-Ordner). Kein automatischer Versand. Sie kann Pfade und Panic-Meldungen enthalten; Weitergabe nur freiwillig durch den Nutzer.
- **Lokale Daten:** Einstellungen in `%APPDATA%\NORTEX Code\settings.json` (Theme, Zoom, Sprache, zuletzt geöffnete Projekte). Projekte bleiben beim Deinstallieren erhalten.
- **Externe Programme:** Git, Compiler (`nortexc`), Terminal-Shell laufen lokal auf Wunsch des Nutzers; was diese übertragen (z. B. `git push`), liegt außerhalb von NORTEX.

## Offen (REVIEW REQUIRED)
Verantwortlicher/Impressum, Rechtsgrundlagen, freiwillige Fehlerberichte per E-Mail (Verarbeitung, Speicherdauer, Informationspflichten), Hosting auf GitHub (Datenverarbeitung durch GitHub).
