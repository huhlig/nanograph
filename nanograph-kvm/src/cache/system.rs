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
    DatabaseId, DatabaseRecord, RegionId, ServerId, ShardId, SystemGroupId, SystemGroupRecord,
    SystemRoleId, SystemRoleRecord, SystemUserRecord, TablespaceId, TablespaceRecord,
    TenantGroupId, TenantGroupRecord, TenantId, TenantRecord, TenantRoleId, TenantRoleRecord,
    TenantUserRecord, UserId,
};
use nanograph_raft::{ClusterRecord, RegionRecord, ServerRecord};
use std::collections::HashMap;

/// Cache for all system-wide metadata.
///
/// This structure maintains an in-memory representation of the system's metadata, including
/// clusters, regions, servers, tenants, databases, and system-level users.
pub struct SystemMetadataCache {
    /// Shard Containing Global System Tables
    shard: ShardId,
    /// Cluster metadata
    cluster: Option<ClusterRecord>,
    /// Regions in the cluster
    regions: HashMap<RegionId, RegionRecord>,
    /// Servers in the cluster
    servers: HashMap<ServerId, ServerRecord>,
    /// Tenants in the cluster
    tenants: HashMap<TenantId, TenantRecord>,
    /// Databases in the cluster
    databases: HashMap<DatabaseId, DatabaseRecord>,
    /// Tablespaces in the cluster
    tablespaces: HashMap<TablespaceId, TablespaceRecord>,
    /// Global System Users
    system_users: HashMap<UserId, SystemUserRecord>,
    /// Global System Roles
    system_roles: HashMap<SystemRoleId, SystemRoleRecord>,
    /// Global System Roles
    system_groups: HashMap<SystemGroupId, SystemGroupRecord>,
    /// Global System Users
    tenant_users: HashMap<TenantId, HashMap<UserId, TenantUserRecord>>,
    /// Global System Roles
    tenant_roles: HashMap<TenantId, HashMap<TenantRoleId, TenantRoleRecord>>,
    /// Global System Roles
    tenant_groups: HashMap<TenantId, HashMap<TenantGroupId, TenantGroupRecord>>,
}

impl SystemMetadataCache {
    /// Create a new `SystemMetadataCache`.
    pub fn new(shard: ShardId) -> Self {
        Self {
            shard,
            cluster: Default::default(),
            regions: Default::default(),
            servers: Default::default(),
            tenants: Default::default(),
            databases: Default::default(),
            tablespaces: Default::default(),
            system_users: Default::default(),
            system_roles: Default::default(),
            system_groups: Default::default(),
            tenant_users: Default::default(),
            tenant_roles: Default::default(),
            tenant_groups: Default::default(),
        }
    }

    /// Returns the shard identifier.
    pub fn shard_id(&self) -> ShardId {
        self.shard
    }

    /// Returns a reference to the cluster record if it exists.
    pub fn get_cluster_record(&self) -> Option<&ClusterRecord> {
        self.cluster.as_ref()
    }

    /// Sets the cluster record.
    pub fn set_cluster_record(&mut self, record: ClusterRecord) {
        self.cluster = Some(record);
    }

    /// Clears the cluster record.
    pub fn clear_cluster_record(&mut self) {
        self.cluster = None;
    }

    /// Returns an iterator over all region records.
    pub fn list_region_records(&self) -> impl Iterator<Item = &RegionRecord> {
        self.regions.values()
    }

    /// Returns a reference to a specific region record if it exists.
    pub fn get_region_record(&self, record_id: &RegionId) -> Option<&RegionRecord> {
        self.regions.get(record_id)
    }

    /// Sets a region record.
    pub fn set_region_record(&mut self, record: RegionRecord) {
        self.regions.insert(record.id, record);
    }

    /// Clears a specific region record.
    pub fn clear_region_record(&mut self, record_id: &RegionId) {
        self.regions.remove(record_id);
    }

