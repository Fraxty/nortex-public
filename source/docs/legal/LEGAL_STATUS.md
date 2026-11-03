# Lizenz- und Rechtsstatus (ENTWURF)

> **REVIEW REQUIRED – nichts hiervon ist juristisch geprüft.** Es werden keine Rechte oder Lizenzbedingungen erfunden.

## Befunde
1. **Lizenzstrategie (entschieden vom Rechteinhaber: MIT-Lizenz, Copyright Jamie Tauber; die Veröffentlichung der Binaries unter MIT erlaubt freie Weitergabe und Nutzung).** Historisch: `LICENSE` und `Cargo.toml` nennen **MIT** (Copyright „NORTEX Contributors“). Ein früherer Auftrag nennt ein *proprietäres, geschlossenes* Modell, der Masterplan ein *öffentliches GitHub-Repository*. Die Lizenzentscheidung ist nicht getroffen; MIT würde den Quellcode unwiderruflich frei nutzbar machen. Nicht geändert.
2. **`LICENSE` ist unvollständig.** Es fehlt der Schlussabsatz des Standardtextes („IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE …“). Erst nach der Lizenzentscheidung korrigieren.
3. **Urheberrechtsinhaber unklar.** `LICENSE`: „NORTEX Contributors“; `installer/nortex_setup.iss` nennt als Publisher einen **Personennamen** (`AppPublisher`). Der Inhaber muss bestätigen, ob dieser Name veröffentlicht werden soll und wer die Rechte hält.
4. **Drittanbieter:** 365 Crates, Lizenzen laut Metadaten (MIT/Apache-2.0/Unicode/Zlib/BSL/…; ein Crate mit `LGPL-2.1-or-later` als *Alternative* neben MIT/Apache; Schriften mit OFL-1.1/Ubuntu-font-1.0). Siehe `THIRD_PARTY_NOTICES.md`. Lizenztexte/NOTICE-Pflichten nicht beigelegt.
5. **Icons, Logos, Marke „NORTEX“:** Herkunft/Rechte und Markenrecherche nicht belegt (`assets/`, `scripts/generate_brand_assets.py` erzeugt Grafiken – Urheberschaft Inhaber zu bestätigen).
6. **Weitere Dokumente** (EULA, Nutzungsbedingungen, Marken-/Haftungshinweis): nicht erstellt, solange das Lizenzmodell offen ist. Bei Bedarf nach deutschem/EU-Recht (Verbraucherrecht, Haftung, Datenschutz) juristisch erstellen lassen.

## Entscheidungen, die der Inhaber treffen muss
juristische Prüfung des Lizenztextes (Rechteinhaber: Jamie Tauber, angegeben; Grafiken/Icons: Eigenwerk laut Angabe; kein Signierzertifikat geplant, Binaries bleiben unsigniert), Kontaktadresse für Security/Verhaltenskodex, Markenpolitik, Signierung (Zertifikat).
