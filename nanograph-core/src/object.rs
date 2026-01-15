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

mod cluster;
mod container;
mod database;
mod function;
mod namespace;
mod node;
mod region;
mod security;
mod server;
mod table;
mod tablespace;
mod tenant;

pub use self::cluster::{ClusterCreate, ClusterId, ClusterMetadata, ClusterUpdate};
pub use self::container::ContainerId;
pub use self::database::{DatabaseCreate, DatabaseId, DatabaseMetadata, DatabaseUpdate};
pub use self::function::{FunctionCreate, FunctionId, FunctionMetadata, FunctionUpdate};
pub use self::namespace::{NamespaceCreate, NamespaceId, NamespaceMetadata, NamespaceUpdate};
pub use self::node::NodeId;
pub use self::region::{RegionCreate, RegionId, RegionMetadata, RegionUpdate};
pub use self::security::{
    GroupId, GroupMetadata, Permission, PermissionGrant, ResourceScope, RoleId, RoleMetadata,
    UserCreate, UserId, UserMetadata, UserUpdate,
};
pub use self::server::{ServerCreate, ServerId, ServerMetadata, ServerUpdate};
pub use self::table::{
    HashFunction, KeyRange, Partitioner, ShardCreate, ShardId, ShardIndex, ShardMetadata,
    ShardState, ShardStatus, ShardUpdate, StorageEngineType, TableCreate, TableId, TableMetadata,
    TableSharding, TableUpdate,
};
pub use self::tablespace::{TablespaceCreate, TablespaceId, TablespaceMetadata, TablespaceUpdate};
pub use self::tenant::{TenantCreate, TenantId, TenantMetadata, TenantUpdate};

/// Object Identifier used by all Database Objects within a container.
pub type ObjectId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Types of objects that can be tracked in the metadata resolver.
pub enum ObjectType {
    /// A namespace which can contain other namespaces, tables, or functions.
    Namespace,
    /// A table which contains data shards.
    Table,
    /// A function which can be executed by the database.
    Function,
}

#[derive(Clone, Debug)]
pub enum ObjectMetadata {
    Function(FunctionMetadata),
    Namespace(NamespaceMetadata),
    Table(TableMetadata),
}
