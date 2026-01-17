//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use crate::object::{ClusterId, NodeId, RegionId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Server identifier (Nanograph instance)
///
/// Represents a single Nanograph server process within a region.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct ServerId(pub u64);

impl ServerId {
    /// Create a new server identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the server identifier as a u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for ServerId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Server({})", self.0)
    }
}

/// Configuration for Server creation
#[derive(Clone, Debug)]
pub struct ServerCreate {
    /// Name of the server
    pub name: String,
    /// Socket Address of Server
    pub addr: SocketAddr,
    /// Public key for server authentication
    pub pubkey: String,
    /// Region this server Belongs to
    pub region: RegionId,
    /// Cluster this server Belongs to
    pub cluster: ClusterId,
    /// Additional options for server configuration
    pub options: HashMap<String, String>,
    /// Server Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl ServerCreate {
    /// Create a new ServerCreate instance with name, address, and public key.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the server.
    /// * `addr`: The socket address of the server.
    /// * `pubkey`: The public key for server authentication.
    pub fn new(
        name: impl Into<String>,
        addr: impl Into<SocketAddr>,
        pubkey: impl Into<String>,
        region: RegionId,
        cluster: ClusterId,
    ) -> Self {
        Self {
            name: name.into(),
            addr: addr.into(),
            pubkey: pubkey.into(),
            region,
            cluster,
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Set the socket address for the server.
    ///
    /// # Arguments
    ///
    /// * `addr`: The new socket address for the server.
    pub fn set_address(mut self, addr: impl Into<SocketAddr>) -> Self {
        self.addr = addr.into();
        self
    }
    /// Set the public key for the server.
    ///
    /// # Arguments
    ///
    /// * `pubkey`: The new public key for the server.
    pub fn set_pubkey(mut self, pubkey: impl Into<String>) -> Self {
        self.pubkey = pubkey.into();
        self
    }
    /// Add a configuration option to the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option.
    /// * `value`: The value of the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Remove a configuration option from the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to remove.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add informative metadata to the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry.
    /// * `value`: The value of the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Remove informative metadata from the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to remove.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Server update
#[derive(Clone, Debug, Default)]
pub struct ServerUpdate {
    /// Name of the server
    pub name: Option<String>,
    /// Hostname or IP address of the server
    pub addr: Option<SocketAddr>,
    /// Public key for server authentication
    pub pubkey: Option<String>,
    /// Additional options for server configuration
    pub options: Vec<PropertyUpdate>,
    /// Server Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl ServerUpdate {
    /// Set the name of the server.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the server.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    /// Clear the name of the server (set to None).
    pub fn clear_name(mut self) -> Self {
        self.name = None;
        self
    }
    /// Set the socket address for the server.
    ///
    /// # Arguments
    ///
    /// * `addr`: The new socket address for the server.
    pub fn set_address(mut self, addr: Option<SocketAddr>) -> Self {
        self.addr = addr;
        self
    }
    /// Clear the socket address for the server (set to None).
    pub fn clear_address(mut self) -> Self {
        self.addr = None;
        self
    }
    /// Set the public key for the server.
    ///
    /// # Arguments
    ///
    /// * `pubkey`: The new public key for the server.
    pub fn set_pubkey(mut self, pubkey: impl Into<String>) -> Self {
        self.pubkey = Some(pubkey.into());
        self
    }
    /// Clear the public key for the server (set to None).
    pub fn clear_pubkey(mut self) -> Self {
        self.pubkey = None;
        self
    }
    /// Add or update a configuration option for the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear a configuration option from the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.options.retain(|opt| match opt {
            PropertyUpdate::Set(k, _) => k != &key,
            PropertyUpdate::Clear(k) => k != &key,
        });
        self
    }
    /// Add or update informative metadata for the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear informative metadata from the server.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.metadata.retain(|k| k.key() != key);
        self
    }
}

/// Metadata for a server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerMetadata {
    /// Unique identifier for the server.
    pub id: NodeId,
    /// Name of the server.
    pub name: String,
    /// Timestamp when the Server was created.
    pub created_at: Timestamp,
    /// Timestamp when the server metadata was last modified.
    pub last_modified: Timestamp,
    /// Configuration Options for the Server.
    pub options: HashMap<String, String>,
    /// Server Metadata (Informative).
    pub metadata: HashMap<String, String>,
}

impl From<ServerRecord> for ServerMetadata {
    fn from(record: ServerRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Metadata Record for a server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerRecord {
    /// Unique identifier for the server.
    pub id: NodeId,
    /// Name of the server.
    pub name: String,
    /// Version of the Server Record.
    pub version: u64,
    /// Timestamp when the Server was created.
    pub created_at: Timestamp,
    /// Timestamp when the server metadata was last modified.
    pub last_modified: Timestamp,
    /// Configuration Options for the Server.
    pub options: HashMap<String, String>,
    /// Server Metadata (Informative).
    pub metadata: HashMap<String, String>,
}
