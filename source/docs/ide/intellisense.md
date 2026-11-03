# NORTEX Code – Completion and IntelliSense

The editor uses the compiler's lexer and error-tolerant parser. It shows keywords, declarations,
variables, functions, types, class members, symbols of imported modules, module and file names in
`using` statements, and function signature help. The popup is bounded and scrollable, follows the
caret and is operated with the keyboard. `Ctrl+Space` opens it explicitly.

Completion application replaces only the typed prefix and keeps the text after the caret. Parsing
errors do not prevent the parser from returning the declarations it could recover; for the
analysis, blocks that are still open at the caret are closed virtually (`close_open_blocks`).

## Components

| File | Task |
|---|---|
| `ide/src/editor/analysis.rs` | Lexing and parsing of the active document with a one-entry cache, incremental re-lexing and parsing after edits, phase counters and timings |
| `ide/src/editor/imports.rs` | Import graph of the active file. Module lookup uses the compiler's `find_module`; the traversal follows the compiler's `ModuleResolver` (post-order, each module once, `System.*` is not a file). Parsed module files are cached per path and invalidated by modification time and length |
| `ide/src/editor/types.rs` | `TypeEnv`: types of variables, parameters, fields, methods and call chains for member completion |
| `ide/src/editor/completion.rs` | Context analysis, candidates, ranking, signature help |
| `ide/src/shell/painting.rs`, `ide/src/shell/frame.rs` | Popup and key handling |

## Behaviour and rules taken from the compiler

- One namespace per file; all imports are merged into one program. There is no top-level
  visibility, so symbols of transitive imports are visible, matching what the compiler accepts.
- Cycles and duplicate type names are the compiler's errors (NX1003, NX1001); IntelliSense only
  skips them.
- Import resolution needs a file on disk. A document without a path sees no imports.
- Type resolution is conservative: when a type cannot be derived unambiguously (generic
  arguments, overloads with different return types, unknown names), no type is returned instead
  of a guess. In a receiver chain, the type arguments after `new Type<…>(` are skipped, so the
  receiver type is the bare type name.

## Measured cost

`docs/ide/benchmarks.md` has the full measurement (Ryzen 9 7900X, release build, 15 runs,
median). Summary: lexer 6.6 ms and parser 8.8 ms for 10,000 lines; member completion in an active
10,000-line file 31.7 ms, one completion per keystroke in that file 33.7 ms; import resolution for
100 modules 24 ms cold and 8.5 ms warm.

## Current limits

- Completion runs on a worker thread (`editor/completion_worker.rs`): a single-slot mailbox
  replaces pending requests, stale results are dropped, a panic in the analysis becomes a status
  message. A running analysis cannot be interrupted, only its result is discarded. The analysis
  cache keeps two entries (text and its block-closed variant) and only helps for unchanged text;
  typing is not faster. Per-phase numbers are in `benchmarks.md`.
- Symbol and type resolution over the whole active file is the largest single cost (about 10 ms
  at 10,000 lines, about 226 ms at 50,000).
- The warm import cache still checks every module file's timestamp on each request (about 85 µs
  per module), so large import graphs cost time on each completion.
- Type inference is not a compiler type checker; generics, overload resolution and control-flow
  dependent typing are not covered.
- Signature help covers parsed functions and class methods; hover documentation, parameter type
  checking and semantic error explanations are unavailable.
- Completion runs on the UI thread; there is no background completion worker.
- Comments and string/character literals suppress the popup.
