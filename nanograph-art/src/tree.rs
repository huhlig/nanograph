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

use crate::error::{Error, Result};
use crate::node::{Node, Node4, Node16, Node48, Node256};
use std::sync::Arc;

/// Adaptive Radix Tree implementation
#[derive(Debug, Clone)]
pub struct AdaptiveRadixTree<V> {
    root: Option<Arc<Node<V>>>,
    size: usize,
}

impl<V: Clone> AdaptiveRadixTree<V> {
    /// Create a new empty ART
    pub fn new() -> Self {
        Self {
            root: None,
            size: 0,
        }
    }

    /// Get the number of entries in the tree
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Get the root node (for iteration)
    pub fn root(&self) -> Option<Arc<Node<V>>> {
        self.root.clone()
    }

    /// Insert a key-value pair into the tree
    pub fn insert(&mut self, key: Vec<u8>, value: V) -> Result<Option<V>> {
        if key.is_empty() {
            return Err(Error::InvalidKey("Key cannot be empty".to_string()));
        }

        let (new_root, old_value) = if let Some(root) = self.root.take() {
            self.insert_recursive(root, &key, 0, value)?
        } else {
            // Empty tree - create a leaf
            (Arc::new(Node::new_leaf(key, value)), None)
        };

        self.root = Some(new_root);
        if old_value.is_none() {
            self.size += 1;
        }
        Ok(old_value)
    }

    /// Search for a value by key
    pub fn get(&self, key: &[u8]) -> Option<V> {
        if key.is_empty() {
            return None;
        }

        let mut node = self.root.as_ref()?;
        let mut depth = 0;

        loop {
            // Check partial key match
            let partial = node.partial();
            if depth + partial.len() <= key.len() {
                if &key[depth..depth + partial.len()] != partial {
                    return None;
                }
                depth += partial.len();
            } else {
                return None;
            }

            // Check if we're at a leaf
            if node.is_leaf() {
                if let Node::Leaf(leaf) = node.as_ref() {
                    if leaf.matches(key) {
                        return Some(leaf.value.clone());
                    }
                }
                return None;
            }

            // Check if we've consumed the entire key - return value from inner node if present
            if depth >= key.len() {
                return match node.as_ref() {
                    Node::Node4(n) => n.header.value.clone(),
                    Node::Node16(n) => n.header.value.clone(),
                    Node::Node48(n) => n.header.value.clone(),
                    Node::Node256(n) => n.header.value.clone(),
                    Node::Leaf(_) => unreachable!(),
                };
            }

            // Find the next child
            let key_byte = key[depth];
            node = node.find_child(key_byte)?;
            depth += 1;
        }
    }

    /// Check if a key exists in the tree
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Remove a key from the tree
    pub fn remove(&mut self, key: &[u8]) -> Result<Option<V>> {
        if key.is_empty() {
            return Err(Error::InvalidKey("Key cannot be empty".to_string()));
        }

        let root = match self.root.take() {
            Some(r) => r,
            None => return Ok(None),
        };

        match self.remove_recursive(root, key, 0)? {
            (Some(new_root), value) => {
                self.root = Some(new_root);
                if value.is_some() {
                    self.size -= 1;
                }
                Ok(value)
            }
            (None, value) => {
                if value.is_some() {
                    self.size -= 1;
                }
                Ok(value)
            }
        }
    }

    /// Recursive insert helper
    fn insert_recursive(
        &self,
        node: Arc<Node<V>>,
        key: &[u8],
        depth: usize,
        value: V,
    ) -> Result<(Arc<Node<V>>, Option<V>)> {
        // Check partial key
        let partial = node.partial();
        let mismatch_idx = self.check_prefix(partial, key, depth);

        if mismatch_idx < partial.len() {
            // Partial key mismatch - need to split the node
            return Ok(self.split_node(node, key, depth, mismatch_idx, value));
        }

        let new_depth = depth + partial.len();

        // If this is a leaf, check if keys match
        if node.is_leaf() {
            if let Node::Leaf(leaf) = node.as_ref() {
                if leaf.matches(key) {
                    // Replace existing value
                    let old_value = leaf.value.clone();
                    return Ok((
                        Arc::new(Node::new_leaf(key.to_vec(), value)),
                        Some(old_value),
                    ));
                } else {
                    // Keys differ - need to create a new inner node
                    return Ok(self.create_inner_node_from_leaves(node, key, new_depth, value));
                }
            }
        }

        // Check if we've consumed the entire key - store value in inner node
        if new_depth >= key.len() {
            // Store value at this inner node
            let old_value = match node.as_ref() {
                Node::Node4(n) => n.header.value.clone(),
                Node::Node16(n) => n.header.value.clone(),
                Node::Node48(n) => n.header.value.clone(),
                Node::Node256(n) => n.header.value.clone(),
                Node::Leaf(_) => unreachable!(),
            };
            return Ok((self.set_node_value(node, value), old_value));
        }

        let key_byte = key[new_depth];

        // Try to find existing child
        if let Some(child) = node.find_child(key_byte) {
            let (new_child, old_value) =
                self.insert_recursive(child.clone(), key, new_depth + 1, value)?;
            return Ok((self.replace_child(node, key_byte, new_child), old_value));
        }

        // Add new child
        let new_leaf = Arc::new(Node::new_leaf(key.to_vec(), value));
        Ok((self.add_child(node, key_byte, new_leaf)?, None))
    }

