# 1 Welcome to Nortex Code

[Contents](README.md) · [Next: Installation](02-installation.md)

## What is Nortex Code?

**Nortex Code** is the graphical development environment (IDE, "Integrated Development Environment") for the **Nortex** programming language. An IDE combines a text editor, file overview, compiler invocation and output in a single window.

Nortex Code is a standalone Windows program. To build, it calls the Nortex compiler **`nortexc`**, which translates `.nx` files into a Windows executable (EXE).

## What is it used for?

- Writing Nortex source code (`.nx`),
- building and running with one click (**Run**, Ctrl+F5),
- reading compiler messages with their positions,
- editing several files in tabs and split views.

## Overview of the main features

| Area | Feature |
|---|---|
| Editor | Syntax highlighting, line numbers, auto-completion, find/replace, split view |
| Build | Run button, overflow checks, `nortex.toml` projects |
| Output | Compiler Log, Runtime Output, Terminal (PowerShell) |
| Customization | Theme, density, UI and editor zoom, language (German/English) |

Not included are, among others, a full debugger backend, project wizards and file rename/delete (see [feature status](README.md#funktionsstand-der-ide)).

## Relation to the language

The language determines what can be compiled. Today the compiler natively builds only a small subset (including `int`/`bool`, `if`/`while`/`for`, functions and UTF-8 string slices for variables, function calls and `Console.writeLine`). The IDE recognizes more syntax than the build can actually produce; details: [Language reference](../../language/README.md).