    /// Clears all region records.
    pub fn clear_region_records(&mut self) {
        self.regions.clear();
    }

    /// Returns an iterator over all server records.
    pub fn list_server_records(&self) -> impl Iterator<Item = &ServerRecord> {
        self.servers.values()
    }

    /// Returns a reference to a specific server record if it exists.
    pub fn get_server_record(&self, record_id: &ServerId) -> Option<&ServerRecord> {
        self.servers.get(record_id)
    }

    /// Sets a server record.
    pub fn set_server_record(&mut self, record: ServerRecord) {
        self.servers.insert(record.id.server_id(), record);
    }

    /// Clears a specific server record.
    pub fn clear_server_record(&mut self, record_id: &ServerId) {
        self.servers.remove(record_id);
    }

    /// Clears all server records.
    pub fn clear_server_records(&mut self) {
        self.servers.clear()
    }

    /// Returns an iterator over all tenant records.
    pub fn list_tenant_records(&self) -> impl Iterator<Item = &TenantRecord> {
        self.tenants.values()
    }

    /// Returns a reference to a specific tenant record if it exists.
    pub fn get_tenant_record(&self, record_id: &TenantId) -> Option<&TenantRecord> {
        self.tenants.get(record_id)
    }

    /// Sets a tenant record.
    pub fn set_tenant_record(&mut self, record: TenantRecord) {
        self.tenants.insert(record.id, record);
    }

    /// Clears a specific tenant record.
    pub fn clear_tenant_record(&mut self, record_id: &TenantId) {
        self.tenants.remove(record_id);
    }

    /// Clears all tenant records.
    pub fn clear_tenant_records(&mut self) {
        self.tenants.clear()
    }

    /// Returns an iterator over all database records.
    pub fn list_database_records(&self) -> impl Iterator<Item = &DatabaseRecord> {
        self.databases.values()
    }

    /// Returns a reference to a specific database record if it exists.
    pub fn get_database_record(&self, record_id: &DatabaseId) -> Option<&DatabaseRecord> {
        self.databases.get(record_id)
    }

    /// Sets a database record.
    pub fn set_database_record(&mut self, record: DatabaseRecord) {
        self.databases.insert(record.id, record);
    }

    /// Clears a specific database record.
    pub fn clear_database_record(&mut self, record_id: &DatabaseId) {
        self.databases.remove(record_id);
    }

    /// Clears all database records.
    pub fn clear_database_records(&mut self) {
        self.databases.clear();
    }

    /// Returns an iterator over all tablespace records.
    pub fn list_tablespace_records(&self) -> impl Iterator<Item = &TablespaceRecord> {
        self.tablespaces.values()
    }

    /// Returns a reference to a specific tablespace record if it exists.
    pub fn get_tablespace_record(&self, record_id: &TablespaceId) -> Option<&TablespaceRecord> {
        self.tablespaces.get(record_id)
    }

    /// Sets a tablespace record.
    pub fn set_tablespace_record(&mut self, record: TablespaceRecord) {
        self.tablespaces.insert(record.id, record);
    }

    /// Clears a specific tablespace record.
    pub fn clear_tablespace_record(&mut self, record_id: &TablespaceId) {
        self.tablespaces.remove(record_id);
    }

    /// Clears all tablespace records.
    pub fn clear_tablespace_records(&mut self) {
        self.tablespaces.clear();
    }

    /// Returns an iterator over all system user records.
    pub fn list_system_user_records(&self) -> impl Iterator<Item = &SystemUserRecord> {
        self.system_users.values()
    }

    /// Returns a reference to a specific system user record if it exists.
    pub fn get_system_user_record(&self, record_id: &UserId) -> Option<&SystemUserRecord> {
        self.system_users.get(record_id)
    }

    /// Sets a system user record.
    pub fn set_system_user_record(&mut self, record: SystemUserRecord) {
        self.system_users.insert(record.id, record);
    }

