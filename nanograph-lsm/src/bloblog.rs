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

//! WiscKey-style blob log for large value separation
//!
//! This module implements value separation where large values (>4KB by default)
//! are stored in separate blob log files, while SSTables store only references
//! (blob_file_id, offset, length).
//!
//! ## Blob Log Format
//!
//! Each blob log file contains a sequence of blob records:
//!
//! ```text
//! [Record 1] [Record 2] ... [Record N]
//! ```
//!
//! ### Blob Record Format:
//!
//! ```text
//! [Magic: 4] [Key Length: 4] [Value Length: 4] [Key] [Value] [CRC32: 4]
//! ```
//!
//! - **Magic**: 0x424C4F42 ("BLOB") - identifies start of record
//! - **Key Length**: u32 little-endian - length of key in bytes
//! - **Value Length**: u32 little-endian - length of value in bytes
//! - **Key**: Variable length key data
//! - **Value**: Variable length value data
//! - **CRC32**: u32 little-endian - CRC32C checksum of entire record (excluding CRC32 itself)
//!
//! The CRC32 covers: [Magic][Key Length][Value Length][Key][Value]
//!
//! ## Garbage Collection
//!
//! Blob files are garbage collected when:
//! 1. A compaction removes all references to blobs in a file
//! 2. The file's live data ratio falls below a threshold (e.g., 50%)
//!
//! GC process:
//! 1. Identify blob files with low live ratio
//! 2. Read live blobs and write to new blob file
//! 3. Update SSTable references during compaction
//! 4. Delete old blob files after references are updated

use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, RwLock};

const BLOB_MAGIC: u32 = 0x424C4F42; // "BLOB"
const BLOB_RECORD_HEADER_SIZE: usize = 12; // magic(4) + key_len(4) + value_len(4)
const BLOB_RECORD_FOOTER_SIZE: usize = 4; // crc32(4)

/// Reference to a blob stored in a blob log file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    /// Blob file ID
    pub file_id: u64,
    /// Offset within the blob file
    pub offset: u64,
    /// Length of the blob record (including header and footer)
    pub length: u32,
}

impl BlobRef {
    /// Create a new blob reference
    pub fn new(file_id: u64, offset: u64, length: u32) -> Self {
        Self {
            file_id,
            offset,
            length,
        }
    }

    /// Calculate the size of the blob reference when serialized
    pub fn serialized_size() -> usize {
        8 + 8 + 4 // file_id + offset + length
    }
}

/// Metadata for a blob log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobFileMetadata {
    /// File ID
    pub file_id: u64,
    /// Total size of the file in bytes
    pub file_size: u64,
    /// Number of blob records in the file
    pub blob_count: u64,
    /// Number of live (referenced) blobs
    pub live_blob_count: u64,
    /// Total size of live blobs
    pub live_data_size: u64,
    /// Creation timestamp
    pub created_at: u64,
}

impl BlobFileMetadata {
    /// Calculate the live data ratio (0.0 to 1.0)
    pub fn live_ratio(&self) -> f64 {
        if self.file_size == 0 {
            return 0.0;
        }
        self.live_data_size as f64 / self.file_size as f64
    }

    /// Check if this file is a candidate for garbage collection
    pub fn needs_gc(&self, threshold: f64) -> bool {
        self.live_ratio() <= threshold && self.blob_count > 0
    }
}

/// Blob record as stored in the blob log
#[derive(Debug, Clone)]
pub struct BlobRecord {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

impl BlobRecord {
    /// Create a new blob record
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        Self { key, value }
    }

    /// Calculate the total size of this record when encoded
    pub fn encoded_size(&self) -> usize {
        BLOB_RECORD_HEADER_SIZE + self.key.len() + self.value.len() + BLOB_RECORD_FOOTER_SIZE
    }

    /// Encode the blob record to bytes
    pub fn encode(&self) -> Vec<u8> {
        let total_size = self.encoded_size();
        let mut buf = Vec::with_capacity(total_size);

        // Write header
        buf.extend_from_slice(&BLOB_MAGIC.to_le_bytes());
        buf.extend_from_slice(&(self.key.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.value.len() as u32).to_le_bytes());

        // Write key and value
        buf.extend_from_slice(&self.key);
        buf.extend_from_slice(&self.value);

        // Calculate and append CRC32
        let checksum = IntegrityAlgorithm::Crc32c.hash(&buf);
        if let IntegrityHash::Hash32(crc) = checksum {
            buf.extend_from_slice(&crc.to_le_bytes());
        }

        buf
    }

