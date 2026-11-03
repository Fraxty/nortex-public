# Kapitel 5 – Operatoren

← [Kapitel 4](04-data-types.md) · [Inhalt](README.md) · Weiter: [Kapitel 6](06-control-flow.md)

## Übersicht

| Kategorie | Operatoren | Nativ |
|---|---|---|
| Arithmetisch | `+ - * / %` (binär), `-` (unär) | ja |
| Vergleich | `== != < > <= >=` | ja |
| Logisch | `&& \|\| !` | ja (kurzschließend) |
| Bitweise | `& \| ^ << >>` | ja |
| Bitweises NOT | `~` | nein (`NX2002`) |
| Zuweisung | `=` `+= -= *= /= %=` | ja |
| Weitere Zuweisungen | `&= \|= ^= <<= >>=` | Frontend |
| Inkrement/Dekrement | `++ --` (prä/post) | nein (`NX2002`); stattdessen `a += 1` |
| Ternär | `bedingung ? a : b` | ja |
| Sonstige Token | `. , ; : :: -> => ( ) [ ] { }` | Syntax |

## Rangfolge (hoch → niedrig), Assoziativität

| Stufe | Operatoren | Assoziativität |
|---|---|---|
| 11 | `* / %` | links |
| 10 | `+ -` | links |
| 9 | `<< >>` | links |
| 8 | `< > <= >=` | links |
| 7 | `== !=` | links |
| 6 | `&` | links |
| 5 | `^` | links |
| 4 | `\|` | links |
| 3 | `&&` | links |
| 2 | `\|\|` | links |
| 2/1 | `?:` (Ternär) | rechts |
| 1 | `=` und zusammengesetzte Zuweisungen | rechts |

Unäre Operatoren (`- ! ~`) binden stärker als alle binären. Quelle: Parser (`compiler/src/parser/mod.rs`).

## Verhalten

```nortex
public int main() {
    int a = 17;
    return a % 5 + (-a) / 4;     // 2 + (-4) = -2 (Division schneidet Richtung 0 ab)
}
```

- Ganzzahldivision rundet zur Null: `-7 / 2` ergibt `-3`.
- `&&`/`||` werten rechts nur bei Bedarf aus: `a != 0 && 10 / a > 1` ist sicher bei `a == 0`.
- **Overflow:** Standard ist Trap. `add(2147483647, 1)` beendet das Programm mit Exitcode `-1073741795` (`0xC000001D`). Mit `--overflow-checks=false` wird umlaufend gerechnet. Division durch 0 trappt ebenfalls.

## Typkompatibilität

Beide Operanden eines arithmetischen/Vergleichsoperators müssen denselben numerischen Typ haben (`NX1102`). Bedingungen müssen `bool` sein. Überladbar sind nur `== != < > <= >=` (static, zwei Parameter, Rückgabe `bool`; nur Frontend).

## Häufige Fehler

- `a++` im nativen Code → `NX2002 Operator 'post++' wird im aktuellen MIR-Subset nicht unterstützt`.
- Mischen von `int` und `uint`/`long` ohne Cast-Möglichkeit.

## Operatorüberladung

Überladbar sind ausschließlich `==`, `!=`, `<`, `>`, `<=`, `>=`. Jede Überladung ist `static`, hat genau zwei Parameter und liefert `bool`:

```nortex
public class Vector3
{
    public static bool operator ==(Vector3 left, Vector3 right)
    {
        return left.x == right.x && left.y == right.y && left.z == right.z;
    }
}
```

Das Frontend prüft Operatorliste, `static`, Parameterzahl und `bool`-Rückgabe. **Geplant:** Prüfung der Parameterbeziehung zum definierenden Typ und der Konsistenz (`==`/`!=`, sinnvolle Ordnung); der Compiler kann mathematische Konsistenz beliebiger Implementierungen nicht beweisen.

Nicht überladbar: `+ - * / %`, `& | ^ ~`, `! && ||`, `<< >>`, `++ --`, Indexierung, Aufruf, Memberzugriff, Zuweisung und eigene Operatorzeichen. `&&` und `||` behalten Kurzschlussauswertung. `?` gehört nur zum Ternäroperator `c ? a : b`; einen `?`-Fehlerfortleitungsoperator gibt es nicht.
