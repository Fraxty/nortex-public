 Sicherheitsrichtlinie / Security Policy

## Schwachstellen melden / Reporting a vulnerability

**Bitte melde Sicherheitslücken nicht in öffentlichen Issues und nenne keine Exploit-Details öffentlich.**
*Do not report vulnerabilities in public issues and do not publish exploit details.*

Nutze die [private Meldefunktion dieses Repositories](../../security/advisories/new) (Tab „Security“ → „Report a vulnerability“, GitHub Private Vulnerability Reporting). Alternativ per E-Mail an **jamietauber@gmx.de** (Fallback, falls die GitHub-Funktion nicht nutzbar ist).

Ziel (kein Rechtsanspruch): Eingangsbestätigung innerhalb von 48 Stunden, Risikobewertung und Zeitplan innerhalb von 5 Werktagen. Bitte veröffentliche die Lücke erst nach einem Fix oder nach ausdrücklicher Freigabe. / *Aim (no legal commitment): acknowledgment within 48 hours, assessment within 5 business days. Please disclose publicly only after a fix or explicit confirmation.*

Bitte angeben: betroffene Version, Windows-Version, Schritte zur Reproduktion, erwartete Auswirkung.

## Unterstützte Versionen / Supported versions

| Version | Status |
|---|---|
| 0.1.0 (Public Preview) | Vorabversion, keine Sicherheitszusagen |

Offiziell vorgesehene Plattform: Windows 11 x64. Windows 10 ist weder getestet noch als unterstützt dokumentiert.

## Sicherheitsmodell (Ist-Stand)

- NORTEX Code ist ein lokales Entwicklerwerkzeug. Terminal, Build und gestartete Programme laufen mit den Rechten des Benutzers; es gibt **keine Sandbox**. Öffne keine nicht vertrauenswürdigen Projekte und baue/starte deren Code nicht.
- Keine Telemetrie, keine automatische Übermittlung, keine Update-Abfrage (siehe `PRIVACY.md`).
- Release-Dateien sind **nicht digital signiert** (siehe README). Prüfsummen: `SHA256SUMS.txt`.
- Es wird keine Aussage getroffen, dass keine unbekannten Schwachstellen existieren.

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
