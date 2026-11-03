# MIR

MIR models locals, places, statements, basic blocks, and terminators.
The current source pipeline lowers its supported scalar HIR subset to MIR,
validates block targets and local references, runs constant folding and
conservative liveness-based dead-code elimination, then validates again.

Validation includes forward definite-assignment analysis across CFG edges,
including moved locals and call results on normal (not cleanup) edges. The
analysis treats constant switch edges as exclusive, so dead branches do not
affect initialization at joins. Lowering emits explicit CFG joins for ternary
and short-circuit expressions, and rejects reads not initialized on every
reachable incoming path. The ownership validator tracks reborrows and
dereference accesses, validates reference arguments passed by MIR calls, and
supports field loans and partial moves when field types are present in the
local declaration. Returns derived from reference parameters are accepted;
reference-valued call results are rejected until call lifetime metadata exists.
Loan ends are lexical, not NLL, and the source frontend does not yet lower
general calls or aggregates.

`nortexc mir <file.nx>` prints validated optimized MIR. Native code generation,
complete ownership checking, full language lowering, and executable emission
are not implemented.
