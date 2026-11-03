# 12 Efficient workflows

[Back](11-first-project.md) · [Contents](README.md) · [Next: FAQ](13-faq-and-troubleshooting.md)

- **Cycle**: write → Ctrl+F5 → check the Compiler Log → correct. The IDE saves by itself before every build.
- **Command palette** (Ctrl+Shift+P) instead of searching menus.
- **Window layout**: split the editor with Ctrl+\ (source next to example/test). Toggle the console with Ctrl+J, Ctrl+Shift+M/U jumps to the matching tab. Zen mode hides secondary elements.
- **Projects**: one folder per project, optionally `nortex.toml` ([manifest](../../build-system/manifest.md)) for project name, entry file and build options; the window title and status bar then show the project name.
- **Navigation**: Ctrl+P (open file), project search Ctrl+Shift+F, Ctrl+Tab between tabs, Ctrl+F in the document.
- **Edit lines** with Alt+↑/↓, Ctrl+/ and Ctrl+D instead of copy/paste.
- **Overview**: small functions, meaningful names; auto-completion uses identifiers of the document.
- Always save before closing the IDE; unsaved tabs are not recovered.
