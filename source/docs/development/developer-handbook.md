# NORTEX Developer Handbook

## Repository rules

- Keep compiler stages independent and pass explicit intermediate representations between them.
- Keep rendering, IDE state and package tooling outside the compiler's semantic source of truth.
- Every language change requires a specification update, parser/semantic tests and an example.
- Preserve source spans through lexer, AST, HIR and MIR.
- Do not mark a feature complete until frontend, runtime/ABI and tests agree.
- A documented target feature is not an implemented target feature.
- Any native code or FFI boundary must document ABI, memory ownership and error handling.

## Development workflow

1. Confirm the relevant language decision in `docs/language/15-faq-and-best-practices.md` (open decisions) and the verified reference in `docs/language/`.
2. Update lexer/parser/AST as required.
3. Add semantic diagnostics and negative tests.
4. Add HIR/MIR lowering only after semantics are stable.
5. Add runtime or ABI support and integration tests.
6. Update examples, status and phase documentation.
7. Run formatting, check, tests and Clippy on a configured Rust toolchain.

## Current implementation boundary

`nortexc check <file.nx>` runs the implemented lexer, parser and initial semantic validation. It does not perform complete type checking or ownership analysis and does not emit native machine code. The exact current boundaries are tracked in `BUILD_STATUS.md`.

## Phase map

The overall architecture is in `docs/architecture.md`; detailed target-design contracts are under `docs/design/` (phases 3–8) and `docs/security/` (quality and security).
