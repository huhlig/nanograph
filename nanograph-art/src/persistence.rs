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

//! ART persistence layer using VFS
//!
//! This module provides disk persistence for Adaptive Radix Tree nodes using the VFS abstraction.
//! The ART is serialized to disk in a compact binary format that preserves the tree structure.

use crate::error::{ArtError, ArtResult};
use crate::node::Node;
use crate::tree::AdaptiveRadixTree;
use nanograph_vfs::{DynamicFileSystem, File as VfsFile};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Arc;

const MAGIC_NUMBER: u32 = 0x4E414E41; // "NANA" (Nanograph ART)
const VERSION: u32 = 1;

/// Serialized node format
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SerializedNode {
    Leaf {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Node4 {
        partial: Vec<u8>,
        keys: Vec<u8>,
        children: Vec<SerializedNode>,
        value: Option<Vec<u8>>,
    },
    Node16 {
        partial: Vec<u8>,
        keys: Vec<u8>,
        children: Vec<SerializedNode>,
        value: Option<Vec<u8>>,
    },
    Node48 {
        partial: Vec<u8>,
        child_index: Vec<u8>,
        children: Vec<Option<SerializedNode>>,
        value: Option<Vec<u8>>,
    },
    Node256 {
        partial: Vec<u8>,
        children: Vec<Option<SerializedNode>>,
        value: Option<Vec<u8>>,
    },
}

/// Tree metadata stored in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeMetadata {
    pub version: u32,
    pub magic: u32,
    pub node_count: usize,
    pub key_count: usize,
    pub created_at: u64,
    pub modified_at: u64,
}

/// ART persistence manager
pub struct ArtPersistence {
    fs: Arc<dyn DynamicFileSystem>,
    base_path: String,
}

impl ArtPersistence {
    /// Create a new persistence manager
    pub fn new(fs: Arc<dyn DynamicFileSystem>, base_path: String) -> ArtResult<Self> {
        // Ensure base directory exists
        fs.create_directory_all(&base_path)
            .map_err(|e| ArtError::Internal(format!("Failed to create directory: {}", e)))?;

        Ok(Self { fs, base_path })
    }

