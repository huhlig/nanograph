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

use crate::memtable::{Entry, ValueLocation};
use nanograph_kvt::{KeyValueIterator, KeyValueResult};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Iterator entry with source information for merging
#[derive(Debug, Clone)]
struct IteratorEntry {
    key: Vec<u8>,
    value: Option<Vec<u8>>,
    sequence: u64,
    source_priority: usize, // Lower is higher priority (0 = memtable, 1+ = levels)
}

impl PartialEq for IteratorEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.sequence == other.sequence
    }
}

impl Eq for IteratorEntry {}

impl PartialOrd for IteratorEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IteratorEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap (we want min-heap behavior for keys)
        match other.key.cmp(&self.key) {
            Ordering::Equal => {
                // For same key, prefer higher priority (lower number)
                // Priority takes precedence over sequence number
                match other.source_priority.cmp(&self.source_priority) {
                    Ordering::Equal => other.sequence.cmp(&self.sequence),
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}

/// Cursor for resuming iteration
#[derive(Debug, Clone)]
pub struct IteratorCursor {
    pub position: usize,
}

/// LSM Tree iterator for range scans
pub struct LSMIterator {
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    position: usize,
    /// TODO: Implement reverse iteration support
    _reverse: bool,
    limit: Option<usize>,
}

impl LSMIterator {
    /// Create a new iterator from a list of entries
    pub fn new(entries: Vec<Entry>, reverse: bool) -> Self {
        // Filter out tombstones and convert to key-value pairs
        // For now, only handle inline values - blob references need to be resolved by caller
        let mut kv_pairs: Vec<(Vec<u8>, Vec<u8>)> = entries
            .into_iter()
            .filter_map(|e| {
                e.value.and_then(|v| match v {
                    ValueLocation::Inline(data) => Some((e.key, data)),
                    ValueLocation::Blob(_) => {
                        // TODO: Blob references should be resolved before creating iterator
                        // For now, skip blob entries
                        None
                    }
                })
            })
            .collect();

        // Sort by key
        kv_pairs.sort_by(|a, b| a.0.cmp(&b.0));

        // Reverse if needed
        if reverse {
            kv_pairs.reverse();
        }

        Self {
            entries: kv_pairs,
            position: 0,
            _reverse: reverse,
            limit: None,
        }
    }

    /// Set a limit on the number of entries to return
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = Some(limit);
    }

    /// Create an iterator by merging multiple sources
    pub fn merge(sources: Vec<Vec<Entry>>, reverse: bool) -> Self {
        let mut heap = BinaryHeap::new();
        let mut iterators: Vec<(usize, Vec<Entry>, usize)> = Vec::new();

        // Initialize iterators for each source
        for (priority, entries) in sources.into_iter().enumerate() {
            if !entries.is_empty() {
                iterators.push((priority, entries, 0));
            }
        }

        // Add first entry from each iterator to heap
        for (idx, (priority, entries, pos)) in iterators.iter().enumerate() {
            if *pos < entries.len() {
                let entry = &entries[*pos];
                // Only handle inline values for now
                let value = entry.value.as_ref().and_then(|v| match v {
                    ValueLocation::Inline(data) => Some(data.clone()),
                    ValueLocation::Blob(_) => None, // TODO: resolve blobs
                });
                
                heap.push((
                    IteratorEntry {
                        key: entry.key.clone(),
                        value,
                        sequence: entry.sequence,
                        source_priority: *priority,
                    },
                    idx,
                ));
            }
        }

        let mut result = Vec::new();
        let mut last_key: Option<Vec<u8>> = None;

        // Merge entries
        while let Some((iter_entry, iter_idx)) = heap.pop() {
            // Skip if same key as previous (keep only newest)
            if let Some(ref lk) = last_key {
                if lk == &iter_entry.key {
                    // Advance iterator
                    let (priority, entries, pos) = &mut iterators[iter_idx];
                    *pos += 1;
                    if *pos < entries.len() {
                        let entry = &entries[*pos];
                        let value = entry.value.as_ref().and_then(|v| match v {
                            ValueLocation::Inline(data) => Some(data.clone()),
                            ValueLocation::Blob(_) => None,
                        });
                        
                        heap.push((
                            IteratorEntry {
                                key: entry.key.clone(),
                                value,
                                sequence: entry.sequence,
                                source_priority: *priority,
                            },
                            iter_idx,
                        ));
                    }
                    continue;
                }
            }

            // Add entry if it has a value (not a tombstone)
            if let Some(value) = iter_entry.value {
                result.push((iter_entry.key.clone(), value));
            }

            last_key = Some(iter_entry.key);

            // Advance iterator
            let (priority, entries, pos) = &mut iterators[iter_idx];
            *pos += 1;
            if *pos < entries.len() {
                let entry = &entries[*pos];
                let value = entry.value.as_ref().and_then(|v| match v {
                    ValueLocation::Inline(data) => Some(data.clone()),
                    ValueLocation::Blob(_) => None,
                });
                
                heap.push((
                    IteratorEntry {
                        key: entry.key.clone(),
                        value,
                        sequence: entry.sequence,
                        source_priority: *priority,
                    },
                    iter_idx,
                ));
            }
        }

        // Reverse if needed
        if reverse {
            result.reverse();
        }

        Self {
            entries: result,
            position: 0,
            _reverse: reverse,
            limit: None,
        }
    }
}

impl futures_core::Stream for LSMIterator {
    type Item = KeyValueResult<(Vec<u8>, Vec<u8>)>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Check if we've reached the limit
        if let Some(limit) = self.limit {
            if self.position >= limit {
                return std::task::Poll::Ready(None);
            }
        }

        if self.position < self.entries.len() {
            let entry = self.entries[self.position].clone();
            self.position += 1;
            std::task::Poll::Ready(Some(Ok(entry)))
        } else {
            std::task::Poll::Ready(None)
        }
    }
}

