# NORTEX-Werkzeuge

Die Programme unter `tools/` sind als Workspace-Crate `nortex-tools` eingebunden und werden als eigene Binaries gebaut:

- `nortex`: Projekt-CLI; derzeit sind `check` und `version` funktionsfähig.
- `nortex-fmt`: Platzhalter für den Formatter.
- `nortex-language-server`: Platzhalter für den LSP-Transport.
- `nortex-pkg`: Platzhalter für den Paketmanager.
- `nortex-doc`: Platzhalter für den Dokumentationsgenerator.

Die übrigen Befehle der CLI (`new`, `build`, `run`, `clean`, `test`, `package`, `format`, `doc`) geben eine explizite Meldung aus, dass sie noch nicht implementiert sind. Für die Compilerprüfung einzelner `.nx`-Dateien steht außerdem `nortexc check <datei.nx>` aus dem Paket `nortex-compiler` zur Verfügung.
