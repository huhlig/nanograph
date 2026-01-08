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

use crate::NodeId;
use crate::config::{NamespaceConfig, TableConfig};
use crate::metadata::{
    ClusterMetadata, NamespaceMetadata, RegionMetadata, ServerMetadata, ShardMetadata,
    TableMetadata,
};
use crate::types::{ClusterId, NamespaceId, ObjectId, RegionId, ServerId, ShardId, TableId};
use chrono::Utc;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Types of objects that can be stored in the metadata cache.
pub enum ObjectType {
    /// A namespace which can contain other namespaces or tables.
    Namespace,
    /// A table which contains data.
    Table,
}

/// Cache for all metadata in the system.
///
/// This structure maintains an in-memory representation of the system's metadata,
/// including clusters, regions, servers, namespaces, tables, and shards.
/// It also includes a name resolver for hierarchical object paths.
///
/// TODO: Store as it's own set of fully replicated tables.
#[derive(Debug)]
pub struct MetadataCache {
    /// Cluster metadata
    cluster: ClusterMetadata,
    /// Regions in the cluster
    regions: HashMap<RegionId, RegionMetadata>,
    /// Servers in the cluster
    servers: HashMap<ServerId, ServerMetadata>,
    /// Namespaces in the system
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    /// Tables in the system
    tables: HashMap<TableId, TableMetadata>,
    /// Shards in the system
    shards: HashMap<ShardId, ShardMetadata>,
    /// Shard assignments (shard -> replica nodes)
    shard_assignments: HashMap<ShardId, Vec<NodeId>>,
    /// Name Resolver
    resolver_nodes: HashMap<ObjectId, Node>,
    /// Resolver Paths
    resolver_paths: HashMap<String, ObjectId>,
    /// Available Nodes from Removed Nodes
    available_nodes: HashSet<ObjectId>,
    /// Next available ID for resolver nodes
    next_resolver_id: ObjectId,
}

