# Eingebaute Funktionen (nativer Compiler)

`nortexc build` löst diese Aufrufe direkt zu Runtime-Funktionen auf. Quelle der Wahrheit ist die Tabelle in `compiler/src/builtins.rs`, die Implementierung steht in `runtime/src/text.rs`. Argumenttypen werden geprüft; Strings werden nur ausgeliehen, sie bleiben also nach dem Aufruf nutzbar.

Längen und Indizes zählen UTF-8-Bytes, nicht Zeichen.

| Aufruf | Ergebnis |
|---|---|
| `Text.length(s)` | `int`, Byte-Länge |
| `Text.substring(s, start, count)` | `string`; Werte außerhalb werden begrenzt |
| `Text.indexOf(s, teil)` | `int`, -1 wenn nicht gefunden |
| `Text.contains / startsWith / endsWith(s, teil)` | `bool` |
| `Text.charAt(s, i)` | `int`, Bytewert oder -1 |
| `Text.trim / toUpper / toLower(s)` | `string` (Unicode-fähig bei gültigem UTF-8) |
| `Text.replace(s, von, nach)` | `string`; leeres `von` ändert nichts |
| `Convert.toString(n)` / `Convert.boolToString(b)` | `string` |
| `Convert.toInt(s)` | `int`, 0 bei ungültiger Zahl |
| `Convert.isInt(s)` | `bool`, prüft `Convert.toInt` vorab |
| `File.readAllText(pfad)` | `string`, leer bei Fehler |
| `File.writeAllText / appendAllText(pfad, text)` | `bool`, Erfolg |
| `File.exists / delete(pfad)` | `bool` |
| `Directory.exists / create(pfad)` | `bool`; `create` legt Elternordner an |
| `Console.readLine()` | `string`, Zeile ohne Zeilenende, leer am Dateiende |
| `Convert.toDouble(n)` / `Convert.doubleToInt(d)` | `double` / `int`; `doubleToInt` schneidet ab, sättigt, NaN ergibt 0 |
| `Convert.doubleToString(d)` | `string`, kürzeste Rundreise-Darstellung, ganze Zahlen mit `.0` (`2.0`) |
| `Convert.parseDouble(s)` / `Convert.isDouble(s)` | `double` (0.0 bei Fehler, `inf`/`NaN` zählen als ungültig) / `bool` |
| `Math.sqrt / abs / floor(d)`, `Math.pow(b, e)` | `double` |
| `IntList.create()` | `IntList`, leere Liste |
| `IntList.add(l, v)` / `IntList.set(l, i, v)` | `bool`, `false` bei ungültigem Index |
| `IntList.get(l, i)` / `IntList.length(l)` | `int`; `get` liefert 0 bei ungültigem Index |
| `StringList.create/add/set/get/length` | wie `IntList`; `add`/`set` kopieren den String, `get` liefert eine neue Kopie (`""` bei ungültigem Index) |
| `IntList.free(l)` / `StringList.free(l)` | `bool`; gibt die Liste frei, danach ist das Handle ungültig |
| `Text.split(s, sep)` | `StringList`; leeres `sep` ergibt den ganzen Text als einziges Element |
| `Text.join(l, sep)` | `string` |

`double` (f64): Literale, Variablen, Parameter, Rückgabewerte, `+ - * /`, Vergleiche und unäres `-`. Es gibt keine implizite `int`→`double`-Umwandlung (außer bei Ganzzahlliterale in `double`-Kontext); `%` ist für `double` nicht unterstützt. `double`-Globals werden nicht unterstützt.

`IntList`/`StringList` sind undurchsichtige Handles (i64) und frei kopierbar. Sie werden **nicht** automatisch freigegeben: ohne `free` bleibt die Liste bis Prozessende bestehen. Zugriff nach `free` ist undefiniert. Von `StringList.get`/`Text.join` gelieferte Strings gehören dem Aufrufer und werden wie andere Heap-Strings am Scope-Ende freigegeben.

Beispiel:

```nortex
using System;

public int main() {
    string path = "notiz.txt";
    File.writeAllText(path, "  Hallo NORTEX  ");
    string text = Text.trim(File.readAllText(path));
    Console.writeLine(Text.toUpper(text));
    Console.writeLine(Convert.toString(Text.length(text)));
    File.delete(path);
    return 0;
}
```

Grenzen: kein `string`-Indexzugriff, keine Dictionaries, keine Klassen im nativen Backend; Fehler werden über Rückgabewerte gemeldet, nicht über Ausnahmen.
