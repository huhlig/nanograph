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

//! Multi-Version Concurrency Control (MVCC) support for B+Tree
//!
//! This module provides snapshot isolation through versioned values.
//! Each key can have multiple versions, allowing concurrent transactions
//! to read consistent snapshots while writes are in progress.

use serde::{Deserialize, Serialize};

/// A versioned value for MVCC support
///
/// Each value is associated with timestamps that determine its visibility
/// to transactions based on their snapshot timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedValue {
    /// The actual value (None represents a deletion marker)
    pub value: Option<Vec<u8>>,

    /// Transaction ID that created this version
    pub created_ts: u64,

    /// Timestamp when this version was committed (0 = uncommitted)
    pub commit_ts: u64,
}

impl VersionedValue {
    /// Create a new versioned value
    ///
    /// # Arguments
    /// * `value` - The value to store (None for deletion marker)
    /// * `created_ts` - Transaction ID that created this version
    pub fn new(value: Option<Vec<u8>>, created_ts: u64) -> Self {
        Self {
            value,
            created_ts,
            commit_ts: 0,
        }
    }

    /// Mark this version as committed
    ///
    /// # Arguments
    /// * `commit_ts` - The commit timestamp to assign
    pub fn commit(&mut self, commit_ts: u64) {
        self.commit_ts = commit_ts;
    }

    /// Check if this version is visible to a transaction
    ///
    /// A version is visible if:
    /// 1. It has been committed (commit_ts > 0)
    /// 2. It was committed before or at the snapshot timestamp
    ///
    /// # Arguments
    /// * `snapshot_ts` - The snapshot timestamp of the reading transaction
    pub fn is_visible(&self, snapshot_ts: u64) -> bool {
        self.commit_ts > 0 && self.commit_ts <= snapshot_ts
    }

    /// Check if this version is uncommitted
    pub fn is_uncommitted(&self) -> bool {
        self.commit_ts == 0
    }

    /// Check if this is a deletion marker
    pub fn is_deletion(&self) -> bool {
        self.value.is_none()
    }
}

/// Version chain for a single key
///
/// Maintains multiple versions of a value, ordered newest first.
/// This allows concurrent transactions to see different versions
/// based on their snapshot timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChain {
    /// List of versions, newest first
    versions: Vec<VersionedValue>,
}

impl VersionChain {
    /// Create a new empty version chain
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// Create a version chain with an initial version
    pub fn with_version(value: Vec<u8>, created_ts: u64) -> Self {
        Self {
            versions: vec![VersionedValue::new(Some(value), created_ts)],
        }
    }

    /// Add a new version to the chain
    ///
    /// New versions are added at the front (newest first).
    pub fn add_version(&mut self, value: Option<Vec<u8>>, created_ts: u64) {
        self.versions
            .insert(0, VersionedValue::new(value, created_ts));
    }

    /// Get the value visible at a specific snapshot timestamp
    ///
    /// Returns the first version that is visible to the transaction.
    /// If the visible version is a deletion marker, returns None.
    pub fn get(&self, snapshot_ts: u64) -> Option<Vec<u8>> {
        for version in &self.versions {
            if version.is_visible(snapshot_ts) {
                return version.value.clone();
            }
        }
        None
    }

    /// Get the value visible at a specific timestamp
    ///
    /// This is an alternative to `get()` that uses the same timestamp
    /// for both snapshot and visibility checks. Useful for point-in-time queries.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to query at
    pub fn get_at(&self, timestamp: u64) -> Option<Vec<u8>> {
        self.get(timestamp)
    }

    /// Check if a value exists at a specific timestamp
    ///
    /// Returns true if there's a visible, non-deleted version at the timestamp.
    pub fn exists_at(&self, timestamp: u64) -> bool {
        for version in &self.versions {
            if version.is_visible(timestamp) {
                return !version.is_deletion();
            }
        }
        false
    }

    /// Get the latest committed version
    pub fn get_latest(&self) -> Option<Vec<u8>> {
        for version in &self.versions {
            if version.commit_ts > 0 {
                return version.value.clone();
            }
        }
        None
    }

    /// Commit all versions created by a specific transaction
    pub fn commit_versions(&mut self, created_ts: u64, commit_ts: u64) {
        for version in &mut self.versions {
            if version.created_ts == created_ts && version.is_uncommitted() {
                version.commit(commit_ts);
            }
        }
    }

    /// Rollback (remove) all versions created by a specific transaction
    pub fn rollback_versions(&mut self, created_ts: u64) {
        self.versions.retain(|v| v.created_ts != created_ts);
    }