impl MetadataCache {
    /// Create a new MetadataCache.
    ///
    /// TODO: Ensure Data is loaded from disk.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the cluster metadata.
    pub fn set_cluster_metadata(&mut self, metadata: ClusterMetadata) {
        self.cluster = metadata;
    }
    /// Returns a reference to the cluster metadata.
    pub fn get_cluster_metadata(&self) -> &ClusterMetadata {
        &self.cluster
    }
    /// Returns a mutable reference to the cluster metadata.
    pub fn get_cluster_metadata_mut(&mut self) -> &mut ClusterMetadata {
        &mut self.cluster
    }
    /// Sets the metadata for a specific region.
    pub fn set_region_metadata(&mut self, metadata: RegionMetadata) {
        self.regions.insert(metadata.id.clone(), metadata);
    }
    /// Returns a reference to the metadata for a specific region if it exists.
    pub fn get_region_metadata(&self, region_id: &RegionId) -> Option<&RegionMetadata> {
        self.regions.get(region_id)
    }
    /// Returns a mutable reference to the metadata for a specific region if it exists.
    pub fn get_region_metadata_mut(&mut self, region_id: &RegionId) -> Option<&mut RegionMetadata> {
        self.regions.get_mut(region_id)
    }
    /// Returns an iterator over all region metadata.
    pub fn get_regions(&self) -> impl Iterator<Item = &RegionMetadata> {
        self.regions.values()
    }
    /// Sets the metadata for a specific server.
    pub fn set_server_metadata(&mut self, metadata: ServerMetadata) {
        self.servers.insert(metadata.id.clone(), metadata);
    }
    /// Returns a reference to the metadata for a specific server if it exists.
    pub fn get_server_metadata(&self, server_id: &ServerId) -> Option<&ServerMetadata> {
        self.servers.get(server_id)
    }
    /// Returns a mutable reference to the metadata for a specific server if it exists.
    pub fn get_server_metadata_mut(&mut self, server_id: &ServerId) -> Option<&mut ServerMetadata> {
        self.servers.get_mut(server_id)
    }
    /// Returns an iterator over all server metadata.
    pub fn get_servers(&self) -> impl Iterator<Item = &ServerMetadata> {
        self.servers.values()
    }
    /// Returns a reference to the metadata for a specific namespace if it exists.
    pub fn get_namespace_metadata(&self, namespace_id: &NamespaceId) -> Option<&NamespaceMetadata> {
        self.namespaces.get(namespace_id)
    }
    /// Returns a mutable reference to the metadata for a specific namespace if it exists.
    pub fn get_namespace_metadata_mut(
        &mut self,
        namespace_id: &NamespaceId,
    ) -> Option<&mut NamespaceMetadata> {
        self.namespaces.get_mut(namespace_id)
    }
    /// Returns a reference to the metadata for a specific table if it exists.
    pub fn get_table_metadata(&self, table_id: &TableId) -> Option<&TableMetadata> {
        self.tables.get(table_id)
    }
    /// Returns a mutable reference to the metadata for a specific table if it exists.
    pub fn get_table_metadata_mut(&mut self, table_id: &TableId) -> Option<&mut TableMetadata> {
        self.tables.get_mut(table_id)
    }
    /// Returns an iterator over all table metadata.
    pub fn get_tables(&self) -> impl Iterator<Item = &TableMetadata> {
        self.tables.values()
    }
    /// Returns a reference to the metadata for a specific shard if it exists.
    pub fn get_shard_metadata(&self, shard_id: &ShardId) -> Option<&ShardMetadata> {
        self.shards.get(shard_id)
    }
    /// Returns a mutable reference to the metadata for a specific shard if it exists.
    pub fn get_shard_metadata_mut(&mut self, shard_id: &ShardId) -> Option<&mut ShardMetadata> {
        self.shards.get_mut(shard_id)
    }
    /// Adds or updates a region in the cache.
    pub fn add_region(&mut self, region: RegionMetadata) {
        self.regions.insert(region.id, region);
    }
    /// Adds or updates a server in the cache.
    pub fn add_server(&mut self, server: ServerMetadata) {
        self.servers.insert(server.id, server);
    }
    /// Adds a new namespace to the cache at the specified path.
    pub fn add_namespace(&mut self, path: &str, namespace: NamespaceConfig) {
        let full_path = if path.is_empty() {
            namespace.name.clone()
        } else {
            format!("{}.{}", path, namespace.name)
        };
        let id = self
            .add_path(&full_path, ObjectType::Namespace)
            .expect("Failed to add namespace path");
        let metadata = NamespaceMetadata {
            id: NamespaceId(id),
            name: namespace.name.clone(),
            path: full_path,
            created_at: Utc::now(),
            last_modified: Utc::now(),
        };
        self.namespaces.insert(metadata.id, metadata);
    }
    /// Adds a new table to the cache at the specified path.
    pub fn add_table(&mut self, path: &str, table: TableConfig) {
        let full_path = if path.is_empty() {
            table.name.clone()
        } else {
            format!("{}.{}", path, table.name)
        };
        let id = self
            .add_path(&full_path, ObjectType::Table)
            .expect("Failed to add table path");
        let metadata = TableMetadata {
            id: TableId(id),
            name: table.name.clone(),
            path: full_path,
            created_at: Utc::now(),
            engine_type: table.engine_type,
            last_modified: Utc::now(),
            shard_count: table.shard_count,
            replication_factor: table.replication_factor,
        };
        self.tables.insert(metadata.id, metadata);
    }
    /// Sets the metadata for a shard.
    ///
    /// TODO: Need to work with engines
    pub fn set_shard(&mut self, shard: ShardMetadata) {
        self.shards.insert(shard.id, shard);
    }

    /// Removes an object from the resolver.
    /// Returns an error if the path is not found or if the node has children.
    pub fn remove(&mut self, path: &str) -> Result<(), String> {
        let idx = *self
            .resolver_paths
            .get(path)
            .ok_or_else(|| format!("Path not found: {}", path))?;

        let node = self
            .resolver_nodes
            .get(&idx)
            .ok_or_else(|| "Node not found".to_string())?;

        if !node.children.is_empty() {
            return Err(format!("Cannot remove node with children: {}", path));
        }

        if let Some(parent_idx) = node.parent {
            if let Some(parent) = self.resolver_nodes.get_mut(&parent_idx) {
                parent.children.retain(|&child| child != idx);
            }
        }

        self.resolver_nodes.remove(&idx);
        self.resolver_paths.remove(path);

        Ok(())
    }

