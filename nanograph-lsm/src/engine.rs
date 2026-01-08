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

use crate::compaction::{CompactionExecutor, CompactionStrategy};
use crate::memtable::MemTable;
use crate::metrics::LSMMetrics;
use crate::options::LSMTreeOptions;
use crate::sstable::{SSTable, SSTableMetadata};
use crate::wal_record::{
    WalRecordKind, decode_checkpoint, decode_commit, decode_delete, decode_delete_committed,
    decode_flush_complete, decode_put, decode_put_committed, encode_checkpoint, encode_commit,
    encode_delete, encode_delete_committed, encode_flush_complete, encode_put,
    encode_put_committed,
};
use nanograph_kvt::KeyValueResult;
use nanograph_vfs::{DynamicFileSystem, File as VfsFile};
use nanograph_wal::{LogSequenceNumber, WriteAheadLogManager, WriteAheadLogRecord};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

// Global timestamp counter for MVCC
static GLOBAL_TIMESTAMP: AtomicI64 = AtomicI64::new(1);

fn next_timestamp() -> i64 {
    GLOBAL_TIMESTAMP.fetch_add(1, Ordering::SeqCst)
}

/// Level information for LSM tree
#[derive(Debug, Clone)]
pub struct Level {
    level_number: usize,
    sstables: Vec<SSTableMetadata>,
    total_size: u64,
}

impl Level {
    fn new(level_number: usize) -> Self {
        Self {
            level_number,
            sstables: Vec::new(),
            total_size: 0,
        }
    }

    fn add_sstable(&mut self, metadata: SSTableMetadata) {
        self.total_size += metadata.file_size;
        self.sstables.push(metadata);
    }

    /// Remove an SSTable from this level
    /// TODO: Use during compaction cleanup to remove merged SSTables
    #[allow(dead_code)]
    fn remove_sstable(&mut self, file_number: u64) {
        if let Some(pos) = self
            .sstables
            .iter()
            .position(|s| s.file_number == file_number)
        {
            let removed = self.sstables.remove(pos);
            self.total_size -= removed.file_size;
        }
    }

    fn needs_compaction(&self, max_size: u64) -> bool {
        if self.level_number == 0 {
            // Level 0 triggers on file count
            self.sstables.len() >= 4
        } else {
            // Other levels trigger on total size
            self.total_size > max_size
        }
    }
}

/// Manifest entry for tracking SSTable metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestEntry {
    file_number: u64,
    level: usize,
    metadata: SSTableMetadata,
}

/// LSM Tree Engine - manages memtable, SSTables, and compaction
pub struct LSMTreeEngine {
    // Configuration
    options: LSMTreeOptions,
    base_path: String,

    // Virtual filesystem
    fs: Arc<dyn DynamicFileSystem>,

    // Write-ahead log
    wal: Arc<WriteAheadLogManager>,
    wal_writer: Arc<Mutex<nanograph_wal::WriteAheadLogWriter>>,

    // Last flushed LSN (for WAL truncation)
    flushed_lsn: Arc<RwLock<Option<LogSequenceNumber>>>,

    // Active memtable
    pub memtable: Arc<RwLock<MemTable>>,

    // Immutable memtable being flushed
    pub immutable_memtable: Arc<RwLock<Option<MemTable>>>,

    // LSM tree levels
    pub levels: Arc<RwLock<Vec<Level>>>,

    // File number generator
    next_file_number: AtomicU64,

    // Flush and compaction flags
    flush_in_progress: AtomicBool,
    compaction_in_progress: AtomicBool,

    // Metrics
    pub metrics: LSMMetrics,

    // Statistics (deprecated - use metrics instead)
    total_writes: AtomicU64,
    total_reads: AtomicU64,
    total_flushes: AtomicU64,
    total_compactions: AtomicU64,
}

