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

use crate::memtable::Entry;
use crate::sstable::{SSTable, SSTableMetadata};
use nanograph_kvt::KeyValueResult;
use nanograph_util::{CompressionAlgorithm, IntegrityAlgorithm};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Compaction task
#[derive(Debug, Clone)]
pub struct CompactionTask {
    pub source_level: usize,
    pub target_level: usize,
    pub source_sstables: Vec<SSTableMetadata>,
    pub target_sstables: Vec<SSTableMetadata>,
}

/// Entry with source information for merge
/// TODO: Use in multi-way merge during compaction execution
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct MergeEntry {
    entry: Entry,
    source_level: usize,
    source_file: u64,
}

impl PartialEq for MergeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.entry.key == other.entry.key && self.entry.sequence == other.entry.sequence
    }
}

impl Eq for MergeEntry {}

impl PartialOrd for MergeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MergeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap (we want min-heap behavior)
        match other.entry.key.cmp(&self.entry.key) {
            Ordering::Equal => {
                // For same key, prefer higher sequence number (newer)
                other.entry.sequence.cmp(&self.entry.sequence)
            }
            ord => ord,
        }
    }
}

/// Compaction strategy
pub struct CompactionStrategy {
    pub level_size_multiplier: usize,
    pub max_level: usize,
    pub level0_file_num_compaction_trigger: usize,
}

impl Default for CompactionStrategy {
    fn default() -> Self {
        Self {
            level_size_multiplier: 10,
            max_level: 7,
            level0_file_num_compaction_trigger: 4,
        }
    }
}

impl CompactionStrategy {
    /// Calculate target size for a level
    pub fn target_size(&self, level: usize, base_size: u64) -> u64 {
        if level == 0 {
            0 // Level 0 uses file count
        } else {
            base_size * (self.level_size_multiplier as u64).pow(level as u32)
        }
    }

    /// Select SSTables for compaction
    pub fn select_compaction(
        &self,
        levels: &[Vec<SSTableMetadata>],
        base_size: u64,
    ) -> Option<CompactionTask> {
        // Check level 0 first (file count based)
        if levels[0].len() >= self.level0_file_num_compaction_trigger {
            return Some(CompactionTask {
                source_level: 0,
                target_level: 1,
                source_sstables: levels[0].clone(),
                target_sstables: self.find_overlapping_sstables(&levels[0], &levels[1]),
            });
        }

        // Check other levels (size based)
        for level in 1..self.max_level {
            if level >= levels.len() {
                break;
            }

            let current_size: u64 = levels[level].iter().map(|s| s.file_size).sum();
            let target_size = self.target_size(level, base_size);

            if current_size > target_size {
                // Select one SSTable from this level
                if let Some(sstable) = levels[level].first() {
                    let source_sstables = vec![sstable.clone()];
                    let target_sstables = if level + 1 < levels.len() {
                        self.find_overlapping_sstables(&source_sstables, &levels[level + 1])
                    } else {
                        Vec::new()
                    };

                    return Some(CompactionTask {
                        source_level: level,
                        target_level: level + 1,
                        source_sstables,
                        target_sstables,
                    });
                }
            }
        }

        None
    }

    /// Find overlapping SSTables in target level
    fn find_overlapping_sstables(
        &self,
        source: &[SSTableMetadata],
        target: &[SSTableMetadata],
    ) -> Vec<SSTableMetadata> {
        if source.is_empty() || target.is_empty() {
            return Vec::new();
        }

        let min_key = source.iter().map(|s| &s.min_key).min().unwrap();
        let max_key = source.iter().map(|s| &s.max_key).max().unwrap();

        target
            .iter()
            .filter(|t| {
                // Check if ranges overlap
                !(t.max_key.as_slice() < min_key.as_slice()
                    || t.min_key.as_slice() > max_key.as_slice())
            })
            .cloned()
            .collect()
    }
}

/// Compaction executor
/// TODO: Integrate into LSM engine's compaction workflow
#[allow(dead_code)]
pub struct CompactionExecutor {
    fs: std::sync::Arc<dyn nanograph_vfs::DynamicFileSystem>,
    base_path: String,
    block_size: usize,
    compression: CompressionAlgorithm,
    integrity: IntegrityAlgorithm,
}

#[allow(dead_code)]
impl CompactionExecutor {
    pub fn new(
        fs: std::sync::Arc<dyn nanograph_vfs::DynamicFileSystem>,
        base_path: String,
        block_size: usize,
        compression: CompressionAlgorithm,
        integrity: IntegrityAlgorithm,
    ) -> Self {
        Self {
            fs,
            base_path,
            block_size,
            compression,
            integrity,
        }
    }

    /// Execute a compaction task
    pub fn execute(
        &self,
        task: &CompactionTask,
        next_file_number: &mut u64,
    ) -> KeyValueResult<Vec<SSTableMetadata>> {
        // Merge all entries from source and target SSTables
        let mut entries = self.merge_sstables(
            &task.source_sstables,
            &task.target_sstables,
            task.source_level,
        )?;

        // Remove duplicates and tombstones
        entries = self.deduplicate_and_filter(entries);

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Split into multiple SSTables if needed
        let max_sstable_size = self.block_size * 1024; // ~4MB per SSTable
        let mut result = Vec::new();
        let mut current_batch = Vec::new();
        let mut current_size = 0;

        for entry in entries {
            let entry_size = entry.size();

            if current_size + entry_size > max_sstable_size && !current_batch.is_empty() {
                // Write current batch
                let metadata =
                    self.write_sstable(current_batch, *next_file_number, task.target_level)?;
                result.push(metadata);
                *next_file_number += 1;

                current_batch = Vec::new();
                current_size = 0;
            }

            current_batch.push(entry);
            current_size += entry_size;
        }

        // Write remaining entries
        if !current_batch.is_empty() {
            let metadata =
                self.write_sstable(current_batch, *next_file_number, task.target_level)?;
            result.push(metadata);
            *next_file_number += 1;
        }

        Ok(result)
    }