    /// Moves a node from one path to another.
    /// This also updates the paths of all descendant nodes.
    pub fn move_object(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from_idx = *self
            .resolver_paths
            .get(from)
            .ok_or_else(|| format!("Source path not found: {}", from))?;

        let to_parts = Self::parse_path(to);
        if to_parts.is_empty() {
            return Err("Empty destination path".to_string());
        }

        let node_name = to_parts.last().unwrap();
        let new_parent_path = if to_parts.len() > 1 {
            Self::build_path(
                &to_parts[..to_parts.len() - 1]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            )
        } else {
            String::new()
        };

        let new_parent_idx = if !new_parent_path.is_empty() {
            Some(
                *self
                    .resolver_paths
                    .get(&new_parent_path)
                    .ok_or_else(|| format!("Parent path not found: {}", new_parent_path))?,
            )
        } else {
            None
        };

        if self.resolver_paths.contains_key(to) {
            return Err(format!("Destination path already exists: {}", to));
        }

        let old_parent_idx = self.resolver_nodes.get(&from_idx).unwrap().parent;

        if let Some(old_parent) = old_parent_idx {
            if let Some(parent_node) = self.resolver_nodes.get_mut(&old_parent) {
                parent_node.children.retain(|&child| child != from_idx);
            }
        }

        if let Some(new_parent) = new_parent_idx {
            if let Some(parent_node) = self.resolver_nodes.get_mut(&new_parent) {
                parent_node.children.push(from_idx);
            }
        }

        if let Some(node) = self.resolver_nodes.get_mut(&from_idx) {
            node.name = node_name.to_string();
            node.parent = new_parent_idx;
        }

        self.resolver_paths.remove(from);
        self.resolver_paths.insert(to.to_string(), from_idx);

        self.update_descendant_paths(from_idx);

        Ok(())
    }

