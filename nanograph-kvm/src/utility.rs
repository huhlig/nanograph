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

use nanograph_core::object::{
    ClusterId, DatabaseId, FunctionId, NamespaceId, RegionId, ServerId, ShardId, SystemGroupId,
    SystemRoleId, TableId, TenantGroupId, TenantId, TenantRoleId, UserId,
};
use nanograph_kvt::{KeyValueError, KeyValueResult};


/// Utility for generating system-level storage keys.
pub struct SystemKeys;

impl SystemKeys {
    /// Generate a key for a cluster.
    pub const fn cluster_key(cluster_id: ClusterId) -> [u8; 5] {
        let p = [0xFF];
        let c = cluster_id.0.to_be_bytes();
        [p[0], c[0], c[1], c[2], c[3]]
    }
    /// Generate a key for a region within a cluster.
    pub const fn region_key(cluster_id: ClusterId, region_id: RegionId) -> [u8; 9] {
        let p = [0xFE];
        let c = cluster_id.0.to_be_bytes();
        let r = region_id.0.to_be_bytes();
        [p[0], c[0], c[1], c[2], c[3], r[0], r[1], r[2], r[3]]
    }
    /// Generate a key for a server within a region.
    pub const fn server_key(
        cluster_id: ClusterId,
        region_id: RegionId,
        server_id: ServerId,
    ) -> [u8; 17] {
        let p = [0xFD];
        let c = cluster_id.0.to_be_bytes();
        let r = region_id.0.to_be_bytes();
        let s = server_id.0.to_be_bytes();
        [
            p[0], c[0], c[1], c[2], c[3], r[0], r[1], r[2], r[3], s[0], s[1], s[2], s[3], s[4],
            s[5], s[6], s[7],
        ]
    }
    /// Generate a key for a tenant.
    pub fn tenant_key(tenant_id: TenantId) -> [u8; 5] {
        let p = [0xFC];
        let t = tenant_id.0.to_be_bytes();
        [p[0], t[0], t[1], t[2], t[3]]
    }
    /// Generate a key for a database within a tenant.
    pub fn database_key(tenant_id: TenantId, database_id: DatabaseId) -> [u8; 9] {
        let p = [0xFB];
        let t = tenant_id.0.to_be_bytes();
        let d = database_id.0.to_be_bytes();
        [p[0], t[0], t[1], t[2], t[3], d[0], d[1], d[2], d[3]]
    }
    /// Generate a key for a system user record.
    pub fn system_user_key(user_id: UserId) -> [u8; 5] {
        let p = [0xFA];
        let u = user_id.0.to_be_bytes();
        [p[0], u[0], u[1], u[2], u[3]]
    }
    /// Generate a key for a system user record.
    pub fn system_role_key(role_id: SystemRoleId) -> [u8; 5] {
        let p = [0xF9];
        let r = role_id.0.to_be_bytes();
        [p[0], r[0], r[1], r[2], r[3]]
    }
    /// Generate a key for a system user record.
    pub fn system_group_key(group_id: SystemGroupId) -> [u8; 5] {
        let p = [0xF8];
        let g = group_id.0.to_be_bytes();
        [p[0], g[0], g[1], g[2], g[3]]
    }
    /// Generate a key for a tenant_user_record.
    pub fn tenant_user_key(tenant_id: TenantId, user_id: UserId) -> [u8; 9] {
        let p = [0xF7];
        let t = tenant_id.0.to_be_bytes();
        let u = user_id.0.to_be_bytes();
        [p[0], t[0], t[1], t[2], t[3], u[0], u[1], u[2], u[3]]
    }
    /// Generate a key for a tenant_user_record.
    pub fn tenant_role_key(tenant_id: TenantId, role_id: TenantRoleId) -> [u8; 9] {
        let p = [0xF6];
        let t = tenant_id.0.to_be_bytes();
        let r = role_id.0.to_be_bytes();
        [p[0], t[0], t[1], t[2], t[3], r[0], r[1], r[2], r[3]]
    }
    /// Generate a key for a tenant_user_record.
    pub fn tenant_group_key(tenant_id: TenantId, group_id: TenantGroupId) -> [u8; 9] {
        let p = [0xF5];
        let t = tenant_id.0.to_be_bytes();
        let g = group_id.0.to_be_bytes();
        [p[0], t[0], t[1], t[2], t[3], g[0], g[1], g[2], g[3]]
    }
}

/// Utility for generating container-level storage keys.
pub struct ContainerKeys;

impl ContainerKeys {
    /// Generate a key for a namespace.
    pub fn namespace_key(namespace_id: NamespaceId) -> Vec<u8> {
        let p = [0xEF];
        let n = namespace_id.0.to_be_bytes();
        vec![p[0], n[0], n[1], n[2], n[3]]
    }
    /// Generate a key for a function.
    pub fn function_key(function_id: FunctionId) -> Vec<u8> {
        let p = [0xEE];
        let f = function_id.0.to_be_bytes();
        vec![p[0], f[0], f[1], f[2], f[3]]
    }
    /// Generate a key for a table.
    pub fn table_key(table_id: TableId) -> Vec<u8> {
        let p = [0xED];
        let t = table_id.0.to_be_bytes();
        vec![p[0], t[0], t[1], t[2], t[3]]
    }
    /// Generate a key for a shard.
    pub fn shard_key(shard_id: ShardId) -> Vec<u8> {
        let p = [0xEC];
        let s = shard_id.0.to_be_bytes();
        vec![p[0], s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]
    }
}
