# HIR

HIR is the function-oriented, typed representation between AST and MIR.
`nortexc mir <file.nx>` currently lowers function/method bodies containing
integer/boolean locals, supported scalar expressions (including nested
conditional and short-circuit logical expressions), assignments, `if`,
`while`, classic `for`, `break`, `continue`, and `return`. Mutable locals may
start uninitialized; MIR definite-assignment validation decides whether each
reachable read is safe. Local references to scalar locals (`&T` and `&mut T`)
are represented in MIR and checked conservatively within lexical scopes.

Unsupported declarations or expressions produce a lowering diagnostic; the
compiler does not emit a partial successful HIR program. Name resolution is
currently local to each function. Generic substitution, full symbol IDs, and
the rest of the language grammar are not represented yet.
