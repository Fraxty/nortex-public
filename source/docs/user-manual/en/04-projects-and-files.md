# 4 Projects and files

[Back](03-interface-overview.md) · [Contents](README.md) · [Next: Editor](05-editor.md)

A **project** is a folder. There is no "New project" wizard.

## Creating a project

1. Create a folder in Windows Explorer.
2. Optionally place a `nortex.toml` in it (manifest; see the [manifest schema](../../build-system/manifest.md)).
3. Start Nortex Code with this folder as working directory, or enter the path in the field at the top of the Explorer and click **Open**.

## Opening / refreshing a project

Enter the path in the Explorer field → **Open**. Status bar message "Project loaded" or "Project could not be refreshed: …". **File → Refresh project** re-reads the file list in the background (also after changes made outside the IDE). With a `nortex.toml` in the project folder the status bar shows "Name → output file" and the window title reads "<Name> — NORTEX Code".

## Files

| Action | How |
|---|---|
| New | **+** in the tab area, *File → New file* or *New* on the dashboard |
| Open | Click in the Explorer · Ctrl+P (Windows file dialog, filter `.nx`/`.nortex`) · *Open file* on the dashboard · drag a file onto the window · recently opened file on the dashboard |
| Save | Ctrl+S or **Save**. For a new document the save dialog opens. |
| Save as | *File → Save as…* or Ctrl+Shift+S opens the file dialog. |
| Close | **×** on the tab. With unsaved changes you can save, discard or cancel. |

Saving is atomic (a failed write leaves the original file unchanged). Errors appear as a status message, e.g. "Save failed: …".

## Not available

Renaming, deleting, creating folders and moving files only work outside the IDE (Windows Explorer, Terminal tab). Afterwards use *Refresh project*.
