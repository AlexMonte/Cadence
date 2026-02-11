//! Strongly typed stable identifiers for scopes, nodes, and edges.

use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_uuid_id {
    ($(#[$meta:meta])* $name:ident $(, $extra_derives:ident)*) => {
        $(#[$meta])*
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            Serialize,
            Deserialize
            $(, $extra_derives)*
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new random identifier.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create from a UUID (useful for deserialization and fixtures).
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Return the inner UUID.
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_uuid_id!(
    /// Stable identifier for a scope (group of nodes and child scopes).
    ScopeId,
    Reflect
);

define_uuid_id!(
    /// Stable identifier for a node (computation unit).
    NodeId,
    Reflect
);

define_uuid_id!(
    /// Stable identifier for an edge (connection between node ports).
    EdgeId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_uniqueness() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_id_serialization() {
        let id = NodeId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
