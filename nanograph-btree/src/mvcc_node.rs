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

//! MVCC-enabled B+Tree node structures
//!
//! This module provides versioned leaf nodes that support snapshot isolation.

use crate::mvcc::VersionChain;
use crate::node::NodeId;
use serde::{Deserialize, Serialize};

/// MVCC-enabled leaf node in the B+Tree
///
/// Unlike the standard LeafNode, this version maintains version chains
/// for each key, enabling snapshot isolation and concurrent transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MvccLeafNode {
    /// Node ID
    pub id: NodeId,
    
    /// Key-value entries with version chains (sorted by key)
    pub entries: Vec<(Vec<u8>, VersionChain)>,
    
    /// Link to the next leaf node (for range scans)
    pub next: Option<NodeId>,
    
    /// Link to the previous leaf node (for reverse scans)
    pub prev: Option<NodeId>,
    
    /// Parent node ID
    pub parent: Option<NodeId>,
}

impl MvccLeafNode {
    /// Create a new empty MVCC leaf node
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            entries: Vec::new(),
            next: None,
            prev: None,
            parent: None,
        }
    }

    /// Find the index of a key in this leaf
    pub fn find_key_index(&self, key: &[u8]) -> Result<usize, usize> {
        self.entries.binary_search_by(|(k, _)| k.as_slice().cmp(key))
    }

    /// Insert or update a key-value pair with versioning
    ///
    /// # Arguments
    /// * `key` - The key to insert/update
    /// * `value` - The value to store
    /// * `created_ts` - Transaction ID creating this version
    ///
    /// # Returns
    /// The previous latest committed value, if any
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>, created_ts: u64) -> Option<Vec<u8>> {
        match self.find_key_index(&key) {
            Ok(idx) => {
                // Key exists, add new version to the chain
                let chain = &mut self.entries[idx].1;
                let old_value = chain.get_latest();
                chain.add_version(Some(value), created_ts);
                old_value
            }
            Err(idx) => {
                // Key doesn't exist, create new entry with first version
                let chain = VersionChain::with_version(value, created_ts);
                self.entries.insert(idx, (key, chain));
                None
            }
        }
    }

    /// Remove a key-value pair by adding a deletion marker
    ///
    /// # Arguments
    /// * `key` - The key to delete
    /// * `created_ts` - Transaction ID creating this deletion
    ///
    /// # Returns
    /// The previous latest committed value, if any
    pub fn remove(&mut self, key: &[u8], created_ts: u64) -> Option<Vec<u8>> {
        match self.find_key_index(key) {
            Ok(idx) => {
                // Add deletion marker to version chain
                let chain = &mut self.entries[idx].1;
                let old_value = chain.get_latest();
                chain.add_version(None, created_ts);
                old_value
            }
            Err(_) => None,
        }
    }

    /// Get a value by key at a specific snapshot timestamp
    ///
    /// # Arguments
    /// * `key` - The key to look up
    /// * `snapshot_ts` - The snapshot timestamp for visibility
    ///
    /// # Returns
    /// The value visible at the snapshot timestamp, or None if not found/deleted
    pub fn get(&self, key: &[u8], snapshot_ts: u64) -> Option<Vec<u8>> {
        match self.find_key_index(key) {
            Ok(idx) => self.entries[idx].1.get(snapshot_ts),
            Err(_) => None,
        }
    }

    /// Get the latest committed value for a key
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// The latest committed value, or None if not found/deleted
    pub fn get_latest(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.find_key_index(key) {
            Ok(idx) => self.entries[idx].1.get_latest(),
            Err(_) => None,
        }
    }

    /// Commit all versions created by a specific transaction
    ///
    /// # Arguments
    /// * `created_ts` - Transaction ID that created the versions
    /// * `commit_ts` - Commit timestamp to assign
    pub fn commit_versions(&mut self, created_ts: u64, commit_ts: u64) {
        for (_, chain) in &mut self.entries {
            chain.commit_versions(created_ts, commit_ts);
        }
    }

    /// Rollback (remove) all versions created by a specific transaction
    ///
    /// # Arguments
    /// * `created_ts` - Transaction ID that created the versions
    pub fn rollback_versions(&mut self, created_ts: u64) {
        for (_, chain) in &mut self.entries {
            chain.rollback_versions(created_ts);
        }
        
        // Remove entries with no versions left
        self.entries.retain(|(_, chain)| !chain.is_empty());
    }

    /// Garbage collect old versions that are no longer visible
    ///
    /// # Arguments
    /// * `min_snapshot_ts` - Minimum snapshot timestamp of active transactions
    /// * `max_versions_per_key` - Maximum number of versions to keep per key
    pub fn gc_versions(&mut self, min_snapshot_ts: u64, max_versions_per_key: usize) {
        for (_, chain) in &mut self.entries {
            chain.gc_versions(min_snapshot_ts, max_versions_per_key);
        }
    }

    /// Check if a key has been modified since a given timestamp
    ///
    /// Used for write conflict detection.
    ///
    /// # Arguments
    /// * `key` - The key to check
    /// * `snapshot_ts` - The snapshot timestamp to compare against
    ///
    /// # Returns
    /// true if the key has been modified after snapshot_ts or has uncommitted versions
    pub fn has_conflict(&self, key: &[u8], snapshot_ts: u64) -> bool {
        match self.find_key_index(key) {
            Ok(idx) => {
                // Check if there are any uncommitted versions or versions committed after snapshot
                self.entries[idx].1.has_conflict(snapshot_ts)
            }
            Err(_) => false,
        }
    }

    /// Split this leaf node into two nodes
    ///
    /// Returns (middle_key, new_right_node)
    pub fn split(&mut self) -> (Vec<u8>, MvccLeafNode) {
        let mid = self.entries.len() / 2;
        
        // Split entries
        let right_entries = self.entries.split_off(mid);
        
        // The first key of the right node becomes the separator
        let middle_key = right_entries[0].0.clone();
        
        // Create the new right node
        let right_node = MvccLeafNode {
            id: self.id, // Will be replaced by caller
            entries: right_entries,
            next: self.next,
            prev: None,  // Will be set by caller
            parent: self.parent,
        };
        
        (middle_key, right_node)
    }

    /// Check if this leaf is underfull (needs merging or redistribution)
    pub fn is_underfull(&self, min_entries: usize) -> bool {
        self.entries.len() < min_entries
    }

    /// Get the number of entries in this leaf
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if this leaf is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvcc_leaf_insert_and_get() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Insert a value
        assert_eq!(leaf.insert(b"key1".to_vec(), b"value1".to_vec(), 1), None);
        
        // Commit it
        leaf.commit_versions(1, 2);
        
        // Read at different timestamps
        assert_eq!(leaf.get(b"key1", 1), None); // Before commit
        assert_eq!(leaf.get(b"key1", 2), Some(b"value1".to_vec())); // At commit
        assert_eq!(leaf.get(b"key1", 10), Some(b"value1".to_vec())); // After commit
    }

    #[test]
    fn test_mvcc_leaf_update() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Insert and commit first version
        leaf.insert(b"key1".to_vec(), b"v1".to_vec(), 1);
        leaf.commit_versions(1, 2);
        
        // Insert and commit second version
        assert_eq!(
            leaf.insert(b"key1".to_vec(), b"v2".to_vec(), 3),
            Some(b"v1".to_vec())
        );
        leaf.commit_versions(3, 4);
        
        // Old transaction sees old version
        assert_eq!(leaf.get(b"key1", 3), Some(b"v1".to_vec()));
        
        // New transaction sees new version
        assert_eq!(leaf.get(b"key1", 5), Some(b"v2".to_vec()));
    }

    #[test]
    fn test_mvcc_leaf_delete() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Insert and commit
        leaf.insert(b"key1".to_vec(), b"value1".to_vec(), 1);
        leaf.commit_versions(1, 2);
        
        // Delete and commit
        assert_eq!(leaf.remove(b"key1", 3), Some(b"value1".to_vec()));
        leaf.commit_versions(3, 4);
        
        // Old transaction sees value
        assert_eq!(leaf.get(b"key1", 3), Some(b"value1".to_vec()));
        
        // New transaction sees deletion
        assert_eq!(leaf.get(b"key1", 5), None);
    }

    #[test]
    fn test_mvcc_leaf_rollback() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Insert and commit first version
        leaf.insert(b"key1".to_vec(), b"v1".to_vec(), 1);
        leaf.commit_versions(1, 2);
        
        // Insert second version but don't commit
        leaf.insert(b"key1".to_vec(), b"v2".to_vec(), 3);
        
        // Rollback the uncommitted version
        leaf.rollback_versions(3);
        
        // Should only see first version
        assert_eq!(leaf.get(b"key1", 10), Some(b"v1".to_vec()));
    }

    #[test]
    fn test_mvcc_leaf_conflict_detection() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Insert and commit at ts=2
        leaf.insert(b"key1".to_vec(), b"value1".to_vec(), 1);
        leaf.commit_versions(1, 2);
        
        // No conflict for transaction that started at ts=2 or later
        assert!(!leaf.has_conflict(b"key1", 2));
        assert!(!leaf.has_conflict(b"key1", 3));
        
        // Update and commit at ts=5
        leaf.insert(b"key1".to_vec(), b"value2".to_vec(), 4);
        leaf.commit_versions(4, 5);
        
        // Conflict for transaction that started before ts=5
        assert!(leaf.has_conflict(b"key1", 4));
        assert!(leaf.has_conflict(b"key1", 3));
        
        // No conflict for transaction that started at ts=5 or later
        assert!(!leaf.has_conflict(b"key1", 5));
        assert!(!leaf.has_conflict(b"key1", 6));
    }

    #[test]
    fn test_mvcc_leaf_gc() {
        let mut leaf = MvccLeafNode::new(NodeId::new(1));
        
        // Create multiple versions
        for i in 1..=5 {
            leaf.insert(b"key1".to_vec(), format!("v{}", i).into_bytes(), i);
            leaf.commit_versions(i, i + 1);
        }
        
        // GC with min_snapshot_ts = 4, max_versions = 3
        leaf.gc_versions(4, 3);
        
        // Should still be able to read recent versions
        assert_eq!(leaf.get(b"key1", 10), Some(b"v5".to_vec()));
    }
}

// Made with Bob
