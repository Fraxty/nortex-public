# 13 FAQ and troubleshooting

[Back](12-efficient-workflows.md) · [Contents](README.md)

**Why can't a project be opened?**
Check the path (the folder must exist). Drag & drop only accepts `.nx` ("Only .nx files are opened"). A broken `nortex.toml` produces a message in the status bar.

**Why does compilation not work?**
1. "Compiler … could not be started": `nortexc.exe` must be next to `nortex-code.exe` or on the PATH.
2. Linker/runtime errors: `nortex_runtime.lib` is missing (next to the EXE, in the parent folder or via `NORTEX_RUNTIME_LIB`).
3. New, not yet saved files: save with Ctrl+S first and choose a name in the file dialog.
4. NX errors: see [Chapter 8](08-debugging.md).

**Where do I find error messages?** In the *Compiler Log* tab (Ctrl+Shift+M), marked in the editor, count in the status bar.

**How do I open the terminal?** *Terminal* tab in the bottom panel; show the panel with Ctrl+J.

**How do I save files?** Ctrl+S or *File → Save*; Ctrl+Shift+S for "Save as". Unsaved tabs show •.

**How do I change the appearance?** Gear → theme, density, zoom, language ([Chapter 9](09-settings-and-customization.md)).

**Edit several files at once?** Tabs and split editor ([Chapter 5](05-editor.md)).

**Why does the Run button show "Exitcode Some(0)"?** That is how the IDE prints exit code 0; it is not an error.

**Why does the IDE report an error for `float` or `struct`?** These constructs cannot be generated natively yet (NX2001/NX2002).

**Is there a debugger, Git commit, rename/delete, AI?** No, see the feature status in the [contents](README.md).

**My settings were reset.** Delete `%APPDATA%\NORTEX Code\settings.json`; it is created again.
