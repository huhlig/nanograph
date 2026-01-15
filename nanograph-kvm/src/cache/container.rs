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
    ContainerId, FunctionId, FunctionMetadata, NamespaceId, NamespaceMetadata, NodeId, ShardId,
    ShardMetadata, TableId, TableMetadata,
};
use std::collections::{BTreeMap, HashMap};

/// Cache for all container-level metadata.
///
/// This structure maintains an in-memory representation of a container's metadata, including
/// users, namespaces, functions, tables, and shards. It also tracks shard-to-node assignments.
///
/// A "container" in this context refers to a specific Database within a Tenant.
#[derive(Debug)]
pub struct ContainerMetadataCache {
    /// Container Id (Tenant + Database)
    container: ContainerId,
    /// Container Metadata Shard ID
    shard: ShardId,
    /// Namespaces in the system
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    /// Functions in the system
    functions: HashMap<FunctionId, FunctionMetadata>,
    /// Tables in the system
    tables: HashMap<TableId, TableMetadata>,
    /// Shards in the system
    shards: HashMap<ShardId, ShardMetadata>,
    /// Shard Assignment Cache (shard -> replica nodes)
    shard_assignments: BTreeMap<ShardId, Vec<NodeId>>,
}

impl ContainerMetadataCache {
    /// Create a new `ContainerMetadataCache`.
    pub fn new(container: ContainerId, shard: ShardId) -> Self {
        ContainerMetadataCache {
            container,
            shard,
            namespaces: Default::default(),
            functions: Default::default(),
            tables: Default::default(),
            shards: Default::default(),
            shard_assignments: Default::default(),
        }
    }

    /// Returns the ID of the container this cache is for.
    pub fn container_id(&self) -> &ContainerId {
        &self.container
    }

    /// Returns the ID of the shard where this container's metadata is stored.
    pub fn metadata_shard_id(&self) -> &ShardId {
        &self.shard
    }

    // --- Namespace Records ---

    /// Returns an iterator over all namespace records.
    pub fn list_namespace_records(&self) -> impl Iterator<Item = &NamespaceMetadata> {
        self.namespaces.values()
    }

    /// Returns a reference to the metadata for a specific namespace if it exists.
    pub fn get_namespace_record(&self, record_id: &NamespaceId) -> Option<&NamespaceMetadata> {
        self.namespaces.get(record_id)
    }

    /// Sets or updates a namespace record.
    pub fn set_namespace_record(&mut self, record: NamespaceMetadata) {
        self.namespaces.insert(record.id, record);
    }

    /// Removes a specific namespace record.
    pub fn clear_namespace_record(&mut self, record_id: NamespaceId) {
        self.namespaces.remove(&record_id);
    }

    /// Clears all namespace records.
    pub fn clear_namespace_records(&mut self) {
        self.namespaces.clear();
    }

    // --- Function Records ---

    /// Returns an iterator over all function records.
    pub fn list_function_records(&self) -> impl Iterator<Item = &FunctionMetadata> {
        self.functions.values()
    }

    /// Returns a reference to the metadata for a specific function if it exists.
    pub fn get_function_record(&self, record_id: &FunctionId) -> Option<&FunctionMetadata> {
        self.functions.get(record_id)
    }

    /// Sets or updates a function record.
    pub fn set_function_record(&mut self, record: FunctionMetadata) {
        self.functions.insert(record.id, record);
    }

    /// Removes a specific function record.
    pub fn clear_function_record(&mut self, record_id: &FunctionId) {
        self.functions.remove(record_id);
    }

    /// Clears all function records.
    pub fn clear_function_records(&mut self) {
        self.functions.clear();
    }

    // --- Table Records ---

    /// Returns an iterator over all table records.
    pub fn list_table_records(&self) -> impl Iterator<Item = &TableMetadata> {
        self.tables.values()
    }

    /// Returns a reference to the metadata for a specific table if it exists.
    pub fn get_table_record(&self, record_id: &TableId) -> Option<&TableMetadata> {
        self.tables.get(record_id)
    }

