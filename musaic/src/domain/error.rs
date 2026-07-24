use std::fmt;

use crate::domain::document::DocumentGraphError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Graph(DocumentGraphError),
    Message(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Graph(error) => write!(f, "{error:?}"),
            DomainError::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for DomainError {}

impl From<DocumentGraphError> for DomainError {
    fn from(value: DocumentGraphError) -> Self {
        DomainError::Graph(value)
    }
}