    /// Check how many bytes of the partial key match
    fn check_prefix(&self, partial: &[u8], key: &[u8], depth: usize) -> usize {
        let max_cmp = std::cmp::min(partial.len(), key.len() - depth);
        for i in 0..max_cmp {
            if partial[i] != key[depth + i] {
                return i;
            }
        }
        max_cmp
    }

    /// Split a node when partial key mismatches
    fn split_node(
        &self,
        node: Arc<Node<V>>,
        key: &[u8],
        depth: usize,
        mismatch_idx: usize,
        value: V,
    ) -> (Arc<Node<V>>, Option<V>) {
        let partial = node.partial();

        // Create new parent node with common prefix
        let common_prefix = partial[..mismatch_idx].to_vec();
        let mut new_parent = Node::new_node4(common_prefix);

        // Update old node's partial to remaining part
        let old_key_byte = partial[mismatch_idx];
        let remaining_partial = partial[mismatch_idx + 1..].to_vec();
        let updated_old = self.update_partial(node, remaining_partial);

        // Create new leaf
        let new_key_byte = key[depth + mismatch_idx];
        let new_leaf = Arc::new(Node::new_leaf(key.to_vec(), value));

        // Add both children to new parent
        new_parent = self.add_child_unchecked(new_parent, old_key_byte, updated_old);
        new_parent = self.add_child_unchecked(new_parent, new_key_byte, new_leaf);

        (Arc::new(new_parent), None)
    }

    /// Create inner node from two leaves
    fn create_inner_node_from_leaves(
        &self,
        existing: Arc<Node<V>>,
        new_key: &[u8],
        depth: usize,
        new_value: V,
    ) -> (Arc<Node<V>>, Option<V>) {
        if let Node::Leaf(existing_leaf) = existing.as_ref() {
            let existing_key = &existing_leaf.key;

            // Find longest common prefix
            let mut common_len = 0;
            while depth + common_len < existing_key.len()
                && depth + common_len < new_key.len()
                && existing_key[depth + common_len] == new_key[depth + common_len]
            {
                common_len += 1;
            }

            let common_prefix = existing_key[depth..depth + common_len].to_vec();

            // Check if one key is a prefix of the other
            if depth + common_len >= existing_key.len() {
                // existing_key is a prefix of new_key
                // Create node with existing key's value, add new key as child
                let new_node = match Node::new_node4(common_prefix) {
                    Node::Node4(mut n) => {
                        n.header.value = Some(existing_leaf.value.clone());
                        Node::Node4(n)
                    }
                    _ => unreachable!(),
                };

                let new_byte = new_key[depth + common_len];
                let new_leaf = Arc::new(Node::new_leaf(new_key.to_vec(), new_value));
                let new_node = self.add_child_unchecked(new_node, new_byte, new_leaf);
                return (Arc::new(new_node), None);
            } else if depth + common_len >= new_key.len() {
                // new_key is a prefix of existing_key
                // Create node with new key's value, add existing key as child
                let new_node = match Node::new_node4(common_prefix) {
                    Node::Node4(mut n) => {
                        n.header.value = Some(new_value);
                        Node::Node4(n)
                    }
                    _ => unreachable!(),
                };

                let existing_byte = existing_key[depth + common_len];
                let new_node = self.add_child_unchecked(new_node, existing_byte, existing);
                return (Arc::new(new_node), None);
            }

            let mut new_node = Node::new_node4(common_prefix);
            let existing_byte = existing_key[depth + common_len];
            let new_byte = new_key[depth + common_len];

            let new_leaf = Arc::new(Node::new_leaf(new_key.to_vec(), new_value));

            new_node = self.add_child_unchecked(new_node, existing_byte, existing);
            new_node = self.add_child_unchecked(new_node, new_byte, new_leaf);

            return (Arc::new(new_node), None);
        }

        unreachable!("Expected leaf node");
    }

