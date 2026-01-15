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

//! MVCC-enabled B+Tree implementation
//!
//! This module provides a B+Tree with Multi-Version Concurrency Control (MVCC)
//! for snapshot isolation. It wraps MvccLeafNode to provide versioned storage.

use crate::error::BTreeResult;
use crate::mvcc_node::MvccLeafNode;
use crate::node::BTreeNodeId;
use std::sync::{Arc, RwLock};

/// Configuration for MVCC B+Tree
#[derive(Debug, Clone)]
pub struct MvccTreeConfig {
    /// Maximum number of keys per node (order)
    pub max_keys: usize,
    /// Minimum number of keys per node (typically max_keys / 2)
    pub min_keys: usize,
    /// Maximum versions to keep per key
    pub max_versions_per_key: usize,
}

impl Default for MvccTreeConfig {
    fn default() -> Self {
        Self {
            max_keys: 128,
            min_keys: 64,
            max_versions_per_key: 10,
        }
    }
}

/// MVCC-enabled B+Tree
///
/// This is a simplified implementation that demonstrates MVCC concepts.
/// For production use, this would need to be integrated with the full
/// B+Tree structure including internal nodes and tree balancing.
pub struct MvccBPlusTree {
    /// Configuration
    config: MvccTreeConfig,

    /// Root node (simplified: single leaf for demonstration)
    root: Arc<RwLock<MvccLeafNode>>,

    /// Next available node ID
    /// TODO: Use for node allocation when implementing full B+Tree structure
    _next_node_id: RwLock<u64>,

    /// Minimum active snapshot timestamp (for GC)
    min_snapshot_ts: RwLock<u64>,
}

impl MvccBPlusTree {
    /// Create a new MVCC B+Tree
    pub fn new(config: MvccTreeConfig) -> Self {
        let root_id = BTreeNodeId::new(0);
        let root = MvccLeafNode::new(root_id);

        Self {
            config,
            root: Arc::new(RwLock::new(root)),
            _next_node_id: RwLock::new(1),
            min_snapshot_ts: RwLock::new(0),
        }
    }

    /// Insert a key-value pair with versioning
    ///
    /// # Arguments
    /// * `key` - The key to insert
    /// * `value` - The value to store
    /// * `created_ts` - Transaction ID creating this version
    pub fn insert(&self, key: Vec<u8>, value: Vec<u8>, created_ts: u64) -> BTreeResult<()> {
        let mut root = self.root.write().unwrap();
        root.insert(key, value, created_ts);
        Ok(())
    }

    /// Get a value at a specific snapshot timestamp
    ///
    /// # Arguments
    /// * `key` - The key to look up
    /// * `snapshot_ts` - The snapshot timestamp for visibility
    pub fn get(&self, key: &[u8], snapshot_ts: u64) -> BTreeResult<Option<Vec<u8>>> {
        let root = self.root.read().unwrap();
        Ok(root.get(key, snapshot_ts))
    }

    /// Get the latest committed value
    pub fn get_latest(&self, key: &[u8]) -> BTreeResult<Option<Vec<u8>>> {
        let root = self.root.read().unwrap();
        Ok(root.get_latest(key))
    }

    /// Delete a key by adding a deletion marker
    ///
    /// # Arguments
    /// * `key` - The key to delete
    /// * `created_ts` - Transaction ID creating this deletion
    pub fn delete(&self, key: &[u8], created_ts: u64) -> BTreeResult<bool> {
        let mut root = self.root.write().unwrap();
        Ok(root.remove(key, created_ts).is_some())
    }

    /// Commit all versions created by a transaction
    ///
    /// # Arguments
    /// * `created_ts` - Transaction ID that created the versions
    /// * `commit_ts` - Commit timestamp to assign
    pub fn commit_versions(&self, created_ts: u64, commit_ts: u64) -> BTreeResult<()> {
        let mut root = self.root.write().unwrap();
        root.commit_versions(created_ts, commit_ts);
        Ok(())
    }

