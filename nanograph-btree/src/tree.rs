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

use crate::error::{BTreeError, BTreeResult};
use crate::node::{BPlusTreeNode, BTreeNodeId};
use crate::persistence::{BTreePersistence, TreeMetadata};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, instrument, warn};

/// Configuration for B+Tree
#[derive(Debug, Clone)]
pub struct BPlusTreeConfig {
    /// Maximum number of keys per node (order)
    pub max_keys: usize,
    /// Minimum number of keys per node (typically max_keys / 2)
    pub min_keys: usize,
}

impl Default for BPlusTreeConfig {
    fn default() -> Self {
        Self {
            max_keys: 128,
            min_keys: 64,
        }
    }
}

/// In-memory B+Tree implementation with optional persistence
/// All data is stored in leaf nodes, internal nodes only contain routing keys
pub struct BPlusTree {
    /// Configuration
    config: BPlusTreeConfig,

    /// Root node ID
    root_id: RwLock<BTreeNodeId>,

    /// All nodes in the tree
    nodes: Arc<RwLock<HashMap<BTreeNodeId, BPlusTreeNode>>>,

    /// Next available node ID
    next_node_id: RwLock<u64>,

    /// ID of the leftmost leaf (for range scans)
    leftmost_leaf: RwLock<Option<BTreeNodeId>>,

    /// Optional persistence layer
    persistence: Option<Arc<BTreePersistence>>,
}

impl BPlusTree {
    /// Create a new B+Tree without persistence
    pub fn new(config: BPlusTreeConfig) -> Self {
        let root_id = BTreeNodeId::new(0);
        let root = BPlusTreeNode::new_leaf(root_id);

        let mut nodes = HashMap::new();
        nodes.insert(root_id, root);

        Self {
            config,
            root_id: RwLock::new(root_id),
            nodes: Arc::new(RwLock::new(nodes)),
            next_node_id: RwLock::new(1),
            leftmost_leaf: RwLock::new(Some(root_id)),
            persistence: None,
        }
    }

    /// Create a new B+Tree with persistence
    pub fn with_persistence(
        config: BPlusTreeConfig,
        persistence: Arc<BTreePersistence>,
    ) -> BTreeResult<Self> {
        // Try to load existing tree from manifest
        if let Some(metadata) = persistence.load_manifest()? {
            // Load root node
            let root = persistence.load_node(metadata.root_id)?;
            let mut nodes = HashMap::new();
            nodes.insert(metadata.root_id, root);

            Ok(Self {
                config,
                root_id: RwLock::new(metadata.root_id),
                nodes: Arc::new(RwLock::new(nodes)),
                next_node_id: RwLock::new(metadata.next_node_id),
                leftmost_leaf: RwLock::new(metadata.leftmost_leaf),
                persistence: Some(persistence),
            })
        } else {
            // Create new tree
            let root_id = BTreeNodeId::new(0);
            let root = BPlusTreeNode::new_leaf(root_id);

            let mut nodes = HashMap::new();
            nodes.insert(root_id, root.clone());

            let tree = Self {
                config,
                root_id: RwLock::new(root_id),
                nodes: Arc::new(RwLock::new(nodes)),
                next_node_id: RwLock::new(1),
                leftmost_leaf: RwLock::new(Some(root_id)),
                persistence: Some(persistence.clone()),
            };

            // Save initial state
            persistence.save_node(&root)?;
            tree.save_manifest()?;

            Ok(tree)
        }
    }

    /// Flush all dirty nodes to disk
    pub fn flush(&self) -> BTreeResult<()> {
        if let Some(ref persistence) = self.persistence {
            let nodes = self.nodes.read().unwrap();
            for node in nodes.values() {
                persistence.save_node(node)?;
            }
            self.save_manifest()?;
        }
        Ok(())
    }

