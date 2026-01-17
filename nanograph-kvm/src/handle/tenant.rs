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
use crate::handle::ContainerHandle;
use nanograph_core::object::{
    ContainerId, DatabaseCreate, DatabaseId, DatabaseMetadata, DatabaseUpdate, SecurityPrincipal,
    TenantId, TenantMetadata, TenantUpdate, TenantUserCreate, TenantUserMetadata, TenantUserUpdate,
    UserId,
};
use nanograph_kvt::KeyValueResult;
use std::sync::Arc;

/// A handle for an authenticated tenant user.
///
/// `TenantHandle` provides access to a specific tenant's databases and users.
/// It encapsulates the tenant identifier, making it easier to perform operations
/// within the scope of that tenant.
///
/// # Hierarchy
///
/// The database hierarchy is:
/// - **Cluster** → **Tenant** → **Database (Container)** → **Namespace** → **Table** → **Key-Value Pairs**
///
/// # Usage
///
/// You typically obtain a `TenantHandle` from a `KeyValueDatabaseManager`:
///
/// ```ignore
/// let tenant = manager.get_tenant_handle(&principal, &tenant_id).await?;
/// ```
///
/// # Operations
///
/// A `TenantHandle` allows you to:
/// - Manage databases within the tenant
/// - Manage tenant users
/// - Get container handles for specific databases
/// - Update tenant metadata
///
/// # Thread Safety
///
/// `TenantHandle` is safe to clone and share across threads. All operations are
/// internally synchronized.
pub struct TenantHandle {
    context: Arc<KeyValueDatabaseContext>,
    principal: SecurityPrincipal,
    tenant_id: TenantId,
}

impl TenantHandle {
    pub(crate) fn new(
        context: Arc<KeyValueDatabaseContext>,
        principal: SecurityPrincipal,
        tenant_id: TenantId,
    ) -> TenantHandle {
        Self {
            context,
            principal,
            tenant_id,
        }
    }

    /// Get the tenant ID associated with this handle.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// Get metadata for this tenant.
    pub async fn get_metadata(&self) -> KeyValueResult<Option<TenantMetadata>> {
        self.context
            .get_tenant(&self.principal, &self.tenant_id)
            .await
    }

    /// Update this tenant's configuration.
    pub async fn update(&self, config: TenantUpdate) -> KeyValueResult<TenantMetadata> {
        self.context
            .update_tenant(&self.principal, &self.tenant_id, config)
            .await
    }

    //
    // Database Management
    //

    /// List all databases belonging to this tenant.
    pub async fn get_databases(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = (DatabaseId, String)>> {
        self.context
            .get_databases(&self.principal, &self.tenant_id)
            .await
    }

    /// Get metadata for a specific database in this tenant.
    pub async fn get_database(
        &self,
        database_id: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseMetadata>> {
        self.context
            .get_database(&self.principal, &self.tenant_id, database_id)
            .await
    }

    /// Get a database ID by its name within this tenant.
    pub async fn get_database_by_name(&self, name: &str) -> KeyValueResult<Option<DatabaseId>> {
        self.context
            .get_database_by_name(&self.principal, &self.tenant_id, name)
            .await
    }

    /// Create a new database for this tenant.
    pub async fn create_database(
        &self,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        self.context
            .create_database(&self.principal, &self.tenant_id, config)
            .await
    }

    /// Update an existing database in this tenant.
    pub async fn update_database(
        &self,
        database_id: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseMetadata> {
        self.context
            .update_database(&self.principal, &self.tenant_id, database_id, config)
            .await
    }

    /// Delete a database from this tenant.
    pub async fn delete_database(&self, database_id: &DatabaseId) -> KeyValueResult<()> {
        self.context
            .delete_database(&self.principal, &self.tenant_id, database_id)
            .await
    }

    //
    // Tenant User Management
    //

    /// List all users for this tenant.
    pub async fn get_users(&self) -> KeyValueResult<impl IntoIterator<Item = TenantUserMetadata>> {
        self.context
            .get_tenant_users(&self.principal, &self.tenant_id)
            .await
    }

    /// Get metadata for a specific user in this tenant.
    pub async fn get_user(&self, user_id: &UserId) -> KeyValueResult<Option<TenantUserMetadata>> {
        self.context
            .get_tenant_user(&self.principal, &self.tenant_id, user_id)
            .await
    }

    /// Create a new user for this tenant.
    pub async fn create_user(
        &self,
        config: TenantUserCreate,
    ) -> KeyValueResult<TenantUserMetadata> {
        self.context
            .create_tenant_user(&self.principal, &self.tenant_id, config)
            .await
    }

    /// Update an existing user in this tenant.
    pub async fn update_user(
        &self,
        user_id: &UserId,
        config: TenantUserUpdate,
    ) -> KeyValueResult<TenantUserMetadata> {
        self.context
            .update_tenant_user(&self.principal, &self.tenant_id, user_id, config)
            .await
    }

    /// Remove a user from this tenant.
    pub async fn remove_user(&self, user_id: &UserId) -> KeyValueResult<()> {
        self.context
            .remove_tenant_user(&self.principal, &self.tenant_id, user_id)
            .await
    }

    //
    // Container Handle
    //

    /// Get a handle for a specific database (container) in this tenant.
    pub async fn get_container_handle(
        &self,
        database_id: &DatabaseId,
    ) -> KeyValueResult<ContainerHandle> {
        let container_id = ContainerId::from_parts(self.tenant_id, *database_id);
        Ok(ContainerHandle::new(
            self.context.clone(),
            self.principal.clone(),
            container_id,
        ))
    }
}
