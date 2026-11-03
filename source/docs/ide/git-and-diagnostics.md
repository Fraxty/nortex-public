# Git und Diagnose in NORTEX Code

**Git-Panel** (`ide/src/git.rs`, `ide/src/shell/git_panel.rs`): ruft die `git`-CLI ohne Shell auf (Pfade stehen hinter `--`, nur relative Pfade ohne `..`).
Je Datei: Stagen/Entstagen, Diff (Arbeitsbaum) und Diff (Index). Der Diff zeigt Hunks (grün = hinzugefügt, rot = entfernt) mit
„Hunk stagen/entstagen“ (`git apply --cached [--reverse]` über stdin). Dateien mit Konfliktmarkern (`<<<<<<<`/`=======`/`>>>>>>>`, auch diff3)
bieten je Bereich „Eigene/Fremde/Beide übernehmen“; die Auflösung wird in die Datei auf der Platte geschrieben.

**Diagnosen** (`ide/src/diagnostics/mod.rs`): `PROVIDERS` ist die Anbieterliste (`compiler` = `analyze_source`, `style` = Leerzeichen am Zeilenende
und TODO/FIXME). `collect` dedupliziert, sortiert und begrenzt (500). `quick_fixes` liefert echte Textedits (Leerzeichen entfernen,
fehlendes `;` einfügen, „Meinten Sie …?“ per Levenshtein bei `NX1004`); `apply_edits` wendet sie hinten nach vorn mit Grenzprüfung an.
Im Problems-Tab erscheint pro Eintrag mit Korrektur der Button „Quick Fix“; er geht über `set_editor_text` und ist damit rückgängig machbar.
