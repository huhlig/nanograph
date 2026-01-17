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

use crate::context::KeyValueDatabaseContext;
use nanograph_core::object::{
    ClusterCreate, ClusterMetadata, ClusterUpdate, NodeId, RegionCreate, RegionId, RegionMetadata,
    RegionUpdate, SecurityPrincipal, ServerCreate, ServerMetadata, ServerUpdate, SystemUserCreate,
    SystemUserMetadata, SystemUserUpdate, TablespaceCreate, TablespaceId, TablespaceRecord,
    TablespaceUpdate, TenantCreate, TenantId, TenantMetadata, TenantUpdate, UserId,
};
use nanograph_kvt::KeyValueResult;
use std::sync::Arc;

/// A handle for an authenticated system administrator.
///
/// `SystemHandle` provides administrative access to the entire cluster, including
/// managing tenants, regions, servers, and system-wide settings.
///
/// # Usage
///
/// You typically obtain a `SystemHandle` from a `KeyValueDatabaseManager`:
///
/// ```ignore
/// let system = manager.get_system_handle(&principal).await?;
/// ```
///
/// # Operations
///
/// A `SystemHandle` allows you to:
/// - Manage the cluster and regions
/// - Manage servers and nodes
/// - Manage system users
/// - Manage tenants and their users
/// - Manage global tablespaces
///
/// # Thread Safety
///
/// `SystemHandle` is safe to clone and share across threads. All operations are
/// internally synchronized.
pub struct SystemHandle {
    context: Arc<KeyValueDatabaseContext>,
    principal: SecurityPrincipal,
}

impl SystemHandle {
    pub(crate) fn new(context: Arc<KeyValueDatabaseContext>, principal: SecurityPrincipal) -> Self {
        Self { context, principal }
    }

    //
    // Cluster Management
    //

    /// Initialize the cluster with the given configuration.
    pub async fn initialize_cluster(
        &self,
        config: ClusterCreate,
    ) -> KeyValueResult<ClusterMetadata> {
        self.context
            .initialize_cluster(&self.principal, config)
            .await
    }

    /// Get the current cluster metadata.
    pub async fn get_cluster(&self) -> KeyValueResult<ClusterMetadata> {
        self.context.get_cluster(&self.principal).await
    }

    /// Update the cluster configuration.
    pub async fn update_cluster(&self, config: ClusterUpdate) -> KeyValueResult<ClusterMetadata> {
        self.context.update_cluster(&self.principal, config).await
    }

    //
    // Region Management
    //

    /// List all regions in the cluster.
    pub async fn get_regions(&self) -> KeyValueResult<impl IntoIterator<Item = RegionMetadata>> {
        self.context.get_regions(&self.principal).await
    }

    /// Get metadata for a specific region.
    pub async fn get_region(&self, region_id: RegionId) -> KeyValueResult<Option<RegionMetadata>> {
        self.context.get_region(&self.principal, region_id).await
    }

    /// Add a new region to the cluster.
    pub async fn add_region(&self, config: RegionCreate) -> KeyValueResult<RegionMetadata> {
        self.context.add_region(&self.principal, config).await
    }

    /// Update an existing region.
    pub async fn update_region(
        &self,
        region_id: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionMetadata> {
        self.context
            .update_region(&self.principal, region_id, config)
            .await
    }

    /// Remove a region from the cluster.
    pub async fn remove_region(&self, region_id: &RegionId) -> KeyValueResult<()> {
        self.context.remove_region(&self.principal, region_id).await
    }

    //
    // Server Management
    //

    /// List all servers in the cluster.
    pub async fn get_servers(&self) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        self.context.get_servers(&self.principal).await
    }

