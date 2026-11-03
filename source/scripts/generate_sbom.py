#!/usr/bin/env python3
"""Erzeugt aus `cargo metadata --locked` die CycloneDX-1.5-SBOM und THIRD_PARTY_NOTICES.md.

Aufruf (Repo-Root): python scripts/generate_sbom.py
Die Lizenzangaben stammen aus den Crate-Metadaten (SPDX-Ausdrücke); Lizenztexte und
Copyright-Vermerke werden NICHT geprüft oder kopiert (siehe Hinweis in der Ausgabe).
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    out = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"],
                         cwd=ROOT, capture_output=True, check=True)
    meta = json.loads(out.stdout)
    workspace = set(meta["workspace_members"])
    third = sorted((p for p in meta["packages"] if p["id"] not in workspace),
                   key=lambda p: (p["name"], p["version"]))
    version = next(p["version"] for p in meta["packages"] if p["name"] == "nortex-ide")

    components = []
    for p in third:
        license_expr = p.get("license") or "NOASSERTION"
        components.append({
            "type": "library",
            "bom-ref": "cargo:%s@%s" % (p["name"], p["version"]),
            "name": p["name"],
            "version": p["version"],
            "purl": "pkg:cargo/%s@%s" % (p["name"], p["version"]),
            "licenses": [{"expression": license_expr}],
            **({"externalReferences": [{"type": "vcs", "url": p["repository"]}]} if p.get("repository") else {}),
        })
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {"type": "application", "name": "NORTEX", "version": version},
            "tools": [{"name": "scripts/generate_sbom.py"}],
        },
        "components": components,
    }
    sec = ROOT / "docs" / "security"
    sec.mkdir(parents=True, exist_ok=True)
    (sec / "sbom.cdx.json").write_text(json.dumps(bom, indent=1) + "\n", encoding="utf-8", newline="\n")

    lines = [
        "# Third-Party Notices",
        "",
        "Automatisch erzeugt von `scripts/generate_sbom.py` aus `Cargo.lock` / `cargo metadata --locked`.",
        "",
        "**Hinweis (REVIEW REQUIRED):** Die Lizenzangaben stammen ausschließlich aus den Crate-Metadaten. "
        "Lizenztexte, Copyright-Vermerke und NOTICE-Dateien der Komponenten wurden nicht einzeln geprüft "
        "und sind nicht beigelegt. Vor einer Weitergabe müssen die Pflichten (Namensnennung, Lizenztext) "
        "je Lizenz erfüllt werden. Die Liste umfasst alle Plattformen des Lockfiles, nicht nur Windows x64.",
        "",
        "Nicht in Cargo erfasst, aber ausgeliefert/verwendet: eingebettete Standardschriften von egui/epaint "
        "(Metadaten nennen `OFL-1.1` und `Ubuntu-font-1.0`), Inno Setup (nur Build-Werkzeug, nicht ausgeliefert). "
        "Eigene Icons und Grafiken (`assets/`): laut Angabe des Rechteinhabers (Jamie Tauber) selbst erstellt; nicht unabhängig geprüft.",
        "",
        "Komponenten: %d" % len(third),
        "",
        "| Crate | Version | Lizenz (SPDX laut Metadaten) |",
        "|---|---|---|",
    ]
    for p in third:
        lines.append("| %s | %s | %s |" % (p["name"], p["version"], (p.get("license") or "NOASSERTION").replace("|", "/")))
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print("%d Komponenten -> docs/security/sbom.cdx.json, THIRD_PARTY_NOTICES.md" % len(third))


if __name__ == "__main__":
    sys.exit(main())