    /// Lists all objects in the resolver, optionally filtered by a prefix.
    pub fn list_objects(&self, prefix: Option<&str>) -> Vec<(String, ObjectType)> {
        let mut results: Vec<(String, ObjectType)> = self
            .resolver_paths
            .iter()
            .filter_map(|(path, &idx)| {
                if let Some(p) = prefix {
                    if !path.starts_with(p) {
                        return None;
                    }
                }
                let node = self.resolver_nodes.get(&idx)?;
                Some((path.clone(), node.object_type))
            })
            .collect();

        results.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    /// Retrieves the object type at the specified path.
    pub fn get_path_reference(&self, path: &str) -> Option<(ObjectId, ObjectType)> {
        let idx = self.resolver_paths.get(path)?;
        let node = self.resolver_nodes.get(idx)?;
        Some((*idx, node.object_type))
    }

    /*============================================================================================*\
    | Resolver Internals                                                                           |
    \*============================================================================================*/

    /// Adds a new object to the resolver at the specified path.
    /// Intermediate namespaces will be created automatically if they don't exist.
    fn add_path(&mut self, path: &str, object_type: ObjectType) -> Result<ObjectId, String> {
        let parts = Self::parse_path(path);
        if parts.is_empty() {
            return Err("Empty path".to_string());
        }

        if self.resolver_paths.contains_key(path) {
            return Err(format!("Path already exists: {}", path));
        }

        let mut current_parent: Option<ObjectId> = None;
        let mut current_path = String::new();

        for (i, &part) in parts.iter().enumerate() {
            if !current_path.is_empty() {
                current_path.push('.');
            }
            current_path.push_str(part);

            if let Some(&existing_idx) = self.resolver_paths.get(&current_path) {
                current_parent = Some(existing_idx);
            } else {
                let is_leaf = i == parts.len() - 1;
                let node_type = if is_leaf {
                    object_type
                } else {
                    ObjectType::Namespace
                };

                let idx = self.next_resolver_id;
                self.next_resolver_id += 1;

                let node = Node {
                    name: part.to_string(),
                    object_type: node_type,
                    parent: current_parent,
                    children: Vec::new(),
                };

                self.resolver_nodes.insert(idx, node);
                self.resolver_paths.insert(current_path.clone(), idx);

                if let Some(parent_idx) = current_parent {
                    if let Some(parent) = self.resolver_nodes.get_mut(&parent_idx) {
                        parent.children.push(idx);
                    }
                }

                current_parent = Some(idx);
            }
        }

        Ok(current_parent.unwrap())
    }

    /// Parses a dot-separated path into its constituent parts.
    fn parse_path(path: &str) -> Vec<&str> {
        path.split('.').filter(|s| !s.is_empty()).collect()
    }

    /// Builds a dot-separated path from its constituent parts.
    fn build_path(parts: &[String]) -> String {
        parts.join(".")
    }

    /// Retrieves the full path of a node by its index.
    fn get_node_path(&self, idx: ObjectId) -> Option<String> {
        let mut parts = Vec::new();
        let mut current = idx;

        loop {
            let node = self.resolver_nodes.get(&current)?;
            parts.push(node.name.clone());

            match node.parent {
                Some(parent) => current = parent,
                None => break,
            }
        }

        parts.reverse();
        Some(Self::build_path(&parts))
    }

    /// Recursively updates the paths of all descendant nodes.
    fn update_descendant_paths(&mut self, idx: ObjectId) {
        let _path = self.get_node_path(idx).unwrap();

        let node = self.resolver_nodes.get(&idx).unwrap();
        let children: Vec<ObjectId> = node.children.clone();

        for &child_idx in &children {
            self.update_descendant_paths(child_idx);
        }
    }
}

impl Default for MetadataCache {
    /// Returns the default MetadataCache.
    fn default() -> Self {
        MetadataCache {
            cluster: ClusterMetadata {
                id: ClusterId(0),
                name: Default::default(),
                version: Default::default(),
                created_at: Utc::now(),
                last_modified: Utc::now(),
            },
            regions: Default::default(),
            servers: Default::default(),
            namespaces: Default::default(),
            tables: Default::default(),
            shards: Default::default(),
            shard_assignments: Default::default(),
            resolver_nodes: Default::default(),
            resolver_paths: Default::default(),
            available_nodes: Default::default(),
            next_resolver_id: 0,
        }
    }
}

/// Node in the name resolver hierarchy.
#[derive(Debug, Clone)]
struct Node {
    /// Name of the node.
    name: String,
    /// Type of object this node represents.
    object_type: ObjectType,
    /// Parent node index if any.
    parent: Option<ObjectId>,
    /// Child node indices.
    children: Vec<ObjectId>,
}

#[cfg(test)]
mod tests {
    use super::MetadataCache;
    use crate::StorageEngineType;
    use crate::config::{NamespaceConfig, TableConfig};

    #[test]
    fn test_resolver() {
        let mut cache = MetadataCache::default();

        println!("Adding nodes...");
        cache.add_namespace("", NamespaceConfig::new("ns1"));
        cache.add_namespace("ns1", NamespaceConfig::new("ns2"));
        cache.add_table(
            "ns1.ns2",
            TableConfig::new("table1", StorageEngineType::new("lsm")),
        );
        cache.add_table(
            "ns1.ns2",
            TableConfig::new("table2", StorageEngineType::new("lsm")),
        );
        cache.add_table(
            "ns1",
            TableConfig::new("table3", StorageEngineType::new("lsm")),
        );

        println!("\nListing all nodes:");
        for (path, obj_type) in cache.list_objects(None) {
            println!("  {} -> {:?}", path, obj_type);
        }

        println!("\nListing nodes under ns1.ns2:");
        for (path, obj_type) in cache.list_objects(Some("ns1.ns2")) {
            println!("  {} -> {:?}", path, obj_type);
        }

        println!("\nMoving ns1.ns2.table1 to ns1.table1:");
        cache.move_object("ns1.ns2.table1", "ns1.table1").unwrap();

        println!("\nListing all nodes after move:");
        for (path, obj_type) in cache.list_objects(None) {
            println!("  {} -> {:?}", path, obj_type);
        }

        println!("\nRemoving ns1.table1:");
        cache.remove("ns1.table1").unwrap();

        println!("\nFinal listing:");
        for (path, obj_type) in cache.list_objects(None) {
            println!("  {} -> {:?}", path, obj_type);
        }
    }
}
