# 8 Error analysis and debugging

[Back](07-terminal-and-consoles.md) · [Contents](README.md) · [Next: Settings](09-settings-and-customization.md)

## Recognizing errors

- The editor checks the text shortly after typing and marks errors directly in the code.
- The **Compiler Log (n)** tab lists all diagnostics; the status bar shows "n errors".
- A red dot **●** in the active tab means: the file has errors.
- After a build the status message is red/green.

## Reading diagnostics

Format in the Compiler Log: `Code [file:line:column] message`, below it optionally `Help: …`. Red = error, amber = warning.

Example (typo `summee`, line 3, column 12):

```
NX1004 [main.nx:3:12] Unknown identifier 'summee'. The value was neither declared nor imported.
  Help: Check the spelling or declare the value before using it.
```

Fix: correct the name (`summe`). A missing semicolon is reported as `NX0100` ("A ';' is expected after a variable declaration."); the position points to the next token, so the semicolon is usually missing on the line **before**.

Meaning of codes: [Diagnostic codes](../../compiler/diagnostic-codes.md) and [Language reference, chapter 11](../../language/11-errors-and-debugging.md). Compiler messages themselves are currently in German.

## Common problems

| Message | Cause / solution |
|---|---|
| NX2001 / NX2002 | Construct is checked but not generated natively yet (e.g. `float`, `string` variables, structs). Restrict yourself to the supported subset |
| Linker error LNK2019 | `public int main()` is missing |
| "Compiler … could not be started" | `nortexc.exe` not found ([Chapter 2](02-installation.md)) |
| "Build failed (exit code …)" | The errors are in the Compiler Log above |

## Debugging features

**Real debugging (pausing, stepping, inspecting variables) is not available.** Reason: `nortexc` does not emit debug information (e.g. line tables). Without it the IDE cannot stop a running program at a source line. The IDE says so openly instead of faking a session.

Breakpoint management and its user interface exist:

| Feature | Status |
|---|---|
| **Set/remove breakpoint** | **F9** on the cursor line. The file must be saved (new documents without a path: notice in the status bar). The status bar reports "Breakpoint set/removed at line n". |
| **Marker in the editor** | A **●** in the column left of the line number marks lines with a breakpoint. |
| **Breakpoint list** | Debugger panel (Activity bar → Debugger or Ctrl+Shift+D): "Breakpoints (n)" with `file:line`. A click jumps to the line, **×** removes an entry, *Remove all* clears the list. |
| **Call stack / variables** | The panel contains the sections *Call stack* and *Variables*. They show "No active session." because no debugger is connected yet. |
| **F5 / ▶ Debug** | Opens the Debugger panel and reports that step-by-step debugging is unavailable. The session stays inactive. |
| **■ Stop** | Ends the (non-existent) session state. |
| **Shift+F5** | Reports "Cancelling running programs is not implemented." |

Breakpoints are only held in the running IDE and are not saved; after a restart they are gone. They have no effect on **Ctrl+F5** (Build & Run).

Troubleshooting currently works through diagnostics, program output and the exit code (e.g. `return` of an intermediate result, see [Chapter 11](11-first-project.md)).