    /// Sets or updates a table record.
    pub fn set_table_record(&mut self, table: TableMetadata) {
        self.tables.insert(table.id, table);
    }

    /// Removes a specific table record.
    pub fn clear_table_record(&mut self, table_id: TableId) {
        self.tables.remove(&table_id);
    }

    /// Clears all table records.
    pub fn clear_table_records(&mut self) {
        self.tables.clear();
    }

    // --- Shard Records ---

    /// Returns an iterator over all shard records.
    pub fn list_shard_records(&self) -> impl Iterator<Item = &ShardMetadata> {
        self.shards.values()
    }

    /// Returns a reference to the metadata for a specific shard if it exists.
    pub fn get_shard_record(&self, shard_id: &ShardId) -> Option<&ShardMetadata> {
        self.shards.get(shard_id)
    }

    /// Sets or updates a shard record.
    pub fn set_shard_record(&mut self, record: ShardMetadata) {
        self.shards.insert(record.id.clone(), record);
    }

    /// Removes a specific shard record.
    pub fn clear_shard_record(&mut self, record_id: ShardId) {
        self.shards.remove(&record_id);
    }

    /// Clears all shard records.
    pub fn clear_shard_records(&mut self) {
        self.shards.clear();
    }

    // --- Shard Assignments ---

    /// Returns an iterator over all shard assignments.
    pub fn list_shard_assignments(&self) -> impl Iterator<Item = (&ShardId, &Vec<NodeId>)> {
        self.shard_assignments.iter()
    }

    /// Returns the node IDs assigned to a specific shard if it exists.
    pub fn get_shard_assignment(&self, shard_id: &ShardId) -> Option<&Vec<NodeId>> {
        self.shard_assignments.get(shard_id)
    }

    /// Sets or updates the node IDs assigned to a specific shard.
    pub fn set_shard_assignment(&mut self, shard_id: ShardId, nodes: Vec<NodeId>) {
        self.shard_assignments.insert(shard_id, nodes);
    }

    /// Removes the assignment for a specific shard.
    pub fn clear_shard_assignment(&mut self, shard_id: &ShardId) {
        self.shard_assignments.remove(shard_id);
    }

