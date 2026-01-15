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

use nanograph_core::object::{ObjectId, ObjectType};
use std::collections::{BTreeMap, BTreeSet};

/// Cache for all container-level metadata.
///
/// This structure maintains an in-memory representation of a container's metadata, including
/// namespaces, tables, and shards. It also includes a name resolver for hierarchical object paths.
#[derive(Debug)]
pub struct HierarchicalResolverCache {
    /// Name Resolver Nodes
    resolver_nodes: BTreeMap<ObjectId, Node>,
    /// Name Resolver Paths
    resolver_paths: BTreeMap<String, ObjectId>,
    /// Available Nodes from Removed Nodes
    resolver_available_nodes: BTreeSet<ObjectId>,
    /// Next available ID for resolver nodes
    resolver_next_id: ObjectId,
}

impl HierarchicalResolverCache {
    /*============================================================================================*\
    | Resolver Functions
    \*============================================================================================*/

    pub fn set_path_object(&mut self, path: &str, object_id: ObjectId, object_type: ObjectType) {
        // TODO: Fix this
        let _ = self.add_path(path, object_type);
    }

    /// Moves a path from the old location to the new.
    pub fn move_object_record(&mut self, old_path: &str, new_path: &str) -> Result<(), String> {
        self.move_path(old_path, new_path)
    }

    /// Returns the next available object ID.
    pub fn get_next_object_id(&mut self) -> ObjectId {
        if let Some(next_id) = self.resolver_available_nodes.pop_first() {
            next_id
        } else {
            let next_id = self.resolver_next_id;
            self.resolver_next_id += 1;
            next_id
        }
    }

    /// Removes a path from the resolver.
    ///
    /// Returns an error if the path is not found or if the node has children.
    fn remove_path(&mut self, path: &str) -> Result<(), String> {
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
    fn move_path(&mut self, from: &str, to: &str) -> Result<(), String> {
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
    fn list_objects(&self, prefix: Option<&str>) -> Vec<(String, ObjectType)> {
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

    /// Returns the object type and ID at the specified path.
    fn get_path_reference(&self, path: &str) -> Option<(ObjectId, ObjectType)> {
        let idx = self.resolver_paths.get(path)?;
        let node = self.resolver_nodes.get(idx)?;
        Some((*idx, node.object_type))
    }

    /// Adds a new object to the resolver at the specified path.
    ///
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

                let idx = self.resolver_next_id;
                self.resolver_next_id += 1;

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

    /// Returns the full path of a node by its index.
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

    /// Updates the paths of all descendant nodes recursively.
    fn update_descendant_paths(&mut self, idx: ObjectId) {
        let _path = self.get_node_path(idx).unwrap();

        let node = self.resolver_nodes.get(&idx).unwrap();
        let children: Vec<ObjectId> = node.children.clone();

        for &child_idx in &children {
            self.update_descendant_paths(child_idx);
        }
    }
}

/// A node in the hierarchical name resolver.
#[derive(Debug, Clone)]
struct Node {
    /// Name of the node.
    name: String,
    /// Type of object this node represents.
    object_type: ObjectType,
    /// Parent node index, if any.
    parent: Option<ObjectId>,
    /// Child node indices.
    children: Vec<ObjectId>,
}