    /// Set the value of an inner node
    fn set_node_value(&self, node: Arc<Node<V>>, value: V) -> Arc<Node<V>> {
        match node.as_ref() {
            Node::Leaf(_) => node,
            Node::Node4(n) => {
                let mut new_node = (**n).clone();
                new_node.header.value = Some(value);
                Arc::new(Node::Node4(Box::new(new_node)))
            }
            Node::Node16(n) => {
                let mut new_node = (**n).clone();
                new_node.header.value = Some(value);
                Arc::new(Node::Node16(Box::new(new_node)))
            }
            Node::Node48(n) => {
                let mut new_node = (**n).clone();
                new_node.header.value = Some(value);
                Arc::new(Node::Node48(Box::new(new_node)))
            }
            Node::Node256(n) => {
                let mut new_node = (**n).clone();
                new_node.header.value = Some(value);
                Arc::new(Node::Node256(Box::new(new_node)))
            }
        }
    }

    /// Set the value of a node (unchecked, for node construction)
    fn set_node_value_unchecked(&self, node: Node<V>, value: V) -> Node<V> {
        match node {
            Node::Node4(mut n) => {
                n.header.value = Some(value);
                Node::Node4(n)
            }
            Node::Node16(mut n) => {
                n.header.value = Some(value);
                Node::Node16(n)
            }
            Node::Node48(mut n) => {
                n.header.value = Some(value);
                Node::Node48(n)
            }
            Node::Node256(mut n) => {
                n.header.value = Some(value);
                Node::Node256(n)
            }
            leaf => leaf,
        }
    }

    /// Update the partial key of a node
    fn update_partial(&self, node: Arc<Node<V>>, new_partial: Vec<u8>) -> Arc<Node<V>> {
        match node.as_ref() {
            Node::Leaf(_) => node,
            Node::Node4(n) => {
                let mut new_node = (**n).clone();
                new_node.header.partial = new_partial;
                Arc::new(Node::Node4(Box::new(new_node)))
            }
            Node::Node16(n) => {
                let mut new_node = (**n).clone();
                new_node.header.partial = new_partial;
                Arc::new(Node::Node16(Box::new(new_node)))
            }
            Node::Node48(n) => {
                let mut new_node = (**n).clone();
                new_node.header.partial = new_partial;
                Arc::new(Node::Node48(Box::new(new_node)))
            }
            Node::Node256(n) => {
                let mut new_node = (**n).clone();
                new_node.header.partial = new_partial;
                Arc::new(Node::Node256(Box::new(new_node)))
            }
        }
    }

    /// Add a child to a node (with growth if needed)
    fn add_child(
        &self,
        node: Arc<Node<V>>,
        key_byte: u8,
        child: Arc<Node<V>>,
    ) -> Result<Arc<Node<V>>> {
        match node.as_ref() {
            Node::Leaf(_) => Err(Error::Internal("Cannot add child to leaf".to_string())),
            Node::Node4(n) if n.header.num_children < 4 => {
                Ok(Arc::new(self.add_child_to_node4(n, key_byte, child)))
            }
            Node::Node4(n) => {
                // Grow to Node16
                let node16 = self.grow_node4_to_node16(n);
                Ok(Arc::new(self.add_child_to_node16(&node16, key_byte, child)))
            }
            Node::Node16(n) if n.header.num_children < 16 => {
                Ok(Arc::new(self.add_child_to_node16(n, key_byte, child)))
            }
            Node::Node16(n) => {
                // Grow to Node48
                let node48 = self.grow_node16_to_node48(n);
                Ok(Arc::new(self.add_child_to_node48(&node48, key_byte, child)))
            }
            Node::Node48(n) if n.header.num_children < 48 => {
                Ok(Arc::new(self.add_child_to_node48(n, key_byte, child)))
            }
            Node::Node48(n) => {
                // Grow to Node256
                let node256 = self.grow_node48_to_node256(n);
                Ok(Arc::new(
                    self.add_child_to_node256(&node256, key_byte, child),
                ))
            }
            Node::Node256(n) if n.header.num_children < 256 => {
                Ok(Arc::new(self.add_child_to_node256(n, key_byte, child)))
            }
            Node::Node256(_) => Err(Error::NodeCapacityExceeded),
        }
    }