    /// Clears all shard assignments.
    pub fn clear_shard_assignments(&mut self) {
        self.shard_assignments.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nanograph_core::object::{DatabaseId, ShardIndex, ShardStatus, TableSharding, TenantId};
    use nanograph_core::types::Timestamp;
    use nanograph_kvt::StorageEngineType;
    use std::collections::HashMap;

    fn create_test_cache() -> ContainerMetadataCache {
        let tenant_id = TenantId::from(1);
        let database_id = DatabaseId::from(1);
        let container_id = ContainerId::from_parts(tenant_id, database_id);
        let table_id = TableId::from(1);
        let shard_index = ShardIndex::from(5);
        let shard_id = ShardId::from_parts(table_id, shard_index);
        ContainerMetadataCache::new(container_id, shard_id)
    }

    #[test]
    fn test_new_and_basic_info() {
        let tenant_id = TenantId::from(1);
        let database_id = DatabaseId::from(1);
        let container_id = ContainerId::from_parts(tenant_id, database_id);
        let table_id = TableId::from(1);
        let shard_index = ShardIndex::from(5);
        let shard_id = ShardId::from_parts(table_id, shard_index);
        let cache = ContainerMetadataCache::new(container_id.clone(), shard_id.clone());

        assert_eq!(cache.container_id(), &container_id);
        assert_eq!(cache.metadata_shard_id(), &shard_id);
    }

    #[test]
    fn test_namespace_records() {
        let mut cache = create_test_cache();
        let ns_id = NamespaceId::from(200);
        let ns = NamespaceMetadata {
            id: ns_id,
            name: "test_ns".to_string(),
            version: 1,
            path: "/test_ns".to_string(),
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        };

        cache.set_namespace_record(ns.clone());
        assert!(cache.get_namespace_record(&ns_id).is_some());
        assert_eq!(cache.get_namespace_record(&ns_id).unwrap().id, ns_id);
        assert_eq!(cache.list_namespace_records().count(), 1);

        cache.clear_namespace_record(ns_id);
        assert!(cache.get_namespace_record(&ns_id).is_none());

        cache.set_namespace_record(ns);
        cache.clear_namespace_records();
        assert_eq!(cache.list_namespace_records().count(), 0);
    }

    #[test]
    fn test_function_records() {
        let mut cache = create_test_cache();
        let func_id = FunctionId::from(300);
        let func = FunctionMetadata {
            id: func_id,
            name: "test_func".to_string(),
            path: "/test_func".to_string(),
            version: 1,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        };

        cache.set_function_record(func.clone());
        assert!(cache.get_function_record(&func_id).is_some());
        assert_eq!(cache.get_function_record(&func_id).unwrap().id, func_id);
        assert_eq!(cache.list_function_records().count(), 1);

        cache.clear_function_record(&func_id);
        assert!(cache.get_function_record(&func_id).is_none());

        cache.set_function_record(func);
        cache.clear_function_records();
        assert_eq!(cache.list_function_records().count(), 0);
    }

    #[test]
    fn test_table_records() {
        let mut cache = create_test_cache();
        let table_id = TableId::from(400);
        let table = TableMetadata {
            id: table_id,
            name: "test_table".to_string(),
            path: "/test_table".to_string(),
            version: 1,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            engine_type: StorageEngineType::from("lsm"),
            sharding: TableSharding::Single,
            options: HashMap::new(),
            metadata: HashMap::new(),
        };

        cache.set_table_record(table.clone());
        assert!(cache.get_table_record(&table_id).is_some());
        assert_eq!(cache.get_table_record(&table_id).unwrap().id, table_id);
        assert_eq!(cache.list_table_records().count(), 1);

        cache.clear_table_record(table_id);
        assert!(cache.get_table_record(&table_id).is_none());

        cache.set_table_record(table);
        cache.clear_table_records();
        assert_eq!(cache.list_table_records().count(), 0);
    }

    #[test]
    fn test_shard_records() {
        let mut cache = create_test_cache();
        let table_id = TableId::from(500);
        let shard_index = ShardIndex::from(1);
        let shard_id = ShardId::from_parts(table_id, shard_index);
        let shard = ShardMetadata {
            id: shard_id.clone(),
            name: "test_shard".to_string(),
            version: 1,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            range: (vec![], vec![]),
            leader: None,
            engine_type: StorageEngineType::from("lsm"),
            status: ShardStatus::Active,
            term: 0,
            replicas: vec![],
            size_bytes: 0,
        };

        cache.set_shard_record(shard.clone());
        assert!(cache.get_shard_record(&shard_id).is_some());
        assert_eq!(cache.get_shard_record(&shard_id).unwrap().id, shard_id);
        assert_eq!(cache.list_shard_records().count(), 1);

        cache.clear_shard_record(shard_id.clone());
        assert!(cache.get_shard_record(&shard_id).is_none());

        cache.set_shard_record(shard);
        cache.clear_shard_records();
        assert_eq!(cache.list_shard_records().count(), 0);
    }

    #[test]
    fn test_shard_assignments() {
        let mut cache = create_test_cache();
        let table_id = TableId::from(600);
        let shard_index = ShardIndex::from(0);
        let shard_id = ShardId::from_parts(table_id, shard_index);
        let nodes = vec![NodeId::from(1), NodeId::from(2)];

        cache.set_shard_assignment(shard_id.clone(), nodes.clone());
        assert_eq!(cache.get_shard_assignment(&shard_id), Some(&nodes));
        assert_eq!(cache.list_shard_assignments().count(), 1);

        cache.clear_shard_assignment(&shard_id);
        assert_eq!(cache.get_shard_assignment(&shard_id), None);

        cache.set_shard_assignment(shard_id, nodes);
        cache.clear_shard_assignments();
        assert_eq!(cache.list_shard_assignments().count(), 0);
    }
}
