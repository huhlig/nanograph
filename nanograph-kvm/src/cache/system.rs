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
    ContainerId, DatabaseId, DatabaseRecord, NamespaceId, NodeId, RegionId, ShardId, SystemGroupId,
    SystemGroupRecord, SystemRoleId, SystemRoleRecord, SystemUserRecord, TablespaceId,
    TablespaceRecord, TenantGroupId, TenantGroupRecord, TenantId, TenantRecord, TenantRoleId,
    TenantRoleRecord, TenantUserRecord, UserId,
};
use nanograph_raft::{ClusterRecord, RegionRecord, ServerRecord};
use nanograph_util::CacheMap;
use std::time::Duration;

/// Cache for all system-wide metadata.
///
/// This structure maintains an in-memory representation of the system's metadata, including
/// clusters, regions, servers, tenants, databases, and system-level users.
pub struct SystemMetadataCache {
    /// Server Node
    node_id: NodeId,
    /// Shard Containing Global System Tables
    shard_id: ShardId,
    /// Cache Time to Live
    cache_ttl: Duration,
    /**********************************************************************************************\
     * Global Record Cache                                                                        *
    \**********************************************************************************************/
    /// Cluster metadata
    cluster: Option<ClusterRecord>,
    /// Regions in the cluster
    regions: CacheMap<RegionId, RegionRecord>,
    /// Servers in the cluster
    servers: CacheMap<NodeId, ServerRecord>,
    /// Tenants in the cluster
    tenants: CacheMap<TenantId, TenantRecord>,
    /// Databases in the cluster
    databases: CacheMap<ContainerId, DatabaseRecord>,
    /// Tablespaces in the cluster
    tablespaces: CacheMap<TablespaceId, TablespaceRecord>,
    /// Global System Users
    system_users: CacheMap<UserId, SystemUserRecord>,
    /// Global System Roles
    system_roles: CacheMap<SystemRoleId, SystemRoleRecord>,
    /// Global System Roles
    system_groups: CacheMap<SystemGroupId, SystemGroupRecord>,
    /// Global System Users
    tenant_users: CacheMap<TenantId, CacheMap<UserId, TenantUserRecord>>,
    /// Global System Roles
    tenant_roles: CacheMap<TenantId, CacheMap<TenantRoleId, TenantRoleRecord>>,
    /// Global System Roles
    tenant_groups: CacheMap<TenantId, CacheMap<TenantGroupId, TenantGroupRecord>>,
    /**********************************************************************************************\
     * Global Name Indices                                                                        *
    \**********************************************************************************************/
    /// Global Username Lookup
    username_index: CacheMap<String, UserId>,
    /// Global Tablespace Lookup
    tablespace_index: CacheMap<String, TablespaceId>,
    /// Global System Role Lookup
    system_role_index: CacheMap<String, SystemRoleId>,
    /// Global System Role Lookup
    system_group_index: CacheMap<String, SystemGroupId>,
    /// Global Tenant Lookup
    tenant_index: CacheMap<String, TenantId>,
    /// Global System Role Lookup
    tenant_role_index: CacheMap<String, TenantRoleId>,
    /// Global System Role Lookup
    tenant_group_index: CacheMap<String, TenantGroupId>,
    /// Per Tenant Database Lookup
    database_index: CacheMap<TenantId, CacheMap<String, DatabaseId>>,
    /// List of Root Namespaces
    root_namespace_index: CacheMap<ContainerId, NamespaceId>,
}

impl SystemMetadataCache {
    /// Create a new `SystemMetadataCache`.
    pub fn new(node_id: NodeId, shard_id: ShardId, cache_ttl: Duration) -> Self {
        Self {
            node_id,
            shard_id,
            cache_ttl,
            cluster: Default::default(),
            regions: CacheMap::new(cache_ttl),
            servers: CacheMap::new(cache_ttl),
            tenants: CacheMap::new(cache_ttl),
            databases: CacheMap::new(cache_ttl),
            tablespaces: CacheMap::new(cache_ttl),
            system_users: CacheMap::new(cache_ttl),
            system_roles: CacheMap::new(cache_ttl),
            system_groups: CacheMap::new(cache_ttl),
            tenant_users: CacheMap::new(cache_ttl),
            tenant_roles: CacheMap::new(cache_ttl),
            tenant_groups: CacheMap::new(cache_ttl),
            username_index: CacheMap::new(cache_ttl),
            tablespace_index: CacheMap::new(cache_ttl),
            system_role_index: CacheMap::new(cache_ttl),
            system_group_index: CacheMap::new(cache_ttl),
            tenant_index: CacheMap::new(cache_ttl),
            tenant_role_index: CacheMap::new(cache_ttl),
            tenant_group_index: CacheMap::new(cache_ttl),
            database_index: CacheMap::new(cache_ttl),
            root_namespace_index: CacheMap::new(cache_ttl),
        }
    }

    /// Returns the shard identifier.
    pub fn shard_id(&self) -> ShardId {
        self.shard_id
    }

