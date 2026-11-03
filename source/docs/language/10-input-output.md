# Kapitel 10 – Ein- und Ausgabe

← [Kapitel 9](09-memory-management.md) · [Inhalt](README.md) · Weiter: [Kapitel 11](11-errors-and-debugging.md)

## Konsolenausgabe

```nortex
Console.write("Text ohne Zeilenumbruch");
Console.writeLine("Text mit Zeilenumbruch");
```

Beide Aufrufe akzeptieren im nativen Build **genau einen Wert vom Typ `string`**. String-Literale und String-Variablen können ausgegeben werden. Die Runtime erhält den UTF-8-Datenzeiger und die Byte-Länge über einen ABI-Aufruf; dies ist keine allgemeine C-ABI-String-Anbindung.

```nortex
using System;

public int main()
{
    string greeting = "Hallo";
    Console.write(greeting);
    Console.writeLine(" NORTEX");
    return 0;
}
```

Interne Stringwerte verwenden einen 16-Byte-Slice-Deskriptor (Datenzeiger und Byte-Länge), der als Adresse gemäß Windows-x64-ABI übergeben wird. String-Literalbytes und Deskriptoren liegen schreibgeschützt im COFF-Datenbereich. Leere Strings und eingebettete NUL-Bytes werden anhand der Länge korrekt behandelt.

```nortex
using System;

public int main() {
    Console.write("a");
    Console.writeLine("b\tc \"zitiert\" \\");
    return 0;
}
```

Ausgabe: `ab`⇥`c "zitiert" \`. Escapes: `\n \r \t \0 \" \\`. Ein leerer String ist erlaubt. Unicode im Literal wird als UTF-8-Bytes ausgegeben; die korrekte Darstellung hängt von der Konsolen-Codepage ab (nicht systematisch geprüft).

## Nicht unterstützt (jeweils `NX2001`/`NX2002`)

| Versuch | Meldung |
|---|---|
| `Console.writeLine(5);` | Argument muss vom Typ `string` sein |
| `Console.readLine()` | nur direkte freie Funktionen und Console-Ausgabe |

## String-Konkatenation

`Console.writeLine("Hallo " + name);` ist erlaubt: `+` zwischen zwei `string`-Werten erzeugt über `nortex_string_concat` (Windows `HeapAlloc`) einen neuen Heap-String. Dieser wird beim Scope-Ende über `nortex_string_free` freigegeben (Drop im Codegen, siehe [04-data-types.md](04-data-types.md)). Top-Level-Strings (`const string`/`mut string`) und `extern "C"`-Aufrufe mit `string`-Argument (`char*`, NUL-terminiert) sind ebenfalls möglich, siehe [04-data-types.md](04-data-types.md).

Konsoleneingabe, Formatierung und Interpolation sind **nicht implementiert**. Fehlerausgabe (stderr) hat keine eigene API.

## Ergebniswerte als Ausgabekanal

Weil Zahlen nicht ausgegeben werden können, liefern Beispielprogramme Ergebnisse als **Exitcode** (`return countPrimes(100);` → `25`). In PowerShell: `$LASTEXITCODE`.

## Standardbibliothek

`standard-library/system/console.nx` und `io/mod.nx` sind Entwürfe ohne Compilerbindung.

## Strings (Zielentwurf)

`string` ist UTF-8 und besitzt seinen Speicher. `length()` zählt Bytes (nicht Skalarwerte oder Graphemcluster); Indexzugriff ist byte-orientiert und liefert `byte`; `chars()` iteriert Unicode-Skalarwerte (`char`, 32 Bit), `bytes()` die UTF-8-Bytes. Strings müssen gültiges UTF-8 bleiben. Vorgesehene API: `length()`, `isEmpty()`, `contains()`, `startsWith()`, `endsWith()`, `replace()`, `split()`, `trim()`, `toUpperCase()`, `toLowerCase()`, `chars()`, `bytes()`. Ein öffentlicher `StringBuilder` ist nicht vorgesehen. Der Typchecker akzeptiert `string + string` und `string +` skalare Primitive (Ergebnis `string`); die Formatierung ist noch nicht End-to-End verfügbar.
