# NORTEX – Installation und Einrichtung

## Voraussetzungen

- Windows x64
- Rust/Cargo (stable, ≥ 1.88, Edition 2024)
- Visual Studio Build Tools mit MSVC-Linker (`link.exe`); fehlt er, scheitert `nortexc build` mit einer Linker-Fehlermeldung
- Git (empfohlen; das Git-Panel der IDE zeigt Status nur, wenn `git` installiert ist)

## Bauen

Es gibt keinen Installer im Repository-Stand. Aus dem Projektordner:

```powershell
cargo build --release -p nortex-compiler   # erzeugt target\release\nortexc.exe
scripts\build_release.ps1                  # vollständiger Release-Build, kopiert nach bin\
.\run_nortex_code.ps1                      # baut die IDE und kopiert sie nach bin\nortex-code.exe
```

Die Runtime `nortex_runtime.lib` muss neben `nortexc.exe`, im Parent-Ordner oder in `target\release` bzw. `target\debug` liegen (`NORTEX_RUNTIME_LIB` hat Vorrang). Details zu Build-Skripten und Verifikation: [BUILD_STATUS.md](../../BUILD_STATUS.md), [Developer-Handbuch](../development/developer-handbook.md).

## Erstes Programm

Datei `hello.nx`:

```nortex
using System;

public int main() {
    Console.writeLine("Hallo, NORTEX!");
    return 0;
}
```

```powershell
nortexc check hello.nx            # nur Frontend-Prüfung
nortexc mir hello.nx              # validiertes MIR ausgeben
nortexc build hello.nx -o hello.exe
.\hello.exe                       # gibt "Hallo, NORTEX!" aus
echo $LASTEXITCODE                # 0 (Rückgabewert von main)
```

Weitere Optionen: `--emit-obj` (nur COFF-Objekt, Ausgabe `.obj`/`.o`), `--overflow-checks[=true|false]`, `--version`.

## Projekt mit Manifest

`nortex.toml` im Projektordner:

```toml
[package]
name = "Demo"
version = "0.1.0"

[build]
main = "src/main.nx"            # Standard
output_name = "demo.exe"        # Standard: <name>.exe
```

`nortexc build <ordner>` schreibt `build/demo.exe`. Das vollständige Schema steht in [build-system/manifest.md](../build-system/manifest.md). Mehrdatei-Projekte nutzen `using util;`.

## Tests und Prüfungen

```powershell
cargo test --workspace
scripts\verify.ps1
```

## Entwicklungsumgebung

„NORTEX Code“ (`bin\nortex-code.exe`): egui-basierte IDE mit Syntaxhervorhebung, Inline-Diagnosen, Strg+F5 / Strg+Shift+B (Build & Run) und Suchen/Ersetzen. Handbuch: [user-manual/README.md](../user-manual/README.md); Installation der IDE: [Kapitel 2 des Handbuchs](../user-manual/02-installation.md).