    /// Decode a blob record from bytes
    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() < BLOB_RECORD_HEADER_SIZE + BLOB_RECORD_FOOTER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Blob record too short",
            ));
        }

        // Split off CRC32 from end
        let (record_data, crc_bytes) = data.split_at(data.len() - BLOB_RECORD_FOOTER_SIZE);
        let stored_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        // Verify CRC32
        let calculated_checksum = IntegrityAlgorithm::Crc32c.hash(record_data);
        if let IntegrityHash::Hash32(calculated_crc) = calculated_checksum {
            if calculated_crc != stored_crc {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Blob record CRC32 mismatch: expected 0x{:08x}, got 0x{:08x}",
                        stored_crc, calculated_crc
                    ),
                ));
            }
        }

        // Parse header
        let magic = u32::from_le_bytes(record_data[0..4].try_into().unwrap());
        if magic != BLOB_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid blob magic: 0x{:08x}", magic),
            ));
        }

        let key_len = u32::from_le_bytes(record_data[4..8].try_into().unwrap()) as usize;
        let value_len = u32::from_le_bytes(record_data[8..12].try_into().unwrap()) as usize;

        if record_data.len() != BLOB_RECORD_HEADER_SIZE + key_len + value_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Blob record length mismatch",
            ));
        }

        // Extract key and value
        let key = record_data[BLOB_RECORD_HEADER_SIZE..BLOB_RECORD_HEADER_SIZE + key_len].to_vec();
        let value = record_data[BLOB_RECORD_HEADER_SIZE + key_len..].to_vec();

        Ok(Self { key, value })
    }
}

/// Blob log manager for handling large value separation
pub struct BlobLog {
    /// Next file ID to use
    next_file_id: Arc<RwLock<u64>>,
    /// Metadata for all blob files
    file_metadata: Arc<RwLock<HashMap<u64, BlobFileMetadata>>>,
    /// Set of blob references that are currently live (referenced by SSTables)
    live_refs: Arc<RwLock<HashSet<(u64, u64)>>>, // (file_id, offset)
    /// GC threshold (files with live ratio below this are candidates for GC)
    gc_threshold: f64,
}

impl BlobLog {
    /// Create a new blob log manager
    pub fn new(gc_threshold: f64) -> Self {
        Self {
            next_file_id: Arc::new(RwLock::new(0)),
            file_metadata: Arc::new(RwLock::new(HashMap::new())),
            live_refs: Arc::new(RwLock::new(HashSet::new())),
            gc_threshold,
        }
    }

    /// Allocate a new blob file ID
    pub fn allocate_file_id(&self) -> u64 {
        let mut next_id = self.next_file_id.write().unwrap();
        let file_id = *next_id;
        *next_id += 1;
        file_id
    }

