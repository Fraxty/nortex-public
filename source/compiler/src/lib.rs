//! NORTEX compiler front end.
//!
//! The crate implements a partial source-to-MIR pipeline and experimental
//! Windows x64 COFF code generation for its supported scalar MIR subset.

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod linker;
pub mod manifest;
pub mod mir;
pub mod optimizer;
pub mod parser;
pub mod semantic;
pub mod type_system;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn compiler_name() -> &'static str {
    "NORTEX"
}
