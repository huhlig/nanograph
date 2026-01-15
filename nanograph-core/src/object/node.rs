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

use crate::object::{ClusterId, RegionId, ServerId};
use serde::{Deserialize, Serialize};

/// Raft Node ID consisting of cluster, region, and server identifiers.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct NodeId(u128);

impl NodeId {
    /// Create a new NodeId from a u128 identifier.
    pub fn new(id: u128) -> Self {
        Self(id)
    }
    /// Create a new NodeId from cluster, region, and server identifiers.
    pub fn from_parts(cluster: ClusterId, region: RegionId, server: ServerId) -> Self {
        Self(((cluster.0 as u128) << 64) | ((region.0 as u128) << 32) | server.0 as u128)
    }

    /// Return the node identifier as a u128.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Extract the ServerId from the NodeId.
    pub fn server_id(&self) -> ServerId {
        ServerId((self.0 >> 64) as u64)
    }

    /// Extract the ClusterId from the NodeId.
    pub fn cluster_id(&self) -> ClusterId {
        ClusterId((self.0 >> 96) as u32)
    }

    /// Extract the RegionId from the NodeId.
    pub fn region_id(&self) -> RegionId {
        RegionId((self.0 >> 32) as u32)
    }
}

impl From<u128> for NodeId {
    fn from(id: u128) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node({:X})", self.0)
    }
}
