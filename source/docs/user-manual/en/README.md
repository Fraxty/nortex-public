# NORTEX Code – User Manual

Status: package version **0.3.7-dev**. All statements and screenshots come from the actual application (Windows, x64). German version: [../README.md](../README.md).

## Chapters

1. [Welcome to Nortex Code](01-welcome.md)
2. [Installation and first start](02-installation.md)
3. [User interface](03-interface-overview.md)
4. [Projects and files](04-projects-and-files.md)
5. [Working with the editor](05-editor.md)
6. [Build and run](06-build-and-run.md)
7. [Terminal and consoles](07-terminal-and-consoles.md)
8. [Error analysis and debugging](08-debugging.md)
9. [Settings and customization](09-settings-and-customization.md)
10. [Keyboard shortcuts](10-keyboard-shortcuts.md)
11. [First steps](11-first-project.md)
12. [Efficient workflows](12-efficient-workflows.md)
13. [FAQ and troubleshooting](13-faq-and-troubleshooting.md)

The programming language itself is described in the [language reference](../../language/README.md) (German). Figures are in [images/](../images).

## Feature status of the IDE

| Level | Features |
|---|---|
| **Complete** | Menu bar (File, Edit, View, Help), breakpoint management (F9, ● marker, breakpoint list), settings view (search, reset, export/import), activity bar, explorer tree, editor with syntax highlighting and line numbers, tabs (drag to reorder), split editor (up to 2×2), find/replace (Ctrl+F/H, RegEx), auto-completion, line commands, Build & Run, compiler log with diagnostics, runtime output, terminal, command palette, themes/density/zoom, German/English UI, `nortex.toml` manifest, status bar |
| **Partial** | Dashboard (display; Git status read-only), Git panel (display only: branch and changed files), project search (simple text search), sidecar (panel without AI provider) |
| **Local only / prepared** | Account (local mode only, no online service; sync options only prepared; source code stays local) |
| **Planned / stub** | Debugger backend (unavailable: `nortexc` emits no debug info; panel and F5 only show a notice), cancel program (Shift+F5), extensions (Ctrl+Shift+X), AI menu (all entries disabled) |
| **Not available** | New-project wizard, rename/delete files, create folders, multi-cursor, automatic bracket completion, console input for running programs, context menus |

Known limits of the compiler are in the [language reference](../../language/README.md).

## Building the PDF manuals

```powershell
python scripts/build_manual_pdf.py          # German and English
python scripts/build_manual_pdf.py --lang en
```

Output: `docs/user-manual/NORTEX-Code-Handbuch.pdf` (German) and `docs/user-manual/NORTEX-Code-Manual.pdf` (English).