    /// Add child without checking capacity (used during node creation)
    fn add_child_unchecked(&self, node: Node<V>, key_byte: u8, child: Arc<Node<V>>) -> Node<V> {
        match node {
            Node::Node4(mut n) => {
                let idx = n.header.num_children as usize;
                n.keys[idx] = key_byte;
                n.children[idx] = Some(child);
                n.header.num_children += 1;
                Node::Node4(n)
            }
            _ => node,
        }
    }

    /// Add child to Node4
    fn add_child_to_node4(&self, node: &Node4<V>, key_byte: u8, child: Arc<Node<V>>) -> Node<V> {
        let mut new_node = node.clone();
        let idx = new_node.header.num_children as usize;

        // Insert in sorted order
        let mut insert_pos = idx;
        for i in 0..idx {
            if new_node.keys[i] > key_byte {
                insert_pos = i;
                break;
            }
        }

        // Shift elements if needed
        if insert_pos < idx {
            for i in (insert_pos..idx).rev() {
                new_node.keys[i + 1] = new_node.keys[i];
                new_node.children[i + 1] = new_node.children[i].clone();
            }
        }

        new_node.keys[insert_pos] = key_byte;
        new_node.children[insert_pos] = Some(child);
        new_node.header.num_children += 1;

        Node::Node4(Box::new(new_node))
    }

    /// Add child to Node16
    fn add_child_to_node16(&self, node: &Node16<V>, key_byte: u8, child: Arc<Node<V>>) -> Node<V> {
        let mut new_node = node.clone();
        let idx = new_node.header.num_children as usize;

        // Insert in sorted order
        let mut insert_pos = idx;
        for i in 0..idx {
            if new_node.keys[i] > key_byte {
                insert_pos = i;
                break;
            }
        }

        // Shift elements if needed
        if insert_pos < idx {
            for i in (insert_pos..idx).rev() {
                new_node.keys[i + 1] = new_node.keys[i];
                new_node.children[i + 1] = new_node.children[i].clone();
            }
        }

        new_node.keys[insert_pos] = key_byte;
        new_node.children[insert_pos] = Some(child);
        new_node.header.num_children += 1;

        Node::Node16(Box::new(new_node))
    }

    /// Add child to Node48
    fn add_child_to_node48(&self, node: &Node48<V>, key_byte: u8, child: Arc<Node<V>>) -> Node<V> {
        let mut new_node = node.clone();
        let idx = new_node.header.num_children as usize;

        new_node.child_index[key_byte as usize] = idx as u8;
        new_node.children[idx] = Some(child);
        new_node.header.num_children += 1;

        Node::Node48(Box::new(new_node))
    }

    /// Add child to Node256
    fn add_child_to_node256(
        &self,
        node: &Node256<V>,
        key_byte: u8,
        child: Arc<Node<V>>,
    ) -> Node<V> {
        let mut new_node = node.clone();
        new_node.children[key_byte as usize] = Some(child);
        new_node.header.num_children += 1;

        Node::Node256(Box::new(new_node))
    }

    /// Grow Node4 to Node16
    fn grow_node4_to_node16(&self, node: &Node4<V>) -> Node16<V> {
        let mut node16 = Node16 {
            header: node.header.clone(),
            keys: [0; 16],
            children: Default::default(),
        };

        for i in 0..node.header.num_children as usize {
            node16.keys[i] = node.keys[i];
            node16.children[i] = node.children[i].clone();
        }

        node16
    }

    /// Grow Node16 to Node48
    fn grow_node16_to_node48(&self, node: &Node16<V>) -> Node48<V> {
        let mut node48 = Node48 {
            header: node.header.clone(),
            child_index: [255; 256],
            children: std::array::from_fn(|_| None),
        };

        for i in 0..node.header.num_children as usize {
            let key_byte = node.keys[i];
            node48.child_index[key_byte as usize] = i as u8;
            node48.children[i] = node.children[i].clone();
        }

        node48
    }