impl LSMTreeEngine {
    /// Create a new LSM Tree Engine with VFS support
    pub fn new(
        fs: Arc<dyn DynamicFileSystem>,
        base_path: String,
        options: LSMTreeOptions,
        wal: WriteAheadLogManager,
    ) -> KeyValueResult<Self> {
        // Ensure base directory exists
        fs.create_directory_all(&base_path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Initialize levels
        let mut levels = Vec::new();
        for i in 0..7 {
            // 7 levels by default
            levels.push(Level::new(i));
        }

        // Create WAL writer
        let wal = Arc::new(wal);
        let wal_writer = wal
            .writer()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        let engine = Self {
            options,
            base_path,
            fs,
            wal: wal.clone(),
            wal_writer: Arc::new(Mutex::new(wal_writer)),
            flushed_lsn: Arc::new(RwLock::new(None)),
            memtable: Arc::new(RwLock::new(MemTable::new())),
            immutable_memtable: Arc::new(RwLock::new(None)),
            levels: Arc::new(RwLock::new(levels)),
            next_file_number: AtomicU64::new(1),
            flush_in_progress: AtomicBool::new(false),
            compaction_in_progress: AtomicBool::new(false),
            metrics: LSMMetrics::new(),
            total_writes: AtomicU64::new(0),
            total_reads: AtomicU64::new(0),
            total_flushes: AtomicU64::new(0),
            total_compactions: AtomicU64::new(0),
        };

        // Load manifest if it exists
        engine.load_manifest()?;

        // Recover from WAL
        engine.recover_from_wal()?;

        Ok(engine)
    }

    /// Get the shard ID for this engine
    pub fn shard_id(&self) -> u64 {
        self.options.shard_id
    }

    /// Load manifest file to restore LSM tree state
    fn load_manifest(&self) -> KeyValueResult<()> {
        let manifest_path = format!("{}/MANIFEST", self.base_path);

        // Check if manifest exists
        if !self
            .fs
            .exists(&manifest_path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?
        {
            // No manifest, this is a new database
            return Ok(());
        }

        // Read manifest file
        let mut file = self
            .fs
            .open_file(&manifest_path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Deserialize manifest
        let manifest: Vec<ManifestEntry> = serde_json::from_slice(&contents)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Restore levels
        let mut levels = self.levels.write().unwrap();
        for entry in manifest {
            if entry.level < levels.len() {
                levels[entry.level].add_sstable(entry.metadata);
                // Update next file number
                let current = self.next_file_number.load(Ordering::SeqCst);
                if entry.file_number >= current {
                    self.next_file_number
                        .store(entry.file_number + 1, Ordering::SeqCst);
                }
            }
        }

        Ok(())
    }

    /// Save manifest file with current LSM tree state
    fn save_manifest(&self) -> KeyValueResult<()> {
        let manifest_path = format!("{}/MANIFEST", self.base_path);

        // Collect all SSTable metadata
        let levels = self.levels.read().unwrap();
        let mut manifest = Vec::new();

        for level in levels.iter() {
            for sstable_meta in &level.sstables {
                manifest.push(ManifestEntry {
                    file_number: sstable_meta.file_number,
                    level: level.level_number,
                    metadata: sstable_meta.clone(),
                });
            }
        }

        // Serialize manifest
        let contents = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Write to file
        let mut file = self
            .fs
            .create_file(&manifest_path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        file.write_all(&contents)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        file.sync_all()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        Ok(())
    }

    /// Recover from WAL by replaying all records
    fn recover_from_wal(&self) -> KeyValueResult<()> {
        // Get WAL reader starting from the beginning
        let mut reader = self.wal.reader_from(LogSequenceNumber::ZERO).map_err(|e| {
            nanograph_kvt::KeyValueError::StorageCorruption(format!(
                "Failed to create WAL reader: {}",
                e
            ))
        })?;

        let mut recovered_count = 0;
        let memtable = self.memtable.write().unwrap();

        // Replay all WAL records
        // Empty WAL will cause a corruption error at offset 0, which we can safely ignore
        loop {
            let entry = match reader.next() {
                Ok(Some(e)) => e,
                Ok(None) => break, // End of WAL
                Err(e) => {
                    // Ignore corruption at the start of an empty WAL
                    let err_str = e.to_string();
                    if err_str.contains("segment_id: 0, offset: 0") {
                        break;
                    }
                    return Err(nanograph_kvt::KeyValueError::StorageCorruption(format!(
                        "WAL read error: {}",
                        e
                    )));
                }
            };

            match WalRecordKind::from_u16(entry.kind) {
                Some(WalRecordKind::Put) => {
                    let (key, value) = decode_put(&entry.payload)?;
                    let ts = next_timestamp();
                    memtable.put_committed(key, value, ts);
                    recovered_count += 1;
                }
                Some(WalRecordKind::PutCommitted) => {
                    let (key, value, ts) = decode_put_committed(&entry.payload)?;
                    memtable.put_committed(key, value, ts);
                    recovered_count += 1;
                }
                Some(WalRecordKind::Delete) => {
                    let key = decode_delete(&entry.payload)?;
                    let ts = next_timestamp();
                    memtable.delete_committed(key, ts);
                    recovered_count += 1;
                }
                Some(WalRecordKind::DeleteCommitted) => {
                    let (key, ts) = decode_delete_committed(&entry.payload)?;
                    memtable.delete_committed(key, ts);
                    recovered_count += 1;
                }
                Some(WalRecordKind::Commit) => {
                    // Transaction commit marker - already handled by committed records
                }
                Some(WalRecordKind::Checkpoint) => {
                    // Checkpoint marker - could truncate WAL here in the future
                }
                Some(WalRecordKind::FlushComplete) => {
                    // Flush completion marker - metadata already in manifest
                }
                None => {
                    // Unknown record type - skip it
                    continue;
                }
            }
        }

        drop(memtable);

        if recovered_count > 0 {
            info!(
                "Recovered {} operations from WAL for shard {}",
                recovered_count, self.options.shard_id
            );
        }

        Ok(())
    }

    /// Create a checkpoint for this engine
    /// Writes checkpoint marker to WAL and saves manifest
    pub fn checkpoint(&self) -> KeyValueResult<()> {
        // Get current memtable sequence and file number
        let sequence = self.memtable.read().unwrap().current_sequence();
        let file_number = self.next_file_number.load(Ordering::SeqCst);

        // Write checkpoint marker to WAL
        let payload = encode_checkpoint(sequence, file_number);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::Checkpoint.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, nanograph_wal::Durability::Sync)
            .map_err(|e| {
                nanograph_kvt::KeyValueError::StorageCorruption(format!(
                    "Checkpoint write failed: {}",
                    e
                ))
            })?;
        drop(writer);

        // Save manifest to persist SSTable metadata
        self.save_manifest()?;

        Ok(())
    }

    /// Get path for SSTable file
    fn sstable_path(&self, file_number: u64) -> String {
        format!("{}/{:06}.sst", self.base_path, file_number)
    }

    /// Put a key-value pair
    #[instrument(skip(self, key, value), fields(key_len = key.len(), value_len = value.len()))]
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> KeyValueResult<()> {
        let start = Instant::now();
        let total_bytes = key.len() + value.len();

        debug!("LSM put operation started");

        // Write to WAL first for durability
        let payload = encode_put(&key, &value);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::Put.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;
        drop(writer);

        // Get current timestamp for immediate commit
        let commit_ts = next_timestamp();

        // Write to memtable as committed (non-transactional writes are immediately visible)
        let memtable = self.memtable.read().unwrap();
        memtable.put_committed(key, value, commit_ts);

        // Update memtable size metric
        self.metrics.set_memtable_size(memtable.size());

        self.total_writes.fetch_add(1, Ordering::Relaxed);

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_write(total_bytes, duration);

        // Check if memtable needs flushing
        if memtable.size() >= self.options.memtable_size {
            drop(memtable);
            info!("Memtable size threshold reached, triggering flush");
            self.maybe_flush_memtable()?;
        }

        debug!("LSM put operation completed in {:?}", duration);
        Ok(())
    }

    /// Put a key-value pair with commit timestamp (for MVCC transactions)
    pub fn put_committed(
        &self,
        key: Vec<u8>,
        value: Vec<u8>,
        commit_ts: i64,
    ) -> KeyValueResult<()> {
        // Write to WAL first for durability with commit timestamp
        let payload = encode_put_committed(&key, &value, commit_ts);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::PutCommitted.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;
        drop(writer);

        // Write to memtable with commit timestamp
        let memtable = self.memtable.read().unwrap();
        memtable.put_committed(key, value, commit_ts);

        self.total_writes.fetch_add(1, Ordering::Relaxed);

        // Check if memtable needs flushing
        if memtable.size() >= self.options.memtable_size {
            drop(memtable);
            self.maybe_flush_memtable()?;
        }

        Ok(())
    }

    /// Delete a key
    pub fn delete(&self, key: Vec<u8>) -> KeyValueResult<()> {
        // Write to WAL first for durability
        let payload = encode_delete(&key);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::Delete.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;
        drop(writer);

        // Get current timestamp for immediate commit
        let commit_ts = next_timestamp();

        // Write tombstone to memtable as committed (non-transactional deletes are immediately visible)
        let memtable = self.memtable.read().unwrap();
        memtable.delete_committed(key, commit_ts);

        self.total_writes.fetch_add(1, Ordering::Relaxed);

        // Check if memtable needs flushing
        if memtable.size() >= self.options.memtable_size {
            drop(memtable);
            self.maybe_flush_memtable()?;
        }

        Ok(())
    }

    /// Delete a key with commit timestamp (for MVCC transactions)
    pub fn delete_committed(&self, key: Vec<u8>, commit_ts: i64) -> KeyValueResult<()> {
        // Write to WAL first for durability with commit timestamp
        let payload = encode_delete_committed(&key, commit_ts);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::DeleteCommitted.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;
        drop(writer);

        // Write tombstone to memtable with commit timestamp
        let memtable = self.memtable.read().unwrap();
        memtable.delete_committed(key, commit_ts);

        self.total_writes.fetch_add(1, Ordering::Relaxed);

        // Check if memtable needs flushing
        if memtable.size() >= self.options.memtable_size {
            drop(memtable);
            self.maybe_flush_memtable()?;
        }

        Ok(())
    }

    /// Get a value by key
    #[instrument(skip(self, key), fields(key_len = key.len()))]
    pub fn get(&self, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let start = Instant::now();
        let mut sstables_checked = 0u64;

        debug!("LSM get operation started");

        self.total_reads.fetch_add(1, Ordering::Relaxed);

        // 1. Check active memtable
        {
            let memtable = self.memtable.read().unwrap();
            if let Some(entry) = memtable.get(key) {
                self.metrics.record_memtable_hit();
                let duration = start.elapsed();
                let bytes = entry.value.as_ref().map_or(0, |v| v.len());
                self.metrics.record_read(bytes, duration);
                debug!("Found in active memtable in {:?}", duration);
                return Ok(entry.value);
            }
        }

        // 2. Check immutable memtable
        {
            let immutable = self.immutable_memtable.read().unwrap();
            if let Some(ref memtable) = *immutable {
                if let Some(entry) = memtable.get(key) {
                    self.metrics.record_memtable_hit();
                    let duration = start.elapsed();
                    let bytes = entry.value.as_ref().map_or(0, |v| v.len());
                    self.metrics.record_read(bytes, duration);
                    debug!("Found in immutable memtable in {:?}", duration);
                    return Ok(entry.value);
                }
            }
        }

        // 3. Check SSTables from newest to oldest
        let levels = self.levels.read().unwrap();

        // Level 0: Check all SSTables (they may overlap)
        for sstable_meta in levels[0].sstables.iter().rev() {
            if key >= sstable_meta.min_key.as_slice() && key <= sstable_meta.max_key.as_slice() {
                sstables_checked += 1;
                let path = self.sstable_path(sstable_meta.file_number);
                if let Ok(mut file) = self.fs.open_file(&path) {
                    if let Ok(Some(entry)) = SSTable::get(&mut file, key, self.options.integrity) {
                        self.metrics.record_sstable_read(true);
                        let duration = start.elapsed();
                        let bytes = entry.value.as_ref().map_or(0, |v| v.len());
                        self.metrics.record_read(bytes, duration);
                        self.metrics.record_sstable_reads_for_get(sstables_checked);
                        debug!(
                            "Found in L0 SSTable after checking {} tables in {:?}",
                            sstables_checked, duration
                        );
                        return Ok(entry.value);
                    }
                }
            }
        }

        // Level 1+: Binary search for the right SSTable (non-overlapping)
        for level in &levels[1..] {
            // Binary search for SSTable containing the key
            let idx = level.sstables.binary_search_by(|s| {
                if key < s.min_key.as_slice() {
                    std::cmp::Ordering::Greater
                } else if key > s.max_key.as_slice() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            if let Ok(idx) = idx {
                sstables_checked += 1;
                let sstable_meta = &level.sstables[idx];
                let path = self.sstable_path(sstable_meta.file_number);
                if let Ok(mut file) = self.fs.open_file(&path) {
                    if let Ok(Some(entry)) = SSTable::get(&mut file, key, self.options.integrity) {
                        self.metrics.record_sstable_read(true);
                        let duration = start.elapsed();
                        let bytes = entry.value.as_ref().map_or(0, |v| v.len());
                        self.metrics.record_read(bytes, duration);
                        self.metrics.record_sstable_reads_for_get(sstables_checked);
                        debug!(
                            "Found in L{} SSTable after checking {} tables in {:?}",
                            level.level_number, sstables_checked, duration
                        );
                        return Ok(entry.value);
                    }
                }
            }
        }

        // Not found
        let duration = start.elapsed();
        self.metrics.record_read(0, duration);
        self.metrics.record_sstable_reads_for_get(sstables_checked);
        debug!(
            "Key not found after checking {} SSTables in {:?}",
            sstables_checked, duration
        );
        Ok(None)
    }

    /// Get a value by key at a specific snapshot timestamp (for MVCC)
    /// Only returns entries that are visible at the given snapshot timestamp
    pub fn get_at_snapshot(&self, key: &[u8], snapshot_ts: i64) -> KeyValueResult<Option<Vec<u8>>> {
        self.total_reads.fetch_add(1, Ordering::Relaxed);

        // 1. Check active memtable with snapshot filtering
        {
            let memtable = self.memtable.read().unwrap();
            if let Some(entry) = memtable.get_at_snapshot(key, snapshot_ts) {
                return Ok(entry.value);
            }
        }

        // 2. Check immutable memtable with snapshot filtering
        {
            let immutable = self.immutable_memtable.read().unwrap();
            if let Some(ref memtable) = *immutable {
                if let Some(entry) = memtable.get_at_snapshot(key, snapshot_ts) {
                    return Ok(entry.value);
                }
            }
        }

        // 3. Check SSTables from newest to oldest
        // Note: SSTables contain committed data, so we need to check if they were
        // written before our snapshot timestamp. For now, we'll read from SSTables
        // as they represent committed state. A full MVCC implementation would need
        // timestamps in SSTable entries as well.
        let levels = self.levels.read().unwrap();

        // Level 0: Check all SSTables (they may overlap)
        for sstable_meta in levels[0].sstables.iter().rev() {
            if key >= sstable_meta.min_key.as_slice() && key <= sstable_meta.max_key.as_slice() {
                let path = self.sstable_path(sstable_meta.file_number);
                if let Ok(mut file) = self.fs.open_file(&path) {
                    if let Ok(Some(entry)) = SSTable::get(&mut file, key, self.options.integrity) {
                        return Ok(entry.value);
                    }
                }
            }
        }

        // Level 1+: Binary search for the right SSTable (non-overlapping)
        for level in &levels[1..] {
            let idx = level.sstables.binary_search_by(|s| {
                if key < s.min_key.as_slice() {
                    std::cmp::Ordering::Greater
                } else if key > s.max_key.as_slice() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            if let Ok(idx) = idx {
                let sstable_meta = &level.sstables[idx];
                let path = self.sstable_path(sstable_meta.file_number);
                if let Ok(mut file) = self.fs.open_file(&path) {
                    if let Ok(Some(entry)) = SSTable::get(&mut file, key, self.options.integrity) {
                        return Ok(entry.value);
                    }
                }
            }
        }

        Ok(None)
    }

    /// Mark an entry as committed with the given timestamp (for MVCC)
    pub fn commit_entry(&self, key: &[u8], commit_ts: i64) -> KeyValueResult<()> {
        // Try to commit in active memtable
        {
            let memtable = self.memtable.read().unwrap();
            if memtable.commit_entry(key, commit_ts) {
                return Ok(());
            }
        }

        // Try to commit in immutable memtable
        {
            let immutable = self.immutable_memtable.read().unwrap();
            if let Some(ref memtable) = *immutable {
                if memtable.commit_entry(key, commit_ts) {
                    return Ok(());
                }
            }
        }

        // Entry not found in memtables (might be in SSTable already, which is fine)
        Ok(())
    }

    /// Flush memtable to SSTable
    fn maybe_flush_memtable(&self) -> KeyValueResult<()> {
        // Try to acquire flush lock
        if self
            .flush_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            // Flush already in progress
            return Ok(());
        }

        // Move current memtable to immutable
        {
            let mut memtable = self.memtable.write().unwrap();
            let mut immutable = self.immutable_memtable.write().unwrap();

            if immutable.is_some() {
                // Previous flush not complete yet
                self.flush_in_progress.store(false, Ordering::SeqCst);
                return Ok(());
            }

            // Swap memtables
            let old_memtable = std::mem::replace(&mut *memtable, MemTable::new());
            *immutable = Some(old_memtable);
        }

        // Flush immutable memtable to disk
        let result = self.flush_immutable_memtable();

        // Clear immutable memtable
        {
            let mut immutable = self.immutable_memtable.write().unwrap();
            *immutable = None;
        }

        // Release flush lock
        self.flush_in_progress.store(false, Ordering::SeqCst);

        result
    }

    /// Flush immutable memtable to SSTable file
    fn flush_immutable_memtable(&self) -> KeyValueResult<()> {
        let immutable = self.immutable_memtable.read().unwrap();
        let memtable = match immutable.as_ref() {
            Some(mt) => mt,
            None => return Ok(()),
        };

        if memtable.is_empty() {
            return Ok(());
        }

        // Get memtable sequence before flushing
        let memtable_sequence = memtable.current_sequence();

        // Get all entries sorted
        let entries = memtable.entries();
        if entries.is_empty() {
            return Ok(());
        }

        // Generate file number
        let file_number = self.next_file_number.fetch_add(1, Ordering::SeqCst);
        let path = self.sstable_path(file_number);

        info!(
            file_number = file_number,
            sequence = memtable_sequence,
            entry_count = entries.len(),
            "Flushing memtable to SSTable"
        );

        // Create SSTable file using VFS
        let mut file = self
            .fs
            .create_file(&path)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Write SSTable
        let metadata = SSTable::create(
            &mut file,
            entries,
            file_number,
            0, // Level 0
            self.options.block_size,
            self.options.compression,
            self.options.integrity,
        )
        .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Sync file to disk
        file.sync_all()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Write checkpoint marker to WAL before updating in-memory state
        self.write_checkpoint(memtable_sequence, file_number)?;

        // Add to level 0
        {
            let mut levels = self.levels.write().unwrap();
            levels[0].add_sstable(metadata);
        }

        // Write flush complete marker to WAL
        self.write_flush_complete(file_number, 0)?;

        self.total_flushes.fetch_add(1, Ordering::Relaxed);

        // Save manifest after adding new SSTable
        self.save_manifest()?;

        // Update flushed LSN and truncate WAL
        self.rotate_wal()?;

        // Check if compaction is needed
        self.maybe_compact()?;

        Ok(())
    }

    /// Rotate WAL after memtable flush
    fn rotate_wal(&self) -> KeyValueResult<()> {
        // Get current tail LSN
        let tail_lsn = self
            .wal
            .tail_lsn()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Update flushed LSN
        {
            let mut flushed = self.flushed_lsn.write().unwrap();
            *flushed = Some(tail_lsn);
        }

        // Truncate WAL before this LSN (keep current segment)
        // We can safely remove old segments since data is now in SSTables
        self.wal
            .truncate_before(tail_lsn)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        Ok(())
    }

    /// Write a checkpoint marker to WAL
    /// Records the memtable sequence number and SSTable file number
    fn write_checkpoint(&self, sequence: u64, file_number: u64) -> KeyValueResult<()> {
        let payload = encode_checkpoint(sequence, file_number);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::Checkpoint.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        debug!(
            sequence = sequence,
            file_number = file_number,
            "Wrote checkpoint marker to WAL"
        );

        Ok(())
    }

    /// Write a flush complete marker to WAL
    /// Records that an SSTable has been successfully written to a level
    fn write_flush_complete(&self, file_number: u64, level: u32) -> KeyValueResult<()> {
        let payload = encode_flush_complete(file_number, level);
        let record = WriteAheadLogRecord {
            kind: WalRecordKind::FlushComplete.to_u16(),
            payload: &payload,
        };

        let mut writer = self.wal_writer.lock().unwrap();
        writer
            .append(record, self.options.durability)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        debug!(
            file_number = file_number,
            level = level,
            "Wrote flush complete marker to WAL"
        );

        Ok(())
    }

    /// Recover from WAL on startup
    pub fn recover(&self) -> KeyValueResult<()> {
        info!("Starting WAL recovery");

        // Get head LSN (oldest available record)
        let head_lsn = self
            .wal
            .head_lsn()
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Create reader from head
        let mut reader = self
            .wal
            .reader_from(head_lsn)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        let mut recovered_count = 0;
        let mut last_checkpoint_seq = None;
        let mut last_checkpoint_file = None;

        // Replay all WAL entries
        loop {
            match reader.next() {
                Ok(Some(entry)) => {
                    // Decode and apply the operation
                    match WalRecordKind::from_u16(entry.kind) {
                        Some(WalRecordKind::Put) => match decode_put(&entry.payload) {
                            Ok((key, value)) => {
                                let memtable = self.memtable.read().unwrap();
                                memtable.put(key, value);
                                recovered_count += 1;
                            }
                            Err(e) => {
                                warn!(
                                    lsn = ?entry.lsn,
                                    error = %e,
                                    "Failed to decode Put record"
                                );
                            }
                        },
                        Some(WalRecordKind::Delete) => match decode_delete(&entry.payload) {
                            Ok(key) => {
                                let memtable = self.memtable.read().unwrap();
                                memtable.delete(key);
                                recovered_count += 1;
                            }
                            Err(e) => {
                                warn!(
                                    lsn = ?entry.lsn,
                                    error = %e,
                                    "Failed to decode Delete record"
                                );
                            }
                        },
                        Some(WalRecordKind::PutCommitted) => {
                            match decode_put_committed(&entry.payload) {
                                Ok((key, value, commit_ts)) => {
                                    let memtable = self.memtable.read().unwrap();
                                    memtable.put_committed(key, value, commit_ts);
                                    recovered_count += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        lsn = ?entry.lsn,
                                        error = %e,
                                        "Failed to decode PutCommitted record"
                                    );
                                }
                            }
                        }
                        Some(WalRecordKind::DeleteCommitted) => {
                            match decode_delete_committed(&entry.payload) {
                                Ok((key, commit_ts)) => {
                                    let memtable = self.memtable.read().unwrap();
                                    memtable.delete_committed(key, commit_ts);
                                    recovered_count += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        lsn = ?entry.lsn,
                                        error = %e,
                                        "Failed to decode DeleteCommitted record"
                                    );
                                }
                            }
                        }
                        Some(WalRecordKind::Commit) => {
                            match decode_commit(&entry.payload) {
                                Ok(commit_ts) => {
                                    debug!(commit_ts = commit_ts, "Transaction commit marker");
                                    // Commit markers are informational for recovery
                                    recovered_count += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        lsn = ?entry.lsn,
                                        error = %e,
                                        "Failed to decode Commit record"
                                    );
                                }
                            }
                        }
                        Some(WalRecordKind::Checkpoint) => {
                            match decode_checkpoint(&entry.payload) {
                                Ok((sequence, file_number)) => {
                                    info!(
                                        sequence = sequence,
                                        file_number = file_number,
                                        "Checkpoint marker found"
                                    );
                                    last_checkpoint_seq = Some(sequence);
                                    last_checkpoint_file = Some(file_number);
                                    recovered_count += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        lsn = ?entry.lsn,
                                        error = %e,
                                        "Failed to decode Checkpoint record"
                                    );
                                }
                            }
                        }
                        Some(WalRecordKind::FlushComplete) => {
                            match decode_flush_complete(&entry.payload) {
                                Ok((file_number, level)) => {
                                    debug!(
                                        file_number = file_number,
                                        level = level,
                                        "Flush complete marker"
                                    );
                                    // Flush markers help track which data is persisted
                                    recovered_count += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        lsn = ?entry.lsn,
                                        error = %e,
                                        "Failed to decode FlushComplete record"
                                    );
                                }
                            }
                        }
                        None => {
                            warn!(
                                kind = entry.kind,
                                lsn = ?entry.lsn,
                                "Unknown WAL record type"
                            );
                        }
                    }
                }
                Ok(None) => {
                    // End of WAL
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "Error reading WAL during recovery");
                    break;
                }
            }
        }

        if let (Some(seq), Some(file)) = (last_checkpoint_seq, last_checkpoint_file) {
            info!(
                sequence = seq,
                file_number = file,
                recovered_ops = recovered_count,
                "WAL recovery complete with checkpoint"
            );
        } else {
            info!(
                recovered_ops = recovered_count,
                "WAL recovery complete without checkpoint"
            );
        }

        Ok(())
    }

