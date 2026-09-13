use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationRole {
    Blocking,
    Streaming,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoadUpSetupError {
    #[error(
        "StateTransition schedule missing; add DefaultPlugins or StatesPlugin before LoadUpPlugin"
    )]
    MissingStateTransitionSchedule,
    #[error("{resource} is already registered as {role:?} for this state")]
    DuplicateRegistration {
        resource: &'static str,
        role: RegistrationRole,
    },
    #[error("{resource} cannot be both blocking and streaming for the same state")]
    ConflictingRegistration { resource: &'static str },
    #[error("invalid dependency configuration for {resource}::{field}: {reason}")]
    InvalidDependencyConfiguration {
        resource: &'static str,
        field: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoadUpOperationError {
    #[error("dependency `{field}` is not registered")]
    DependencyNotFound { field: &'static str },
    #[error("prepared resource `{resource}` is not registered")]
    PreparedResourceNotFound { resource: &'static str },
}
