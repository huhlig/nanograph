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

//! # Nanograph System Record Types.
//!
//! ## Types
//! - ???Id types are unique identifiers in the system ([`NodeId`] and [`ContainerId`] are both composite identifiers).
//! - ???Record types are serialized within the cluster.
//! - ???Metadata types are for external consumption and may be lacking internal information.
//!
//!
mod cluster;
mod container;
mod database;
mod function;
mod index;
mod namespace;
mod node;
mod region;
mod security;
mod server;
mod shard;
mod table;
mod tablespace;
mod tenant;

pub use self::cluster::{ClusterCreate, ClusterId, ClusterMetadata, ClusterRecord, ClusterUpdate};
pub use self::container::ContainerId;
pub use self::database::{
    DatabaseCreate, DatabaseId, DatabaseMetadata, DatabaseRecord, DatabaseUpdate,
};
pub use self::function::{
    FunctionCreate, FunctionId, FunctionMetadata, FunctionRecord, FunctionUpdate,
};
pub use self::index::{
    IndexCreate, IndexId, IndexMetadata, IndexRecord, IndexShardId, IndexSharding, IndexStatus,
    IndexType, IndexUpdate,
};
pub use self::namespace::{
    NamespaceCreate, NamespaceId, NamespaceMetadata, NamespaceRecord, NamespaceUpdate,
};
pub use self::node::NodeId;
pub use self::region::{RegionCreate, RegionId, RegionMetadata, RegionRecord, RegionUpdate};
pub use self::security::{
    Permission, PermissionGrant, ResourceScope, SecurityPrincipal, SubjectId, SystemGroupId,
    SystemGroupMetadata, SystemGroupRecord, SystemRoleId, SystemRoleMetadata, SystemRoleRecord,
    SystemUserCreate, SystemUserMetadata, SystemUserRecord, SystemUserUpdate, TenantGroupId,
    TenantGroupMetadata, TenantGroupRecord, TenantRoleId, TenantRoleMetadata, TenantRoleRecord,
    TenantUserCreate, TenantUserMetadata, TenantUserRecord, TenantUserUpdate, UserId,
};
pub use self::server::{ServerCreate, ServerId, ServerMetadata, ServerRecord, ServerUpdate};
pub use self::shard::{
    HashFunction, KeyRange, Partitioner, ShardCreate, ShardId, ShardNumber, ShardRecord,
    ShardState, ShardStatus, ShardType, ShardUpdate, StorageEngineType,
};
pub use self::table::{
    TableCreate, TableId, TableMetadata, TableRecord, TableShardId, TableSharding, TableUpdate,
};
pub use self::tablespace::{
    LocalTablespaceRecord, StorageTier, TablespaceCreate, TablespaceId, TablespaceMetadata,
    TablespaceRecord, TablespaceUpdate,
};
pub use self::tenant::{TenantCreate, TenantId, TenantMetadata, TenantRecord, TenantUpdate};
use serde::{Deserialize, Serialize};

/// Object Identifier used by all Database Objects within a container.
///
/// **IMPORTANT**: ObjectIds are allocated from a unified pool per database.
/// This means Tables, Indexes, Functions, and Namespaces all share the same
/// ID space to prevent collisions when constructing ShardIds.
///
/// For example:
/// - TableId(1) and IndexId(1) cannot both exist in the same database
/// - Each ObjectId uniquely identifies exactly one object
/// - The object type is tracked separately in metadata
///
/// This design ensures that when TableId or IndexId is used in ShardId construction,
/// there are no collisions in the storage layer.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct ObjectId(u32);

impl ObjectId {
    /// Create a new table identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the table identifier as a u64.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for ObjectId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object({:X})", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Types of objects that can be tracked in the metadata resolver.
///
/// All object types share the same ObjectId allocation pool within a database.
pub enum ObjectType {
    /// A namespace which can contain other namespaces, tables, or functions.
    Namespace,
    /// A table which contains data shards.
    Table,
    /// An index on a table for efficient queries.
    Index,
    /// A function which can be executed by the database.
    Function,
}

#[derive(Clone, Debug)]
pub enum ObjectMetadata {
    Function(FunctionRecord),
    Index(IndexRecord),
    Namespace(NamespaceRecord),
    Table(TableRecord),
}
