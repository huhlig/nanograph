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

use crate::bloblog::BlobRef;
use std::collections::BTreeMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};

/// Value storage location - either inline or in blob log
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueLocation {
    /// Value stored inline in the entry
    Inline(Vec<u8>),
    /// Value stored in blob log (reference only)
    Blob(BlobRef),
}

impl ValueLocation {
    /// Get the size of the value (for size tracking)
    pub fn size(&self) -> usize {
        match self {
            ValueLocation::Inline(data) => data.len(),
            ValueLocation::Blob(_) => BlobRef::serialized_size(),
        }
    }

    /// Check if this is a blob reference
    pub fn is_blob(&self) -> bool {
        matches!(self, ValueLocation::Blob(_))
    }

    /// Get blob reference if this is a blob
    pub fn as_blob(&self) -> Option<&BlobRef> {
        match self {
            ValueLocation::Blob(blob_ref) => Some(blob_ref),
            _ => None,
        }
    }

    /// Get inline value if this is inline
    pub fn as_inline(&self) -> Option<&[u8]> {
        match self {
            ValueLocation::Inline(data) => Some(data),
            _ => None,
        }
    }

    /// Get the inline value as Vec<u8>, cloning if necessary
    /// Returns None if this is a blob reference
    pub fn to_vec(&self) -> Option<Vec<u8>> {
        match self {
            ValueLocation::Inline(data) => Some(data.clone()),
            ValueLocation::Blob(_) => None,
        }
    }

    /// Get the length of the inline value
    /// For blob references, returns the serialized size of the reference
    pub fn len(&self) -> usize {
        self.size()
    }

    /// Check if empty (only for inline values)
    pub fn is_empty(&self) -> bool {
        match self {
            ValueLocation::Inline(data) => data.is_empty(),
            ValueLocation::Blob(_) => false,
        }
    }
}

/// Entry in the memtable with MVCC support and blob reference support
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: Vec<u8>,
    pub value: Option<ValueLocation>, // None represents a deletion
    pub sequence: u64,
    pub commit_ts: Option<i64>, // None = uncommitted, Some = committed at this timestamp
}

impl Entry {
    /// Create a new entry with inline value
    pub fn new(key: Vec<u8>, value: Option<Vec<u8>>, sequence: u64) -> Self {
        Self {
            key,
            value: value.map(ValueLocation::Inline),
            sequence,
            commit_ts: None, // Initially uncommitted
        }
    }

    /// Create a new entry with blob reference
    pub fn new_with_blob(key: Vec<u8>, blob_ref: BlobRef, sequence: u64) -> Self {
        Self {
            key,
            value: Some(ValueLocation::Blob(blob_ref)),
            sequence,
            commit_ts: None,
        }
    }

    /// Create an entry with inline value and commit timestamp
    pub fn with_commit_ts(
        key: Vec<u8>,
        value: Option<Vec<u8>>,
        sequence: u64,
        commit_ts: i64,
    ) -> Self {
        Self {
            key,
            value: value.map(ValueLocation::Inline),
            sequence,
            commit_ts: Some(commit_ts),
        }
    }

    /// Create an entry with blob reference and commit timestamp
    pub fn with_blob_and_commit_ts(
        key: Vec<u8>,
        blob_ref: BlobRef,
        sequence: u64,
        commit_ts: i64,
    ) -> Self {
        Self {
            key,
            value: Some(ValueLocation::Blob(blob_ref)),
            sequence,
            commit_ts: Some(commit_ts),
        }
    }

    /// Check if this entry is visible to a transaction with the given snapshot timestamp
    pub fn is_visible(&self, snapshot_ts: i64) -> bool {
        match self.commit_ts {
            None => false,                 // Uncommitted entries are not visible
            Some(ts) => ts <= snapshot_ts, // Only see entries committed before or at snapshot
        }
    }

    /// Get the size of this entry for memory tracking
    pub fn size(&self) -> usize {
        self.key.len() + self.value.as_ref().map_or(0, |v| v.size()) + 16 // +16 for sequence and commit_ts
    }

    /// Check if this entry has a blob reference
    pub fn has_blob(&self) -> bool {
        self.value.as_ref().map_or(false, |v| v.is_blob())
    }

    /// Get the blob reference if this entry has one
    pub fn blob_ref(&self) -> Option<&BlobRef> {
        self.value.as_ref().and_then(|v| v.as_blob())
    }

    /// Get the inline value if this entry has one
    pub fn inline_value(&self) -> Option<&[u8]> {
        self.value.as_ref().and_then(|v| v.as_inline())
    }

    /// Get the value as Vec<u8> for backward compatibility
    /// Returns None if this is a deletion or a blob reference
    /// For blob references, the caller must resolve the blob separately
    pub fn value_as_vec(&self) -> Option<Vec<u8>> {
        self.value.as_ref().and_then(|v| v.to_vec())
    }

