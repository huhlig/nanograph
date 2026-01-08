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

//! B+Tree node structures and operations.
//!
//! This module defines the core node types for the B+Tree implementation:
//! - Internal nodes: contain keys and child pointers for routing
//! - Leaf nodes: contain key-value pairs and links to adjacent leaves

use serde::{Deserialize, Serialize};

/// Unique identifier for a node in the B+Tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Create a new NodeId
    pub fn new(id: u64) -> Self {
        NodeId(id)
    }

    /// Get the raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// A node in the B+Tree, either internal or leaf
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BPlusTreeNode {
    /// Internal node containing keys and child node IDs
    Internal(InternalNode),
    /// Leaf node containing key-value pairs
    Leaf(LeafNode),
}

impl BPlusTreeNode {
    /// Create a new leaf node with the given ID
    pub fn new_leaf(id: NodeId) -> Self {
        BPlusTreeNode::Leaf(LeafNode {
            id,
            entries: Vec::new(),
            next: None,
            prev: None,
            parent: None,
        })
    }

    /// Create a new internal node with the given ID
    pub fn new_internal(id: NodeId) -> Self {
        BPlusTreeNode::Internal(InternalNode {
            id,
            keys: Vec::new(),
            children: Vec::new(),
            parent: None,
        })
    }

    /// Get the node ID
    pub fn id(&self) -> NodeId {
        match self {
            BPlusTreeNode::Internal(node) => node.id,
            BPlusTreeNode::Leaf(node) => node.id,
        }
    }

    /// Get the parent node ID
    pub fn parent(&self) -> Option<NodeId> {
        match self {
            BPlusTreeNode::Internal(node) => node.parent,
            BPlusTreeNode::Leaf(node) => node.parent,
        }
    }

    /// Set the parent node ID
    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        match self {
            BPlusTreeNode::Internal(node) => node.parent = parent,
            BPlusTreeNode::Leaf(node) => node.parent = parent,
        }
    }

    /// Check if this node is a leaf
    pub fn is_leaf(&self) -> bool {
        matches!(self, BPlusTreeNode::Leaf(_))
    }

    /// Check if this node is an internal node
    pub fn is_internal(&self) -> bool {
        matches!(self, BPlusTreeNode::Internal(_))
    }

    /// Get the number of keys in this node
    pub fn key_count(&self) -> usize {
        match self {
            BPlusTreeNode::Internal(node) => node.keys.len(),
            BPlusTreeNode::Leaf(node) => node.entries.len(),
        }
    }

    /// Get the number of entries/keys in this node
    pub fn len(&self) -> usize {
        self.key_count()
    }

    /// Check if the node is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if this node is full (needs splitting)
    pub fn is_full(&self, order: usize) -> bool {
        self.key_count() >= order
    }

    /// Get a value from a leaf node
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self {
            BPlusTreeNode::Leaf(node) => node.get(key).cloned(),
            _ => None,
        }
    }

    /// Insert an entry into a leaf node
    pub fn insert_entry(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            BPlusTreeNode::Leaf(node) => node.insert(key, value),
            _ => None,
        }
    }

    /// Delete an entry from a leaf node
    pub fn delete_entry(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        match self {
            BPlusTreeNode::Leaf(node) => node.remove(key),
            _ => None,
        }
    }

    /// Get the child node ID for a given key (internal nodes only)
    pub fn get_child_for_key(&self, key: &[u8]) -> Option<NodeId> {
        match self {
            BPlusTreeNode::Internal(node) => {
                let idx = node.find_child_index(key);
                node.children.get(idx).copied()
            }
            _ => None,
        }
    }

    /// Insert a key and child into an internal node
    pub fn insert_internal_entry(&mut self, key: Vec<u8>, child: NodeId) {
        if let BPlusTreeNode::Internal(node) = self {
            node.insert_key_child(key, child);
        }
    }

    /// Get all children of an internal node
    pub fn get_all_children(&self) -> Vec<NodeId> {
        match self {
            BPlusTreeNode::Internal(node) => node.children.clone(),
            _ => Vec::new(),
        }
    }

    /// Split a leaf node
    pub fn split_leaf(&mut self, new_node_id: NodeId) -> Result<(Vec<u8>, BPlusTreeNode), String> {
        match self {
            BPlusTreeNode::Leaf(node) => {
                let (separator_key, new_leaf) = node.split();

                // Create new leaf node with proper ID and parent
                let new_node = BPlusTreeNode::Leaf(LeafNode {
                    id: new_node_id,
                    entries: new_leaf.entries,
                    next: new_leaf.next,
                    prev: Some(node.id),
                    parent: node.parent,
                });

                // Update this node's next pointer
                node.next = Some(new_node_id);

                Ok((separator_key, new_node))
            }
            _ => Err("Cannot split non-leaf node as leaf".to_string()),
        }
    }

    /// Split an internal node
    pub fn split_internal(
        &mut self,
        new_node_id: NodeId,
    ) -> Result<(Vec<u8>, BPlusTreeNode), String> {
        match self {
            BPlusTreeNode::Internal(node) => {
                let (separator_key, new_internal) = node.split();

                // Create new internal node with proper ID and parent
                let new_node = BPlusTreeNode::Internal(InternalNode {
                    id: new_node_id,
                    keys: new_internal.keys,
                    children: new_internal.children,
                    parent: node.parent,
                });

                Ok((separator_key, new_node))
            }
            _ => Err("Cannot split non-internal node as internal".to_string()),
        }
    }

    /// Get a reference to the internal node, if this is one
    pub fn as_internal(&self) -> Option<&InternalNode> {
        match self {
            BPlusTreeNode::Internal(node) => Some(node),
            _ => None,
        }
    }

    /// Get a mutable reference to the internal node, if this is one
    pub fn as_internal_mut(&mut self) -> Option<&mut InternalNode> {
        match self {
            BPlusTreeNode::Internal(node) => Some(node),
            _ => None,
        }
    }

    /// Get a reference to the leaf node, if this is one
    pub fn as_leaf(&self) -> Option<&LeafNode> {
        match self {
            BPlusTreeNode::Leaf(node) => Some(node),
            _ => None,
        }
    }

    /// Get a mutable reference to the leaf node, if this is one
    pub fn as_leaf_mut(&mut self) -> Option<&mut LeafNode> {
        match self {
            BPlusTreeNode::Leaf(node) => Some(node),
            _ => None,
        }
    }
}