    /// List servers in a specific region.
    pub async fn get_servers_by_region(
        &self,
        region_id: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item = NodeId>> {
        self.context
            .get_servers_by_region(&self.principal, region_id)
            .await
    }

    /// Get metadata for a specific server.
    pub async fn get_server(&self, node_id: &NodeId) -> KeyValueResult<Option<ServerMetadata>> {
        self.context.get_server(&self.principal, node_id).await
    }

    /// Add a new server to the cluster.
    pub async fn add_server(&self, config: ServerCreate) -> KeyValueResult<ServerMetadata> {
        self.context.add_server(&self.principal, config).await
    }

    /// Update an existing server.
    pub async fn update_server(
        &self,
        node_id: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerMetadata> {
        self.context
            .update_server(&self.principal, node_id, config)
            .await
    }

    /// Remove a server from the cluster.
    pub async fn remove_server(&self, node_id: &NodeId) -> KeyValueResult<()> {
        self.context.remove_server(&self.principal, node_id).await
    }

    //
    // System User Management
    //

    /// List all system users.
    pub async fn get_system_users(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = SystemUserMetadata>> {
        self.context.get_system_users(&self.principal).await
    }

    /// Get metadata for a specific system user.
    pub async fn get_system_user(
        &self,
        user_id: &UserId,
    ) -> KeyValueResult<Option<SystemUserMetadata>> {
        self.context.get_system_user(&self.principal, user_id).await
    }

    /// Get a user ID by their username.
    pub async fn get_user_by_username(&self, username: &str) -> KeyValueResult<Option<UserId>> {
        self.context
            .get_user_by_username(&self.principal, username)
            .await
    }

    /// Create a new system user.
    pub async fn create_system_user(
        &self,
        config: SystemUserCreate,
    ) -> KeyValueResult<SystemUserMetadata> {
        self.context
            .create_system_user(&self.principal, config)
            .await
    }

    /// Update an existing system user.
    pub async fn update_system_user(
        &self,
        user_id: &UserId,
        config: SystemUserUpdate,
    ) -> KeyValueResult<SystemUserMetadata> {
        self.context
            .update_system_user(&self.principal, user_id, config)
            .await
    }

    /// Remove a system user.
    pub async fn remove_system_user(&self, user_id: &UserId) -> KeyValueResult<()> {
        self.context
            .remove_system_user(&self.principal, user_id)
            .await
    }

    //
    // Tenant Management
    //

    /// List all tenants.
    pub async fn get_tenants(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = (TenantId, String)>> {
        self.context.get_tenants(&self.principal).await
    }

    /// Get metadata for a specific tenant.
    pub async fn get_tenant(&self, tenant_id: &TenantId) -> KeyValueResult<Option<TenantMetadata>> {
        self.context.get_tenant(&self.principal, tenant_id).await
    }

    /// Get a tenant ID by its name.
    pub async fn get_tenant_by_name(&self, name: &str) -> KeyValueResult<Option<TenantId>> {
        self.context.get_tenant_by_name(&self.principal, name).await
    }

    /// Create a new tenant.
    pub async fn create_tenant(&self, config: TenantCreate) -> KeyValueResult<TenantMetadata> {
        self.context.create_tenant(&self.principal, config).await
    }

    /// Update an existing tenant.
    pub async fn update_tenant(
        &self,
        tenant_id: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantMetadata> {
        self.context
            .update_tenant(&self.principal, tenant_id, config)
            .await
    }

    /// Delete a tenant.
    pub async fn delete_tenant(&self, tenant_id: &TenantId) -> KeyValueResult<()> {
        self.context.delete_tenant(&self.principal, tenant_id).await
    }

    //
    // Tablespace Management
    //

    /// List all tablespaces.
    pub async fn get_tablespaces(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = (TablespaceId, String)>> {
        self.context.get_tablespaces(&self.principal).await
    }

    /// Get metadata for a specific tablespace.
    pub async fn get_tablespace(
        &self,
        tablespace_id: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceRecord>> {
        self.context
            .get_tablespace(&self.principal, tablespace_id)
            .await
    }

    /// Get a tablespace ID by its name.
    pub async fn get_tablespace_by_name(&self, name: &str) -> KeyValueResult<Option<TablespaceId>> {
        self.context
            .get_tablespace_by_name(&self.principal, name)
            .await
    }

    /// Create a new tablespace.
    pub async fn create_tablespace(
        &self,
        config: TablespaceCreate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context
            .create_tablespace(&self.principal, config)
            .await
    }

    /// Update an existing tablespace.
    pub async fn update_tablespace(
        &self,
        tablespace_id: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context
            .update_tablespace(&self.principal, tablespace_id, config)
            .await
    }

    /// Delete a tablespace.
    pub async fn delete_tablespace(&self, tablespace_id: &TablespaceId) -> KeyValueResult<()> {
        self.context
            .delete_tablespace(&self.principal, tablespace_id)
            .await
    }
}