    /// Rollback (remove) all versions created by a transaction
    ///
    /// # Arguments
    /// * `created_ts` - Transaction ID that created the versions
    pub fn rollback_versions(&self, created_ts: u64) -> BTreeResult<()> {
        let mut root = self.root.write().unwrap();
        root.rollback_versions(created_ts);
        Ok(())
    }

    /// Check if a key has been modified since a snapshot timestamp
    ///
    /// Used for write conflict detection.
    ///
    /// # Arguments
    /// * `key` - The key to check
    /// * `snapshot_ts` - The snapshot timestamp to compare against
    pub fn has_conflict(&self, key: &[u8], snapshot_ts: u64) -> bool {
        let root = self.root.read().unwrap();
        root.has_conflict(key, snapshot_ts)
    }

    /// Atomic commit operation: check conflicts, apply writes, and commit versions
    ///
    /// This method holds the write lock for the entire operation to prevent deadlocks
    /// that can occur when multiple transactions interleave their lock acquisitions.
    ///
    /// # Arguments
    /// * `writes` - The write operations to apply
    /// * `snapshot_ts` - The snapshot timestamp for conflict detection
    /// * `created_ts` - Transaction ID that created the versions
    /// * `commit_ts` - Commit timestamp to assign
    pub fn atomic_commit(
        &self,
        writes: &[(Vec<u8>, Option<Vec<u8>>)], // (key, Some(value) for put, None for delete)
        snapshot_ts: u64,
        created_ts: u64,
        commit_ts: u64,
    ) -> BTreeResult<()> {
        // Hold write lock for entire commit operation to prevent deadlocks
        let mut root = self.root.write().unwrap();

        // Check for conflicts
        for (key, _) in writes {
            if root.has_conflict(key, snapshot_ts) {
                // Conflict detected - rollback and return error
                root.rollback_versions(created_ts);
                return Err(crate::error::BTreeError::Internal(format!(
                    "Write conflict detected for key: {:?}",
                    key
                )));
            }
        }

        // Apply all writes
        for (key, value_opt) in writes {
            match value_opt {
                Some(value) => {
                    root.insert(key.clone(), value.clone(), created_ts);
                }
                None => {
                    root.remove(key, created_ts);
                }
            }
        }

        // Commit all versions
        root.commit_versions(created_ts, commit_ts);

        Ok(())
    }

    /// Update the minimum active snapshot timestamp
    ///
    /// This is used by the transaction manager to track the oldest
    /// active transaction for garbage collection purposes.
    pub fn update_min_snapshot_ts(&self, min_ts: u64) {
        let mut min_snapshot = self.min_snapshot_ts.write().unwrap();
        *min_snapshot = min_ts;
    }

    /// Run garbage collection on old versions
    ///
    /// Removes versions that are no longer visible to any active transaction.
    pub fn gc_versions(&self) -> BTreeResult<()> {
        // Get min_snapshot and release lock before acquiring root lock
        let min_snapshot = {
            let min_snap = self.min_snapshot_ts.read().unwrap();
            *min_snap
        };

        let mut root = self.root.write().unwrap();
        root.gc_versions(min_snapshot, self.config.max_versions_per_key);
        Ok(())
    }

    /// Get statistics about the tree
    pub fn stats(&self) -> MvccTreeStats {
        let root = self.root.read().unwrap();
        MvccTreeStats {
            key_count: root.len(),
            node_count: 1, // Simplified: single node
        }
    }
}

/// Statistics about the MVCC tree
#[derive(Debug, Clone)]
pub struct MvccTreeStats {
    /// Number of keys in the tree
    pub key_count: usize,
    /// Number of nodes in the tree
    pub node_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvcc_tree_basic_operations() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Insert and commit
        tree.insert(b"key1".to_vec(), b"value1".to_vec(), 1)
            .unwrap();
        tree.commit_versions(1, 2).unwrap();