/// Internal node in the B+Tree
///
/// Internal nodes contain keys for routing and child pointers.
/// For n keys, there are n+1 children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalNode {
    /// Node ID
    pub id: NodeId,
    /// Routing keys (sorted)
    pub keys: Vec<Vec<u8>>,
    /// Child node IDs (one more than keys)
    pub children: Vec<NodeId>,
    /// Parent node ID
    pub parent: Option<NodeId>,
}

impl InternalNode {
    /// Create a new empty internal node
    pub fn new(id: NodeId) -> Self {
        InternalNode {
            id,
            keys: Vec::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    /// Find the child index for a given key
    pub fn find_child_index(&self, key: &[u8]) -> usize {
        // Binary search to find the appropriate child
        match self.keys.binary_search_by(|k| k.as_slice().cmp(key)) {
            Ok(idx) => idx + 1, // Key found, go to right child
            Err(idx) => idx,    // Key not found, idx is the insertion point
        }
    }

    /// Insert a key and child at the appropriate position
    pub fn insert_key_child(&mut self, key: Vec<u8>, child: NodeId) {
        let idx = match self.keys.binary_search(&key) {
            Ok(idx) | Err(idx) => idx,
        };
        self.keys.insert(idx, key);
        self.children.insert(idx + 1, child);
    }

    /// Split this internal node into two nodes
    ///
    /// Returns (middle_key, new_right_node)
    pub fn split(&mut self) -> (Vec<u8>, InternalNode) {
        let mid = self.keys.len() / 2;

        // The middle key moves up to the parent
        let middle_key = self.keys[mid].clone();

        // Create the new right node with keys after middle
        let right_keys = self.keys.split_off(mid + 1);
        let right_children = self.children.split_off(mid + 1);

        // Remove the middle key from left node (it goes to parent)
        self.keys.pop();

        let right_node = InternalNode {
            id: self.id, // Will be replaced by caller
            keys: right_keys,
            children: right_children,
            parent: self.parent,
        };

        (middle_key, right_node)
    }

    /// Remove a key at the given index
    pub fn remove_key(&mut self, idx: usize) {
        if idx < self.keys.len() {
            self.keys.remove(idx);
        }
    }

    /// Remove a child at the given index
    pub fn remove_child(&mut self, idx: usize) {
        if idx < self.children.len() {
            self.children.remove(idx);
        }
    }

    /// Replace a key at the given index
    pub fn replace_key(&mut self, idx: usize, key: Vec<u8>) {
        if idx < self.keys.len() {
            self.keys[idx] = key;
        }
    }
}

/// Leaf node in the B+Tree
///
/// Leaf nodes contain the actual key-value pairs and are linked together
/// for efficient range scans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafNode {
    /// Node ID
    pub id: NodeId,
    /// Key-value entries (sorted by key)
    pub entries: Vec<(Vec<u8>, Vec<u8>)>,
    /// Link to the next leaf node (for range scans)
    pub next: Option<NodeId>,
    /// Link to the previous leaf node (for reverse scans)
    pub prev: Option<NodeId>,
    /// Parent node ID
    pub parent: Option<NodeId>,
}

impl LeafNode {
    /// Create a new empty leaf node
    pub fn new(id: NodeId) -> Self {
        LeafNode {
            id,
            entries: Vec::new(),
            next: None,
            prev: None,
            parent: None,
        }
    }