    /// Grow Node48 to Node256
    fn grow_node48_to_node256(&self, node: &Node48<V>) -> Node256<V> {
        let mut node256 = Node256 {
            header: node.header.clone(),
            children: std::array::from_fn(|_| None),
        };

        for key_byte in 0..256 {
            let idx = node.child_index[key_byte];
            if idx != 255 {
                node256.children[key_byte] = node.children[idx as usize].clone();
            }
        }

        node256
    }

    /// Replace a child in a node
    fn replace_child(
        &self,
        node: Arc<Node<V>>,
        key_byte: u8,
        new_child: Arc<Node<V>>,
    ) -> Arc<Node<V>> {
        match node.as_ref() {
            Node::Leaf(_) => node,
            Node::Node4(n) => {
                let mut new_node = (**n).clone();
                for i in 0..new_node.header.num_children as usize {
                    if new_node.keys[i] == key_byte {
                        new_node.children[i] = Some(new_child);
                        break;
                    }
                }
                Arc::new(Node::Node4(Box::new(new_node)))
            }
            Node::Node16(n) => {
                let mut new_node = (**n).clone();
                for i in 0..new_node.header.num_children as usize {
                    if new_node.keys[i] == key_byte {
                        new_node.children[i] = Some(new_child);
                        break;
                    }
                }
                Arc::new(Node::Node16(Box::new(new_node)))
            }
            Node::Node48(n) => {
                let mut new_node = (**n).clone();
                let idx = new_node.child_index[key_byte as usize];
                if idx != 255 {
                    new_node.children[idx as usize] = Some(new_child);
                }
                Arc::new(Node::Node48(Box::new(new_node)))
            }
            Node::Node256(n) => {
                let mut new_node = (**n).clone();
                new_node.children[key_byte as usize] = Some(new_child);
                Arc::new(Node::Node256(Box::new(new_node)))
            }
        }
    }

    /// Recursive remove helper
    fn remove_recursive(
        &self,
        node: Arc<Node<V>>,
        key: &[u8],
        depth: usize,
    ) -> Result<(Option<Arc<Node<V>>>, Option<V>)> {
        // Check partial key
        let partial = node.partial();
        let mismatch_idx = self.check_prefix(partial, key, depth);

        if mismatch_idx < partial.len() {
            // Partial key mismatch - key not found
            return Ok((Some(node), None));
        }

        let new_depth = depth + partial.len();

        // If this is a leaf, check if keys match
        if node.is_leaf() {
            if let Node::Leaf(leaf) = node.as_ref() {
                if leaf.matches(key) {
                    return Ok((None, Some(leaf.value.clone())));
                }
            }
            return Ok((Some(node), None));
        }

        // Check if we've consumed the entire key - remove value from inner node if present
        if new_depth >= key.len() {
            let old_value = match node.as_ref() {
                Node::Node4(n) => n.header.value.clone(),
                Node::Node16(n) => n.header.value.clone(),
                Node::Node48(n) => n.header.value.clone(),
                Node::Node256(n) => n.header.value.clone(),
                Node::Leaf(_) => unreachable!(),
            };
            
            if old_value.is_some() {
                // Remove the value from this inner node
                let new_node = match node.as_ref() {
                    Node::Node4(n) => {
                        let mut new_n = (**n).clone();
                        new_n.header.value = None;
                        Arc::new(Node::Node4(Box::new(new_n)))
                    }
                    Node::Node16(n) => {
                        let mut new_n = (**n).clone();
                        new_n.header.value = None;
                        Arc::new(Node::Node16(Box::new(new_n)))
                    }
                    Node::Node48(n) => {
                        let mut new_n = (**n).clone();
                        new_n.header.value = None;
                        Arc::new(Node::Node48(Box::new(new_n)))
                    }
                    Node::Node256(n) => {
                        let mut new_n = (**n).clone();
                        new_n.header.value = None;
                        Arc::new(Node::Node256(Box::new(new_n)))
                    }
                    Node::Leaf(_) => unreachable!(),
                };
                return Ok((Some(new_node), old_value));
            }
            
            return Ok((Some(node), None));
        }

        let key_byte = key[new_depth];

        // Find child
        if let Some(child) = node.find_child(key_byte) {
            let (new_child, value) = self.remove_recursive(child.clone(), key, new_depth + 1)?;

            if let Some(new_child) = new_child {
                // Child still exists, replace it
                return Ok((Some(self.replace_child(node, key_byte, new_child)), value));
            } else {
                // Child was removed, remove from this node
                let (new_node, _) = self.remove_child(node, key_byte)?;
                return Ok((new_node, value));
            }
        }

        Ok((Some(node), None))
    }

