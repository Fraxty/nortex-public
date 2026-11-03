# Chapter 13 – Practical Examples

← [Chapter 12](12-compiler-architecture.md) · [Contents](README.md) · Next: [Chapter 14](14-language-reference.md)

All files are in [`examples/language/`](../../../examples/language/). Each example was built and run with `nortexc build` (except 07/08, which were checked with `check` only). Invocation: `nortexc build examples\language\NN_name.nx -o out.exe; .\out.exe; $LASTEXITCODE`.

## 1. Hello World – [01_hello.nx](../../../examples/language/01_hello.nx)

```nortex
using System;

public int main() {
    Console.writeLine("Hello, NORTEX!");
    return 0;
}
```

Prints `Hello, NORTEX!`, exit code `0`. `main` is the entry point; the `return` value is the exit code.

## 2. Variables and calculations – [02_variables.nx](../../../examples/language/02_variables.nx)

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

Values are immutable by default; `mut` makes `total` mutable. Calculation: 42 → 50 → 47 → 50. **Exit code 50.**

## 3. Conditions – [03_conditions.nx](../../../examples/language/03_conditions.nx)

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
    Console.writeLine("Classify the value 72");
    return classify(72);
}
```

72 satisfies only the second condition. Output: `Classify the value 72`, **exit code 2.**

## 4. Loops – [04_loops.nx](../../../examples/language/04_loops.nx)

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

`for` sums the even numbers (2+4+6+8+10 = 30); `continue` skips odd numbers; `while (true)` ends at 5 via `break`. **Exit code 35.**

## 5. Functions and recursion – [05_functions.nx](../../../examples/language/05_functions.nx)

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

`gcd(48,18) = 6`, `fib(10) = 55`. **Exit code 61.**

## 6. Complete application: prime numbers – [06_primes.nx](../../../examples/language/06_primes.nx)

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
    Console.writeLine("Count primes up to 100");
    return countPrimes(100);
}
```

Combines `bool` functions, loops, early returns, call chains, and console output. There are 25 primes ≤ 100. **Exit code 25.**

## 7. Structs – [07_struct_frontend.nx](../../../examples/language/07_struct_frontend.nx) (`check` only)

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

`nortexc check` succeeds; `nortexc build` → `NX2001`. Shows the intended syntax, but is not yet executable.

## 8. Methods – [08_class_frontend.nx](../../../examples/language/08_class_frontend.nx) (`check` only)

Class `Counter` with `inc()`/`get()`; same status as Example 7 (see [Chapter 8](08-structs-and-data-structures.md)).

## 9. Heap allocation

Not executable: `new` always produces `NX2001` in native builds. Therefore, there is no runnable heap example ([Chapter 9](09-memory-management.md)).
