# Kapitel 8 – Structs und Datenstrukturen

← [Kapitel 7](07-functions-and-methods.md) · [Inhalt](README.md) · Weiter: [Kapitel 9](09-memory-management.md)

> **Nativer Status:** Klasseninstanzen werden als Heap-Zeiger erzeugt. Skalare, `string`- und Klassenreferenz-Felder, Feldinitialisierer, Konstruktoren, Instanz-/statische Methoden sowie Objektübergabe, -rückgabe und -aliasierung sind unterstützt. Einzelvererbung mit Parameterlos-Aufruf des Basiskonstruktors und dynamischer Dispatch für `virtual`/`override` werden unterstützt. Geschlossene generische Klassen sowie generische Instanzmethoden mit expliziten konkreten Typargumenten werden monomorphisiert. Die Runtime gibt die Arena bei normaler Rückkehr aus generiertem `main` frei; Details und Grenzen stehen in [Kapitel 9](09-memory-management.md).

## Struct-Definition (Frontend)

```nortex
public struct Point {
    public int x;
    public int y;
}
```

Felder besitzen Typ und Sichtbarkeit; `public int value = 0;` erlaubt einen Initialwert (Klassenfeld, getestet).

`struct`, `record`, `interface`, `trait` und `enum` sind weiterhin nicht Teil des nativen Klassen-Backends. Die folgenden Beispiele zur Werttyp-Syntax dokumentieren den Parser-/Sprachentwurf.

## Initialisierung und Zugriff

```nortex
mut Point p = new Point();
p.x = 3;
p.y = 4;
```

Eine Zuweisung an ein Feld einer **unveränderlichen** Bindung wird abgelehnt: `NX1201 Die Veränderung über ein Member oder einen Index erfordert einen exklusiven &mut-Borrow`. Deshalb `mut Point p`.

## Klassen mit Methoden

```nortex
public class Counter {
    public int value = 0;
    public void inc() { value = value + 1; }
    public int get() { return value; }
}
```

Konstruktoren führen Feldinitialisierer aus; `this.field = parameter` und implizite Feldzugriffe werden unterstützt. Objektvariablen sind Referenzwerte: Zuweisung und Übergabe teilen dieselbe Instanz. Ein `mut`-Binding ist zum Ersetzen einer lokalen Referenz erforderlich.

## Weitere Deklarationen

Geschlossene generische Klassen wie `Container<int>` werden für konkrete `new`-Instanziierungen monomorphisiert. Generische Instanzmethoden können mit expliziten konkreten Typargumenten aufgerufen werden, etwa `converter.echo<int>(42)`; generische freie Funktionen, implizite Typargument-Inferenz und Bounds sind nicht Teil dieses nativen Pfads. Einzelvererbung verwendet abgeflachte Feldlayouts; ein abgeleiteter Konstruktor ruft zuerst den parameterlosen Basiskonstruktor auf. Basiskonstruktoren mit Parametern, Mehrfachvererbung, Interfaces/Traits, Überladung und mehrere Konstruktoren werden nicht unterstützt. Virtuelle Methoden müssen mit `virtual` deklariert und mit `override` überschrieben werden. `record`, `interface`, `trait` und `enum` sind weiterhin Frontend-/Entwurfssyntax.

## Arrays und Sammlungen

Array-Typen `T[]`, `[T; N]` werden in Signaturen geparst; Array-Erzeugung `new T[n]` fehlt (`NX0100`). Nativ sind Arrays nicht erlaubt (`NX2002 Parametertyp 'int[]' liegt außerhalb des skalaren MIR-Subsets`). Collections liegen nur als `.nx`-Entwürfe in `standard-library/collections/` vor.

## Verschachtelung

Verschachtelte Strukturen sind im Typmodell vorgesehen (`type_system/struct_layout.rs` berechnet Layouts), aber nicht ausführbar.

## Weitere noch nicht nativ unterstützte Typsystemfunktionen

Der native Klassenpfad ist oben beschrieben. Weiterhin fehlen:

- **Structs:** Werttyp, Move als Standard und geprüfte `Copy`-Semantik; Heap-Boxen wie `Box<T>` sind nicht implementiert.
- **Records:** unveränderliche Werttypen mit struktureller Gleichheit.
- **Interfaces und Traits:** Vertragsprüfung und Implementierung fehlen; Mehrfachvererbung und mehrere Interface-Basistypen sind nicht unterstützt.
- **Generics:** generische freie Funktionen, implizite Typargument-Inferenz und Bounds wie `where T: Drawable`. Klassen und Instanzmethoden benötigen explizite konkrete Typargumente; siehe oben.
- **Collections** (`System.Collections`, nur API-Signaturen in `standard-library/collections/*.nx`): `List<T>`, `Vec<T>`, `Dictionary<K,V>`, `HashSet<T>`, `Queue<T>` und `Stack<T>` sind noch keine nativen Laufzeittypen; Speicherverwaltung, Hashing und Iteratoren fehlen.
