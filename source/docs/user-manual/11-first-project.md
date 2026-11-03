# 11 Praktischer Einstieg

[Zurück](10-keyboard-shortcuts.md) · [Inhaltsverzeichnis](README.md) · [Weiter: Effizientes Arbeiten](12-efficient-workflows.md)

Ziel: ein Programm schreiben, bauen, einen Fehler beheben und speichern.

1. **Starten**: `nortex-code.exe` doppelklicken. Es erscheint das Dashboard.
2. **Projektordner wählen**: Activity Bar → *Explorer*, Ordnerpfad in das Pfadfeld eingeben (z. B. `C:\Nortex\erstes`), *Öffnen*. Der Ordner muss existieren; ein „Neues Projekt“ kann die IDE nicht anlegen.
3. **Neue Datei**: Activity Bar → *Editor*, im Tab-Streifen **+** klicken. Mit Strg+S oder *Datei → Speichern unter…* den gewünschten Pfad und Dateinamen wählen (Endung `.nx`).
4. **Programm schreiben**:

```nortex
public int main() {
    int summe = 2 + 3;
    return summe;
}
```

5. **Speichern**: Strg+S. Statusleiste: „Datei gespeichert“.
6. **Bauen und ausführen**: Strg+F5 (oder **▶ Run**). Im *Runtime Output* steht „Programm beendet (Exitcode Some(5))“. Der Rückgabewert von `main` ist der Exitcode.
7. **Fehler provozieren**: `return summe;` zu `return summee;` ändern. Der Editor markiert die Stelle; Strg+F5 ergibt im *Compiler Log* `NX1004 … Unbekannter Bezeichner 'summee'`, Statusleiste rot.
8. **Beheben**: zurück zu `summe`, Strg+F5 → wieder Exitcode 5, Statusleiste grün.
9. **Ausgabe ergänzen** (optional; ein Stringwert je Aufruf):

```nortex
using System;

public int main() {
    Console.writeLine("Hallo, NORTEX!");
    return 0;
}
```

   Runtime Output: `Hallo, NORTEX!`.
10. **Speichern**: Strg+S. Der Punkt • im Tab verschwindet, sobald alles gespeichert ist.

Weitere lauffähige Beispiele: `examples\language\` ([Sprachreferenz](../language/README.md)).
