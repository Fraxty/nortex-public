# Kapitel 13 – Praktische Beispiele

← [Kapitel 12](12-compiler-architecture.md) · [Inhalt](README.md) · Weiter: [Kapitel 14](14-language-reference.md)

Alle Dateien liegen in [`examples/language/`](../../examples/language/). Jedes Beispiel wurde mit `nortexc build` gebaut und ausgeführt (Ausnahmen: 07/08, nur `check`). Aufruf: `nortexc build examples\language\NN_name.nx -o out.exe; .\out.exe; $LASTEXITCODE`.

## 1. Hello World – [01_hello.nx](../../examples/language/01_hello.nx)

```nortex
using System;

public int main() {
    Console.writeLine("Hallo, NORTEX!");
    return 0;
}
```

Gibt `Hallo, NORTEX!` aus, Exitcode `0`. `main` ist der Einstieg, der `return`-Wert der Exitcode.

## 2. Variablen und Berechnungen – [02_variables.nx](../../examples/language/02_variables.nx)

```nortex
public int main() {
    int width = 6;
    int height = 7;
    int area = width * height;
    mut int total = area;
    total += 8;
    total -= 3;
    const int OFFSET = 3;
    return total + OFFSET;
}
```

Unveränderliche Werte per Default, `mut` für `total`. Rechnung: 42 → 50 → 47 → 50. **Exitcode 50.**

## 3. Bedingungen – [03_conditions.nx](../../examples/language/03_conditions.nx)

```nortex
public int classify(int score) {
    if (score >= 90) {
        return 1;
    } else if (score >= 50) {
        return 2;
    }
    return 3;
}

public int main() {
    Console.writeLine("Klassifiziere den Wert 72");
    return classify(72);
}
```

72 erfüllt nur die zweite Bedingung. Ausgabe `Klassifiziere den Wert 72`, **Exitcode 2.**

## 4. Schleifen – [04_loops.nx](../../examples/language/04_loops.nx)

```nortex
public int main() {
    mut int sum = 0;
    for (mut int i = 1; i <= 10; i += 1) {
        if (i % 2 != 0) {
            continue;
        }
        sum += i;
    }

    mut int n = 0;
    while (true) {
        n += 1;
        if (n == 5) {
            break;
        }
    }
    return sum + n;
}
```

`for` summiert gerade Zahlen (2+4+6+8+10 = 30), `continue` überspringt ungerade; `while (true)` endet per `break` bei 5. **Exitcode 35.**

## 5. Funktionen und Rekursion – [05_functions.nx](../../examples/language/05_functions.nx)

```nortex
public int gcd(int a, int b) {
    if (b == 0) {
        return a;
    }
    return gcd(b, a % b);
}

public int fib(int n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

public int main() {
    return gcd(48, 18) + fib(10);
}
```

`gcd(48,18) = 6`, `fib(10) = 55`. **Exitcode 61.**

## 6. Vollständige Anwendung: Primzahlen – [06_primes.nx](../../examples/language/06_primes.nx)

```nortex
using System;

public bool isPrime(int n) {
    if (n < 2) {
        return false;
    }
    mut int d = 2;
    while (d * d <= n) {
        if (n % d == 0) {
            return false;
        }
        d += 1;
    }
    return true;
}

public int countPrimes(int limit) {
    mut int count = 0;
    for (mut int n = 2; n <= limit; n += 1) {
        if (isPrime(n)) {
            count += 1;
        }
    }
    return count;
}

public int main() {
    Console.writeLine("Zaehle Primzahlen bis 100");
    return countPrimes(100);
}
```

Kombiniert `bool`-Funktionen, Schleifen, frühe Rückgaben, Aufrufketten und Konsolenausgabe. Es gibt 25 Primzahlen ≤ 100. **Exitcode 25.**

## 7. Structs – [07_struct_frontend.nx](../../examples/language/07_struct_frontend.nx) (nur `check`)

```nortex
public struct Point {
    public int x;
    public int y;
}

public int main() {
    mut Point p = new Point();
    p.x = 3;
    p.y = 4;
    return p.x + p.y;
}
```

`nortexc check` erfolgreich; `nortexc build` → `NX2001`. Zeigt die vorgesehene Syntax, ist aber noch nicht ausführbar.

## 8. Methoden – [08_class_frontend.nx](../../examples/language/08_class_frontend.nx) (nur `check`)

Klasse `Counter` mit `inc()`/`get()`; gleicher Status wie Beispiel 7 (siehe [Kapitel 8](08-structs-and-data-structures.md)).

## 9. Heap-Allokation

Nicht ausführbar: `new` erzeugt im nativen Build immer `NX2001`. Es gibt daher kein lauffähiges Heap-Beispiel ([Kapitel 9](09-memory-management.md)).
