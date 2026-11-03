# 11 First steps

[Back](10-keyboard-shortcuts.md) · [Contents](README.md) · [Next: Efficient workflows](12-efficient-workflows.md)

Goal: write a program, build it, fix an error and save.

1. **Start**: double-click `nortex-code.exe`. The dashboard appears.
2. **Choose the project folder**: Activity bar → *Explorer*, enter the folder path in the path field (e.g. `C:\Nortex\first`), *Open*. The folder must exist; the IDE cannot create a "new project".
3. **New file**: Activity bar → *Editor*, click **+** in the tab strip. With Ctrl+S or *File → Save as…* choose the desired path and file name (extension `.nx`).
4. **Write the program**:

```nortex
public int main() {
    int total = 2 + 3;
    return total;
}
```

5. **Save**: Ctrl+S. Status bar: "File saved".
6. **Build and run**: Ctrl+F5 (or **▶ Run**). *Runtime Output* shows "Program finished (exit code Some(5))". The return value of `main` is the exit code.
7. **Provoke an error**: change `return total;` to `return totall;`. The editor marks the spot; Ctrl+F5 gives `NX1004 … Unknown identifier 'totall'` in the *Compiler Log*, status bar red.
8. **Fix it**: back to `total`, Ctrl+F5 → exit code 5 again, status bar green.
9. **Add output** (optional; one string value per call):

```nortex
using System;

public int main() {
    Console.writeLine("Hello, NORTEX!");
    return 0;
}
```

   Runtime Output: `Hello, NORTEX!`.
10. **Save**: Ctrl+S. The dot • in the tab disappears as soon as everything is saved.

More runnable examples: `examples\language\` ([Language reference](../../language/README.md)).
