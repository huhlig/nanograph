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
        Self(((cluster.0 as u128) << 96) | ((region.0 as u128) << 64) | server.0 as u128)
    }

    /// Return the node identifier as a u128.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Extract the ServerId from the NodeId.
    pub fn server_id(&self) -> ServerId {
        ServerId(self.0 as u64)
    }

    /// Extract the ClusterId from the NodeId.
    pub fn cluster_id(&self) -> ClusterId {
        ClusterId((self.0 >> 96) as u32)
    }

    /// Extract the RegionId from the NodeId.
    pub fn region_id(&self) -> RegionId {
        RegionId((self.0 >> 64) as u32)
    }
}
impl From<u32> for NodeId {
    fn from(value: u32) -> Self {
        Self(value as u128)
    }
}

impl From<u64> for NodeId {
    fn from(value: u64) -> Self {
        Self(value as u128)
    }
}

impl From<u128> for NodeId {
    fn from(id: u128) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{ClusterId, RegionId, ServerId, TableShardId};

    #[test]
    fn test_node_id() {
        let cluster = ClusterId(0x12345678);
        let region = RegionId(0xABCDEF01);
        let server = ServerId(0x1122334455667788);

        let node_id = NodeId::from_parts(cluster, region, server);

        assert_eq!(node_id.cluster_id(), cluster);
        assert_eq!(node_id.region_id(), region);
        assert_eq!(node_id.server_id(), server);
        assert_eq!(node_id.as_u128(), 0x12345678_ABCDEF01_1122334455667788);

        let node_id_new = NodeId::new(0x12345678_ABCDEF01_1122334455667788);
        assert_eq!(node_id, node_id_new);

        let node_id_from_u128 = NodeId::from(0x12345678_ABCDEF01_1122334455667788u128);
        assert_eq!(node_id, node_id_from_u128);

        let node_id_from_u64 = NodeId::from(0x1122334455667788u64);
        assert_eq!(node_id_from_u64.as_u128(), 0x1122334455667788u128);

        assert_eq!(
            format!("{}", node_id),
            "24197857208548607642070844851154745224"
        );
    }
}