    /// Returns a reference to the cluster record if it exists.
    pub fn get_cluster_record(&self) -> Option<&ClusterRecord> {
        self.cluster.as_ref()
    }

    /// Sets the cluster record.
    pub fn set_cluster_record(&mut self, cluster_record: ClusterRecord) {
        self.cluster = Some(cluster_record);
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
    pub fn get_region_record(&self, region_id: &RegionId) -> Option<&RegionRecord> {
        self.regions.get(region_id)
    }

    /// Sets a region record.
    pub fn set_region_record(&mut self, region_record: RegionRecord) {
        self.regions.insert(region_record.id, region_record);
    }

    /// Clears a specific region record.
    pub fn clear_region_record(&mut self, region_id: &RegionId) {
        self.regions.remove(region_id);
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
    pub fn get_server_record(&self, node_id: &NodeId) -> Option<&ServerRecord> {
        self.servers.get(node_id)
    }

    /// Sets a server record.
    pub fn set_server_record(&mut self, server_record: ServerRecord) {
        self.servers.insert(server_record.id, server_record);
    }

    /// Clears a specific server record.
    pub fn clear_server_record(&mut self, node_id: &NodeId) {
        self.servers.remove(node_id);
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
    pub fn get_tenant_by_name(&self, name: &str) -> Option<&TenantRecord> {
        let tenant_id = self.tenant_index.get(&name.to_string())?;
        self.tenants.get(tenant_id)
    }

    /// Returns a reference to a specific tenant record if it exists.
    pub fn get_tenant_record(&self, tenant_id: &TenantId) -> Option<&TenantRecord> {
        self.tenants.get(tenant_id)
    }

    /// Sets a tenant record.
    pub fn set_tenant_record(&mut self, tenant_record: TenantRecord) {
        self.tenant_index
            .insert(tenant_record.name.clone(), tenant_record.id);
        self.tenants.insert(tenant_record.id, tenant_record);
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
    pub fn get_database_record(&self, container_id: &ContainerId) -> Option<&DatabaseRecord> {
        self.databases.get(container_id)
    }

    /// Sets a database record.
    pub fn set_database_record(&mut self, database_record: DatabaseRecord) {
        self.databases.insert(
            ContainerId::from_parts(database_record.tenant_id, database_record.database_id),
            database_record,
        );
    }

    /// Clears a specific database record.
    pub fn clear_database_record(&mut self, container_id: &ContainerId) {
        self.databases.remove(container_id);
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
    pub fn get_tablespace_by_name(&self, name: &str) -> Option<&TablespaceRecord> {
        let tablespace_id = self.tablespace_index.get(&name.to_string())?;
        self.tablespaces.get(tablespace_id)
    }

    /// Returns a reference to a specific tablespace record if it exists.
    pub fn get_tablespace_record(&self, tablespace_id: &TablespaceId) -> Option<&TablespaceRecord> {
        self.tablespaces.get(tablespace_id)
    }

    /// Sets a tablespace record.
    pub fn set_tablespace_record(&mut self, tablespace_record: TablespaceRecord) {
        self.tablespace_index
            .insert(tablespace_record.name.clone(), tablespace_record.id);
        self.tablespaces
            .insert(tablespace_record.id, tablespace_record);
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
    pub fn get_system_user_by_name(&self, name: &str) -> Option<&SystemUserRecord> {
        let record_id = self.username_index.get(&name.to_string())?;
        self.system_users.get(record_id)
    }

    /// Returns a reference to a specific system user record if it exists.
    pub fn get_system_user_record(&self, record_id: &UserId) -> Option<&SystemUserRecord> {
        self.system_users.get(record_id)
    }

    /// Sets a system user record.
    pub fn set_system_user_record(&mut self, system_user_record: SystemUserRecord) {
        self.username_index.insert(
            system_user_record.username.clone(),
            system_user_record.user_id,
        );
        self.system_users
            .insert(system_user_record.user_id, system_user_record);
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
    ) -> Box<dyn Iterator<Item = &TenantUserRecord> + '_> {
        if let Some(tenant_users) = self.tenant_users.peek(tenant) {
            Box::new(tenant_users.values())
        } else {
            Box::new(std::iter::empty())
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
            .or_insert(CacheMap::new(self.cache_ttl))
            .insert(record.user_id, record);
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
    ) -> Box<dyn Iterator<Item = &TenantRoleRecord> + '_> {
        if let Some(tenant_roles) = self.tenant_roles.peek(tenant) {
            Box::new(tenant_roles.values())
        } else {
            Box::new(std::iter::empty())
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
    ) -> Box<dyn Iterator<Item = &TenantGroupRecord> + '_> {
        if let Some(tenant_groups) = self.tenant_groups.peek(tenant) {
            Box::new(tenant_groups.values())
        } else {
            Box::new(std::iter::empty())
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
            .or_insert(CacheMap::new(self.cache_ttl))
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
    /// Returns the default `SystemMetastore` for standalone and testing use.
    fn default() -> Self {
        Self::new(
            NodeId::new(0),
            ShardId::new(0),
            Duration::from_secs(60 * 60),
        )
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
