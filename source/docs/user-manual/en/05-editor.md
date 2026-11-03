# 5 Working with the editor

[Back](04-projects-and-files.md) · [Contents](README.md) · [Next: Build and run](06-build-and-run.md)

## Basic features

- **Syntax highlighting** for keywords, identifiers, numbers, strings, characters, operators, punctuation and comments (colors depend on the theme).
- **Line numbers** in the left column.
- **Undo/Redo**: Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z (also in the *Edit* menu).
- Errors are marked directly in the editor (inline diagnostics, updated shortly after typing) and listed in the Compiler Log ([Chapter 8](08-debugging.md)).
- **There is no bracket completion and no multi-cursor.**

## Auto-completion

From **2 characters** of an identifier a popup opens with up to **5 suggestions**: keywords first, then identifiers from the current document.

| Key | Effect |
|---|---|
| Arrow up/down | Move selection (wraps around) |
| Tab or Enter | Accept suggestion |
| Esc | Close popup |

## Line commands

| Command | Key |
|---|---|
| Toggle comment | Ctrl+/ |
| Move line(s) up/down | Alt+↑ / Alt+↓ |
| Duplicate line(s) | Shift+Alt+↓ |
| Select next occurrence of the selected text | Ctrl+D |

## Find and replace

**Ctrl+F** opens a floating field at the top right of the editor, **Ctrl+H** additionally with "Replace with". If text is selected it is taken over.

| Control | Function |
|---|---|
| Search field | Matches are searched live |
| Switches *Aa* / *W* / *.\** | Match case · Whole word · Regular expressions (with capture groups in the replacement text) |
| ▲ / ▼ or Enter / Shift+Enter | previous / next match |
| *Replace* / *All* | replace current / all matches |
| × or Esc | close |

An invalid regular expression is shown as an error without crashing the IDE. Search across files: **Search** side bar (Ctrl+Shift+F): enter a term, *Search* or Enter; clicking a match opens the file (simple text search, no line highlighting).

## Tabs and split editor

Several files live in tabs; **Ctrl+Tab** switches to the next tab. Via *View → Editor layout* (or keyboard) the editor can be split:

| Layout | Key |
|---|---|
| Two columns | Ctrl+\ (also Ctrl+Shift+2) |
| Two rows | Ctrl+Shift+\ (also Ctrl+Shift+3) |
| 2×2 grid | Ctrl+Shift+4 |
| Single | *View → Editor layout → Single* |

Focus: Ctrl+1 … Ctrl+4 selects the pane; Ctrl+Alt+Arrow or Alt+←/→ moves in that direction; Ctrl+Shift+W closes the active pane.

## Navigation

There is no "Go to definition". Use: Ctrl+F, project search, tabs, Ctrl+P (open file), and *Edit → Go to line…*.

## Rename, breadcrumbs and minimap

- **F2** opens an input field at the cursor with the identifier under the cursor. Enter renames all occurrences in the current file, Esc cancels. Comments, strings and keywords are untouched; ASCII names only, no project-wide rename.
- **Breadcrumbs** above the editor show the file and enclosing scopes (e.g. struct and function). A click jumps to the start line of the scope.
- **Minimap** at the right edge (60 px); a click scrolls. Toggle with **Ctrl+Alt+M**, via the View menu or the command palette; stored as editor.minimap.enabled.
