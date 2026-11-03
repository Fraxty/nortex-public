# Chapter 8 – Structs and Data Structures

← [Chapter 7](07-functions-and-methods.md) · [Contents](README.md) · Next: [Chapter 9](09-memory-management.md)

> **Native status:** class instances are heap pointers. Scalar, `string`, and class-reference fields, field initializers, constructors, instance/static methods, and object passing, returning, and aliasing are supported. Single inheritance (with a parameterless base constructor) and dynamic dispatch for `virtual`/`override` are supported. Closed generic classes and generic instance methods with explicit concrete type arguments are monomorphized. The runtime frees its arena on normal return from generated `main`; see [Chapter 9](09-memory-management.md) for the exact boundary.

## Struct definition (frontend)

```nortex
public struct Point {
    public int x;
    public int y;
}
```

Fields have a type and visibility; `public int value = 0;` allows an initial value (class field, tested).

`struct`, `record`, `interface`, `trait`, and `enum` are not yet part of the native class backend. The examples below describe parser/language-design syntax.

## Initialization and access

```nortex
mut Point p = new Point();
p.x = 3;
p.y = 4;
```

Assignment to a field of an **immutable** binding is rejected: `NX1201 Die Veränderung über ein Member oder einen Index erfordert einen exklusiven &mut-Borrow`. Therefore use `mut Point p`.

## Classes with methods

```nortex
public class Counter {
    public int value = 0;
    public void inc() { value = value + 1; }
    public int get() { return value; }
}
```

Constructors run field initializers; `this.field = parameter` and implicit field access are supported. Object variables are references: assignment and parameter passing share the same instance. A `mut` binding is required to replace a local reference.

## Other declarations

Closed generic classes such as `Container<int>` are monomorphized for concrete `new` instantiations. Generic instance methods can be called with explicit concrete type arguments, for example `converter.echo<int>(42)`; generic free functions, implicit method type-argument inference, and bounds are not part of this native path. Single inheritance flattens field layouts; a derived constructor first calls the parameterless base constructor. Parameterized base constructors, multiple inheritance, interfaces/traits, overloads, and multiple constructors are unsupported. Virtual methods must be declared `virtual` and overridden with `override`. `record`, `interface`, `trait`, and `enum` remain frontend/design syntax.

## Arrays and collections

Array types `T[]`, `[T; N]` are parsed in signatures; array creation `new T[n]` is missing (`NX0100`). Arrays are not allowed natively (`NX2002 Parametertyp 'int[]' liegt außerhalb des skalaren MIR-Subsets`). Collections exist only as `.nx` drafts in `standard-library/collections/`.

## Nesting

Nested structures are planned in the type model (`type_system/struct_layout.rs` computes layouts), but are not executable.

## Classes, inheritance, generics, collections (design target)

The parser and initial declaration checks exist; receiver borrowing, interface/trait checking, and trait coherence are missing. Everything below is **planned**:

- **Class:** nominal type with identity; the native backend shares references and retains objects in a runtime arena. Allocations remain valid until normal return from `main`, when they are released together. There is no individual runtime reclamation or tracing GC; programs must stop all threads using object pointers before `main` returns.
- **Struct:** value type, Move by default; copying only with a proven `Copy` property; heap allocation only explicitly via `Box<T>` or similar.
- **Record:** immutable value type with structural equality (field types must support equality); does not inherit from classes.
- **Interface:** signatures only, no fields or default implementations. **Trait:** capability contract with default methods and associated types. Both appear in the type's `:` list; there is no `impl Trait for Type`.
- **Inheritance:** one concrete base class is native; the base constructor must be parameterless. Multiple interfaces/traits and their contract checking are missing.
- **Generics:** explicit closed class and instance-method instantiations are native. Generic free functions, implicit method type-argument inference, and bounds such as `where T: Drawable` (including multiple bounds with `+`) are missing. Deduplication and coherence rules remain design goals.
- **Collections** (`System.Collections`, API signatures only in `standard-library/collections/*.nx`): `List<T>` (convenient, amortized O(1) append), `Vec<T>` (low-level, explicit capacity; no implicit conversion to `List<T>`), `Dictionary<K,V>`, `HashSet<T>`, `Queue<T>`, `Stack<T>`. Lengths and indices are `usize`; read access returns `Option<&T>`, mutable access returns `Option<&mut T>`. Memory management, hashing, and iterators are missing.