    /// Remove a child from a node (with shrinking if needed)
    fn remove_child(
        &self,
        node: Arc<Node<V>>,
        key_byte: u8,
    ) -> Result<(Option<Arc<Node<V>>>, Option<V>)> {
        match node.as_ref() {
            Node::Leaf(_) => Err(Error::Internal("Cannot remove child from leaf".to_string())),
            Node::Node4(n) => {
                let new_node = self.remove_child_from_node4(n, key_byte);
                if new_node.header.num_children == 1 {
                    // Compress path if only one child remains
                    Ok((
                        Some(self.compress_node(Arc::new(Node::Node4(Box::new(new_node))))),
                        None,
                    ))
                } else if new_node.header.num_children == 0 {
                    // If node has a value, keep it as an inner node; otherwise remove it
                    if new_node.header.value.is_some() {
                        Ok((Some(Arc::new(Node::Node4(Box::new(new_node)))), None))
                    } else {
                        Ok((None, None))
                    }
                } else {
                    Ok((Some(Arc::new(Node::Node4(Box::new(new_node)))), None))
                }
            }
            Node::Node16(n) => {
                let new_node = self.remove_child_from_node16(n, key_byte);
                if new_node.header.num_children <= 4 {
                    // Shrink to Node4
                    let node4 = self.shrink_node16_to_node4(&new_node);
                    Ok((Some(Arc::new(Node::Node4(Box::new(node4)))), None))
                } else {
                    Ok((Some(Arc::new(Node::Node16(Box::new(new_node)))), None))
                }
            }
            Node::Node48(n) => {
                let new_node = self.remove_child_from_node48(n, key_byte);
                if new_node.header.num_children <= 16 {
                    // Shrink to Node16
                    let node16 = self.shrink_node48_to_node16(&new_node);
                    Ok((Some(Arc::new(Node::Node16(Box::new(node16)))), None))
                } else {
                    Ok((Some(Arc::new(Node::Node48(Box::new(new_node)))), None))
                }
            }
            Node::Node256(n) => {
                let new_node = self.remove_child_from_node256(n, key_byte);
                if new_node.header.num_children <= 48 {
                    // Shrink to Node48
                    let node48 = self.shrink_node256_to_node48(&new_node);
                    Ok((Some(Arc::new(Node::Node48(Box::new(node48)))), None))
                } else {
                    Ok((Some(Arc::new(Node::Node256(Box::new(new_node)))), None))
                }
            }
        }
    }

    /// Remove child from Node4
    fn remove_child_from_node4(&self, node: &Node4<V>, key_byte: u8) -> Node4<V> {
        let mut new_node = node.clone();

        for i in 0..new_node.header.num_children as usize {
            if new_node.keys[i] == key_byte {
                // Shift remaining elements
                for j in i..new_node.header.num_children as usize - 1 {
                    new_node.keys[j] = new_node.keys[j + 1];
                    new_node.children[j] = new_node.children[j + 1].clone();
                }
                new_node.children[new_node.header.num_children as usize - 1] = None;
                new_node.header.num_children -= 1;
                break;
            }
        }

        new_node
    }

    /// Remove child from Node16
    fn remove_child_from_node16(&self, node: &Node16<V>, key_byte: u8) -> Node16<V> {
        let mut new_node = node.clone();

        for i in 0..new_node.header.num_children as usize {
            if new_node.keys[i] == key_byte {
                // Shift remaining elements
                for j in i..new_node.header.num_children as usize - 1 {
                    new_node.keys[j] = new_node.keys[j + 1];
                    new_node.children[j] = new_node.children[j + 1].clone();
                }
                new_node.children[new_node.header.num_children as usize - 1] = None;
                new_node.header.num_children -= 1;
                break;
            }
        }

        new_node
    }

    /// Remove child from Node48
    fn remove_child_from_node48(&self, node: &Node48<V>, key_byte: u8) -> Node48<V> {
        let mut new_node = node.clone();
        let idx = new_node.child_index[key_byte as usize];

        if idx != 255 {
            new_node.children[idx as usize] = None;
            new_node.child_index[key_byte as usize] = 255;
            new_node.header.num_children -= 1;
        }

        new_node
    }