impl KeyValueIterator for LSMIterator {
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()> {
        // Binary search for the key
        self.position = self
            .entries
            .binary_search_by(|(k, _)| k.as_slice().cmp(key))
            .unwrap_or_else(|i| i);
        Ok(())
    }

    fn position(&self) -> Option<Vec<u8>> {
        if self.position > 0 && self.position <= self.entries.len() {
            Some(self.entries[self.position - 1].0.clone())
        } else {
            None
        }
    }

    fn valid(&self) -> bool {
        self.position < self.entries.len()
    }
}

impl LSMIterator {
    /// Get current cursor position for resuming iteration later
    pub fn get_cursor(&self) -> IteratorCursor {
        IteratorCursor {
            position: self.position,
        }
    }

    /// Create iterator from a saved cursor position
    pub fn from_cursor(
        entries: Vec<(Vec<u8>, Vec<u8>)>,
        cursor: IteratorCursor,
        reverse: bool,
    ) -> Self {
        Self {
            entries,
            position: cursor.position,
            _reverse: reverse,
            limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iterator_basic() {
        let entries = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
            Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        ];

        let iter = LSMIterator::new(entries, false);

        // Verify entries are in order
        assert_eq!(iter.entries.len(), 3);
        assert_eq!(iter.entries[0].0, b"key1");
        assert_eq!(iter.entries[1].0, b"key2");
        assert_eq!(iter.entries[2].0, b"key3");
    }

    #[test]
    fn test_iterator_reverse() {
        let entries = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
            Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        ];

        let iter = LSMIterator::new(entries, true);

        // Verify entries are in reverse order
        assert_eq!(iter.entries.len(), 3);
        assert_eq!(iter.entries[0].0, b"key3");
        assert_eq!(iter.entries[1].0, b"key2");
        assert_eq!(iter.entries[2].0, b"key1");
    }

    #[test]
    fn test_iterator_merge() {
        let source1 = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1a".to_vec()), 1),
            Entry::new(b"key3".to_vec(), Some(b"value3a".to_vec()), 1),
        ];

        let source2 = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1b".to_vec()), 2), // Newer
            Entry::new(b"key2".to_vec(), Some(b"value2b".to_vec()), 2),
        ];

        let iter = LSMIterator::merge(vec![source1, source2], false);

        // Verify merged entries (source1 has priority, duplicates removed)
        assert_eq!(iter.entries.len(), 3);
        assert_eq!(iter.entries[0].0, b"key1");
        assert_eq!(iter.entries[0].1, b"value1a"); // Source 1 has higher priority
        assert_eq!(iter.entries[1].0, b"key2");
        assert_eq!(iter.entries[2].0, b"key3");
    }

    #[test]
    fn test_iterator_tombstones() {
        let entries = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            Entry::new(b"key2".to_vec(), None, 2), // Tombstone
            Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        ];

        let iter = LSMIterator::new(entries, false);

        // Verify tombstones are filtered out
        assert_eq!(iter.entries.len(), 2);
        assert_eq!(iter.entries[0].0, b"key1");
        assert_eq!(iter.entries[1].0, b"key3"); // key2 skipped (tombstone)
    }

    #[test]
    fn test_iterator_seek() {
        let entries = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
            Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        ];

        let mut iter = LSMIterator::new(entries, false);

        // Seek to key2
        iter.seek(b"key2").unwrap();
        assert_eq!(iter.position, 1);
        assert!(iter.valid());
    }

    #[test]
    fn test_iterator_position() {
        let entries = vec![Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1)];

        let mut iter = LSMIterator::new(entries, false);
        assert_eq!(iter.position(), None); // No position yet

        iter.position = 1;
        assert_eq!(iter.position(), Some(b"key1".to_vec()));
    }
}