    /// Check if this entry is a deletion (tombstone)
    pub fn is_deletion(&self) -> bool {
        self.value.is_none()
    }
}

/// In-memory table for efficient key-value storage with MVCC support
///
/// Uses a BTreeMap for now (simple and correct). In production, we'd use a skip list
/// or other concurrent data structure for better performance.
///
/// Note: For simplicity, we store only the latest version per key. True MVCC with
/// multiple versions would require a more complex data structure.
#[derive(Debug)]
pub struct MemTable {
    data: RwLock<BTreeMap<Vec<u8>, Entry>>,
    size: AtomicUsize,
    sequence: AtomicU64,
}

impl MemTable {
    /// Create a new empty memtable
    pub fn new() -> Self {
        Self {
            data: RwLock::new(BTreeMap::new()),
            size: AtomicUsize::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    /// Insert or update a key-value pair
    /// For MVCC: entries are initially uncommitted (commit_ts = None)
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> u64 {
        let sequence = self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
        // Create entry with commit_ts = None (uncommitted)
        let entry = Entry::new(key.clone(), Some(value), sequence);
        let entry_size = entry.size();

        let mut data = self.data.write().unwrap();

        if let Some(old_entry) = data.insert(key, entry) {
            // Subtract old entry size, add new entry size
            let old_size = old_entry.size();
            if entry_size > old_size {
                self.size
                    .fetch_add(entry_size - old_size, AtomicOrdering::Release);
            } else {
                self.size
                    .fetch_sub(old_size - entry_size, AtomicOrdering::Release);
            }
        } else {
            self.size.fetch_add(entry_size, AtomicOrdering::Release);
        }

        sequence
    }

    /// Insert or update a key-value pair with immediate commit timestamp
    /// Used when writing already-committed data (e.g., from transaction commit)
    pub fn put_committed(&self, key: Vec<u8>, value: Vec<u8>, commit_ts: i64) -> u64 {
        let sequence = self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
        // Create entry with commit_ts set (committed)
        let entry = Entry::with_commit_ts(key.clone(), Some(value), sequence, commit_ts);
        let entry_size = entry.size();

        let mut data = self.data.write().unwrap();

        if let Some(old_entry) = data.insert(key, entry) {
            let old_size = old_entry.size();
            if entry_size > old_size {
                self.size
                    .fetch_add(entry_size - old_size, AtomicOrdering::Release);
            } else {
                self.size
                    .fetch_sub(old_size - entry_size, AtomicOrdering::Release);
            }
        } else {
            self.size.fetch_add(entry_size, AtomicOrdering::Release);
        }

        sequence
    }

    /// Mark a key as deleted
    pub fn delete(&self, key: Vec<u8>) -> u64 {
        let sequence = self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
        let entry = Entry::new(key.clone(), None, sequence);
        let entry_size = entry.size();

        let mut data = self.data.write().unwrap();

        if let Some(old_entry) = data.insert(key, entry) {
            let old_size = old_entry.size();
            if entry_size > old_size {
                self.size
                    .fetch_add(entry_size - old_size, AtomicOrdering::Release);
            } else {
                self.size
                    .fetch_sub(old_size - entry_size, AtomicOrdering::Release);
            }
        } else {
            self.size.fetch_add(entry_size, AtomicOrdering::Release);
        }

        sequence
    }

    /// Mark a key as deleted with immediate commit timestamp
    pub fn delete_committed(&self, key: Vec<u8>, commit_ts: i64) -> u64 {
        let sequence = self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
        let entry = Entry::with_commit_ts(key.clone(), None, sequence, commit_ts);
        let entry_size = entry.size();

        let mut data = self.data.write().unwrap();

        if let Some(old_entry) = data.insert(key, entry) {
            let old_size = old_entry.size();
            if entry_size > old_size {
                self.size
                    .fetch_add(entry_size - old_size, AtomicOrdering::Release);
            } else {
                self.size
                    .fetch_sub(old_size - entry_size, AtomicOrdering::Release);
            }
        } else {
            self.size.fetch_add(entry_size, AtomicOrdering::Release);
        }

        sequence
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Option<Entry> {
        let data = self.data.read().unwrap();
        data.get(key).cloned()
    }

    /// Get a value by key with snapshot isolation
    /// Only returns entries that are visible at the given snapshot timestamp
    pub fn get_at_snapshot(&self, key: &[u8], snapshot_ts: i64) -> Option<Entry> {
        let data = self.data.read().unwrap();
        if let Some(entry) = data.get(key) {
            if entry.is_visible(snapshot_ts) {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Mark an entry as committed with the given timestamp
    /// This is called when a transaction commits
    pub fn commit_entry(&self, key: &[u8], commit_ts: i64) -> bool {
        let mut data = self.data.write().unwrap();
        if let Some(entry) = data.get_mut(key) {
            if entry.commit_ts.is_none() {
                entry.commit_ts = Some(commit_ts);
                return true;
            }
        }
        false
    }

    /// Get approximate size in bytes
    pub fn size(&self) -> usize {
        self.size.load(AtomicOrdering::Acquire)
    }

    /// Get number of entries
    pub fn entry_count(&self) -> usize {
        let data = self.data.read().unwrap();
        data.len()
    }

    /// Check if memtable is empty
    pub fn is_empty(&self) -> bool {
        self.entry_count() == 0
    }

    /// Get current sequence number
    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(AtomicOrdering::Acquire)
    }

    /// Create an iterator over all entries in sorted order
    pub fn iter(&self) -> MemTableIterator {
        let data = self.data.read().unwrap();
        let entries: Vec<Entry> = data.values().cloned().collect();
        MemTableIterator { entries, index: 0 }
    }

    /// Create an iterator over a range of keys
    pub fn range(&self, start: &[u8], end: &[u8]) -> MemTableIterator {
        let data = self.data.read().unwrap();
        let entries: Vec<Entry> = data
            .range(start.to_vec()..end.to_vec())
            .map(|(_, entry)| entry.clone())
            .collect();
        MemTableIterator { entries, index: 0 }
    }

    /// Get all entries (for flushing to SSTable)
    pub fn entries(&self) -> Vec<Entry> {
        let data = self.data.read().unwrap();
        data.values().cloned().collect()
    }

    /// Clear all entries
    pub fn clear(&self) {
        let mut data = self.data.write().unwrap();
        data.clear();
        self.size.store(0, AtomicOrdering::Release);
        self.sequence.store(0, AtomicOrdering::Release);
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over memtable entries
pub struct MemTableIterator {
    entries: Vec<Entry>,
    index: usize,
}

impl Iterator for MemTableIterator {
    type Item = Entry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.entries.len() {
            let entry = self.entries[self.index].clone();
            self.index += 1;
            Some(entry)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memtable_basic_operations() {
        let memtable = MemTable::new();

        // Test put and get
        let seq1 = memtable.put(b"key1".to_vec(), b"value1".to_vec());
        let entry = memtable.get(b"key1").unwrap();
        assert_eq!(entry.value.as_ref().unwrap().as_inline().unwrap(), b"value1");
        assert_eq!(entry.sequence, seq1);

        // Test update
        let seq2 = memtable.put(b"key1".to_vec(), b"value2".to_vec());
        let entry = memtable.get(b"key1").unwrap();
        assert_eq!(entry.value.as_ref().unwrap().as_inline().unwrap(), b"value2");
        assert_eq!(entry.sequence, seq2);
        assert!(seq2 > seq1);

        // Test delete
        let seq3 = memtable.delete(b"key1".to_vec());
        let entry = memtable.get(b"key1").unwrap();
        assert!(entry.value.is_none());
        assert_eq!(entry.sequence, seq3);
        assert!(seq3 > seq2);
    }

    #[test]
    fn test_memtable_iteration() {
        let memtable = MemTable::new();

        memtable.put(b"key3".to_vec(), b"value3".to_vec());
        memtable.put(b"key1".to_vec(), b"value1".to_vec());
        memtable.put(b"key2".to_vec(), b"value2".to_vec());

        let entries: Vec<_> = memtable.iter().collect();
        assert_eq!(entries.len(), 3);
        // BTreeMap keeps keys sorted
        assert_eq!(entries[0].key, b"key1");
        assert_eq!(entries[1].key, b"key2");
        assert_eq!(entries[2].key, b"key3");
    }

    #[test]
    fn test_memtable_range() {
        let memtable = MemTable::new();

        memtable.put(b"key1".to_vec(), b"value1".to_vec());
        memtable.put(b"key2".to_vec(), b"value2".to_vec());
        memtable.put(b"key3".to_vec(), b"value3".to_vec());
        memtable.put(b"key4".to_vec(), b"value4".to_vec());

        let entries: Vec<_> = memtable.range(b"key2", b"key4").collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, b"key2");
        assert_eq!(entries[1].key, b"key3");
    }

    #[test]
    fn test_memtable_size() {
        let memtable = MemTable::new();

        assert_eq!(memtable.size(), 0);
        assert_eq!(memtable.entry_count(), 0);

        memtable.put(b"key1".to_vec(), b"value1".to_vec());
        assert!(memtable.size() > 0);
        assert_eq!(memtable.entry_count(), 1);

        memtable.put(b"key2".to_vec(), b"value2".to_vec());
        assert_eq!(memtable.entry_count(), 2);
    }

    #[test]
    fn test_memtable_sequence() {
        let memtable = MemTable::new();

        let seq1 = memtable.put(b"key1".to_vec(), b"value1".to_vec());
        let seq2 = memtable.put(b"key2".to_vec(), b"value2".to_vec());
        let seq3 = memtable.delete(b"key1".to_vec());

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        assert_eq!(seq3, 2);
        assert_eq!(memtable.current_sequence(), 3);
    }
}