    /// Garbage collect old versions
    ///
    /// Removes versions that are no longer visible to any active transaction.
    /// Always keeps at least one version.
    ///
    /// # Arguments
    /// * `min_snapshot_ts` - Minimum snapshot timestamp of active transactions
    /// * `max_versions` - Maximum number of versions to keep per key
    pub fn gc_versions(&mut self, min_snapshot_ts: u64, max_versions: usize) {
        let mut keep_count = 0;
        self.versions.retain(|v| {
            // Always keep at least one version
            if keep_count == 0 {
                keep_count += 1;
                return true;
            }

            // Keep versions that might be visible to active transactions
            if v.commit_ts > min_snapshot_ts && keep_count < max_versions {
                keep_count += 1;
                return true;
            }

            false
        });
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    /// Get the number of versions in the chain
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// Get the latest committed version's commit timestamp
    ///
    /// Used for write conflict detection.
    pub fn latest_commit_ts(&self) -> Option<u64> {
        for version in &self.versions {
            if version.commit_ts > 0 {
                return Some(version.commit_ts);
            }
        }
        None
    }

    /// Check if there's a write conflict for a transaction with given snapshot
    ///
    /// A conflict exists if:
    /// 1. There are uncommitted versions (from other transactions)
    /// 2. There are versions committed after the snapshot timestamp
    ///
    /// # Arguments
    /// * `snapshot_ts` - The snapshot timestamp to check against
    pub fn has_conflict(&self, snapshot_ts: u64) -> bool {
        for version in &self.versions {
            // Check for uncommitted versions (potential conflict with other transactions)
            if version.is_uncommitted() {
                return true;
            }
            // Check for versions committed after snapshot
            if version.commit_ts > snapshot_ts {
                return true;
            }
        }
        false
    }
}

impl Default for VersionChain {
    fn default() -> Self {
        Self::new()
    }
}

/// MVCC timestamp manager
///
/// Manages timestamp generation and tracks active snapshots for garbage collection.
pub struct TimestampManager {
    /// Current timestamp counter
    current: std::sync::atomic::AtomicU64,
    /// Active snapshots (timestamp -> reference count)
    active_snapshots: std::sync::Arc<std::sync::RwLock<std::collections::BTreeMap<u64, usize>>>,
}

impl TimestampManager {
    /// Create a new timestamp manager
    pub fn new() -> Self {
        Self {
            current: std::sync::atomic::AtomicU64::new(1),
            active_snapshots: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::BTreeMap::new(),
            )),
        }
    }

    /// Get the next timestamp
    pub fn next(&self) -> u64 {
        self.current
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the current timestamp without incrementing
    pub fn current(&self) -> u64 {
        self.current.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Register a new snapshot
    pub fn register_snapshot(&self, timestamp: u64) {
        let mut snapshots = self.active_snapshots.write().unwrap();
        *snapshots.entry(timestamp).or_insert(0) += 1;
    }

    /// Unregister a snapshot
    pub fn unregister_snapshot(&self, timestamp: u64) {
        let mut snapshots = self.active_snapshots.write().unwrap();
        if let Some(count) = snapshots.get_mut(&timestamp) {
            *count -= 1;
            if *count == 0 {
                snapshots.remove(&timestamp);
            }
        }
    }

    /// Get the minimum active snapshot timestamp
    /// This is used for garbage collection - versions older than this can be removed
    pub fn min_active_snapshot(&self) -> Option<u64> {
        let snapshots = self.active_snapshots.read().unwrap();
        snapshots.keys().next().copied()
    }

    /// Get the number of active snapshots
    pub fn active_snapshot_count(&self) -> usize {
        let snapshots = self.active_snapshots.read().unwrap();
        snapshots.len()
    }
}

impl Default for TimestampManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_versioned_value_visibility() {
        let mut v = VersionedValue::new(Some(b"value".to_vec()), 1);

        // Uncommitted version is not visible
        assert!(!v.is_visible(10));

        // Commit the version
        v.commit(5);

        // Visible to transactions with snapshot_ts >= commit_ts
        assert!(v.is_visible(5));
        assert!(v.is_visible(10));

        // Not visible to transactions with snapshot_ts < commit_ts
        assert!(!v.is_visible(4));
    }

    #[test]
    fn test_version_chain_basic() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Can read committed version
        assert_eq!(chain.get(5), Some(b"v1".to_vec()));
        assert_eq!(chain.get(2), Some(b"v1".to_vec()));

        // Cannot read before commit
        assert_eq!(chain.get(1), None);
    }

    #[test]
    fn test_version_chain_updates() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Add new version
        chain.add_version(Some(b"v2".to_vec()), 3);
        chain.versions[0].commit(4);

        // Old transaction sees old version
        assert_eq!(chain.get(3), Some(b"v1".to_vec()));

        // New transaction sees new version
        assert_eq!(chain.get(5), Some(b"v2".to_vec()));
    }

    #[test]
    fn test_version_chain_deletion() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Add deletion marker
        chain.add_version(None, 3);
        chain.versions[0].commit(4);

        // Old transaction sees value
        assert_eq!(chain.get(3), Some(b"v1".to_vec()));

        // New transaction sees deletion
        assert_eq!(chain.get(5), None);
    }

    #[test]
    fn test_version_chain_commit_rollback() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Add uncommitted version
        chain.add_version(Some(b"v2".to_vec()), 3);

        // Rollback the uncommitted version
        chain.rollback_versions(3);

        // Only original version remains
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.get(5), Some(b"v1".to_vec()));
    }

    #[test]
    fn test_version_chain_gc() {
        let mut chain = VersionChain::new();

        // Add multiple versions
        for i in 1..=5 {
            chain.add_version(Some(format!("v{}", i).into_bytes()), i);
            chain.versions[0].commit(i + 1);
        }

        assert_eq!(chain.len(), 5);

        // GC with min_snapshot_ts = 4, max_versions = 3
        chain.gc_versions(4, 3);

        // Should keep at most 3 versions, all with commit_ts > 4
        assert!(chain.len() <= 3);
        assert!(chain.len() >= 1); // Always keep at least one
    }

    #[test]
    fn test_timestamp_manager() {
        let mgr = TimestampManager::new();

        let ts1 = mgr.next();
        let ts2 = mgr.next();
        let ts3 = mgr.next();

        assert!(ts2 > ts1);
        assert!(ts3 > ts2);

        // Register snapshots
        mgr.register_snapshot(ts1);
        mgr.register_snapshot(ts2);
        mgr.register_snapshot(ts2); // Register ts2 twice

        assert_eq!(mgr.active_snapshot_count(), 2);
        assert_eq!(mgr.min_active_snapshot(), Some(ts1));

        // Unregister snapshots
        mgr.unregister_snapshot(ts1);
        assert_eq!(mgr.min_active_snapshot(), Some(ts2));

        mgr.unregister_snapshot(ts2);
        assert_eq!(mgr.active_snapshot_count(), 1); // Still one reference to ts2

        mgr.unregister_snapshot(ts2);
        assert_eq!(mgr.active_snapshot_count(), 0);
        assert_eq!(mgr.min_active_snapshot(), None);
    }

    #[test]
    fn test_timestamp_manager_current() {
        let mgr = TimestampManager::new();

        let current1 = mgr.current();
        let ts1 = mgr.next();
        let current2 = mgr.current();

        assert_eq!(ts1, current1);
        assert!(current2 > current1);
    }

    #[test]
    fn test_version_chain_get_at() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Test get_at (should work same as get)
        assert_eq!(chain.get_at(5), Some(b"v1".to_vec()));
        assert_eq!(chain.get_at(2), Some(b"v1".to_vec()));
        assert_eq!(chain.get_at(1), None);

        // Add new version
        chain.add_version(Some(b"v2".to_vec()), 3);
        chain.versions[0].commit(4);

        assert_eq!(chain.get_at(3), Some(b"v1".to_vec()));
        assert_eq!(chain.get_at(5), Some(b"v2".to_vec()));
    }

    #[test]
    fn test_version_chain_exists_at() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // Value exists at timestamp 5
        assert!(chain.exists_at(5));
        assert!(chain.exists_at(2));

        // Value doesn't exist before commit
        assert!(!chain.exists_at(1));

        // Add deletion marker
        chain.add_version(None, 3);
        chain.versions[0].commit(4);

        // Value still exists before deletion
        assert!(chain.exists_at(3));

        // Value doesn't exist after deletion
        assert!(!chain.exists_at(5));
    }

    #[test]
    fn test_write_conflict_detection() {
        let mut chain = VersionChain::with_version(b"v1".to_vec(), 1);
        chain.versions[0].commit(2);

        // No conflict at snapshot 5
        assert!(!chain.has_conflict(5));

        // Add uncommitted version
        chain.add_version(Some(b"v2".to_vec()), 3);

        // Now there's a conflict due to uncommitted version
        assert!(chain.has_conflict(5));

        // Commit the version
        chain.versions[0].commit(6);

        // No conflict at snapshot 7 (after commit)
        assert!(!chain.has_conflict(7));

        // But conflict at snapshot 5 (version committed after snapshot)
        assert!(chain.has_conflict(5));
    }
}
