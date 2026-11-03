# Kapitel 7 – Funktionen und Methoden

← [Kapitel 6](06-control-flow.md) · [Inhalt](README.md) · Weiter: [Kapitel 8](08-structs-and-data-structures.md)

## Funktionsdefinition

```nortex
[Modifier] Rückgabetyp name(Typ param1, Typ param2) { Rumpf }
```

```nortex
public int add3(int a, int b, int c) {
    return a + b + c;
}
```

- Modifier: `public`, `private`, `static` sind nativ getestet (`private int helper()`, `public static int twice(int n)`).
- Parameter sind **unveränderlich** (`a = a + 1;` → `NX1200`); `mut`-Parameter existieren nicht (`NX0100 Typname erwartet`).
- Jede Funktion mit Nicht-`void`-Rückgabetyp braucht einen Rumpf und auf allen Pfaden ein `return` (`NX1103`). Eine Signatur ohne Rumpf → `NX1005`.

## Aufruf, Rekursion

Aufrufe sind nativ nur **direkte Aufrufe freier, nicht-generischer Funktionen**. Reihenfolge der Definitionen ist egal (wechselseitige Rekursion `even`/`odd` getestet).

```nortex
public int fib(int n) {
    if (n < 2) { return n; }
    return fib(n - 1) + fib(n - 2);
}
```

Falsche Argumentzahl: `NX1107 Keine Überladung von 'f' akzeptiert 2 Argumente. Kandidaten: f(int).` Unbekannte Funktion: `NX1004`.

## Rückgabewerte

Genau ein Wert über `return`. String-Literale können lokalen Variablen zugewiesen, als Parameter übergeben und von internen Funktionen zurückgegeben werden. `void`-Funktionen sind im nativen Build **nicht** aufrufbar, außer `Console.write/writeLine` (`NX2002 Void-Aufrufe sind im MIR-Subset nur für Console.write und Console.writeLine verfügbar`). String-Parameter/Rückgaben sowie `bool`, `int`, `uint` werden nativ unterstützt; String-ABI in `extern "C"` ist nicht unterstützt.

## Lokale Variablen

Gelten ab Deklaration bis Blockende, sind pro Aufruf getrennt ([Kapitel 4](04-data-types.md#gültigkeitsbereiche)).

## Methoden und Sichtbarkeit (nur Frontend)

Methoden stehen in `class`/`struct`; `public`/`private`/`protected`/`internal` werden geparst. Aufruf `c.inc()` besteht `check`, der Build lehnt Klassen/`new` ab (`NX2001`; siehe [Kapitel 8](08-structs-and-data-structures.md)). Konstruktoren heißen wie der Typ; die Signatur- und Überladungsprüfung fehlt.

## Einschränkungen

Generische Instanzmethoden in Klassen werden nativ monomorphisiert, wenn ein Aufruf konkrete Typargumente angibt (z. B. `converter.echo<int>(42)`). Generische freie Funktionen, implizite Typargument-Inferenz, Bounds, Lambdas, Überladungsauflösung nativ, Funktionszeiger und Standardparameter fehlen.

## Entwurfsregeln und Frontend-Stand

- Die C#-ähnliche Deklaration (`public int add(int a, int b)`) ist die einzige Form; `fn name(...) -> T` gehört nicht zur Sprache. Konstruktoren heißen wie der Typ und haben keinen Rückgabetyp.
- Generische Parameter stehen hinter dem Namen: `public T identity<T>(T value)`.
- Überladungen unterscheiden sich durch Parametertypen, nicht allein durch den Rückgabetyp. Unsuffigierte Integer-Literale bevorzugen bei sonst gültigen Überladungen ganzzahlige vor Fließkomma-Parametern; gleichrangige Kandidaten bleiben mehrdeutig (`NX1107`).
- Das Frontend erkennt fehlende Rückgabepfade, generische Methodenaufrufe und Typargument-Substitution in einfachen Signaturen; ein nicht-statischer Methodenaufruf über den Typnamen wird abgelehnt.
- **Geplant:** vollständige generische Inferenz, Constraint-Prüfung, modulübergreifende Auflösung, generische freie Funktionen.
