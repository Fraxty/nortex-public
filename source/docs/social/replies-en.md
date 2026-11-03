# Draft replies for social media comments

## On asking for technical feedback without a public source link

You're right to call this out. Without a source link, nobody can honestly review the implementation. My earlier post was a project overview, not a code-review request, and I should have made that distinction explicit. The complete source is still in a private repository; the public-facing materials currently contain release/docs content rather than the full codebase.

To answer the example question: `add` and `main` are top-level free functions. `Console`, `Convert`, and `Text` are built-in namespace-like APIs, not instance methods. The parser accepts parts of class/method syntax, but the native compiler does not yet lower classes or compile instance-method calls. The example only uses free functions.

When I ask for code review, I'll include the actual public source link and point to a specific subsystem or design question.

## On compiler stages, diagnostics, and crate boundaries

Thanks — that is useful advice, especially the warning about splitting crates too early. NORTEX currently has distinct AST, HIR, and MIR representations inside the compiler, with the backend consuming MIR rather than parser structures. MIR validation handles definite assignment and conservative borrow/loan checks for the supported subset.

Compiler diagnostics currently share a structure with a code, message, source span, severity, and optional file/help text, and the IDE renders them. It is not yet a claim that every internal failure follows one fully unified error path, and structured suggestions are still missing. Improving that boundary and keeping the IDE-facing diagnostics stable are both good next steps.

The most useful question for me is: when did a concrete IR boundary start paying off in your project, and what would you keep out of it until the language subset grew?
