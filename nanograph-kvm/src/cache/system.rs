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
    DatabaseId, DatabaseMetadata, RegionId, ServerId, ShardId, TablespaceId, TablespaceMetadata,
    TenantId, TenantMetadata, UserId, UserMetadata,
};
use nanograph_raft::{ClusterMetadata, RegionMetadata, ServerMetadata};
use std::collections::HashMap;

/// Cache for all system-wide metadata.
///
/// This structure maintains an in-memory representation of the system's metadata, including
/// clusters, regions, servers, tenants, databases, and system-level users.
pub struct SystemMetadataCache {
    /// Shard Containing Global System Tables
    shard: ShardId,
    /// Cluster metadata
    cluster: Option<ClusterMetadata>,
    /// Regions in the cluster
    regions: HashMap<RegionId, RegionMetadata>,
    /// Servers in the cluster
    servers: HashMap<ServerId, ServerMetadata>,
    /// Tenants in the cluster
    tenants: HashMap<TenantId, TenantMetadata>,
    /// Databases in the cluster
    databases: HashMap<DatabaseId, DatabaseMetadata>,
    /// Tablespaces in the cluster
    tablespaces: HashMap<TablespaceId, TablespaceMetadata>,
    /// Global System Users
    super_users: HashMap<UserId, UserMetadata>,
}

impl SystemMetadataCache {
    /// Create a new System Metastore Cache.
    pub fn new(shard: ShardId) -> Self {
        Self {
            shard,
            cluster: Default::default(),
            regions: Default::default(),
            servers: Default::default(),
            tenants: Default::default(),
            databases: Default::default(),
            tablespaces: Default::default(),
            super_users: Default::default(),
        }
    }

    pub fn shard_id(&self) -> ShardId {
        self.shard
    }

    /// Returns a reference to the cluster metadata.
    pub fn get_cluster_record(&self) -> Option<&ClusterMetadata> {
        self.cluster.as_ref()
    }

    /// Returns a reference to the cluster metadata.
    pub fn set_cluster_record(&mut self, record: ClusterMetadata) {
        self.cluster = Some(record);
    }

    pub fn clear_cluster_record(&mut self) {
        self.cluster = None;
    }

    /// Returns an iterator over all region records.
    pub fn list_region_records(&self) -> impl Iterator<Item = &RegionMetadata> {
        self.regions.values()
    }

    /// Returns a reference to the record for a specific region if it exists.
    pub fn get_region_record(&self, record_id: &RegionId) -> Option<&RegionMetadata> {
        self.regions.get(record_id)
    }

    pub fn set_region_record(&mut self, record: RegionMetadata) {
        self.regions.insert(record.id, record);
    }

    pub fn clear_region_record(&mut self, record_id: &RegionId) {
        self.regions.remove(record_id);
    }

    pub fn clear_region_records(&mut self) {
        self.regions.clear();
    }

    /// Returns an iterator over all server metadata.
    pub fn list_server_records(&self) -> impl Iterator<Item = &ServerMetadata> {
        self.servers.values()
    }

    /// Returns a reference to the metadata for a specific server if it exists.
    pub fn get_server_record(&self, record_id: &ServerId) -> Option<&ServerMetadata> {
        self.servers.get(record_id)
    }

    pub fn set_server_record(&mut self, record: ServerMetadata) {
        self.servers.insert(record.id.server_id(), record);
    }
    pub fn clear_server_record(&mut self, record_id: &ServerId) {
        self.servers.remove(record_id);
    }
    pub fn clear_server_records(&mut self) {
        self.servers.clear()
    }

    /// Returns an iterator over all tenant records.
    pub fn list_tenant_records(&self) -> impl Iterator<Item = &TenantMetadata> {
        self.tenants.values()
    }

    /// Returns a reference to the metadata for a specific tenant if it exists.
    pub fn get_tenant_record(&self, record_id: &TenantId) -> Option<&TenantMetadata> {
        self.tenants.get(record_id)
    }
    pub fn set_tenant_record(&mut self, record: TenantMetadata) {
        self.tenants.insert(record.id, record);
    }
    pub fn clear_tenant_record(&mut self, record_id: &TenantId) {
        self.tenants.remove(record_id);
    }
    pub fn clear_tenant_records(&mut self) {
        self.tenants.clear()
    }

    /// Returns an iterator over all database records.
    pub fn list_database_records(&self) -> impl Iterator<Item = &DatabaseMetadata> {
        self.databases.values()
    }

    /// Returns a reference to the metadata for a specific database if it exists.
    pub fn get_database_record(&self, record_id: &DatabaseId) -> Option<&DatabaseMetadata> {
        self.databases.get(record_id)
    }
    pub fn set_database_record(&mut self, record: DatabaseMetadata) {
        self.databases.insert(record.id, record);
    }
    pub fn clear_database_record(&mut self, record_id: &DatabaseId) {
        self.databases.remove(record_id);
    }
    pub fn clear_database_records(&mut self) {
        self.databases.clear();
    }

    /// Returns an iterator over all tablespace records.
    pub fn list_tablespace_records(&self) -> impl Iterator<Item = &TablespaceMetadata> {
        self.tablespaces.values()
    }

    /// Returns a reference to the metadata for a specific tablespace if it exists.
    pub fn get_tablespace_record(&self, record_id: &TablespaceId) -> Option<&TablespaceMetadata> {
        self.tablespaces.get(record_id)
    }

    pub fn set_tablespace_record(&mut self, record: TablespaceMetadata) {
        self.tablespaces.insert(record.id, record);
    }

    pub fn clear_tablespace_record(&mut self, record_id: &TablespaceId) {
        self.tablespaces.remove(record_id);
    }

    pub fn clear_tablespace_records(&mut self) {
        self.tablespaces.clear();
    }

    /// Returns an iterator over all user records.
    pub fn list_user_records(&self) -> impl Iterator<Item = &UserMetadata> {
        self.super_users.values()
    }

    /// Returns a reference to the metadata for a specific user if it exists.
    pub fn get_user_record(&self, record_id: &UserId) -> Option<&UserMetadata> {
        self.super_users.get(record_id)
    }

    pub fn set_user_record(&mut self, record: UserMetadata) {
        self.super_users.insert(record.id, record);
    }

    pub fn clear_user_record(&mut self, record_id: &UserId) {
        self.super_users.remove(record_id);
    }

    pub fn clear_user_records(&mut self) {
        self.super_users.clear();
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
            super_users: Default::default(),
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
            .field("system_users", &self.super_users.len())
            .finish()
    }
}