    /// Save tree metadata to manifest
    fn save_manifest(&self) -> BTreeResult<()> {
        if let Some(ref persistence) = self.persistence {
            let metadata = TreeMetadata {
                version: 1,
                root_id: *self.root_id.read().unwrap(),
                next_node_id: *self.next_node_id.read().unwrap(),
                leftmost_leaf: *self.leftmost_leaf.read().unwrap(),
                node_count: self.nodes.read().unwrap().len(),
            };
            persistence.save_manifest(&metadata)?;
        }
        Ok(())
    }

    /// Clear all data from the tree
    pub fn clear(&self) -> BTreeResult<()> {
        let root_id = BTreeNodeId::new(0);
        let root = BPlusTreeNode::new_leaf(root_id);

        {
            let mut nodes = self.nodes.write().unwrap();
            nodes.clear();
            nodes.insert(root_id, root.clone());
        }

        *self.root_id.write().unwrap() = root_id;
        *self.next_node_id.write().unwrap() = 1;
        *self.leftmost_leaf.write().unwrap() = Some(root_id);

        if let Some(ref persistence) = self.persistence {
            persistence.save_node(&root)?;
            self.save_manifest()?;
        }

        Ok(())
    }

    /// Allocate a new node ID
    fn allocate_node_id(&self) -> BTreeNodeId {
        let mut next_id = self.next_node_id.write().unwrap();
        let id = BTreeNodeId::new(*next_id);
        *next_id += 1;
        id
    }

    /// Get a node by ID
    fn get_node(&self, node_id: BTreeNodeId) -> BTreeResult<BPlusTreeNode> {
        let nodes = self.nodes.read().unwrap();
        nodes
            .get(&node_id)
            .cloned()
            .ok_or(BTreeError::Internal(format!(
                "Node {:?} not found",
                node_id
            )))
    }

