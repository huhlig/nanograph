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

//! B+Tree persistence layer using VFS
//!
//! This module provides disk persistence for B+Tree nodes using the VFS abstraction.
//! Nodes are serialized to disk and can be loaded on demand (lazy loading).

use crate::error::{BTreeError, BTreeResult};
use crate::node::{BPlusTreeNode, InternalNode, LeafNode, NodeId};
use nanograph_vfs::{DynamicFileSystem, File as VfsFile};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Arc;

/// Serialized node format
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SerializedNode {
    Leaf {
        id: NodeId,
        parent: Option<NodeId>,
        entries: Vec<(Vec<u8>, Vec<u8>)>,
        next: Option<NodeId>,
        prev: Option<NodeId>,
    },
    Internal {
        id: NodeId,
        parent: Option<NodeId>,
        keys: Vec<Vec<u8>>,
        children: Vec<NodeId>,
    },
}

/// Tree metadata stored in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeMetadata {
    pub version: u32,
    pub root_id: NodeId,
    pub next_node_id: u64,
    pub leftmost_leaf: Option<NodeId>,
    pub node_count: usize,
}

/// B+Tree persistence manager
pub struct BTreePersistence {
    fs: Arc<dyn DynamicFileSystem>,
    base_path: String,
}

impl BTreePersistence {
    /// Create a new persistence manager
    pub fn new(fs: Arc<dyn DynamicFileSystem>, base_path: String) -> BTreeResult<Self> {
        // Ensure base directory exists
        fs.create_directory_all(&base_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to create directory: {}", e)))?;

        Ok(Self { fs, base_path })
    }

    /// Save a node to disk
    pub fn save_node(&self, node: &BPlusTreeNode) -> BTreeResult<()> {
        let serialized = match node {
            BPlusTreeNode::Leaf(leaf) => SerializedNode::Leaf {
                id: leaf.id,
                parent: leaf.parent,
                entries: leaf.entries.clone(),
                next: leaf.next,
                prev: leaf.prev,
            },
            BPlusTreeNode::Internal(internal) => SerializedNode::Internal {
                id: internal.id,
                parent: internal.parent,
                keys: internal.keys.clone(),
                children: internal.children.clone(),
            },
        };

        let node_path = self.node_path(node.id());
        let data = serde_json::to_vec(&serialized)
            .map_err(|e| BTreeError::Internal(format!("Serialization error: {}", e)))?;

        let mut file = self
            .fs
            .create_file(&node_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to create file: {}", e)))?;

        file.write_all(&data)
            .map_err(|e| BTreeError::Internal(format!("Failed to write node: {}", e)))?;

        file.sync_all()
            .map_err(|e| BTreeError::Internal(format!("Failed to sync node: {}", e)))?;

        Ok(())
    }

    /// Load a node from disk
    pub fn load_node(&self, node_id: NodeId) -> BTreeResult<BPlusTreeNode> {
        let node_path = self.node_path(node_id);

        let mut file = self
            .fs
            .open_file(&node_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to open file: {}", e)))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| BTreeError::Internal(format!("Failed to read node: {}", e)))?;

        let serialized: SerializedNode = serde_json::from_slice(&data)
            .map_err(|e| BTreeError::Internal(format!("Deserialization error: {}", e)))?;

        let node = match serialized {
            SerializedNode::Leaf {
                id,
                parent,
                entries,
                next,
                prev,
            } => BPlusTreeNode::Leaf(LeafNode {
                id,
                parent,
                entries,
                next,
                prev,
            }),
            SerializedNode::Internal {
                id,
                parent,
                keys,
                children,
            } => BPlusTreeNode::Internal(InternalNode {
                id,
                parent,
                keys,
                children,
            }),
        };

        Ok(node)
    }

    /// Save tree metadata (manifest)
    pub fn save_manifest(&self, metadata: &TreeMetadata) -> BTreeResult<()> {
        let manifest_path = format!("{}/MANIFEST", self.base_path);

        let data = serde_json::to_vec_pretty(&metadata)
            .map_err(|e| BTreeError::Internal(format!("Serialization error: {}", e)))?;

        let mut file = self
            .fs
            .create_file(&manifest_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to create manifest: {}", e)))?;

        file.write_all(&data)
            .map_err(|e| BTreeError::Internal(format!("Failed to write manifest: {}", e)))?;

        file.sync_all()
            .map_err(|e| BTreeError::Internal(format!("Failed to sync manifest: {}", e)))?;

        Ok(())
    }

    /// Load tree metadata (manifest)
    pub fn load_manifest(&self) -> BTreeResult<Option<TreeMetadata>> {
        let manifest_path = format!("{}/MANIFEST", self.base_path);

        // Check if manifest exists
        let exists = self
            .fs
            .exists(&manifest_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to check manifest: {}", e)))?;

        if !exists {
            return Ok(None);
        }

        let mut file = self
            .fs
            .open_file(&manifest_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to open manifest: {}", e)))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| BTreeError::Internal(format!("Failed to read manifest: {}", e)))?;

        let metadata: TreeMetadata = serde_json::from_slice(&data)
            .map_err(|e| BTreeError::Internal(format!("Deserialization error: {}", e)))?;

        Ok(Some(metadata))
    }

    /// Delete a node from disk
    pub fn delete_node(&self, node_id: NodeId) -> BTreeResult<()> {
        let node_path = self.node_path(node_id);

        self.fs
            .remove_file(&node_path)
            .map_err(|e| BTreeError::Internal(format!("Failed to delete node: {}", e)))?;

        Ok(())
    }

    /// Get the file path for a node
    fn node_path(&self, node_id: NodeId) -> String {
        format!("{}/node_{:016x}.json", self.base_path, node_id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::MemoryFileSystem;

    #[test]
    fn test_save_and_load_leaf_node() {
        let fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
        let persistence = BTreePersistence::new(fs, "/btree".to_string()).unwrap();

        let node_id = NodeId::new(1);
        let leaf = BPlusTreeNode::Leaf(LeafNode {
            id: node_id,
            parent: None,
            entries: vec![
                (b"key1".to_vec(), b"value1".to_vec()),
                (b"key2".to_vec(), b"value2".to_vec()),
            ],
            next: None,
            prev: None,
        });

        persistence.save_node(&leaf).unwrap();
        let loaded = persistence.load_node(node_id).unwrap();

        match (leaf, loaded) {
            (BPlusTreeNode::Leaf(original), BPlusTreeNode::Leaf(loaded)) => {
                assert_eq!(original.id, loaded.id);
                assert_eq!(original.entries, loaded.entries);
            }
            _ => panic!("Node type mismatch"),
        }
    }

    #[test]
    fn test_save_and_load_manifest() {
        let fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
        let persistence = BTreePersistence::new(fs, "/btree".to_string()).unwrap();

        let metadata = TreeMetadata {
            version: 1,
            root_id: NodeId::new(0),
            next_node_id: 10,
            leftmost_leaf: Some(NodeId::new(5)),
            node_count: 15,
        };

        persistence.save_manifest(&metadata).unwrap();
        let loaded = persistence.load_manifest().unwrap().unwrap();

        assert_eq!(metadata.version, loaded.version);
        assert_eq!(metadata.root_id, loaded.root_id);
        assert_eq!(metadata.next_node_id, loaded.next_node_id);
        assert_eq!(metadata.node_count, loaded.node_count);
    }
}
