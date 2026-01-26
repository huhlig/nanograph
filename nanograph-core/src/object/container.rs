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

use crate::object::{DatabaseId, TenantId};
use serde::{Deserialize, Serialize};

/// Container ID
///
/// Container ID is the Combination of Tenant and Database used for isolation.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct ContainerId(pub u64);

impl ContainerId {
    pub fn system() -> Self {
        Self(0)
    }
    /// Create a new container identifier.
    pub fn new(id: u64) -> Self {
        assert_ne!(id, 0, "Container ID cannot be zero.");
        Self(id)
    }

    pub fn from_parts(tenant: TenantId, database: DatabaseId) -> Self {
        Self(((tenant.0 as u64) << 32) | database.0 as u64)
    }

    /// Return the container identifier as a u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn tenant(&self) -> TenantId {
        TenantId((self.0 >> 32) as u32)
    }

    pub fn database(&self) -> DatabaseId {
        DatabaseId(self.0 as u32)
    }
}

impl From<u64> for ContainerId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ContainerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Container({:X})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_id() {
        let tenant = TenantId(0x12345678);
        let database = DatabaseId(0xABCDEF01);
        let container_id = ContainerId::from_parts(tenant, database);

        assert_eq!(container_id.tenant(), tenant);
        assert_eq!(container_id.database(), database);
        assert_eq!(container_id.as_u64(), 0x12345678_ABCDEF01);
        assert_eq!(ContainerId::new(0x12345678_ABCDEF01), container_id);
        assert_eq!(ContainerId::from(0x12345678_ABCDEF01), container_id);
        assert_eq!(format!("{}", container_id), "Container(12345678ABCDEF01)");
    }
}