    /// Update a node
    fn update_node(&self, node: BPlusTreeNode) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id(), node);
    }

    /// Remove a node
    /// TODO: Use during node merging and rebalancing operations
    #[allow(dead_code)]
    fn remove_node(&self, node_id: BTreeNodeId) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.remove(&node_id);
    }

    /// Search for a key and return its value
    #[instrument(skip(self, key), fields(key_len = key.len()))]
    pub fn get(&self, key: &[u8]) -> BTreeResult<Option<Vec<u8>>> {
        debug!("B-tree get operation started");
        let root_id = *self.root_id.read().unwrap();
        let leaf_id = self.find_leaf(root_id, key)?;
        let leaf = self.get_node(leaf_id)?;
        let result = leaf.get(key);
        debug!(
            "B-tree get operation completed, found: {}",
            result.is_some()
        );
        Ok(result)
    }

    /// Find the leaf node that should contain the given key
    pub fn find_leaf(&self, mut node_id: BTreeNodeId, key: &[u8]) -> BTreeResult<BTreeNodeId> {
        loop {
            let node = self.get_node(node_id)?;

            if node.is_leaf() {
                return Ok(node_id);
            }

            // Navigate to child
            node_id = node
                .get_child_for_key(key)
                .ok_or(BTreeError::NodeNotFound)?;
        }
    }

    /// Get the root node ID (public accessor for iterator)
    pub fn root_id(&self) -> &RwLock<BTreeNodeId> {
        &self.root_id
    }

    /// Insert a key-value pair
    #[instrument(skip(self, key, value), fields(key_len = key.len(), value_len = value.len()))]
    pub fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> BTreeResult<()> {
        debug!("B-tree insert operation started");
        let root_id = *self.root_id.read().unwrap();

        // Find the leaf node
        let leaf_id = self.find_leaf(root_id, &key)?;
        let mut leaf = self.get_node(leaf_id)?;

        // Insert into leaf
        leaf.insert_entry(key.clone(), value);

        // Check if leaf needs to be split
        if leaf.is_full(self.config.max_keys) {
            info!("Leaf node is full, triggering split");
            self.split_leaf(leaf)?;
        } else {
            self.update_node(leaf);
        }

        debug!("B-tree insert operation completed");
        Ok(())
    }

    /// Split a leaf node and propagate splits up the tree
    fn split_leaf(&self, mut leaf: BPlusTreeNode) -> BTreeResult<()> {
        let new_node_id = self.allocate_node_id();
        let (separator_key, new_leaf) = leaf.split_leaf(new_node_id)?;

        // Update the original leaf
        self.update_node(leaf.clone());

        // Insert the new leaf
        self.update_node(new_leaf.clone());

        // Insert separator key into parent
        if let Some(parent_id) = leaf.parent() {
            self.insert_into_parent(parent_id, separator_key, new_leaf.id())?;
        } else {
            // Leaf was root, create new root
            self.create_new_root(leaf.id(), separator_key, new_leaf.id())?;
        }

        Ok(())
    }

    /// Insert a key and child pointer into a parent node
    fn insert_into_parent(
        &self,
        parent_id: BTreeNodeId,
        key: Vec<u8>,
        right_child: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut parent = self.get_node(parent_id)?;

        // Insert the key and child
        parent.insert_internal_entry(key.clone(), right_child);

        // Check if parent needs to be split
        if parent.is_full(self.config.max_keys) {
            self.split_internal(parent)?;
        } else {
            self.update_node(parent);
        }

        Ok(())
    }

    /// Split an internal node and propagate splits up the tree
    fn split_internal(&self, mut internal: BPlusTreeNode) -> BTreeResult<()> {
        let new_node_id = self.allocate_node_id();
        let (separator_key, new_internal) = internal.split_internal(new_node_id)?;

        // Update children's parent pointers for the new node
        for child_id in new_internal.get_all_children() {
            let mut child = self.get_node(child_id)?;
            child.set_parent(Some(new_internal.id()));
            self.update_node(child);
        }

        // Update the original internal node
        self.update_node(internal.clone());

        // Insert the new internal node
        self.update_node(new_internal.clone());

        // Insert separator key into parent
        if let Some(parent_id) = internal.parent() {
            self.insert_into_parent(parent_id, separator_key, new_internal.id())?;
        } else {
            // Internal was root, create new root
            self.create_new_root(internal.id(), separator_key, new_internal.id())?;
        }

        Ok(())
    }

    /// Create a new root node
    fn create_new_root(
        &self,
        left_child: BTreeNodeId,
        key: Vec<u8>,
        right_child: BTreeNodeId,
    ) -> BTreeResult<()> {
        let new_root_id = self.allocate_node_id();
        let mut new_root = BPlusTreeNode::new_internal(new_root_id);
        // Add the left child first (it goes before the key)
        if let BPlusTreeNode::Internal(ref mut internal) = new_root {
            internal.children.push(left_child);
        }
        new_root.insert_internal_entry(key, right_child);

        // Update children's parent pointers
        let mut left = self.get_node(left_child)?;
        left.set_parent(Some(new_root_id));
        self.update_node(left);

        let mut right = self.get_node(right_child)?;
        right.set_parent(Some(new_root_id));
        self.update_node(right);

        // Insert new root
        self.update_node(new_root);

        // Update root ID
        *self.root_id.write().unwrap() = new_root_id;

        Ok(())
    }

    /// Delete a key
    #[instrument(skip(self, key), fields(key_len = key.len()))]
    pub fn delete(&self, key: &[u8]) -> BTreeResult<bool> {
        debug!("B-tree delete operation started");
        let root_id = *self.root_id.read().unwrap();

        // Find the leaf node
        let leaf_id = self.find_leaf(root_id, key)?;
        let mut leaf = self.get_node(leaf_id)?;

        // Delete from leaf
        let deleted = leaf.delete_entry(key);

        if deleted.is_none() {
            debug!("B-tree delete operation completed, key not found");
            return Ok(false);
        }

        // Check if leaf has too few keys (underflow)
        if leaf.len() < self.config.min_keys && leaf.parent().is_some() {
            info!("Leaf underflow detected, handling rebalance");
            self.handle_underflow(leaf)?;
        } else {
            self.update_node(leaf);
        }

        debug!("B-tree delete operation completed successfully");
        Ok(true)
    }

    /// Handle node underflow after deletion by borrowing from siblings or merging
    fn handle_underflow(&self, mut node: BPlusTreeNode) -> BTreeResult<()> {
        let parent_id = node
            .parent()
            .ok_or(BTreeError::Internal("Node has no parent".to_string()))?;
        let parent = self.get_node(parent_id)?;

        // Try to borrow from left sibling
        if let Some(left_sibling_id) = self.get_left_sibling(&node, &parent)? {
            let left_sibling = self.get_node(left_sibling_id)?;
            if left_sibling.len() > self.config.min_keys {
                return self.borrow_from_left(&mut node, left_sibling_id);
            }
        }

        // Try to borrow from right sibling
        if let Some(right_sibling_id) = self.get_right_sibling(&node, &parent)? {
            let right_sibling = self.get_node(right_sibling_id)?;
            if right_sibling.len() > self.config.min_keys {
                return self.borrow_from_right(&mut node, right_sibling_id);
            }
        }

        // Can't borrow, must merge with a sibling
        if let Some(left_sibling_id) = self.get_left_sibling(&node, &parent)? {
            return self.merge_with_left(&mut node, left_sibling_id);
        }

        if let Some(right_sibling_id) = self.get_right_sibling(&node, &parent)? {
            return self.merge_with_right(&mut node, right_sibling_id);
        }

        // No siblings, just update
        self.update_node(node);
        Ok(())
    }

    /// Get the left sibling of a node
    fn get_left_sibling(
        &self,
        node: &BPlusTreeNode,
        parent: &BPlusTreeNode,
    ) -> BTreeResult<Option<BTreeNodeId>> {
        if let BPlusTreeNode::Internal(parent_internal) = parent {
            let node_id = node.id();
            if let Some(pos) = parent_internal
                .children
                .iter()
                .position(|&id| id == node_id)
            {
                if pos > 0 {
                    return Ok(Some(parent_internal.children[pos - 1]));
                }
            }
        }
        Ok(None)
    }

    /// Get the right sibling of a node
    fn get_right_sibling(
        &self,
        node: &BPlusTreeNode,
        parent: &BPlusTreeNode,
    ) -> BTreeResult<Option<BTreeNodeId>> {
        if let BPlusTreeNode::Internal(parent_internal) = parent {
            let node_id = node.id();
            if let Some(pos) = parent_internal
                .children
                .iter()
                .position(|&id| id == node_id)
            {
                if pos + 1 < parent_internal.children.len() {
                    return Ok(Some(parent_internal.children[pos + 1]));
                }
            }
        }
        Ok(None)
    }

    /// Update the separator key in parent for a given child node
    fn update_separator_key(
        &self,
        parent_id: BTreeNodeId,
        child_id: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut parent = self.get_node(parent_id)?;
        let child = self.get_node(child_id)?;

        if let BPlusTreeNode::Internal(ref mut parent_internal) = parent {
            // Find the position of this child
            if let Some(pos) = parent_internal
                .children
                .iter()
                .position(|&id| id == child_id)
            {
                // Update the separator key (the key before this child, if it exists)
                if pos > 0 && pos <= parent_internal.keys.len() {
                    // Get the first key from the child node
                    let new_separator = match &child {
                        BPlusTreeNode::Leaf(leaf) => {
                            if !leaf.entries.is_empty() {
                                leaf.entries[0].0.clone()
                            } else {
                                return Ok(());
                            }
                        }
                        BPlusTreeNode::Internal(internal) => {
                            if !internal.keys.is_empty() {
                                internal.keys[0].clone()
                            } else {
                                return Ok(());
                            }
                        }
                    };
                    parent_internal.keys[pos - 1] = new_separator;
                    self.update_node(parent);
                }
            }
        }

        Ok(())
    }

    /// Borrow an entry from the left sibling
    fn borrow_from_left(
        &self,
        node: &mut BPlusTreeNode,
        left_sibling_id: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut left_sibling = self.get_node(left_sibling_id)?;
        let node_id = node.id();
        let parent_id = node
            .parent()
            .ok_or(BTreeError::Internal("Node has no parent".to_string()))?;

        match (&mut *node, &mut left_sibling) {
            (BPlusTreeNode::Leaf(leaf), BPlusTreeNode::Leaf(left_leaf)) => {
                // Move last entry from left sibling to front of this node
                if let Some(entry) = left_leaf.entries.pop() {
                    leaf.entries.insert(0, entry);
                }
            }
            (BPlusTreeNode::Internal(internal), BPlusTreeNode::Internal(left_internal)) => {
                // For internal nodes, we need to rotate through the parent
                // Get the separator key from parent
                let mut parent = self.get_node(parent_id)?;
                if let BPlusTreeNode::Internal(ref mut parent_internal) = parent {
                    if let Some(pos) = parent_internal
                        .children
                        .iter()
                        .position(|&id| id == node_id)
                    {
                        if pos > 0 && pos <= parent_internal.keys.len() {
                            // Move separator key down to this node
                            let separator = parent_internal.keys[pos - 1].clone();
                            internal.keys.insert(0, separator);

                            // Move last child from left sibling
                            if let Some(child) = left_internal.children.pop() {
                                internal.children.insert(0, child);

                                // Update child's parent pointer
                                let mut child_node = self.get_node(child)?;
                                child_node.set_parent(Some(node_id));
                                self.update_node(child_node);
                            }

                            // Move last key from left sibling up to parent
                            if let Some(key) = left_internal.keys.pop() {
                                parent_internal.keys[pos - 1] = key;
                            }

                            self.update_node(parent);
                        }
                    }
                }
            }
            _ => return Err(BTreeError::Internal("Node type mismatch".to_string())),
        }

        self.update_node(left_sibling);
        self.update_node(node.clone());

        // Update separator key in parent for leaf nodes
        if node.is_leaf() {
            self.update_separator_key(parent_id, node_id)?;
        }

        Ok(())
    }

    /// Borrow an entry from the right sibling
    fn borrow_from_right(
        &self,
        node: &mut BPlusTreeNode,
        right_sibling_id: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut right_sibling = self.get_node(right_sibling_id)?;
        let node_id = node.id();
        let parent_id = node
            .parent()
            .ok_or(BTreeError::Internal("Node has no parent".to_string()))?;

        match (&mut *node, &mut right_sibling) {
            (BPlusTreeNode::Leaf(leaf), BPlusTreeNode::Leaf(right_leaf)) => {
                // Move first entry from right sibling to end of this node
                if !right_leaf.entries.is_empty() {
                    let entry = right_leaf.entries.remove(0);
                    leaf.entries.push(entry);
                }
            }
            (BPlusTreeNode::Internal(internal), BPlusTreeNode::Internal(right_internal)) => {
                // For internal nodes, we need to rotate through the parent
                // Get the separator key from parent
                let mut parent = self.get_node(parent_id)?;
                if let BPlusTreeNode::Internal(ref mut parent_internal) = parent {
                    if let Some(pos) = parent_internal
                        .children
                        .iter()
                        .position(|&id| id == node_id)
                    {
                        if pos < parent_internal.keys.len() {
                            // Move separator key down to this node
                            let separator = parent_internal.keys[pos].clone();
                            internal.keys.push(separator);

                            // Move first child from right sibling
                            if !right_internal.children.is_empty() {
                                let child = right_internal.children.remove(0);
                                internal.children.push(child);

                                // Update child's parent pointer
                                let mut child_node = self.get_node(child)?;
                                child_node.set_parent(Some(node_id));
                                self.update_node(child_node);
                            }

                            // Move first key from right sibling up to parent
                            if !right_internal.keys.is_empty() {
                                let key = right_internal.keys.remove(0);
                                parent_internal.keys[pos] = key;
                            }

                            self.update_node(parent);
                        }
                    }
                }
            }
            _ => return Err(BTreeError::Internal("Node type mismatch".to_string())),
        }

        self.update_node(right_sibling.clone());
        self.update_node(node.clone());

        // Update separator key in parent for the right sibling (leaf nodes only)
        if right_sibling.is_leaf() {
            self.update_separator_key(parent_id, right_sibling_id)?;
        }

        Ok(())
    }

    /// Merge node with its left sibling
    fn merge_with_left(
        &self,
        node: &mut BPlusTreeNode,
        left_sibling_id: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut left_sibling = self.get_node(left_sibling_id)?;
        let node_id = node.id();
        let parent_id = node
            .parent()
            .ok_or(BTreeError::Internal("Node has no parent".to_string()))?;

        match (&mut *node, &mut left_sibling) {
            (BPlusTreeNode::Leaf(leaf), BPlusTreeNode::Leaf(left_leaf)) => {
                // Move all entries from this node to left sibling
                left_leaf.entries.append(&mut leaf.entries);
                left_leaf.next = leaf.next;

                // Update next leaf's prev pointer
                if let Some(next_id) = leaf.next {
                    let mut next_leaf = self.get_node(next_id)?;
                    if let BPlusTreeNode::Leaf(next) = &mut next_leaf {
                        next.prev = Some(left_sibling_id);
                    }
                    self.update_node(next_leaf);
                }
            }
            (BPlusTreeNode::Internal(internal), BPlusTreeNode::Internal(left_internal)) => {
                // For internal nodes, we need to pull down the separator key from parent
                let parent = self.get_node(parent_id)?;
                if let BPlusTreeNode::Internal(parent_internal) = &parent {
                    if let Some(pos) = parent_internal
                        .children
                        .iter()
                        .position(|&id| id == node_id)
                    {
                        if pos > 0 && pos <= parent_internal.keys.len() {
                            // Pull down the separator key
                            let separator = parent_internal.keys[pos - 1].clone();
                            left_internal.keys.push(separator);
                        }
                    }
                }

                // Merge internal nodes
                left_internal.keys.append(&mut internal.keys);
                left_internal.children.append(&mut internal.children);

                // Update children's parent pointers
                for child_id in left_internal.children.iter() {
                    let mut child = self.get_node(*child_id)?;
                    child.set_parent(Some(left_sibling_id));
                    self.update_node(child);
                }
            }
            _ => return Err(BTreeError::Internal("Node type mismatch".to_string())),
        }

        self.update_node(left_sibling);
        self.remove_node(node_id);

        // Remove separator key from parent
        self.remove_from_parent(parent_id, node_id)?;

        Ok(())
    }

    /// Merge node with its right sibling
    fn merge_with_right(
        &self,
        node: &mut BPlusTreeNode,
        right_sibling_id: BTreeNodeId,
    ) -> BTreeResult<()> {
        let mut right_sibling = self.get_node(right_sibling_id)?;
        let node_id = node.id();
        let parent_id = node
            .parent()
            .ok_or(BTreeError::Internal("Node has no parent".to_string()))?;

        match (&mut *node, &mut right_sibling) {
            (BPlusTreeNode::Leaf(leaf), BPlusTreeNode::Leaf(right_leaf)) => {
                // Move all entries from right sibling to this node
                leaf.entries.append(&mut right_leaf.entries);
                leaf.next = right_leaf.next;

                // Update next leaf's prev pointer
                if let Some(next_id) = right_leaf.next {
                    let mut next_leaf = self.get_node(next_id)?;
                    if let BPlusTreeNode::Leaf(next) = &mut next_leaf {
                        next.prev = Some(node_id);
                    }
                    self.update_node(next_leaf);
                }
            }
            (BPlusTreeNode::Internal(internal), BPlusTreeNode::Internal(right_internal)) => {
                // For internal nodes, we need to pull down the separator key from parent
                let parent = self.get_node(parent_id)?;
                if let BPlusTreeNode::Internal(parent_internal) = &parent {
                    if let Some(pos) = parent_internal
                        .children
                        .iter()
                        .position(|&id| id == node_id)
                    {
                        if pos < parent_internal.keys.len() {
                            // Pull down the separator key
                            let separator = parent_internal.keys[pos].clone();
                            internal.keys.push(separator);
                        }
                    }
                }

                // Merge internal nodes
                internal.keys.append(&mut right_internal.keys);
                internal.children.append(&mut right_internal.children);

                // Update children's parent pointers
                for child_id in internal.children.iter() {
                    let mut child = self.get_node(*child_id)?;
                    child.set_parent(Some(node_id));
                    self.update_node(child);
                }
            }
            _ => return Err(BTreeError::Internal("Node type mismatch".to_string())),
        }

        self.update_node(node.clone());
        self.remove_node(right_sibling_id);

        // Remove separator key from parent
        self.remove_from_parent(parent_id, right_sibling_id)?;

        Ok(())
    }

    /// Remove a child reference from parent node
    fn remove_from_parent(&self, parent_id: BTreeNodeId, child_id: BTreeNodeId) -> BTreeResult<()> {
        let mut parent = self.get_node(parent_id)?;

        if let BPlusTreeNode::Internal(ref mut internal) = parent {
            if let Some(pos) = internal.children.iter().position(|&id| id == child_id) {
                internal.children.remove(pos);
                // Remove the separator key before this child (if it exists)
                if pos > 0 && pos <= internal.keys.len() {
                    internal.keys.remove(pos - 1);
                } else if pos == 0 && !internal.keys.is_empty() {
                    // If removing the first child, remove the first key
                    internal.keys.remove(0);
                }
            }
        }

        // Special case: if parent is root and has only one child left, make that child the new root
        let is_root = parent.parent().is_none();
        if is_root {
            if let BPlusTreeNode::Internal(ref internal) = parent {
                if internal.children.len() == 1 {
                    let new_root_id = internal.children[0];
                    let mut new_root = self.get_node(new_root_id)?;
                    new_root.set_parent(None);
                    self.update_node(new_root);
                    *self.root_id.write().unwrap() = new_root_id;
                    self.remove_node(parent_id);
                    return Ok(());
                }
            }
        }

        // Check if parent now has underflow
        if parent.len() < self.config.min_keys && parent.parent().is_some() {
            self.handle_underflow(parent)?;
        } else {
            self.update_node(parent);
        }

        Ok(())
    }

    /// Get the leftmost leaf node (for range scans)
    pub fn get_leftmost_leaf(&self) -> BTreeResult<BTreeNodeId> {
        self.leftmost_leaf
            .read()
            .unwrap()
            .ok_or(BTreeError::Internal("No leftmost leaf".to_string()))
    }

    /// Get all entries in a leaf node
    pub fn get_leaf_entries(&self, leaf_id: BTreeNodeId) -> BTreeResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let leaf = self.get_node(leaf_id)?;

        match leaf {
            BPlusTreeNode::Leaf(leaf) => Ok(leaf.entries.clone()),
            BPlusTreeNode::Internal { .. } => {
                Err(BTreeError::Internal("Not a leaf node".to_string()))
            }
        }
    }

    /// Get the next leaf node in the linked list
    pub fn get_next_leaf(&self, leaf_id: BTreeNodeId) -> BTreeResult<Option<BTreeNodeId>> {
        let leaf = self.get_node(leaf_id)?;

        match leaf {
            BPlusTreeNode::Leaf(leaf) => Ok(leaf.next),
            BPlusTreeNode::Internal { .. } => {
                Err(BTreeError::Internal("Not a leaf node".to_string()))
            }
        }
    }

    /// Get the previous leaf node in the linked list
    pub fn get_prev_leaf(&self, leaf_id: BTreeNodeId) -> BTreeResult<Option<BTreeNodeId>> {
        let leaf = self.get_node(leaf_id)?;

        match leaf {
            BPlusTreeNode::Leaf(leaf) => Ok(leaf.prev),
            BPlusTreeNode::Internal { .. } => {
                Err(BTreeError::Internal("Not a leaf node".to_string()))
            }
        }
    }

    /// Get statistics about the tree
    pub fn stats(&self) -> BTreeStats {
        let nodes = self.nodes.read().unwrap();
        let root_id = *self.root_id.read().unwrap();

        let mut num_internal = 0;
        let mut num_leaves = 0;
        let mut total_keys = 0;
        let mut height = 0;

        for node in nodes.values() {
            match node {
                BPlusTreeNode::Internal { .. } => num_internal += 1,
                BPlusTreeNode::Leaf(leaf) => {
                    num_leaves += 1;
                    total_keys += leaf.entries.len();
                }
            }
        }

        // Calculate height by traversing from root to a leaf
        if let Ok(mut node) = self.get_node(root_id) {
            height = 1;
            while node.is_internal() {
                if let Some(child_id) = node.get_child_for_key(&[]) {
                    if let Ok(child) = self.get_node(child_id) {
                        node = child;
                        height += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        BTreeStats {
            num_keys: total_keys,
            num_internal_nodes: num_internal,
            num_leaf_nodes: num_leaves,
            height,
        }
    }
}

/// Statistics about the B+Tree
#[derive(Debug, Clone)]
pub struct BTreeStats {
    pub num_keys: usize,
    pub num_internal_nodes: usize,
    pub num_leaf_nodes: usize,
    pub height: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let tree = BPlusTree::new(BPlusTreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        tree.insert(b"key2".to_vec(), b"value2".to_vec()).unwrap();
        tree.insert(b"key3".to_vec(), b"value3".to_vec()).unwrap();

        assert_eq!(tree.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(tree.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(tree.get(b"key3").unwrap(), Some(b"value3".to_vec()));
        assert_eq!(tree.get(b"key4").unwrap(), None);
    }

    #[test]
    fn test_update() {
        let tree = BPlusTree::new(BPlusTreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        assert_eq!(tree.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        tree.insert(b"key1".to_vec(), b"value2".to_vec()).unwrap();
        assert_eq!(tree.get(b"key1").unwrap(), Some(b"value2".to_vec()));
    }

    #[test]
    fn test_delete() {
        let tree = BPlusTree::new(BPlusTreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        tree.insert(b"key2".to_vec(), b"value2".to_vec()).unwrap();

        assert!(tree.delete(b"key1").unwrap());
        assert_eq!(tree.get(b"key1").unwrap(), None);
        assert_eq!(tree.get(b"key2").unwrap(), Some(b"value2".to_vec()));

        assert!(!tree.delete(b"key3").unwrap());
    }

    #[test]
    fn test_many_inserts() {
        let config = BPlusTreeConfig {
            max_keys: 4,
            min_keys: 2,
        };
        let tree = BPlusTree::new(config);

        // Insert many keys to trigger splits
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Verify all keys
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            assert_eq!(tree.get(key.as_bytes()).unwrap(), Some(value.into_bytes()));
        }

        let stats = tree.stats();
        println!("Tree stats: {:?}", stats);
        assert!(stats.height > 1); // Should have split
        assert_eq!(stats.num_keys, 100);
    }
}