    /// Merge entries from multiple SSTables
    fn merge_sstables(
        &self,
        source: &[SSTableMetadata],
        target: &[SSTableMetadata],
        source_level: usize,
    ) -> KeyValueResult<Vec<Entry>> {
        let mut heap = BinaryHeap::new();
        let mut iterators: Vec<(usize, u64, Vec<Entry>, usize)> = Vec::new();

        // Load entries from source SSTables
        for sstable in source {
            let entries = self.load_sstable_entries(sstable)?;
            if !entries.is_empty() {
                iterators.push((source_level, sstable.file_number, entries, 0));
            }
        }

        // Load entries from target SSTables
        for sstable in target {
            let entries = self.load_sstable_entries(sstable)?;
            if !entries.is_empty() {
                iterators.push((source_level + 1, sstable.file_number, entries, 0));
            }
        }

        // Initialize heap with first entry from each iterator
        for (idx, (level, file, entries, pos)) in iterators.iter().enumerate() {
            if *pos < entries.len() {
                heap.push((
                    MergeEntry {
                        entry: entries[*pos].clone(),
                        source_level: *level,
                        source_file: *file,
                    },
                    idx,
                ));
            }
        }

        let mut result = Vec::new();

        // Merge entries
        while let Some((merge_entry, iter_idx)) = heap.pop() {
            result.push(merge_entry.entry);

            // Advance iterator and add next entry to heap
            let (level, file, entries, pos) = &mut iterators[iter_idx];
            *pos += 1;
            if *pos < entries.len() {
                heap.push((
                    MergeEntry {
                        entry: entries[*pos].clone(),
                        source_level: *level,
                        source_file: *file,
                    },
                    iter_idx,
                ));
            }
        }

        Ok(result)
    }

    /// Load all entries from an SSTable using VFS
    fn load_sstable_entries(&self, metadata: &SSTableMetadata) -> KeyValueResult<Vec<Entry>> {
        let path = format!("{}/{:06}.sst", self.base_path, metadata.file_number);

        // Open the SSTable file using VFS
        let file = self.fs.open_file(&path).map_err(|e| {
            nanograph_kvt::KeyValueError::StorageCorruption(format!(
                "Failed to open SSTable {}: {}",
                metadata.file_number, e
            ))
        })?;

        // Create iterator (takes ownership of reader) and collect all entries
        let iter = SSTable::iter(file).map_err(|e| {
            nanograph_kvt::KeyValueError::StorageCorruption(format!(
                "Failed to create SSTable iterator for {}: {}",
                metadata.file_number, e
            ))
        })?;

        // Collect entries, handling any I/O errors
        let mut entries = Vec::new();
        for entry_result in iter {
            let entry = entry_result.map_err(|e| {
                nanograph_kvt::KeyValueError::StorageCorruption(format!(
                    "Failed to read entry from SSTable {}: {}",
                    metadata.file_number, e
                ))
            })?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Remove duplicates and filter tombstones
    fn deduplicate_and_filter(&self, entries: Vec<Entry>) -> Vec<Entry> {
        let mut result = Vec::new();
        let mut prev_key: Option<Vec<u8>> = None;

        for entry in entries {
            // Skip if same key as previous (keep only newest)
            if let Some(ref pk) = prev_key {
                if pk == &entry.key {
                    continue;
                }
            }

            // Skip tombstones (deletions) during compaction
            if entry.value.is_some() {
                result.push(entry.clone());
            }

            prev_key = Some(entry.key);
        }

        result
    }

    /// Write entries to a new SSTable using VFS
    fn write_sstable(
        &self,
        entries: Vec<Entry>,
        file_number: u64,
        level: usize,
    ) -> KeyValueResult<SSTableMetadata> {
        let path = format!("{}/{:06}.sst", self.base_path, file_number);
        let mut file = self
            .fs
            .create_file(&path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        let metadata = SSTable::create(
            &mut file,
            entries,
            file_number,
            level,
            self.block_size,
            self.compression,
            self.integrity,
        )
        .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Sync file to ensure durability
        file.sync_all()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_strategy() {
        let strategy = CompactionStrategy::default();

        // Test target sizes
        assert_eq!(strategy.target_size(0, 64 * 1024 * 1024), 0);
        assert_eq!(strategy.target_size(1, 64 * 1024 * 1024), 640 * 1024 * 1024);
        assert_eq!(
            strategy.target_size(2, 64 * 1024 * 1024),
            6400 * 1024 * 1024
        );
    }

    #[test]
    fn test_merge_entry_ordering() {
        let entry1 = MergeEntry {
            entry: Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            source_level: 0,
            source_file: 1,
        };

        let entry2 = MergeEntry {
            entry: Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
            source_level: 0,
            source_file: 1,
        };

        // entry1 should come before entry2 (smaller key)
        assert!(entry1 > entry2); // Reversed for min-heap
    }
}
