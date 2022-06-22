use super::*;

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt::Debug, str::FromStr};

/// Child information
#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    /// uri of the child device
    pub uri: ChildUri,
    /// state of the child
    pub state: ChildState,
    /// current rebuild progress (%)
    pub rebuild_progress: Option<u8>,
}

impl From<Child> for models::Child {
    fn from(src: Child) -> Self {
        Self {
            rebuild_progress: src.rebuild_progress,
            state: src.state.into(),
            uri: src.uri.into(),
        }
    }
}

bus_impl_string_id_percent_decoding!(ChildUri, "URI of a nexus child");

impl ChildUri {
    /// Get the io-engine bdev uuid from the ChildUri
    pub fn uuid_str(&self) -> Option<String> {
        match url::Url::from_str(self.as_str()) {
            Ok(url) => {
                let uuid = url.query_pairs().find(|(name, _)| name == "uuid");
                uuid.map(|(_, uuid)| uuid.to_string())
            }
            Err(_) => None,
        }
    }
}
impl PartialEq<Child> for ChildUri {
    fn eq(&self, other: &Child) -> bool {
        self == &other.uri
    }
}
impl PartialEq<String> for ChildUri {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

/// Child State information
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum ChildState {
    /// Default Unknown state
    Unknown = 0,
    /// healthy and contains the latest bits
    Online = 1,
    /// rebuild is in progress (or other recoverable error)
    Degraded = 2,
    /// unrecoverable error (control plane must act)
    Faulted = 3,
}
impl ChildState {
    /// Check if the child is `Faulted`
    pub fn faulted(&self) -> bool {
        self == &Self::Faulted
    }
}
impl PartialOrd for ChildState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match &self {
            ChildState::Unknown => match &other {
                ChildState::Unknown => Some(Ordering::Equal),
                ChildState::Online => Some(Ordering::Less),
                ChildState::Degraded => Some(Ordering::Less),
                ChildState::Faulted => Some(Ordering::Greater),
            },
            ChildState::Online => match &other {
                ChildState::Unknown => Some(Ordering::Greater),
                ChildState::Online => Some(Ordering::Equal),
                ChildState::Degraded => Some(Ordering::Greater),
                ChildState::Faulted => Some(Ordering::Greater),
            },
            ChildState::Degraded => match &other {
                ChildState::Unknown => Some(Ordering::Greater),
                ChildState::Online => Some(Ordering::Less),
                ChildState::Degraded => Some(Ordering::Equal),
                ChildState::Faulted => Some(Ordering::Greater),
            },
            ChildState::Faulted => match &other {
                ChildState::Unknown => Some(Ordering::Less),
                ChildState::Online => Some(Ordering::Less),
                ChildState::Degraded => Some(Ordering::Less),
                ChildState::Faulted => Some(Ordering::Equal),
            },
        }
    }
}

impl Default for ChildState {
    fn default() -> Self {
        Self::Unknown
    }
}
impl From<i32> for ChildState {
    fn from(src: i32) -> Self {
        match src {
            1 => Self::Online,
            2 => Self::Degraded,
            3 => Self::Faulted,
            _ => Self::Unknown,
        }
    }
}
impl From<ChildState> for models::ChildState {
    fn from(src: ChildState) -> Self {
        match src {
            ChildState::Unknown => Self::Unknown,
            ChildState::Online => Self::Online,
            ChildState::Degraded => Self::Degraded,
            ChildState::Faulted => Self::Faulted,
        }
    }
}

/// Remove Child from Nexus Request
#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveNexusChild {
    /// id of the io-engine instance
    pub node: NodeId,
    /// uuid of the nexus
    pub nexus: NexusId,
    /// URI of the child device to be removed
    pub uri: ChildUri,
}
impl RemoveNexusChild {
    /// Return new `Self`
    pub fn new(node: &NodeId, nexus: &NexusId, uri: &ChildUri) -> Self {
        Self {
            node: node.clone(),
            nexus: nexus.clone(),
            uri: uri.clone(),
        }
    }
}
impl From<AddNexusChild> for RemoveNexusChild {
    fn from(add: AddNexusChild) -> Self {
        Self {
            node: add.node,
            nexus: add.nexus,
            uri: add.uri,
        }
    }
}

/// Add child to Nexus Request
#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddNexusChild {
    /// id of the io-engine instance
    pub node: NodeId,
    /// uuid of the nexus
    pub nexus: NexusId,
    /// URI of the child device to be added
    pub uri: ChildUri,
    /// auto start rebuilding
    pub auto_rebuild: bool,
}
