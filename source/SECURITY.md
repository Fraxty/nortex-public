# Sicherheitsrichtlinie / Security Policy

## Schwachstellen melden / Reporting a vulnerability

**Bitte melde Sicherheitslücken nicht in öffentlichen Issues und nenne keine Exploit-Details öffentlich.**
*Do not report vulnerabilities in public issues and do not publish exploit details.*

Nutze die private Meldefunktion des Repositories (Tab „Security“ → „Report a vulnerability“, GitHub Private Vulnerability Reporting). Alternativ per E-Mail an **NorisTec@protonmail.com** (Fallback, falls die GitHub-Funktion nicht nutzbar ist).

Ziel (kein Rechtsanspruch): Eingangsbestätigung innerhalb von 48 Stunden, Risikobewertung und Zeitplan innerhalb von 5 Werktagen. Bitte veröffentliche die Lücke erst nach einem Fix oder nach ausdrücklicher Freigabe. / *Aim (no legal commitment): acknowledgment within 48 hours, assessment within 5 business days. Please disclose publicly only after a fix or explicit confirmation.*

Bitte angeben: betroffene Version, Windows-Version, Schritte zur Reproduktion, erwartete Auswirkung.

## Unterstützte Versionen / Supported versions

| Version | Status |
|---|---|
| 0.1.0 (Public Preview) | Vorabversion, keine Sicherheitszusagen |

Offiziell vorgesehene Plattform: Windows 11 x64. Windows 10 ist weder getestet noch als unterstützt dokumentiert.

## Sicherheitsmodell (Ist-Stand)

- NORTEX Code ist ein lokales Entwicklerwerkzeug. Terminal, Build und gestartete Programme laufen mit den Rechten des Benutzers; für diese Prozesse gibt es **keine Sandbox**. Gehostete Extensions werden unter Windows separat in einem AppContainer mit freigegebenen Berechtigungen gestartet. Öffne keine nicht vertrauenswürdigen Projekte und baue/starte deren Code nicht.
- Keine Telemetrie, keine automatische Übermittlung, keine Update-Abfrage (siehe `docs/legal/PRIVACY_POLICY.md`).
- Release-Dateien sind **nicht digital signiert**. Öffentliche Downloads und Prüfsummen stehen im [NORTEX-Public-Repository](https://github.com/Fraxty/nortex-public).
- Es wird keine Aussage getroffen, dass keine unbekannten Schwachstellen existieren.