    /// Save the entire tree to disk
    pub fn save_tree<V: Clone + Serialize>(&self, tree: &AdaptiveRadixTree<V>) -> ArtResult<()> {
        let tree_path = format!("{}/tree.art", self.base_path);
        let metadata_path = format!("{}/metadata.json", self.base_path);

        // Serialize the tree
        let serialized = self.serialize_tree(tree)?;
        let data = serde_json::to_vec(&serialized)
            .map_err(|e| ArtError::Internal(format!("Serialization error: {}", e)))?;

        // Write tree data
        let mut file = self
            .fs
            .create_file(&tree_path)
            .map_err(|e| ArtError::Internal(format!("Failed to create file: {}", e)))?;

        file.write_all(&data)
            .map_err(|e| ArtError::Internal(format!("Failed to write tree: {}", e)))?;

        file.sync_all()
            .map_err(|e| ArtError::Internal(format!("Failed to sync tree: {}", e)))?;

        // Write metadata
        let metadata = TreeMetadata {
            version: VERSION,
            magic: MAGIC_NUMBER,
            node_count: self.count_nodes(&serialized),
            key_count: tree.len(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            modified_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let metadata_data = serde_json::to_vec_pretty(&metadata)
            .map_err(|e| ArtError::Internal(format!("Metadata serialization error: {}", e)))?;

        let mut metadata_file = self
            .fs
            .create_file(&metadata_path)
            .map_err(|e| ArtError::Internal(format!("Failed to create metadata file: {}", e)))?;

        metadata_file
            .write_all(&metadata_data)
            .map_err(|e| ArtError::Internal(format!("Failed to write metadata: {}", e)))?;

        metadata_file
            .sync_all()
            .map_err(|e| ArtError::Internal(format!("Failed to sync metadata: {}", e)))?;

        Ok(())
    }

    /// Load the entire tree from disk
    pub fn load_tree<V: Clone + for<'de> Deserialize<'de>>(
        &self,
    ) -> ArtResult<AdaptiveRadixTree<V>> {
        let tree_path = format!("{}/tree.art", self.base_path);
        let metadata_path = format!("{}/metadata.json", self.base_path);

        // Load and verify metadata
        let mut metadata_file = self
            .fs
            .open_file(&metadata_path)
            .map_err(|e| ArtError::Internal(format!("Failed to open metadata file: {}", e)))?;

        let mut metadata_data = Vec::new();
        metadata_file
            .read_to_end(&mut metadata_data)
            .map_err(|e| ArtError::Internal(format!("Failed to read metadata: {}", e)))?;

        let metadata: TreeMetadata = serde_json::from_slice(&metadata_data)
            .map_err(|e| ArtError::Internal(format!("Metadata deserialization error: {}", e)))?;

        if metadata.magic != MAGIC_NUMBER {
            return Err(ArtError::Internal("Invalid magic number".to_string()));
        }

        if metadata.version != VERSION {
            return Err(ArtError::Internal(format!(
                "Unsupported version: {}",
                metadata.version
            )));
        }

        // Load tree data
        let mut file = self
            .fs
            .open_file(&tree_path)
            .map_err(|e| ArtError::Internal(format!("Failed to open file: {}", e)))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| ArtError::Internal(format!("Failed to read tree: {}", e)))?;

        let serialized: Option<SerializedNode> = serde_json::from_slice(&data)
            .map_err(|e| ArtError::Internal(format!("Deserialization error: {}", e)))?;

        // Deserialize the tree
        self.deserialize_tree(serialized)
    }

    /// Serialize a node recursively
    fn serialize_node<V: Clone + Serialize>(
        &self,
        node: &Arc<Node<V>>,
    ) -> ArtResult<SerializedNode> {
        match node.as_ref() {
            Node::Leaf(leaf) => {
                let value = serde_json::to_vec(&leaf.value)
                    .map_err(|e| ArtError::Internal(format!("Value serialization error: {}", e)))?;
                Ok(SerializedNode::Leaf {
                    key: leaf.key.clone(),
                    value,
                })
            }
            Node::Node4(n) => {
                let mut children = Vec::new();
                for child in &n.children {
                    if let Some(c) = child {
                        children.push(self.serialize_node(c)?);
                    }
                }
                let value = if let Some(ref v) = n.header.value {
                    Some(serde_json::to_vec(v).map_err(|e| {
                        ArtError::Internal(format!("Value serialization error: {}", e))
                    })?)
                } else {
                    None
                };
                Ok(SerializedNode::Node4 {
                    partial: n.header.partial.clone(),
                    keys: n.keys[..n.header.num_children as usize].to_vec(),
                    children,
                    value,
                })
            }
            Node::Node16(n) => {
                let mut children = Vec::new();
                for child in &n.children[..n.header.num_children as usize] {
                    if let Some(c) = child {
                        children.push(self.serialize_node(c)?);
                    }
                }
                let value = if let Some(ref v) = n.header.value {
                    Some(serde_json::to_vec(v).map_err(|e| {
                        ArtError::Internal(format!("Value serialization error: {}", e))
                    })?)
                } else {
                    None
                };
                Ok(SerializedNode::Node16 {
                    partial: n.header.partial.clone(),
                    keys: n.keys[..n.header.num_children as usize].to_vec(),
                    children,
                    value,
                })
            }
            Node::Node48(n) => {
                let mut children = Vec::new();
                for i in 0..48 {
                    if let Some(c) = &n.children[i] {
                        children.push(Some(self.serialize_node(c)?));
                    } else {
                        children.push(None);
                    }
                }
                let value = if let Some(ref v) = n.header.value {
                    Some(serde_json::to_vec(v).map_err(|e| {
                        ArtError::Internal(format!("Value serialization error: {}", e))
                    })?)
                } else {
                    None
                };
                Ok(SerializedNode::Node48 {
                    partial: n.header.partial.clone(),
                    child_index: n.child_index.to_vec(),
                    children,
                    value,
                })
            }
            Node::Node256(n) => {
                let mut children = Vec::new();
                for child in &n.children {
                    if let Some(c) = child {
                        children.push(Some(self.serialize_node(c)?));
                    } else {
                        children.push(None);
                    }
                }
                let value = if let Some(ref v) = n.header.value {
                    Some(serde_json::to_vec(v).map_err(|e| {
                        ArtError::Internal(format!("Value serialization error: {}", e))
                    })?)
                } else {
                    None
                };
                Ok(SerializedNode::Node256 {
                    partial: n.header.partial.clone(),
                    children,
                    value,
                })
            }
        }
    }

    /// Serialize the entire tree
    fn serialize_tree<V: Clone + Serialize>(
        &self,
        tree: &AdaptiveRadixTree<V>,
    ) -> ArtResult<Option<SerializedNode>> {
        if let Some(root) = tree.root() {
            Ok(Some(self.serialize_node(&root)?))
        } else {
            Ok(None)
        }
    }

    /// Deserialize a node recursively
    fn deserialize_node<V: Clone + for<'de> Deserialize<'de>>(
        &self,
        serialized: SerializedNode,
    ) -> ArtResult<Arc<Node<V>>> {
        match serialized {
            SerializedNode::Leaf { key, value } => {
                let v: V = serde_json::from_slice(&value).map_err(|e| {
                    ArtError::Internal(format!("Value deserialization error: {}", e))
                })?;
                Ok(Arc::new(Node::new_leaf(key, v)))
            }
            SerializedNode::Node4 {
                partial,
                keys,
                children,
                value,
            } => {
                let mut node = Node::new_node4(partial);
                let v = if let Some(val_bytes) = value {
                    Some(serde_json::from_slice(&val_bytes).map_err(|e| {
                        ArtError::Internal(format!("Value deserialization error: {}", e))
                    })?)
                } else {
                    None
                };

                // Reconstruct node with children
                if let Node::Node4(ref mut n) = node {
                    n.header.value = v;
                    n.header.num_children = children.len() as u16;
                    for (i, (key, child)) in keys.iter().zip(children.iter()).enumerate() {
                        n.keys[i] = *key;
                        n.children[i] = Some(self.deserialize_node(child.clone())?);
                    }
                }
                Ok(Arc::new(node))
            }
            SerializedNode::Node16 {
                partial,
                keys,
                children,
                value,
            } => {
                let mut node = Node::new_node16(partial);
                let v = if let Some(val_bytes) = value {
                    Some(serde_json::from_slice(&val_bytes).map_err(|e| {
                        ArtError::Internal(format!("Value deserialization error: {}", e))
                    })?)
                } else {
                    None
                };

                if let Node::Node16(ref mut n) = node {
                    n.header.value = v;
                    n.header.num_children = children.len() as u16;
                    for (i, (key, child)) in keys.iter().zip(children.iter()).enumerate() {
                        n.keys[i] = *key;
                        n.children[i] = Some(self.deserialize_node(child.clone())?);
                    }
                }
                Ok(Arc::new(node))
            }
            SerializedNode::Node48 {
                partial,
                child_index,
                children,
                value,
            } => {
                let mut node = Node::new_node48(partial);
                let v = if let Some(val_bytes) = value {
                    Some(serde_json::from_slice(&val_bytes).map_err(|e| {
                        ArtError::Internal(format!("Value deserialization error: {}", e))
                    })?)
                } else {
                    None
                };

                if let Node::Node48(ref mut n) = node {
                    n.header.value = v;
                    n.child_index = child_index.try_into().unwrap();
                    let mut count = 0;
                    for (i, child_opt) in children.iter().enumerate() {
                        if let Some(child) = child_opt {
                            n.children[i] = Some(self.deserialize_node(child.clone())?);
                            count += 1;
                        }
                    }
                    n.header.num_children = count as u16;
                }
                Ok(Arc::new(node))
            }
            SerializedNode::Node256 {
                partial,
                children,
                value,
            } => {
                let mut node = Node::new_node256(partial);
                let v = if let Some(val_bytes) = value {
                    Some(serde_json::from_slice(&val_bytes).map_err(|e| {
                        ArtError::Internal(format!("Value deserialization error: {}", e))
                    })?)
                } else {
                    None
                };

                if let Node::Node256(ref mut n) = node {
                    n.header.value = v;
                    let mut count = 0;
                    for (i, child_opt) in children.iter().enumerate() {
                        if let Some(child) = child_opt {
                            n.children[i] = Some(self.deserialize_node(child.clone())?);
                            count += 1;
                        }
                    }
                    n.header.num_children = count as u16;
                }
                Ok(Arc::new(node))
            }
        }
    }

    /// Deserialize the entire tree
    fn deserialize_tree<V: Clone + for<'de> Deserialize<'de>>(
        &self,
        serialized: Option<SerializedNode>,
    ) -> ArtResult<AdaptiveRadixTree<V>> {
        let mut tree = AdaptiveRadixTree::new();

        if let Some(root_serialized) = serialized {
            // Reconstruct tree by traversing and inserting all key-value pairs
            self.reconstruct_tree(&mut tree, root_serialized)?;
        }

        Ok(tree)
    }

    /// Reconstruct tree by inserting all entries
    fn reconstruct_tree<V: Clone + for<'de> Deserialize<'de>>(
        &self,
        tree: &mut AdaptiveRadixTree<V>,
        node: SerializedNode,
    ) -> ArtResult<()> {
        match node {
            SerializedNode::Leaf { key, value } => {
                let v: V = serde_json::from_slice(&value).map_err(|e| {
                    ArtError::Internal(format!("Value deserialization error: {}", e))
                })?;
                tree.insert(key, v)?;
            }
            SerializedNode::Node4 { children, .. } | SerializedNode::Node16 { children, .. } => {
                for child in children {
                    self.reconstruct_tree(tree, child)?;
                }
            }
            SerializedNode::Node48 { children, .. } | SerializedNode::Node256 { children, .. } => {
                for child_opt in children {
                    if let Some(child) = child_opt {
                        self.reconstruct_tree(tree, child)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Count nodes in serialized tree
    fn count_nodes(&self, node: &Option<SerializedNode>) -> usize {
        match node {
            None => 0,
            Some(SerializedNode::Leaf { .. }) => 1,
            Some(SerializedNode::Node4 { children, .. })
            | Some(SerializedNode::Node16 { children, .. }) => {
                1 + children
                    .iter()
                    .map(|c| self.count_nodes(&Some(c.clone())))
                    .sum::<usize>()
            }
            Some(SerializedNode::Node48 { children, .. })
            | Some(SerializedNode::Node256 { children, .. }) => {
                1 + children.iter().map(|c| self.count_nodes(c)).sum::<usize>()
            }
        }
    }

    /// Check if persisted tree exists
    pub fn exists(&self) -> bool {
        let tree_path = format!("{}/tree.art", self.base_path);
        self.fs.exists(&tree_path).unwrap_or(false)
    }

    /// Delete persisted tree
    pub fn delete(&self) -> ArtResult<()> {
        let tree_path = format!("{}/tree.art", self.base_path);
        let metadata_path = format!("{}/metadata.json", self.base_path);

        if self.fs.exists(&tree_path).unwrap_or(false) {
            self.fs
                .remove_file(&tree_path)
                .map_err(|e| ArtError::Internal(format!("Failed to delete tree: {}", e)))?;
        }

        if self.fs.exists(&metadata_path).unwrap_or(false) {
            self.fs
                .remove_file(&metadata_path)
                .map_err(|e| ArtError::Internal(format!("Failed to delete metadata: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::MemoryFileSystem;

    #[test]
    fn test_save_and_load_empty_tree() {
        let fs = Arc::new(MemoryFileSystem::new());
        let persistence = ArtPersistence::new(fs, "/test".to_string()).unwrap();

        let tree: AdaptiveRadixTree<String> = AdaptiveRadixTree::new();
        persistence.save_tree(&tree).unwrap();

        let loaded: AdaptiveRadixTree<String> = persistence.load_tree().unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn test_save_and_load_tree_with_data() {
        let fs = Arc::new(MemoryFileSystem::new());
        let persistence = ArtPersistence::new(fs, "/test".to_string()).unwrap();

        let mut tree = AdaptiveRadixTree::new();
        tree.insert(b"key1".to_vec(), "value1".to_string()).unwrap();
        tree.insert(b"key2".to_vec(), "value2".to_string()).unwrap();
        tree.insert(b"key3".to_vec(), "value3".to_string()).unwrap();

        persistence.save_tree(&tree).unwrap();

        let loaded: AdaptiveRadixTree<String> = persistence.load_tree().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get(b"key1"), Some("value1".to_string()));
        assert_eq!(loaded.get(b"key2"), Some("value2".to_string()));
        assert_eq!(loaded.get(b"key3"), Some("value3".to_string()));
    }

    #[test]
    fn test_persistence_exists() {
        let fs = Arc::new(MemoryFileSystem::new());
        let persistence = ArtPersistence::new(fs, "/test".to_string()).unwrap();

        assert!(!persistence.exists());

        let tree: AdaptiveRadixTree<String> = AdaptiveRadixTree::new();
        persistence.save_tree(&tree).unwrap();

        assert!(persistence.exists());
    }

    #[test]
    fn test_persistence_delete() {
        let fs = Arc::new(MemoryFileSystem::new());
        let persistence = ArtPersistence::new(fs, "/test".to_string()).unwrap();

        let tree: AdaptiveRadixTree<String> = AdaptiveRadixTree::new();
        persistence.save_tree(&tree).unwrap();
        assert!(persistence.exists());

        persistence.delete().unwrap();
        assert!(!persistence.exists());
    }
}