    /// Remove child from Node256
    fn remove_child_from_node256(&self, node: &Node256<V>, key_byte: u8) -> Node256<V> {
        let mut new_node = node.clone();

        if new_node.children[key_byte as usize].is_some() {
            new_node.children[key_byte as usize] = None;
            new_node.header.num_children -= 1;
        }

        new_node
    }

    /// Shrink Node16 to Node4
    fn shrink_node16_to_node4(&self, node: &Node16<V>) -> Node4<V> {
        let mut node4 = Node4 {
            header: node.header.clone(),
            keys: [0; 4],
            children: Default::default(),
        };

        let mut j = 0;
        for i in 0..node.header.num_children as usize {
            if node.children[i].is_some() {
                node4.keys[j] = node.keys[i];
                node4.children[j] = node.children[i].clone();
                j += 1;
            }
        }

        node4
    }

    /// Shrink Node48 to Node16
    fn shrink_node48_to_node16(&self, node: &Node48<V>) -> Node16<V> {
        let mut node16 = Node16 {
            header: node.header.clone(),
            keys: [0; 16],
            children: Default::default(),
        };

        let mut j = 0;
        for key_byte in 0..256 {
            let idx = node.child_index[key_byte];
            if idx != 255 {
                node16.keys[j] = key_byte as u8;
                node16.children[j] = node.children[idx as usize].clone();
                j += 1;
            }
        }

        node16
    }

    /// Shrink Node256 to Node48
    fn shrink_node256_to_node48(&self, node: &Node256<V>) -> Node48<V> {
        let mut node48 = Node48 {
            header: node.header.clone(),
            child_index: [255; 256],
            children: std::array::from_fn(|_| None),
        };

        let mut j = 0;
        for key_byte in 0..256 {
            if node.children[key_byte].is_some() {
                node48.child_index[key_byte] = j;
                node48.children[j as usize] = node.children[key_byte].clone();
                j += 1;
            }
        }

        node48
    }

    /// Compress path when node has only one child
    fn compress_node(&self, node: Arc<Node<V>>) -> Arc<Node<V>> {
        if let Node::Node4(n) = node.as_ref() {
            if n.header.num_children == 1 && n.header.value.is_none() {
                // Only compress if the node doesn't have a value stored at it
                if let Some(child) = &n.children[0] {
                    let key_byte = n.keys[0];
                    let mut new_partial = n.header.partial.clone();
                    new_partial.push(key_byte);
                    new_partial.extend_from_slice(child.partial());

                    return self.update_partial(child.clone(), new_partial);
                }
            }
        }
        node
    }

    /// Get total memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_usage_recursive(self.root.as_ref())
    }

    fn memory_usage_recursive(&self, node: Option<&Arc<Node<V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let mut total = n.memory_usage();

                match n.as_ref() {
                    Node::Leaf(_) => total,
                    Node::Node4(inner) => {
                        for child in &inner.children {
                            total += self.memory_usage_recursive(child.as_ref());
                        }
                        total
                    }
                    Node::Node16(inner) => {
                        for child in &inner.children {
                            total += self.memory_usage_recursive(child.as_ref());
                        }
                        total
                    }
                    Node::Node48(inner) => {
                        for child in &inner.children {
                            total += self.memory_usage_recursive(child.as_ref());
                        }
                        total
                    }
                    Node::Node256(inner) => {
                        for child in &inner.children {
                            total += self.memory_usage_recursive(child.as_ref());
                        }
                        total
                    }
                }
            }
        }
    }

    /// Create an iterator over all entries in the tree
    pub fn iter(&self) -> crate::iterator::ArtIterator<V> {
        crate::iterator::ArtIterator::new(self.root.clone())
    }

    /// Create a range iterator
    pub fn range(
        &self,
        start: Option<Vec<u8>>,
        end: Option<Vec<u8>>,
        inclusive: bool,
    ) -> crate::iterator::ArtRangeIterator<V> {
        crate::iterator::ArtRangeIterator::new(self.root.clone(), start, end, inclusive)
    }

    /// Get all keys in the tree
    pub fn keys(&self) -> impl Iterator<Item = Vec<u8>> + '_ {
        self.iter().map(|(k, _)| k)
    }

    /// Get all values in the tree
    pub fn values(&self) -> impl Iterator<Item = V> + '_ {
        self.iter().map(|(_, v)| v)
    }
}

impl<V: Clone> Default for AdaptiveRadixTree<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Made with Bob
