# NORTEX – Ownership and Borrowing 0.3.7-dev

NORTEX has no garbage collector. Every owned value has a unique owner. The rules are checked at compile time by ownership and borrow analysis.

## Binding rules

1. An owned value has exactly one owner.
2. Passing or assigning by value transfers ownership (Move), unless the type explicitly implements `Copy`.
3. After a Move, the old binding cannot be used until it has been fully reinitialized.
4. Any number of simultaneous immutable borrows (`&T`) are allowed.
5. A mutable borrow (`&mut T`) is exclusive; no other active borrows of the same memory region may exist at that time.
6. A borrow may not outlive the referenced value.
7. `clone()` is explicit and type-specific. There is no implicit deep copy.
8. Shared ownership is explicitly selected through `Rc<T>` (single-threaded) or `Arc<T>` (thread-safe).
9. `Span<T>` is a non-owning, lifetime-bound view. `Memory<T>` is an owning runtime handle.
10. Destructors run deterministically when a lifetime ends and during exception unwinding; they may not throw exceptions.

## Syntax

```nortex
public void displayPlayer(&Player player)
{
    print(player.health);
}

public void damagePlayer(&mut Player player)
{
    player.health = player.health - 10;
}

Player duplicate = player.clone();
```

## Classes and heap

A class instance created with `new` has a unique heap owner. The variable binding holds this owner, and a Move transfers it. `Box<T>` is a separate unique heap owner for arbitrary values and explicit indirection. `Rc<T>`/`Arc<T>` allow shared ownership only with their respective synchronization guarantees.

## Unsafe

Raw pointers, manual memory management, pointer arithmetic, and certain FFI operations require a delimited `unsafe { ... }` block. The block documents the author's responsibility for validity, alignment, aliasing rules, and lifetime. `unsafe` does not disable type checking for the rest of the program.

## Implementation status

The compiler currently checks Moves from local bindings in by-value calls, variable initializations, returns, and assignments. A `mut` binding that has been fully reassigned can be used again. Primitive scalars are treated as copyable; other values are conservatively treated as owned. `&T` parameters prevent a Move; multiple borrows within a call are checked for conflicts with `&mut T`. An `&mut` borrow from an immutable binding is rejected. Move states are tied to individual local bindings, not just their names, so same-named variables in separate lexical scopes do not affect one another. For borrow arguments to the same call, known distinct field projections are recognized as disjoint; unknown roots are conservatively treated as aliasing.

The return-path checker considers sequences, branches, switches, loops, match arms, throws, and `finally`. Exhaustive enum matches over enum parameters are recognized as complete control-flow branches. `Result<T,E>` call results may not be discarded as standalone expression statements.

The MIR pipeline tracks local loans through reference copies, reborrows, control flow, and lexical storage regions. Dereferenced writes are traced back to the borrowed memory region; parent references are locked during active reborrows. MIR field projections support disjoint loans, partial moves, and reinitialization of moved fields when the field types are declared in MIR. Reference arguments to a MIR call are checked for overlapping shared/mutable loans. Reference returns from reference parameters are allowed; reference results from unknown calls are rejected due to missing lifetime metadata. Negative fixtures are in `compiler/tests/negatives/`.

This is still not a complete ownership/borrow checker: loan ends are lexical rather than NLL, and explicit lifetime parameters and region-based inference are missing. The frontend lowerer does not yet fully map function calls and aggregates to MIR; therefore, field projections are currently available at the MIR level and not as a complete source-language feature. `Copy` trait rules and drop order are also incomplete. The current status must not be presented as complete lifetime safety.

A complete borrow checker additionally requires control-flow analysis, loan tracking, lifetime inference/checking, field and partial-field loans across call boundaries, reborrow rules, pinning contracts, and correct drop order.