    /// Find the index of a key in this leaf
    pub fn find_key_index(&self, key: &[u8]) -> Result<usize, usize> {
        self.entries
            .binary_search_by(|(k, _)| k.as_slice().cmp(key))
    }

    /// Insert or update a key-value pair
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        match self.find_key_index(&key) {
            Ok(idx) => {
                // Key exists, update value
                let old_value = self.entries[idx].1.clone();
                self.entries[idx].1 = value;
                Some(old_value)
            }
            Err(idx) => {
                // Key doesn't exist, insert new entry
                self.entries.insert(idx, (key, value));
                None
            }
        }
    }

    /// Remove a key-value pair
    pub fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        match self.find_key_index(key) {
            Ok(idx) => {
                let (_, value) = self.entries.remove(idx);
                Some(value)
            }
            Err(_) => None,
        }
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        match self.find_key_index(key) {
            Ok(idx) => Some(&self.entries[idx].1),
            Err(_) => None,
        }
    }

    /// Split this leaf node into two nodes
    ///
    /// Returns (middle_key, new_right_node)
    pub fn split(&mut self) -> (Vec<u8>, LeafNode) {
        let mid = self.entries.len() / 2;

        // Split entries
        let right_entries = self.entries.split_off(mid);

        // The first key of the right node becomes the separator
        let middle_key = right_entries[0].0.clone();

        // Create the new right node
        let right_node = LeafNode {
            id: self.id, // Will be replaced by caller
            entries: right_entries,
            next: self.next,
            prev: None, // Will be set by caller
            parent: self.parent,
        };

        (middle_key, right_node)
    }

    /// Check if this leaf is underfull (needs merging or redistribution)
    pub fn is_underfull(&self, min_entries: usize) -> bool {
        self.entries.len() < min_entries
    }

    /// Merge this leaf with another leaf (used during deletion)
    pub fn merge(&mut self, other: &mut LeafNode) {
        self.entries.append(&mut other.entries);
        self.next = other.next;
    }

    /// Borrow entries from a sibling leaf (used during deletion)
    pub fn borrow_from_right(&mut self, right: &mut LeafNode) -> Vec<u8> {
        if let Some(entry) = right.entries.first().cloned() {
            right.entries.remove(0);
            self.entries.push(entry.clone());
            entry.0
        } else {
            Vec::new()
        }
    }

    /// Borrow entries from a left sibling (used during deletion)
    pub fn borrow_from_left(&mut self, left: &mut LeafNode) -> Vec<u8> {
        if let Some(entry) = left.entries.pop() {
            self.entries.insert(0, entry.clone());
            self.entries[0].0.clone()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id = NodeId::new(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn test_internal_node_find_child() {
        let mut node = InternalNode::new(NodeId::new(1));
        node.keys = vec![b"b".to_vec(), b"d".to_vec(), b"f".to_vec()];
        node.children = vec![
            NodeId::new(1),
            NodeId::new(2),
            NodeId::new(3),
            NodeId::new(4),
        ];

        assert_eq!(node.find_child_index(b"a"), 0);
        assert_eq!(node.find_child_index(b"b"), 1);
        assert_eq!(node.find_child_index(b"c"), 1);
        assert_eq!(node.find_child_index(b"d"), 2);
        assert_eq!(node.find_child_index(b"e"), 2);
        assert_eq!(node.find_child_index(b"f"), 3);
        assert_eq!(node.find_child_index(b"g"), 3);
    }

    #[test]
    fn test_leaf_node_insert() {
        let mut node = LeafNode::new(NodeId::new(1));

        assert_eq!(node.insert(b"key1".to_vec(), b"value1".to_vec()), None);
        assert_eq!(node.insert(b"key2".to_vec(), b"value2".to_vec()), None);
        assert_eq!(node.entries.len(), 2);

        // Update existing key
        assert_eq!(
            node.insert(b"key1".to_vec(), b"new_value".to_vec()),
            Some(b"value1".to_vec())
        );
        assert_eq!(node.entries.len(), 2);
    }

    #[test]
    fn test_leaf_node_remove() {
        let mut node = LeafNode::new(NodeId::new(1));
        node.insert(b"key1".to_vec(), b"value1".to_vec());
        node.insert(b"key2".to_vec(), b"value2".to_vec());

        assert_eq!(node.remove(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(node.entries.len(), 1);
        assert_eq!(node.remove(b"key1"), None);
    }

    #[test]
    fn test_leaf_node_split() {
        let mut node = LeafNode::new(NodeId::new(1));
        for i in 0..10 {
            let key = format!("key{:02}", i).into_bytes();
            let value = format!("value{}", i).into_bytes();
            node.insert(key, value);
        }

        let (middle_key, right) = node.split();

        assert_eq!(node.entries.len(), 5);
        assert_eq!(right.entries.len(), 5);
        assert_eq!(middle_key, b"key05".to_vec());
    }
}

// Made with Bob
