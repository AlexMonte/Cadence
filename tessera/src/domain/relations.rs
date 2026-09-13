use serde::{Deserialize, Serialize};

use super::{InputPort, NodeId, OutputPort, PortGroupId, PortMemberId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputEndpoint {
    Socket(InputPort),
    GroupMember {
        group: PortGroupId,
        member: PortMemberId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutputEndpoint {
    Socket(OutputPort),
    GroupMember {
        group: PortGroupId,
        member: PortMemberId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StreamSource {
    pub node: NodeId,
    pub endpoint: OutputEndpoint,
}

impl StreamSource {
    pub fn socket(node: NodeId, port: impl Into<String>) -> Self {
        Self {
            node,
            endpoint: OutputEndpoint::Socket(OutputPort::new(port)),
        }
    }

    pub fn node(node: NodeId) -> Self {
        Self::socket(node, "out")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamTarget {
    OutputInput {
        node: NodeId,
        endpoint: InputEndpoint,
    },
    TransformInput {
        node: NodeId,
        endpoint: InputEndpoint,
    },
    FlowControlInput {
        node: NodeId,
        endpoint: InputEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RootRelation {
    ChainedTo {
        from: StreamSource,
        to: NodeId,
    },
    FlowsTo {
        from: StreamSource,
        to: StreamTarget,
    },
}

impl Serialize for InputEndpoint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire<'a> {
            Socket {
                port: &'a InputPort,
            },
            GroupMember {
                group: &'a PortGroupId,
                member: &'a PortMemberId,
            },
        }
        match self {
            Self::Socket(port) => Wire::Socket { port }.serialize(serializer),
            Self::GroupMember { group, member } => {
                Wire::GroupMember { group, member }.serialize(serializer)
            }
        }
    }
}
impl<'de> Deserialize<'de> for InputEndpoint {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire {
            Socket {
                port: InputPort,
            },
            GroupMember {
                group: PortGroupId,
                member: PortMemberId,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Socket { port } => Self::Socket(port),
            Wire::GroupMember { group, member } => Self::GroupMember { group, member },
        })
    }
}

impl Serialize for OutputEndpoint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire<'a> {
            Socket {
                port: &'a OutputPort,
            },
            GroupMember {
                group: &'a PortGroupId,
                member: &'a PortMemberId,
            },
        }
        match self {
            Self::Socket(port) => Wire::Socket { port }.serialize(serializer),
            Self::GroupMember { group, member } => {
                Wire::GroupMember { group, member }.serialize(serializer)
            }
        }
    }
}
impl<'de> Deserialize<'de> for OutputEndpoint {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire {
            Socket {
                port: OutputPort,
            },
            GroupMember {
                group: PortGroupId,
                member: PortMemberId,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Socket { port } => Self::Socket(port),
            Wire::GroupMember { group, member } => Self::GroupMember { group, member },
        })
    }
}
