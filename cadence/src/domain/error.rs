//! Small shared error type for legacy or host-facing seams.

#[derive(thiserror::Error, Debug)]
/// Generic crate error used by older helper paths.
pub enum Error {
    /// Free-form error message.
    #[error("Generic {0}")]
    Generic(String),

    /// Wrapped I/O error.
    #[error(transparent)]
    IO(#[from] std::io::Error),
}
