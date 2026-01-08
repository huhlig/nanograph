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

//! Multi-Version Concurrency Control (MVCC) for ART
//!
//! This module provides MVCC support for the Adaptive Radix Tree, allowing
//! multiple versions of values to coexist for snapshot isolation.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// Timestamp for MVCC versioning
pub type Timestamp = u64;

/// A versioned value with creation and deletion timestamps
#[derive(Debug, Clone)]
pub struct VersionedValue {
    /// The actual value
    pub value: Vec<u8>,
    /// Timestamp when this version was created
    pub created_at: Timestamp,
    /// Timestamp when this version was deleted (None if still active)
    pub deleted_at: Option<Timestamp>,
}

impl VersionedValue {
    /// Create a new versioned value
    pub fn new(value: Vec<u8>, created_at: Timestamp) -> Self {
        Self {
            value,
            created_at,
            deleted_at: None,
        }
    }

    /// Mark this version as deleted
    pub fn mark_deleted(&mut self, deleted_at: Timestamp) {
        self.deleted_at = Some(deleted_at);
    }

    /// Check if this version is visible at the given timestamp
    pub fn is_visible_at(&self, timestamp: Timestamp) -> bool {
        // Version is visible if:
        // 1. It was created before or at the snapshot timestamp
        // 2. It wasn't deleted, or was deleted after the snapshot timestamp
        self.created_at <= timestamp && self.deleted_at.map_or(true, |deleted| deleted > timestamp)
    }
}

/// Version chain for a single key
#[derive(Debug, Clone)]
pub struct VersionChain {
    /// All versions of this key, ordered by timestamp
    versions: BTreeMap<Timestamp, VersionedValue>,
}

impl VersionChain {
    /// Create a new empty version chain
    pub fn new() -> Self {
        Self {
            versions: BTreeMap::new(),
        }
    }

    /// Add a new version
    pub fn add_version(&mut self, timestamp: Timestamp, value: Vec<u8>) {
        self.versions
            .insert(timestamp, VersionedValue::new(value, timestamp));
    }

    /// Mark the latest version as deleted
    pub fn mark_deleted(&mut self, timestamp: Timestamp) {
        if let Some((_, version)) = self.versions.iter_mut().last() {
            version.mark_deleted(timestamp);
        }
    }

    /// Get the value visible at the given timestamp
    pub fn get_at(&self, timestamp: Timestamp) -> Option<Vec<u8>> {
        // Find the latest version that is visible at the given timestamp
        self.versions
            .values()
            .rev()
            .find(|v| v.is_visible_at(timestamp))
            .map(|v| v.value.clone())
    }

    /// Check if any version exists at the given timestamp
    pub fn exists_at(&self, timestamp: Timestamp) -> bool {
        self.versions.values().any(|v| v.is_visible_at(timestamp))
    }

    /// Garbage collect old versions that are no longer visible
    /// Keeps versions created after min_timestamp
    pub fn gc(&mut self, min_timestamp: Timestamp) {
        self.versions.retain(|&ts, _| ts >= min_timestamp);
    }

    /// Get the number of versions
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }
}

impl Default for VersionChain {
    fn default() -> Self {
        Self::new()
    }
}

/// MVCC timestamp manager
pub struct TimestampManager {
    /// Current timestamp counter
    current: AtomicU64,
    /// Active snapshots (timestamp -> reference count)
    active_snapshots: Arc<RwLock<BTreeMap<Timestamp, usize>>>,
}

impl TimestampManager {
    /// Create a new timestamp manager
    pub fn new() -> Self {
        Self {
            current: AtomicU64::new(1),
            active_snapshots: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Get the next timestamp
    pub fn next(&self) -> Timestamp {
        self.current.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current timestamp without incrementing
    pub fn current(&self) -> Timestamp {
        self.current.load(Ordering::SeqCst)
    }

    /// Register a new snapshot
    pub fn register_snapshot(&self, timestamp: Timestamp) {
        let mut snapshots = self.active_snapshots.write().unwrap();
        *snapshots.entry(timestamp).or_insert(0) += 1;
    }

    /// Unregister a snapshot
    pub fn unregister_snapshot(&self, timestamp: Timestamp) {
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
    pub fn min_active_snapshot(&self) -> Option<Timestamp> {
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
        let mut value = VersionedValue::new(b"test".to_vec(), 10);

        // Visible at creation time and after
        assert!(value.is_visible_at(10));
        assert!(value.is_visible_at(15));

        // Not visible before creation
        assert!(!value.is_visible_at(5));

        // Mark as deleted
        value.mark_deleted(20);

        // Still visible before deletion
        assert!(value.is_visible_at(15));

        // Not visible at or after deletion
        assert!(!value.is_visible_at(20));
        assert!(!value.is_visible_at(25));
    }

    #[test]
    fn test_version_chain() {
        let mut chain = VersionChain::new();

        // Add versions
        chain.add_version(10, b"v1".to_vec());
        chain.add_version(20, b"v2".to_vec());
        chain.add_version(30, b"v3".to_vec());

        // Get values at different timestamps
        assert_eq!(chain.get_at(5), None);
        assert_eq!(chain.get_at(10), Some(b"v1".to_vec()));
        assert_eq!(chain.get_at(15), Some(b"v1".to_vec()));
        assert_eq!(chain.get_at(20), Some(b"v2".to_vec()));
        assert_eq!(chain.get_at(25), Some(b"v2".to_vec()));
        assert_eq!(chain.get_at(30), Some(b"v3".to_vec()));
        assert_eq!(chain.get_at(35), Some(b"v3".to_vec()));

        // Mark latest as deleted
        chain.mark_deleted(40);
        assert_eq!(chain.get_at(35), Some(b"v3".to_vec()));
        assert_eq!(chain.get_at(40), Some(b"v2".to_vec()));
        assert_eq!(chain.get_at(45), Some(b"v2".to_vec()));
    }

    #[test]
    fn test_version_chain_gc() {
        let mut chain = VersionChain::new();

        chain.add_version(10, b"v1".to_vec());
        chain.add_version(20, b"v2".to_vec());
        chain.add_version(30, b"v3".to_vec());

        assert_eq!(chain.version_count(), 3);

        // GC versions older than 25 (keeps versions with timestamp >= 25)
        chain.gc(25);
        assert_eq!(chain.version_count(), 1); // Only v3 (30) remains
        assert_eq!(chain.get_at(30), Some(b"v3".to_vec()));
        assert_eq!(chain.get_at(25), None); // v2 was GC'd
        assert_eq!(chain.get_at(15), None); // v1 was GC'd
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
}

// Made with Bob