    /// Clears a specific system user record.
    pub fn clear_system_user_record(&mut self, record_id: &UserId) {
        self.system_users.remove(record_id);
    }

    /// Clears all system user records.
    pub fn clear_system_user_records(&mut self) {
        self.system_users.clear();
    }

    /// Returns an iterator over all system role records.
    pub fn list_system_role_records(&self) -> impl Iterator<Item = &SystemRoleRecord> {
        self.system_roles.values()
    }

    /// Returns a reference to a specific system role record if it exists.
    pub fn get_system_role_record(&self, record_id: &SystemRoleId) -> Option<&SystemRoleRecord> {
        self.system_roles.get(record_id)
    }

    /// Sets a system role record.
    pub fn set_system_role_record(&mut self, record: SystemRoleRecord) {
        self.system_roles.insert(record.id, record);
    }

    /// Clears a specific system role record.
    pub fn clear_system_role_record(&mut self, record_id: &SystemRoleId) {
        self.system_roles.remove(record_id);
    }

    /// Clears all system role records.
    pub fn clear_system_role_records(&mut self) {
        self.system_roles.clear();
    }

    /// Returns an iterator over all system group records.
    pub fn list_system_group_records(&self) -> impl Iterator<Item = &SystemGroupRecord> {
        self.system_groups.values()
    }

    /// Returns a reference to a specific system group record if it exists.
    pub fn get_system_group_record(&self, record_id: &SystemGroupId) -> Option<&SystemGroupRecord> {
        self.system_groups.get(record_id)
    }

    /// Sets a system group record.
    pub fn set_system_groups_record(&mut self, record: SystemGroupRecord) {
        self.system_groups.insert(record.id, record);
    }

    /// Clears a specific system group record.
    pub fn clear_system_group_record(&mut self, record_id: &SystemGroupId) {
        self.system_groups.remove(record_id);
    }

    /// Clears all system group records.
    pub fn clear_system_group_records(&mut self) {
        self.system_groups.clear();
    }

    /// Returns an iterator over all user records for a specific tenant.
    pub fn list_tenant_user_records(
        &self,
        tenant: &TenantId,
    ) -> impl IntoIterator<Item = &TenantUserRecord> {
        if let Some(tenant_users) = self.tenant_users.get(tenant) {
            tenant_users.values()
        } else {
            std::collections::hash_map::Values::default()
        }
    }

    /// Returns a reference to a specific tenant user record if it exists.
    pub fn get_tenant_user_record(
        &self,
        tenant: &TenantId,
        record_id: &UserId,
    ) -> Option<&TenantUserRecord> {
        self.tenant_users
            .get(tenant)
            .and_then(|users| users.get(record_id))
    }

    /// Sets a tenant user record.
    pub fn set_tenant_user_record(&mut self, tenant: &TenantId, record: TenantUserRecord) {
        self.tenant_users
            .entry(*tenant)
            .or_insert(HashMap::default())
            .insert(record.user, record);
    }

    /// Clears a specific tenant user record.
    pub fn clear_tenant_user_record(&mut self, tenant: &TenantId, record_id: &UserId) {
        if let Some(users) = self.tenant_users.get_mut(tenant) {
            users.remove(record_id);
        }
    }

    /// Clears all user records for a specific tenant.
    pub fn clear_tenant_user_records(&mut self, tenant: &TenantId) {
        if let Some(users) = self.tenant_users.get_mut(tenant) {
            users.clear();
        }
    }

    /// Returns an iterator over all role records for a specific tenant.
    pub fn list_tenant_role_records(
        &self,
        tenant: &TenantId,
    ) -> impl Iterator<Item = &TenantRoleRecord> {
        if let Some(tenant_users) = self.tenant_roles.get(tenant) {
            tenant_users.values()
        } else {
            std::collections::hash_map::Values::default()
        }
    }

