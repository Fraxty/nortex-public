# Kapitel 2 – Installation und Einrichtung

← [Kapitel 1](01-introduction.md) · [Inhalt](README.md) · Weiter: [Kapitel 3](03-language-basics.md)

Voraussetzungen (Windows x64, Rust ≥ 1.88, MSVC Build Tools), Build-Befehle, erstes Programm und das Manifest `nortex.toml` stehen zentral in der [Installationsanleitung](../getting-started/installation.md).

Kurzfassung:

```powershell
cargo build --release -p nortex-compiler
nortexc build hello.nx -o hello.exe
```

Hinweis: `using util;` löst eigene `.nx`-Dateien auf (flacher Namensraum), siehe [Ist-Spezifikation](SPECIFICATION_IMPLEMENTED.md).
