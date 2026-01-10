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

use nanograph_core::types::{ClusterId, RegionId, ServerId, Timestamp};

/// Configuration for Cluster creation
pub struct ClusterConfig {
    /// Name of the cluster
    pub name: String,
}

impl ClusterConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Metadata for a cluster.
#[derive(Clone, Debug)]
pub struct ClusterMetadata {
    /// Unique identifier for the cluster
    pub id: ClusterId,
    /// Name of the cluster
    pub name: String,
    /// Metadata version (incremented on each change)
    pub version: u64,
    /// Timestamp when the cluster was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
}

/// Configuration for Region creation
pub struct RegionConfig {
    /// Name of the region
    pub name: String,
}

impl RegionConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Metadata for a region.
#[derive(Clone, Debug)]
pub struct RegionMetadata {
    /// Unique identifier for the region
    pub id: RegionId,
    /// Name of the region
    pub name: String,
    /// Identifier of the cluster this region belongs to
    pub cluster: ClusterId,
    /// Timestamp when the Region was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
}

/// Configuration for Server creation
pub struct ServerConfig {
    /// Name of the server
    pub name: String,
}

impl ServerConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Metadata for a server.
#[derive(Clone, Debug)]
pub struct ServerMetadata {
    /// Unique identifier for the server
    pub id: ServerId,
    /// Name of the server
    pub name: String,
    /// Identifier of the region this server belongs to
    pub region: RegionId,
    /// Identifier of the cluster this server belongs to
    pub cluster: ClusterId,
    /// Timestamp when the Server was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
}