    /// Write a blob record to a writer and return the blob reference
    pub fn write_blob<W: Write + Seek>(
        &self,
        writer: &mut W,
        file_id: u64,
        key: &[u8],
        value: &[u8],
    ) -> io::Result<BlobRef> {
        let record = BlobRecord::new(key.to_vec(), value.to_vec());
        let encoded = record.encode();
        let offset = writer.stream_position()?;

        writer.write_all(&encoded)?;

        // Update metadata
        let mut metadata = self.file_metadata.write().unwrap();
        let file_meta = metadata.entry(file_id).or_insert_with(|| BlobFileMetadata {
            file_id,
            file_size: 0,
            blob_count: 0,
            live_blob_count: 0,
            live_data_size: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        file_meta.file_size += encoded.len() as u64;
        file_meta.blob_count += 1;
        file_meta.live_blob_count += 1;
        file_meta.live_data_size += encoded.len() as u64;

        // Mark as live
        let mut live_refs = self.live_refs.write().unwrap();
        live_refs.insert((file_id, offset));

        Ok(BlobRef::new(file_id, offset, encoded.len() as u32))
    }

    /// Read a blob value using a blob reference
    pub fn read_blob<R: Read + Seek>(&self, reader: &mut R, blob_ref: &BlobRef) -> io::Result<Vec<u8>> {
        reader.seek(SeekFrom::Start(blob_ref.offset))?;

        let mut buf = vec![0u8; blob_ref.length as usize];
        reader.read_exact(&mut buf)?;

        let record = BlobRecord::decode(&buf)?;
        Ok(record.value)
    }

    /// Mark a blob reference as no longer live (called during compaction)
    pub fn mark_dead(&self, blob_ref: &BlobRef) {
        let mut live_refs = self.live_refs.write().unwrap();
        live_refs.remove(&(blob_ref.file_id, blob_ref.offset));

        // Update metadata
        let mut metadata = self.file_metadata.write().unwrap();
        if let Some(file_meta) = metadata.get_mut(&blob_ref.file_id) {
            file_meta.live_blob_count = file_meta.live_blob_count.saturating_sub(1);
            file_meta.live_data_size = file_meta.live_data_size.saturating_sub(blob_ref.length as u64);
        }
    }

    /// Get files that need garbage collection
    pub fn get_gc_candidates(&self) -> Vec<u64> {
        let metadata = self.file_metadata.read().unwrap();
        metadata
            .values()
            .filter(|meta| meta.needs_gc(self.gc_threshold))
            .map(|meta| meta.file_id)
            .collect()
    }

    /// Get metadata for a blob file
    pub fn get_file_metadata(&self, file_id: u64) -> Option<BlobFileMetadata> {
        let metadata = self.file_metadata.read().unwrap();
        metadata.get(&file_id).cloned()
    }

    /// Remove metadata for a deleted blob file
    pub fn remove_file(&self, file_id: u64) {
        let mut metadata = self.file_metadata.write().unwrap();
        metadata.remove(&file_id);

        // Remove all live refs for this file
        let mut live_refs = self.live_refs.write().unwrap();
        live_refs.retain(|(fid, _)| *fid != file_id);
    }

    /// Get total number of blob files
    pub fn file_count(&self) -> usize {
        let metadata = self.file_metadata.read().unwrap();
        metadata.len()
    }

    /// Get total size of all blob files
    pub fn total_size(&self) -> u64 {
        let metadata = self.file_metadata.read().unwrap();
        metadata.values().map(|m| m.file_size).sum()
    }

    /// Get total live data size across all blob files
    pub fn live_data_size(&self) -> u64 {
        let metadata = self.file_metadata.read().unwrap();
        metadata.values().map(|m| m.live_data_size).sum()
    }

    /// Get overall live data ratio
    pub fn overall_live_ratio(&self) -> f64 {
        let total = self.total_size();
        if total == 0 {
            return 0.0;
        }
        self.live_data_size() as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_blob_record_encode_decode() {
        let record = BlobRecord::new(b"test_key".to_vec(), b"test_value".to_vec());
        let encoded = record.encode();

        let decoded = BlobRecord::decode(&encoded).unwrap();
        assert_eq!(decoded.key, record.key);
        assert_eq!(decoded.value, record.value);
    }

    #[test]
    fn test_blob_record_crc_validation() {
        let record = BlobRecord::new(b"key".to_vec(), b"value".to_vec());
        let mut encoded = record.encode();

        // Corrupt the data
        encoded[20] ^= 0xFF;

        let result = BlobRecord::decode(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC32 mismatch"));
    }

    #[test]
    fn test_blob_log_write_read() {
        let blob_log = BlobLog::new(0.5);
        let mut writer = Cursor::new(Vec::new());

        let file_id = blob_log.allocate_file_id();
        let blob_ref = blob_log
            .write_blob(&mut writer, file_id, b"key1", b"value1")
            .unwrap();

        let mut reader = Cursor::new(writer.into_inner());
        let value = blob_log.read_blob(&mut reader, &blob_ref).unwrap();

        assert_eq!(value, b"value1");
    }

    #[test]
    fn test_blob_log_gc_candidates() {
        let blob_log = BlobLog::new(0.5);
        let mut writer = Cursor::new(Vec::new());

        let file_id = blob_log.allocate_file_id();
        let blob_ref1 = blob_log
            .write_blob(&mut writer, file_id, b"key1", b"value1")
            .unwrap();
        let _blob_ref2 = blob_log
            .write_blob(&mut writer, file_id, b"key2", b"value2")
            .unwrap();

        // Mark one blob as dead
        blob_log.mark_dead(&blob_ref1);

        // Check if file needs GC (should have ~50% live ratio)
        let metadata = blob_log.get_file_metadata(file_id).unwrap();
        assert!(metadata.live_ratio() < 0.6);
        assert!(metadata.live_ratio() > 0.4);

        let candidates = blob_log.get_gc_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], file_id);
    }

    #[test]
    fn test_blob_ref_serialization() {
        let blob_ref = BlobRef::new(42, 1024, 256);
        assert_eq!(blob_ref.file_id, 42);
        assert_eq!(blob_ref.offset, 1024);
        assert_eq!(blob_ref.length, 256);
    }
}

// Made with Bob
