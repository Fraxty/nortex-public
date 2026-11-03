# Chapter 9 – Memory Management

← [Chapter 8](08-structs-and-data-structures.md) · [Contents](README.md) · Next: [Chapter 10](10-input-output.md)

## Current native memory model

Scalar locals and parameters use registers/stack slots. Class instances are heap allocated and represented by pointer values. Assignment, parameter passing, and returns copy the pointer, not the object; multiple bindings can therefore mutate the same instance. String fields contain the existing NORTEX string-slice descriptor.

| Area | Behavior |
|---|---|
| Object allocation | `new Class(...)` calls `nortex_object_alloc`; storage is zero-initialized and 8-byte aligned. |
| Object lifetime | The arena retains each instance until normal return from generated `main`, where all allocations are released together. |
| Cycles | Reference cycles are valid and do not require reference counting. |
| Reclamation / GC | No individual reclamation and no tracing GC. Memory use grows until `main` returns and can grow without bound during long-running programs. |
| Null references | There is no null literal. Unassigned class-reference fields start internally as null; dereferencing a null object traps instead of accessing memory. Initialize reference fields before use. |

The runtime owns allocations in a synchronized arena and releases them on normal return from generated `main`. A trap or process abort bypasses this explicit shutdown; the operating system cleans up at process termination. When embedding code without generated `main`, the host must call `nortex_object_arena_shutdown` only after no object pointers are in use. There is no individual reclamation or tracing GC: the compiler emits no stack maps or registered roots, so mark-and-sweep cannot safely determine reachability. If threads use object pointers, they must be joined before `main` returns. Destructors and deterministic resource release are unavailable.

## Other limits

- `struct`, `record`, arrays, `Box<T>`, `Rc<T>`, and `Arc<T>` are not implemented in the native backend.
- Closed generic classes, generic instance methods with explicit concrete type arguments, single inheritance, and `virtual`/`override` dispatch are supported. Base constructors must be parameterless; generic free functions, implicit method type-argument inference, bounds, multiple inheritance, and interfaces/traits are unsupported; see [Chapter 8](08-structs-and-data-structures.md).
- Raw pointers and `unsafe` are not supported source-language features.
- Scalars are Copy. Class references are aliasable pointers; complete ownership/borrow/lifetime checking does not yet apply to these references.

The runtime arena is linked automatically when native code contains object allocations. See [Compiler code generation](../../compiler/codegen.md).
