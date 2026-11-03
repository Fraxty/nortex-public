//! Runtime exception payloads.
//!
//! The compiler currently parses try/catch/finally and throw syntax. A full
//! language-level exception ABI and stack unwinding implementation is not yet
//! available; these typed payloads are the runtime-side foundation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NortexException {
    pub type_name: String,
    pub message: String,
}

impl NortexException {
    pub fn new(type_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            message: message.into(),
        }
    }
}

pub type ExceptionResult<T> = Result<T, NortexException>;

pub fn raise<T>(exception: NortexException) -> ExceptionResult<T> {
    Err(exception)
}