    /// Returns a reference to a specific tenant role record if it exists.
    pub fn get_tenant_role_record(
        &self,
        tenant: &TenantId,
        record_id: &TenantRoleId,
    ) -> Option<&TenantRoleRecord> {
        self.tenant_roles
            .get(tenant)
            .and_then(|users| users.get(record_id))
    }

    /// Sets a tenant role record.
    pub fn set_tenant_role_record(&mut self, record: TenantRoleRecord) {
        if let Some(roles) = self.tenant_roles.get_mut(&record.tenant) {
            roles.insert(record.id, record);
        }
    }

    /// Clears a specific tenant role record.
    pub fn clear_tenant_role_record(&mut self, tenant: &TenantId, record_id: &TenantRoleId) {
        if let Some(roles) = self.tenant_roles.get_mut(tenant) {
            roles.remove(record_id);
        }
    }

    /// Clears all role records for a specific tenant.
    pub fn clear_tenant_role_records(&mut self, tenant: &TenantId) {
        if let Some(roles) = self.tenant_roles.get_mut(tenant) {
            roles.clear();
        }
    }

    /// Returns an iterator over all group records for a specific tenant.
    pub fn list_tenant_group_records(
        &self,
        tenant: &TenantId,
    ) -> impl IntoIterator<Item = &TenantGroupRecord> {
        if let Some(tenant_groups) = self.tenant_groups.get(tenant) {
            tenant_groups.values()
        } else {
            std::collections::hash_map::Values::default()
        }
    }

    /// Returns a reference to a specific tenant group record if it exists.
    pub fn get_tenant_group_record(
        &self,
        tenant: &TenantId,
        record_id: &TenantGroupId,
    ) -> Option<&TenantGroupRecord> {
        self.tenant_groups
            .get(tenant)
            .and_then(|groups| groups.get(record_id))
    }

    /// Sets a tenant group record.
    pub fn set_tenant_group_record(&mut self, tenant: &TenantId, record: TenantGroupRecord) {
        self.tenant_groups
            .entry(*tenant)
            .or_insert(HashMap::default())
            .insert(record.id, record);
    }

    /// Clears a specific tenant group record.
    pub fn clear_tenant_group_record(&mut self, tenant: &TenantId, record_id: &TenantGroupId) {
        if let Some(groups) = self.tenant_groups.get_mut(tenant) {
            groups.remove(record_id);
        }
    }

    /// Clears all group records for a specific tenant.
    pub fn clear_tenant_group_records(&mut self, tenant: &TenantId) {
        if let Some(groups) = self.tenant_groups.get_mut(tenant) {
            groups.clear();
        }
    }
}

impl Default for SystemMetadataCache {
    /// Returns the default `SystemMetastore`.
    fn default() -> Self {
        SystemMetadataCache {
            shard: Default::default(),
            cluster: None,
            regions: Default::default(),
            servers: Default::default(),
            tenants: Default::default(),
            databases: Default::default(),
            tablespaces: Default::default(),
            system_users: Default::default(),
            system_roles: Default::default(),
            system_groups: Default::default(),
            tenant_users: Default::default(),
            tenant_roles: Default::default(),
            tenant_groups: Default::default(),
        }
    }
}

impl std::fmt::Debug for SystemMetadataCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemMetastore")
            .field("cluster", &self.cluster.as_ref().map(|c| &c.name))
            .field("regions", &self.regions.len())
            .field("servers", &self.servers.len())
            .field("tenants", &self.tenants.len())
            .field("databases", &self.databases.len())
            .field("system_users", &self.system_users.len())
            .field("system_roles", &self.system_roles.len())
            .field("system_groups", &self.system_groups.len())
            .field("tenant_users", &self.tenant_users.len())
            .field("tenant_roles", &self.tenant_roles.len())
            .field("tenant_groups", &self.tenant_groups.len())
            .finish()
    }
}