    /// Check if compaction is needed and trigger it
    fn maybe_compact(&self) -> KeyValueResult<()> {
        // Try to acquire compaction lock
        if self
            .compaction_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            // Compaction already in progress
            return Ok(());
        }

        let result = self.compact_if_needed();

        // Release compaction lock
        self.compaction_in_progress.store(false, Ordering::SeqCst);

        result
    }

    /// Perform compaction if needed
    fn compact_if_needed(&self) -> KeyValueResult<()> {
        let levels = self.levels.read().unwrap();

        // Check each level for compaction needs
        for (i, level) in levels.iter().enumerate() {
            let max_size = if i == 0 {
                0 // Level 0 uses file count
            } else {
                self.options.memtable_size as u64 * 10u64.pow(i as u32)
            };

            if level.needs_compaction(max_size) {
                drop(levels);
                return self.compact_level(i);
            }
        }

        Ok(())
    }

    /// Compact a specific level
    fn compact_level(&self, _level: usize) -> KeyValueResult<()> {
        self.total_compactions.fetch_add(1, Ordering::Relaxed);

        // Create compaction strategy
        let strategy = CompactionStrategy::default();

        // Get current levels snapshot
        let levels = self.levels.read().unwrap();
        let level_snapshots: Vec<Vec<SSTableMetadata>> =
            levels.iter().map(|l| l.sstables.clone()).collect();
        drop(levels);

        // Select compaction task
        let task =
            match strategy.select_compaction(&level_snapshots, self.options.memtable_size as u64) {
                Some(task) => task,
                None => return Ok(()), // No compaction needed
            };

        println!(
            "Starting compaction: Level {} -> Level {} ({} source files, {} target files)",
            task.source_level,
            task.target_level,
            task.source_sstables.len(),
            task.target_sstables.len()
        );

        // Create compaction executor
        let executor = CompactionExecutor::new(
            self.fs.clone(),
            self.base_path.clone(),
            self.options.block_size,
            self.options.compression,
            self.options.integrity,
        );

        // Execute compaction
        let mut next_file_number = self.next_file_number.load(Ordering::SeqCst);
        let new_sstables = executor.execute(&task, &mut next_file_number)?;
        self.next_file_number
            .store(next_file_number, Ordering::SeqCst);

        // Atomically update levels
        {
            let mut levels = self.levels.write().unwrap();

            // Remove old SSTables from source level
            for old_meta in &task.source_sstables {
                levels[task.source_level].remove_sstable(old_meta.file_number);
            }

            // Remove old SSTables from target level
            for old_meta in &task.target_sstables {
                levels[task.target_level].remove_sstable(old_meta.file_number);
            }

            // Add new SSTables to target level
            for new_meta in &new_sstables {
                levels[task.target_level].add_sstable(new_meta.clone());
            }
        }

        // Save updated manifest
        self.save_manifest()?;

        // Delete old SSTable files
        for old_meta in task
            .source_sstables
            .iter()
            .chain(task.target_sstables.iter())
        {
            let path = self.sstable_path(old_meta.file_number);
            // Ignore errors on deletion - files might already be gone
            let _ = self.fs.remove_file(&path);
        }

        println!(
            "Compaction complete: {} new files created at level {}",
            new_sstables.len(),
            task.target_level
        );

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> EngineStats {
        let levels = self.levels.read().unwrap();
        let level_stats: Vec<LevelStats> = levels
            .iter()
            .map(|level| LevelStats {
                level: level.level_number,
                num_sstables: level.sstables.len(),
                total_size: level.total_size,
            })
            .collect();

        EngineStats {
            memtable_size: self.memtable.read().unwrap().size(),
            immutable_memtable_size: self
                .immutable_memtable
                .read()
                .unwrap()
                .as_ref()
                .map_or(0, |m| m.size()),
            total_writes: self.total_writes.load(Ordering::Relaxed),
            total_reads: self.total_reads.load(Ordering::Relaxed),
            total_flushes: self.total_flushes.load(Ordering::Relaxed),
            total_compactions: self.total_compactions.load(Ordering::Relaxed),
            levels: level_stats,
        }
    }

    /// Force flush memtable
    pub fn flush(&self) -> KeyValueResult<()> {
        self.maybe_flush_memtable()
    }

    /// Force compaction
    pub fn compact(&self) -> KeyValueResult<()> {
        self.maybe_compact()
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub memtable_size: usize,
    pub immutable_memtable_size: usize,
    pub total_writes: u64,
    pub total_reads: u64,
    pub total_flushes: u64,
    pub total_compactions: u64,
    pub levels: Vec<LevelStats>,
}

/// Level statistics
#[derive(Debug, Clone)]
pub struct LevelStats {
    pub level: usize,
    pub num_sstables: usize,
    pub total_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::{MemoryFileSystem, Path};
    use nanograph_wal::WriteAheadLogConfig;
    use tempfile::TempDir;

    fn create_test_engine() -> (LSMTreeEngine, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_str().unwrap().to_string();

        // Create a memory filesystem for WAL and SSTables
        let wal_fs = MemoryFileSystem::new();
        let sstable_fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
        let wal_path = Path::from("/wal");

        // Use shard_id 0 for test engine
        let shard_id = 0;
        let wal_config = WriteAheadLogConfig::new(shard_id);
        let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config).unwrap();

        // Configure options with matching shard_id
        let options = LSMTreeOptions::default().with_shard_id(shard_id);
        let engine = LSMTreeEngine::new(sstable_fs, base_path, options, wal).unwrap();

        (engine, temp_dir)
    }

    #[test]
    fn test_engine_basic_operations() {
        let (engine, _temp_dir) = create_test_engine();

        // Test put and get
        engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        let value = engine.get(b"key1").unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test update
        engine.put(b"key1".to_vec(), b"value2".to_vec()).unwrap();
        let value = engine.get(b"key1").unwrap();
        assert_eq!(value, Some(b"value2".to_vec()));

        // Test delete
        engine.delete(b"key1".to_vec()).unwrap();
        let value = engine.get(b"key1").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_engine_stats() {
        let (engine, _temp_dir) = create_test_engine();

        engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        engine.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
        engine.get(b"key1").unwrap();

        let stats = engine.stats();
        assert_eq!(stats.total_writes, 2);
        assert_eq!(stats.total_reads, 1);
        assert!(stats.memtable_size > 0);
    }
}