        // Read at different timestamps
        assert_eq!(tree.get(b"key1", 1).unwrap(), None); // Before commit
        assert_eq!(tree.get(b"key1", 2).unwrap(), Some(b"value1".to_vec())); // At commit
        assert_eq!(tree.get(b"key1", 10).unwrap(), Some(b"value1".to_vec())); // After commit
    }

    #[test]
    fn test_mvcc_tree_snapshot_isolation() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Transaction 1: Insert key1=v1 at ts=1, commit at ts=2
        tree.insert(b"key1".to_vec(), b"v1".to_vec(), 1).unwrap();
        tree.commit_versions(1, 2).unwrap();

        // Transaction 2 starts at ts=3 (sees v1)
        let snapshot_ts = 3;
        assert_eq!(
            tree.get(b"key1", snapshot_ts).unwrap(),
            Some(b"v1".to_vec())
        );

        // Transaction 3: Update key1=v2 at ts=4, commit at ts=5
        tree.insert(b"key1".to_vec(), b"v2".to_vec(), 4).unwrap();
        tree.commit_versions(4, 5).unwrap();

        // Transaction 2 still sees v1 (snapshot isolation)
        assert_eq!(
            tree.get(b"key1", snapshot_ts).unwrap(),
            Some(b"v1".to_vec())
        );

        // New transaction at ts=6 sees v2
        assert_eq!(tree.get(b"key1", 6).unwrap(), Some(b"v2".to_vec()));
    }

    #[test]
    fn test_mvcc_tree_rollback() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Insert and commit first version
        tree.insert(b"key1".to_vec(), b"v1".to_vec(), 1).unwrap();
        tree.commit_versions(1, 2).unwrap();

        // Insert second version but rollback
        tree.insert(b"key1".to_vec(), b"v2".to_vec(), 3).unwrap();
        tree.rollback_versions(3).unwrap();

        // Should only see first version
        assert_eq!(tree.get(b"key1", 10).unwrap(), Some(b"v1".to_vec()));
    }

    #[test]
    fn test_mvcc_tree_conflict_detection() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Insert and commit at ts=2
        tree.insert(b"key1".to_vec(), b"v1".to_vec(), 1).unwrap();
        tree.commit_versions(1, 2).unwrap();

        // Transaction starts at ts=2
        let snapshot_ts = 2;

        // No conflict yet
        assert!(!tree.has_conflict(b"key1", snapshot_ts));

        // Another transaction updates and commits at ts=5
        tree.insert(b"key1".to_vec(), b"v2".to_vec(), 4).unwrap();
        tree.commit_versions(4, 5).unwrap();

        // Now there's a conflict
        assert!(tree.has_conflict(b"key1", snapshot_ts));
    }

    #[test]
    fn test_mvcc_tree_gc() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Create multiple versions
        for i in 1..=5 {
            tree.insert(b"key1".to_vec(), format!("v{}", i).into_bytes(), i)
                .unwrap();
            tree.commit_versions(i, i + 1).unwrap();
        }

        // Set min snapshot to 4 (versions before this can be GC'd)
        tree.update_min_snapshot_ts(4);

        // Run GC
        tree.gc_versions().unwrap();

        // Should still be able to read latest version
        assert_eq!(tree.get(b"key1", 10).unwrap(), Some(b"v5".to_vec()));
    }

    #[test]
    fn test_mvcc_tree_delete() {
        let tree = MvccBPlusTree::new(MvccTreeConfig::default());

        // Insert and commit
        tree.insert(b"key1".to_vec(), b"value1".to_vec(), 1)
            .unwrap();
        tree.commit_versions(1, 2).unwrap();

        // Transaction at ts=3 sees the value
        assert_eq!(tree.get(b"key1", 3).unwrap(), Some(b"value1".to_vec()));

        // Delete and commit at ts=5
        tree.delete(b"key1", 4).unwrap();
        tree.commit_versions(4, 5).unwrap();

        // Old transaction still sees value
        assert_eq!(tree.get(b"key1", 3).unwrap(), Some(b"value1".to_vec()));

        // New transaction sees deletion
        assert_eq!(tree.get(b"key1", 6).unwrap(), None);
    }
}
