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

mod group;
mod permission;
mod policy;
mod principal;
mod role;
mod user;

pub use self::group::{
    SystemGroupCreate, SystemGroupId, SystemGroupMetadata, SystemGroupRecord, SystemGroupUpdate,
    TenantGroupCreate, TenantGroupId, TenantGroupMetadata, TenantGroupRecord, TenantGroupUpdate,
};
pub use self::permission::{Permission, PermissionGrant, ResourceScope};
pub use self::principal::SecurityPrincipal;
pub use self::role::{
    SystemRoleCreate, SystemRoleId, SystemRoleMetadata, SystemRoleRecord, SystemRoleUpdate,
    TenantRoleCreate, TenantRoleId, TenantRoleMetadata, TenantRoleRecord, TenantRoleUpdate,
};
pub use self::user::{
    SystemUserCreate, SystemUserMetadata, SystemUserRecord, SystemUserUpdate, TenantUserCreate,
    TenantUserMetadata, TenantUserRecord, TenantUserUpdate, UserId,
};
use serde::{Deserialize, Serialize};

/// Security Subject ID (Used by User, Role, and Group)
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct SubjectId(u32);

impl SubjectId {
    /// Create a new cluster identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the cluster identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for SubjectId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for SubjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Subject({:X})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
