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

//! # Streaming snapshot format with compression + CRC
//!
//! ## Shapshot File Format
//! ```notrust
//! ╔═══════════════════╗
//! ║    File Header    ║
//! ╟───────────────────╢
//! ║ Snapshot Metadata ║
//! ╟───────────────────╢
//! ║   Data Chunk #1   ║
//! ╟───────────────────╢
//! ║   Data Chunk #2   ║
//! ╟───────────────────╢
//! ║       ....        ║
//! ╟───────────────────╢
//! ║   Data Chunk #N   ║
//! ╚═══════════════════╝
//! ```
//!
//! ### File Header
//!
//! | Field          | Type    | Size | Range     | Notes                                    |
//! |----------------|---------|------|-----------|------------------------------------------|
//! | magic          | u64     | 8    | 0x00-0x08 | 'SNAPSHOT'                               |
//! | version        | u16     | 2    | 0x08-0x0A | Version 1                                |
//! | shard_id       | u128    | 16   | 0x0A-0x1A | Tenant + Database + Table + Shard        |
//! | created_at     | u64     | 8    | 0x1A-0x22 | Creation Timestamp in MS since Epoch     |
//! | metadata_len   | u32     | 4    | 0x22-0x26 | Length of Raft Metadata following header |
//! | start_offset   | u64     | 8    | 0x26-0x2E | Starting Offset of First Chunk           |
//! | integrity      | u8      | 1    | 0x2E-0x2F | Integrity Algorithm for Chunk Data       |
//! | compression    | u8      | 1    | 0x2F-0x30 | Compression Algorithm for Chunk Data     |
//! | encryption     | u8      | 1    | 0x30-0x31 | Encryption Algorithm for Chunk Data      |
//! | encryption_key | u128    | 16   | 0x31-0x41 | Encryption Key ID if applicable          |
//! | reserved       | [u8;32] | 32   | 0x41-0x61 | Reserved for Future Use                  |
//! | checksum       | u32     | 4    | 0x61-0x65 | Checksum of All previous fields          |
//!
//! ### Consensus Metadata
//! ```notrust
//! SnapshotMeta {
//!     last_log_id: LogId {
//!         LeaderId {
//!             term: u64
//!             voted_for: Null | NodeId {
//!                 cluster: u32;
//!                 region: u32;
//!                 server: u64;
//!             }
//!         }
//!         Index: u64
//!     }
//!     last_membership: StoredMembership {
//!         log_id: Null | LogId {
//!             LeaderId {
//!                 term: u64
//!                 voted_for: Null | NodeId {
//!                     cluster: u32;
//!                     region: u32;
//!                     server: u64;
//!                 }
//!             }
//!         }
//!         membership: Membership {
//!             configs: List<
//!                 Set<
//!                     NodeId {
//!                         cluster: u32;
//!                         region: u32;
//!                         server: u64;
//!                     }
//!                 >
//!             >
//!             nodes: Map<NodeId, Node>
//!         }
//!     }
//!     snapshot_id: SnapshotId(String)
//! }
//! ```
//! ### Chunk Header
//!
//! | Field             | Type    | Size | Range     | Notes                      |
//! |-------------------|---------|------|-----------|----------------------------|
//! | magic             | u32     | 4    | 0x00-0x04 | 'CHNK'                     |
//! | record_count      | u32     | 4    | 0x04-0x08 | Records in the Chunk       |
//! | compressed_size   | u32     | 4    | 0x08-0x0C | Compressed Size of Chunk   |
//! | uncompressed_size | u32     | 4    | 0x0C-0x10 | Uncompressed Size of Chunk |
//! | checksum          | u32     | 4    | 0x10-0x14 | Chunk Checksum             |
//!
//!

use crate::types::ConsensusTypeConfig;
use byteorder::{BigEndian, ByteOrder};
use nanograph_kvt::{KeyValueError, KeyValueResult};
use nanograph_util::{
    CompressionAlgorithm, EncryptionAlgorithm, EncryptionKey, EncryptionKeyId, IntegrityAlgorithm,
    deserialize, serialize,
};
use nanograph_vfs::{DynamicFileSystem, File, Path};
use openraft::SnapshotMeta;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SnapshotManager {
    fs: Arc<dyn DynamicFileSystem>,
    path: Path,
    config: SnapshotConfig,
}

impl SnapshotManager {
    pub fn new(fs: Arc<dyn DynamicFileSystem>, path: Path) -> Self {
        Self {
            fs,
            path,
            config: SnapshotConfig::default(),
        }
    }

    pub fn with_config(fs: Arc<dyn DynamicFileSystem>, path: Path, config: SnapshotConfig) -> Self {
        Self { fs, path, config }
    }
    pub fn create_directory_all(&self, path: &str) -> KeyValueResult<()> {
        self.fs.create_directory_all(path).map_err(|e| e.into())
    }
    pub fn list_snapshots(&self) -> KeyValueResult<Vec<String>> {
        let files = self.fs.list_directory(&self.path.to_string())?;

        // Extract snapshot IDs from filenames (remove .snapshot extension)
        let snapshot_ids: Vec<String> = files
            .into_iter()
            .filter_map(|filename| {
                if filename.ends_with(".snapshot") {
                    Some(filename.trim_end_matches(".snapshot").to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(snapshot_ids)
    }
    pub fn delete_snapshot(&self, snapshot_id: &str) -> KeyValueResult<()> {
        let mut path = self.path.clone();
        path.push(format!("{}.snapshot", snapshot_id));
        self.fs.remove_file(&path.to_string()).map_err(|e| e.into())
    }
    pub fn snapshot_data(&self, snapshot_id: &str) -> KeyValueResult<SnapshotData<Box<dyn File>>> {
        let mut path = self.path.clone();
        path.push(&format!("{}.snapshot", snapshot_id));
        let file = self.fs.create_file(&path.to_string())?;
        Ok(SnapshotData::new(
            path,
            Box::new(file),
            snapshot_id.to_string(),
        ))
    }

    pub fn open_snapshot_reader(
        &self,
        snapshot_id: &str,
    ) -> KeyValueResult<(SnapshotReader<Box<dyn File>>, SnapshotFileMetadata)> {
        let mut path = self.path.clone();
        path.push(&format!("{}.snapshot", snapshot_id));
        let file = self.fs.open_file(&path.to_string())?;
        SnapshotReader::open(file).map_err(|e| e.into())
    }

    pub fn snapshot_reader(
        &self,
        snapshot_id: &str,
    ) -> KeyValueResult<(SnapshotReader<Box<dyn File>>, SnapshotFileMetadata)> {
        self.open_snapshot_reader(snapshot_id)
    }

    pub fn create_snapshot_writer(
        &self,
        metadata: SnapshotMeta<ConsensusTypeConfig>,
    ) -> KeyValueResult<SnapshotWriter<Box<dyn File>>> {
        let mut path = self.path.clone();
        path.push(&format!("{}.snapshot", &metadata.snapshot_id));
        let file = self.fs.create_file(&path.to_string())?;
        SnapshotWriter::new(self.config.clone(), metadata, file)
    }

    pub fn snapshot_writer(
        &self,
        metadata: SnapshotMeta<ConsensusTypeConfig>,
    ) -> KeyValueResult<SnapshotWriter<Box<dyn File>>> {
        self.create_snapshot_writer(metadata)
    }
}

pub struct SnapshotData<F: Read + Write> {
    path: Path,
    file: F,
    snapshot_id: String,
}

impl<F: Read + Write + Seek> SnapshotData<F> {
    pub fn new(path: Path, file: F, snapshot_id: String) -> Self {
        Self {
            path,
            file,
            snapshot_id,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn file(&self) -> &F {
        &self.file
    }
    pub fn file_mut(&mut self) -> &mut F {
        &mut self.file
    }
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }
    pub fn validate(&mut self) -> KeyValueResult<()> {
        self.file.seek(SeekFrom::Start(0))?;

        // Read and validate header
        let mut header = [0u8; FILE_HEADER_SIZE];
        self.file.read_exact(&mut header)?;

        // Check magic
        if BigEndian::read_u64(&header[FILE_RANGE_MAGIC]) != FILE_HEADER_MAGIC {
            return Err(KeyValueError::InvalidSnapshotFormat);
        }

        // Check version
        if BigEndian::read_u16(&header[FILE_RANGE_VERSION]) != FILE_HEADER_VERSION {
            return Err(KeyValueError::UnsupportedSnapshotVersion);
        }

        // Validate header integrity
        let header_checksum = BigEndian::read_u32(&header[FILE_RANGE_CHECKSUM]);
        let mut header_for_checksum = header;
        BigEndian::write_u32(&mut header_for_checksum[FILE_RANGE_CHECKSUM], 0);
        let calculated_checksum = IntegrityAlgorithm::Crc32c
            .hash(&header_for_checksum)
            .as_u32()
            .unwrap_or_default();
        if header_checksum != calculated_checksum {
            return Err(KeyValueError::StorageCorruption(
                "Invalid header checksum".to_string(),
            ));
        }

        // Read and validate metadata
        let metadata_len = BigEndian::read_u32(&header[FILE_RANGE_METADATA_LEN]) as usize;
        let mut metadata_buffer = vec![0u8; metadata_len];
        self.file.read_exact(&mut metadata_buffer)?;

        let metadata: SnapshotMeta<ConsensusTypeConfig> = deserialize(&metadata_buffer)
            .map_err(|e| KeyValueError::Deserialization(e.to_string()))?;

        // Validate snapshot ID matches
        if metadata.snapshot_id != self.snapshot_id {
            return Err(KeyValueError::StorageCorruption(format!(
                "Snapshot ID mismatch: expected {}, found {}",
                self.snapshot_id, metadata.snapshot_id
            )));
        }

        // Get configuration from header
        let integrity =
            IntegrityAlgorithm::from_u8(header[FILE_RANGE_INTEGRITY.start]).unwrap_or_default();
        let compression =
            CompressionAlgorithm::from_u8(header[FILE_RANGE_COMPRESSION.start]).unwrap_or_default();

        // Validate each chunk
        loop {
            let mut chunk_header = [0u8; CHUNK_HEADER_SIZE];
            match self.file.read_exact(&mut chunk_header) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }

            // Check chunk magic
            if BigEndian::read_u32(&chunk_header[CHUNK_RANGE_MAGIC]) != CHUNK_HEADER_MAGIC {
                return Err(KeyValueError::StorageCorruption(
                    "Invalid chunk magic".to_string(),
                ));
            }

            let compressed_size =
                BigEndian::read_u32(&chunk_header[CHUNK_RANGE_COMPRESSED_SIZE]) as usize;
            let chunk_checksum = BigEndian::read_u32(&chunk_header[CHUNK_RANGE_CHECKSUM]);

            // Read and validate chunk data
            let mut compressed_data = vec![0u8; compressed_size];
            self.file.read_exact(&mut compressed_data)?;

            let calculated_checksum = integrity
                .hash(&compressed_data)
                .as_u32()
                .unwrap_or_default();

            if chunk_checksum != calculated_checksum {
                return Err(KeyValueError::StorageCorruption(
                    "Invalid chunk checksum".to_string(),
                ));
            }
        }

        Ok(())
    }
}

const FILE_HEADER_SIZE: usize = FILE_RANGE_HEADER.end - FILE_RANGE_HEADER.start;
const FILE_HEADER_MAGIC: u64 = 0x53_4E_41_50_53_48_4F_54;
const FILE_HEADER_VERSION: u16 = 1;
const FILE_RANGE_HEADER: Range<usize> = 0..0x65;
const FILE_RANGE_MAGIC: Range<usize> = 0x00..0x08;
const FILE_RANGE_VERSION: Range<usize> = 0x08..0x0A;
const FILE_RANGE_SHARD_ID: Range<usize> = 0x0A..0x1A;
const FILE_RANGE_CREATED_AT: Range<usize> = 0x1A..0x22;
const FILE_RANGE_METADATA_LEN: Range<usize> = 0x22..0x26;
const FILE_RANGE_START_OFFSET: Range<usize> = 0x26..0x2E;
const FILE_RANGE_INTEGRITY: Range<usize> = 0x2E..0x2F;
const FILE_RANGE_COMPRESSION: Range<usize> = 0x2F..0x30;
const FILE_RANGE_ENCRYPTION: Range<usize> = 0x30..0x31;
const FILE_RANGE_ENCRYPTION_KEY_ID: Range<usize> = 0x31..0x41;
const FILE_RANGE_RESERVED: Range<usize> = 0x41..0x61;
const FILE_RANGE_CHECKSUM: Range<usize> = 0x61..0x65;
const CHUNK_HEADER_SIZE: usize = CHUNK_RANGE_HEADER.end - CHUNK_RANGE_HEADER.start;
const CHUNK_HEADER_MAGIC: u32 = 0x43_48_4E_4B;
const CHUNK_RANGE_HEADER: Range<usize> = 0x00..0x14;
const CHUNK_RANGE_MAGIC: Range<usize> = 0x00..0x04;
const CHUNK_RANGE_RECORD_COUNT: Range<usize> = 0x04..0x08;
const CHUNK_RANGE_COMPRESSED_SIZE: Range<usize> = 0x08..0x0C;
const CHUNK_RANGE_UNCOMPRESSED_SIZE: Range<usize> = 0x0C..0x10;
const CHUNK_RANGE_CHECKSUM: Range<usize> = 0x10..0x14;

#[derive(Clone, Debug)]
pub struct SnapshotConfig {
    pub integrity: IntegrityAlgorithm,
    pub compression: CompressionAlgorithm,
    pub encryption: EncryptionAlgorithm,
    #[allow(dead_code)]
    pub encryption_key: EncryptionKey,
    pub block_size: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            integrity: IntegrityAlgorithm::Crc32c,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key: EncryptionKey::default(),
            block_size: 1024 * 1024, // 1MB default block size
        }
    }
}

/// # Snapshot File Metadata
#[repr(C)]
#[derive(Clone, Debug)]
pub struct SnapshotFileMetadata {
    pub shard_id: u128,
    pub created_at: u64,
    pub integrity: IntegrityAlgorithm,
    pub compression: CompressionAlgorithm,
    pub encryption: EncryptionAlgorithm,
    pub encryption_key: EncryptionKeyId,
    pub metadata: SnapshotMeta<ConsensusTypeConfig>,
}

/// Snapshot Chunk Metadata
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SnapshotChunkMetadata {
    pub record_count: u32,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
}

/// Snapshot Writer
pub struct SnapshotWriter<W: Write + Seek> {
    config: SnapshotConfig,
    record_buffer: Vec<u8>,
    record_count: u32,
    writer: W,
}

impl<W: Write + Seek> SnapshotWriter<W> {
    pub fn new(
        config: SnapshotConfig,
        metadata: SnapshotMeta<ConsensusTypeConfig>,
        mut writer: W,
    ) -> KeyValueResult<Self> {
        // reserve header
        let mut header = [0u8; FILE_HEADER_SIZE];
        writer.write_all(&header)?;

        // serialize metadata
        let meta_bytes = serialize(&metadata)?;
        let start_offset = FILE_HEADER_SIZE + meta_bytes.len();
        writer.write_all(&meta_bytes)?;

        // write header to buffer
        BigEndian::write_u64(&mut header[FILE_RANGE_MAGIC], FILE_HEADER_MAGIC);
        BigEndian::write_u16(&mut header[FILE_RANGE_VERSION], FILE_HEADER_VERSION);
        BigEndian::write_u128(&mut header[FILE_RANGE_SHARD_ID], meta_bytes.len() as u128);
        BigEndian::write_u64(&mut header[FILE_RANGE_CREATED_AT], 0);
        BigEndian::write_u32(
            &mut header[FILE_RANGE_METADATA_LEN],
            meta_bytes.len() as u32,
        );
        BigEndian::write_u64(&mut header[FILE_RANGE_START_OFFSET], start_offset as u64);
        header[FILE_RANGE_INTEGRITY.start] = config.integrity.as_u8();
        header[FILE_RANGE_COMPRESSION.start] = config.compression.as_u8();
        header[FILE_RANGE_ENCRYPTION.start] = config.encryption.as_u8();
        BigEndian::write_u128(&mut header[FILE_RANGE_ENCRYPTION_KEY_ID], 0);

        // Calculate Checksum on buffer (Always use CRC32c)
        let mut header_for_checksum = header;
        BigEndian::write_u32(&mut header_for_checksum[FILE_RANGE_CHECKSUM], 0);
        let crc = IntegrityAlgorithm::Crc32c.hash(&header_for_checksum);
        BigEndian::write_u32(
            &mut header[FILE_RANGE_CHECKSUM],
            crc.as_u32().unwrap_or_default(),
        );

        // Write Header to writer
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&header)?;
        writer.seek(SeekFrom::End(0))?;

        let record_buffer = Vec::with_capacity(config.block_size);
        Ok(Self {
            config,
            record_buffer,
            record_count: 0,
            writer,
        })
    }

    pub fn write_kv(&mut self, key: &[u8], value: &[u8]) -> std::io::Result<()> {
        self.record_buffer
            .extend_from_slice(&(key.len() as u32).to_le_bytes());
        self.record_buffer
            .extend_from_slice(&(value.len() as u32).to_le_bytes());
        self.record_buffer.extend_from_slice(key);
        self.record_buffer.extend_from_slice(value);
        self.record_count += 1;

        if self.record_buffer.len() >= self.config.block_size {
            self.flush_block()?;
        }
        Ok(())
    }

    fn flush_block(&mut self) -> std::io::Result<()> {
        if self.record_count == 0 {
            return Ok(());
        }

        let compressed = self
            .config
            .compression
            .compress(&self.record_buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let integrity = self.config.integrity.hash(&compressed);

        // Write chunk header
        let mut header = [0; CHUNK_HEADER_SIZE];
        BigEndian::write_u32(&mut header[CHUNK_RANGE_MAGIC], CHUNK_HEADER_MAGIC);
        BigEndian::write_u32(&mut header[CHUNK_RANGE_RECORD_COUNT], self.record_count);
        BigEndian::write_u32(
            &mut header[CHUNK_RANGE_COMPRESSED_SIZE],
            compressed.len() as u32,
        );
        BigEndian::write_u32(
            &mut header[CHUNK_RANGE_UNCOMPRESSED_SIZE],
            self.record_buffer.len() as u32,
        );
        BigEndian::write_u32(
            &mut header[CHUNK_RANGE_CHECKSUM],
            integrity.as_u32().unwrap_or_default(),
        );

        // Write chunk header to writer
        self.writer.write_all(&header)?;

        // Write Compressed Chunk Data
        self.writer.write_all(&compressed)?;

        // Clear current data
        self.record_buffer.clear();
        self.record_count = 0;

        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.flush_block()?;
        self.writer.flush()?;
        Ok(())
    }
}

/// ==========================
/// Reader
/// ==========================

#[derive(Debug)]
pub struct SnapshotReader<R: Read + Seek> {
    config: SnapshotConfig,
    reader: R,
}

impl<R: Read + Seek> SnapshotReader<R> {
    pub fn open(mut reader: R) -> std::io::Result<(Self, SnapshotFileMetadata)> {
        // Read Header into buffer
        let mut header = [0; FILE_HEADER_SIZE];
        reader.read_exact(&mut header)?;

        // Validate Magic and Version
        if FILE_HEADER_MAGIC != BigEndian::read_u64(&mut header[FILE_RANGE_MAGIC]) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid file header magic",
            ));
        }
        if FILE_HEADER_VERSION != BigEndian::read_u16(&mut header[FILE_RANGE_VERSION]) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid file header version",
            ));
        }
        // Read Raft State Metadata
        let raft_metadata_len = BigEndian::read_u32(&mut header[FILE_RANGE_METADATA_LEN]);
        let mut raft_metadata_buffer = vec![0u8; raft_metadata_len as usize];
        reader.read_exact(&mut raft_metadata_buffer)?;

        // Check Header Integrity
        let header_checksum = BigEndian::read_u32(&header[FILE_RANGE_CHECKSUM]);
        let mut header_for_checksum = header;
        BigEndian::write_u32(&mut header_for_checksum[FILE_RANGE_CHECKSUM], 0);
        let calculated_checksum = IntegrityAlgorithm::Crc32c
            .hash(&header_for_checksum)
            .as_u32()
            .unwrap_or_default();
        if header_checksum != calculated_checksum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid file header checksum",
            ));
        }

        let metadata = SnapshotFileMetadata {
            shard_id: BigEndian::read_u128(&header[FILE_RANGE_SHARD_ID]),
            created_at: BigEndian::read_u64(&header[FILE_RANGE_CREATED_AT]),
            integrity: IntegrityAlgorithm::from_u8(header[FILE_RANGE_INTEGRITY.start])
                .unwrap_or_default(),
            compression: CompressionAlgorithm::from_u8(header[FILE_RANGE_COMPRESSION.start])
                .unwrap_or_default(),
            encryption: EncryptionAlgorithm::from_u8(header[FILE_RANGE_ENCRYPTION.start])
                .unwrap_or_default(),
            encryption_key: EncryptionKeyId::from(BigEndian::read_u128(
                &header[FILE_RANGE_ENCRYPTION_KEY_ID],
            )),
            metadata: deserialize(&raft_metadata_buffer)?,
        };

        let config = SnapshotConfig {
            integrity: metadata.integrity,
            compression: metadata.compression,
            encryption: metadata.encryption,
            // NOTE: Encryption key retrieval would require a key management system
            // For now, we use the default (no encryption). In production, this should
            // look up the key using metadata.encryption_key from a secure key store.
            encryption_key: EncryptionKey::default(),
            // Block Size isn't needed for Reading
            block_size: 0,
        };

        Ok((Self { config, reader }, metadata))
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    pub fn next_block(
        &mut self,
    ) -> std::io::Result<impl Iterator<Item = std::io::Result<(Vec<u8>, Vec<u8>)>>> {
        let mut blocks = Vec::new();

        // Read Header from Reader
        let mut header = [0; CHUNK_HEADER_SIZE];
        if let Err(e) = self.reader.read_exact(&mut header) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                // End of file, return an empty iterator or specific error
                // For this implementation, we'll return an error that can be checked by the caller
                return Err(e);
            }
            return Err(e);
        }

        // Check Chunk Magic
        if CHUNK_HEADER_MAGIC != BigEndian::read_u32(&mut header[CHUNK_RANGE_MAGIC]) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid chunk header magic",
            ));
        }

        let metadata = SnapshotChunkMetadata {
            record_count: BigEndian::read_u32(&header[CHUNK_RANGE_RECORD_COUNT]),
            compressed_size: BigEndian::read_u32(&header[CHUNK_RANGE_COMPRESSED_SIZE]),
            uncompressed_size: BigEndian::read_u32(&header[CHUNK_RANGE_UNCOMPRESSED_SIZE]),
        };

        // Read the compressed data
        let mut compressed = vec![0u8; metadata.compressed_size as usize];
        self.reader.read_exact(&mut compressed)?;

        let data_checksum = BigEndian::read_u32(&header[CHUNK_RANGE_CHECKSUM]);
        let calc_checksum = self
            .config
            .integrity
            .hash(&compressed)
            .as_u32()
            .unwrap_or_default();
        if data_checksum != calc_checksum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid chunk checksum",
            ));
        }

        let decompressed = self
            .config
            .compression
            .decompress(
                compressed.as_slice(),
                Some(metadata.uncompressed_size as usize),
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut ptr = decompressed.as_slice();

        for _ in 0..metadata.record_count {
            let key_len = u32::from_le_bytes(ptr[0..4].try_into().unwrap()) as usize;
            let val_len = u32::from_le_bytes(ptr[4..8].try_into().unwrap()) as usize;
            ptr = &ptr[8..];

            let key = ptr[..key_len].to_vec();
            ptr = &ptr[key_len..];

            let val = ptr[..val_len].to_vec();
            ptr = &ptr[val_len..];

            blocks.push(Ok((key, val)));
        }

        Ok(blocks.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::snapshot::SnapshotManager;
    use nanograph_core::object::NodeId;
    use nanograph_util::{
        CompressionAlgorithm, EncryptionAlgorithm, EncryptionKey, IntegrityAlgorithm,
    };
    use openraft::{SnapshotMeta, StoredMembership};

    #[test]
    fn test_snapshot_roundtrip() {
        use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
        use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
        use nanograph_core::object::NodeId;
        use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
        use openraft::vote::RaftLeaderId;
        let fs = std::sync::Arc::new(MemoryFileSystem::new());

        // Create snapshot directory
        fs.create_directory_all("/snapshots").unwrap();

        let config = SnapshotConfig {
            integrity: IntegrityAlgorithm::Crc32c,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key: EncryptionKey::default(),
            block_size: 1024,
        };
        let manager = SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

        let metadata = SnapshotMeta {
            last_log_id: Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(0)).to_committed(),
                10,
            )),
            last_membership: StoredMembership::new(
                Some(ConsensusLogId::new(
                    ConsensusLeaderId::new(1, NodeId::new(0)).to_committed(),
                    5,
                )),
                {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                },
            ),
            snapshot_id: "test-snapshot".to_string(),
        };

        // Write snapshot
        {
            let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
            writer.write_kv(b"key1", b"value1").unwrap();
            writer.write_kv(b"key2", b"value2").unwrap();
            writer.finish().unwrap();
        }

        // Read snapshot
        {
            let (mut reader, read_metadata) =
                manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
            assert_eq!(read_metadata.metadata.snapshot_id, metadata.snapshot_id);
            assert_eq!(read_metadata.metadata.last_log_id, metadata.last_log_id);

            let mut items = reader.next_block().unwrap();
            let (k1, v1) = items.next().unwrap().unwrap();
            assert_eq!(k1, b"key1");
            assert_eq!(v1, b"value1");

            let (k2, v2) = items.next().unwrap().unwrap();
            assert_eq!(k2, b"key2");
            assert_eq!(v2, b"value2");

            assert!(items.next().is_none());
        }
    }

    #[test]
    fn test_snapshot_multi_block() {
        use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
        use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
        use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
        use openraft::vote::RaftLeaderId;
        let fs = std::sync::Arc::new(MemoryFileSystem::new());

        // Create snapshot directory
        fs.create_directory_all("/snapshots").unwrap();

        let config = SnapshotConfig {
            integrity: IntegrityAlgorithm::Crc32c,
            compression: CompressionAlgorithm::Zstd,
            encryption: EncryptionAlgorithm::None,
            encryption_key: EncryptionKey::default(),
            block_size: 20, // Small block size to force multiple blocks
        };
        let manager = SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

        let metadata = SnapshotMeta {
            last_log_id: Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(0)).to_committed(),
                10,
            )),
            last_membership: StoredMembership::new(None, {
                let mut config = std::collections::BTreeSet::new();
                config.insert(NodeId::new(0));
                let mut nodes = std::collections::BTreeMap::new();
                nodes.insert(NodeId::new(0), NodeInfo::default());
                openraft::Membership::new(vec![config], nodes).expect("Failed to create membership")
            }),
            snapshot_id: "test-snapshot-multi".to_string(),
        };

        // Write snapshot
        {
            let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
            // Each KV is ~14 bytes (4 len + 4 len + 3 key + 3 val)
            writer.write_kv(b"k01", b"v01").unwrap();
            writer.write_kv(b"k02", b"v02").unwrap(); // Should trigger flush
            writer.write_kv(b"k03", b"v03").unwrap();
            writer.finish().unwrap();
        }

        // Read snapshot
        {
            let (mut reader, _read_metadata) =
                manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
            let mut all_kvs = Vec::new();

            // Read first block
            if let Ok(items) = reader.next_block() {
                for item in items {
                    let (k, v): (Vec<u8>, Vec<u8>) = item.unwrap();
                    all_kvs.push((k, v));
                }
            }

            // Read second block
            if let Ok(items) = reader.next_block() {
                for item in items {
                    let (k, v): (Vec<u8>, Vec<u8>) = item.unwrap();
                    all_kvs.push((k, v));
                }
            }

            assert_eq!(all_kvs.len(), 3);
            assert_eq!(all_kvs[0].0, b"k01");
            assert_eq!(all_kvs[1].0, b"k02");
            assert_eq!(all_kvs[2].0, b"k03");
        }
    }

    #[test]
    fn test_snapshot_compression_algorithms() {
        use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
        use crate::types::NodeInfo;
        use nanograph_core::object::NodeId;
        use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
        use openraft::{SnapshotMeta, StoredMembership};
        let fs = std::sync::Arc::new(MemoryFileSystem::new());

        // Create snapshot directory
        fs.create_directory_all("/snapshots").unwrap();

        let algos = vec![
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ];
        for algo in algos {
            let config = SnapshotConfig {
                integrity: IntegrityAlgorithm::Crc32c,
                compression: algo,
                encryption: EncryptionAlgorithm::None,
                encryption_key: EncryptionKey::default(),
                block_size: 1024,
            };
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: None,
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: format!("test-snapshot-{}", algo.as_u8()),
            };

            {
                let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                writer.write_kv(b"key", b"value").unwrap();
                writer.finish().unwrap();
            }

            {
                let (mut reader, _read_metadata) =
                    manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
                let mut items = reader.next_block().unwrap();
                let (k, v): (Vec<u8>, Vec<u8>) = items.next().unwrap().unwrap();
                assert_eq!(k, b"key");
                assert_eq!(v, b"value");
            }
        }
    }

    #[test]
    fn test_snapshot_manager_management() {
        use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
        use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
        let fs = std::sync::Arc::new(MemoryFileSystem::new());
        let config = SnapshotConfig {
            integrity: IntegrityAlgorithm::Crc32c,
            compression: CompressionAlgorithm::Lz4,
            encryption: EncryptionAlgorithm::None,
            encryption_key: EncryptionKey::default(),
            block_size: 1024,
        };
        let manager = SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

        manager.create_directory_all("/snapshots").unwrap();

        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 0);

        // Create snapshot files with .snapshot extension
        fs.create_file("/snapshots/snap1.snapshot").unwrap();
        fs.create_file("/snapshots/snap2.snapshot").unwrap();

        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);
        assert!(snapshots.contains(&"snap1".to_string()));
        assert!(snapshots.contains(&"snap2".to_string()));

        manager.delete_snapshot("snap1").unwrap();
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(!snapshots.contains(&"snap1".to_string()));
        assert!(snapshots.contains(&"snap2".to_string()));

        #[test]
        fn test_snapshot_config_default() {
            let config = SnapshotConfig::default();
            assert_eq!(config.integrity, IntegrityAlgorithm::Crc32c);
            assert_eq!(config.compression, CompressionAlgorithm::None);
            assert_eq!(config.encryption, EncryptionAlgorithm::None);
            assert_eq!(config.block_size, 1024 * 1024);
        }

        #[test]
        fn test_snapshot_config_custom() {
            let config = SnapshotConfig {
                integrity: IntegrityAlgorithm::Crc32c,
                compression: CompressionAlgorithm::Zstd,
                encryption: EncryptionAlgorithm::None,
                encryption_key: EncryptionKey::default(),
                block_size: 512 * 1024,
            };
            assert_eq!(config.block_size, 512 * 1024);
            assert_eq!(config.compression, CompressionAlgorithm::Zstd);
        }

        #[test]
        fn test_snapshot_writer_empty() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotWriter};
            use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
            use nanograph_vfs::MemoryFileSystem;
            use openraft::vote::RaftLeaderId;

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();
            let file = fs.create_file("/snapshots/empty.snapshot").unwrap();

            let config = SnapshotConfig::default();
            let metadata = SnapshotMeta {
                last_log_id: None,
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "empty".to_string(),
            };

            let writer = SnapshotWriter::new(config, metadata, file).unwrap();
            writer.finish().unwrap();
        }

        #[test]
        fn test_snapshot_writer_large_values() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
            use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
            use openraft::vote::RaftLeaderId;

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let config = SnapshotConfig {
                integrity: IntegrityAlgorithm::Crc32c,
                compression: CompressionAlgorithm::Lz4,
                encryption: EncryptionAlgorithm::None,
                encryption_key: EncryptionKey::default(),
                block_size: 1024,
            };
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: Some(ConsensusLogId::new(
                    ConsensusLeaderId::new(1, NodeId::new(0)).to_committed(),
                    100,
                )),
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "large-values".to_string(),
            };

            // Write large values
            {
                let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                let large_value = vec![0xAB; 2048]; // 2KB value
                writer.write_kv(b"large1", &large_value).unwrap();
                writer.write_kv(b"large2", &large_value).unwrap();
                writer.finish().unwrap();
            }

            // Read and verify
            {
                let (mut reader, _) = manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
                let mut count = 0;
                while let Ok(items) = reader.next_block() {
                    for item in items {
                        let (k, v) = item.unwrap();
                        assert!(k == b"large1" || k == b"large2");
                        assert_eq!(v.len(), 2048);
                        assert!(v.iter().all(|&b| b == 0xAB));
                        count += 1;
                    }
                }
                assert_eq!(count, 2);
            }
        }

        #[test]
        fn test_snapshot_reader_invalid_magic() {
            use crate::storage::snapshot::SnapshotReader;
            use std::io::Cursor;

            let invalid_data = vec![0u8; 100];
            let cursor = Cursor::new(invalid_data);

            match SnapshotReader::open(cursor) {
                Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::InvalidData),
                Ok(_) => panic!("Expected error for invalid magic"),
            }
        }

        #[test]
        fn test_snapshot_reader_invalid_version() {
            use crate::storage::snapshot::SnapshotReader;
            use byteorder::{BigEndian, ByteOrder};
            use std::io::Cursor;

            let mut data = vec![0u8; 100];
            // Write correct magic
            BigEndian::write_u64(&mut data[0..8], 0x53_4E_41_50_53_48_4F_54);
            // Write invalid version
            BigEndian::write_u16(&mut data[8..10], 999);

            let cursor = Cursor::new(data);
            match SnapshotReader::open(cursor) {
                Err(_) => {} // Expected
                Ok(_) => panic!("Expected error for invalid version"),
            }
        }

        #[test]
        fn test_snapshot_manager_list_snapshots() {
            use crate::storage::snapshot::SnapshotManager;
            use nanograph_vfs::{MemoryFileSystem, Path};

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();
            let manager = SnapshotManager::new(fs.clone(), Path::parse("/snapshots"));

            // Create some snapshot files
            fs.create_file("/snapshots/snap1.snapshot").unwrap();
            fs.create_file("/snapshots/snap2.snapshot").unwrap();
            fs.create_file("/snapshots/other.txt").unwrap(); // Should be ignored

            let snapshots = manager.list_snapshots().unwrap();
            assert_eq!(snapshots.len(), 2);
            assert!(snapshots.contains(&"snap1".to_string()));
            assert!(snapshots.contains(&"snap2".to_string()));
            assert!(!snapshots.contains(&"other.txt".to_string()));
        }

        #[test]
        fn test_snapshot_manager_delete() {
            use crate::storage::snapshot::SnapshotManager;
            use nanograph_vfs::{MemoryFileSystem, Path};

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();
            let manager = SnapshotManager::new(fs.clone(), Path::parse("/snapshots"));

            fs.create_file("/snapshots/test.snapshot").unwrap();
            assert_eq!(manager.list_snapshots().unwrap().len(), 1);

            manager.delete_snapshot("test").unwrap();
            assert_eq!(manager.list_snapshots().unwrap().len(), 0);
        }

        #[test]
        fn test_snapshot_data_validation() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotData, SnapshotManager};
            use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
            use openraft::vote::RaftLeaderId;

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let config = SnapshotConfig::default();
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: Some(ConsensusLogId::new(
                    ConsensusLeaderId::new(1, NodeId::new(0)).to_committed(),
                    10,
                )),
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "validate-test".to_string(),
            };

            // Write a valid snapshot
            {
                let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                writer.write_kv(b"key1", b"value1").unwrap();
                writer.finish().unwrap();
            }

            // Validate it
            {
                let file = fs.open_file("/snapshots/validate-test.snapshot").unwrap();
                let mut snapshot_data = SnapshotData::new(
                    Path::parse("/snapshots/validate-test.snapshot"),
                    file,
                    "validate-test".to_string(),
                );
                assert!(snapshot_data.validate().is_ok());
            }
        }

        #[test]
        fn test_snapshot_data_validation_wrong_id() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotData, SnapshotManager};
            use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};
            use openraft::vote::RaftLeaderId;

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let config = SnapshotConfig::default();
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: None,
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "correct-id".to_string(),
            };

            // Write a snapshot
            {
                let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                writer.write_kv(b"key1", b"value1").unwrap();
                writer.finish().unwrap();
            }

            // Try to validate with wrong ID
            {
                let file = fs.open_file("/snapshots/correct-id.snapshot").unwrap();
                let mut snapshot_data = SnapshotData::new(
                    Path::parse("/snapshots/correct-id.snapshot"),
                    file,
                    "wrong-id".to_string(),
                );
                let result = snapshot_data.validate();
                assert!(result.is_err());
            }
        }

        #[test]
        fn test_snapshot_integrity_algorithms() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
            use crate::types::NodeInfo;
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let algos = vec![IntegrityAlgorithm::Crc32c];

            for algo in algos {
                let config = SnapshotConfig {
                    integrity: algo,
                    compression: CompressionAlgorithm::None,
                    encryption: EncryptionAlgorithm::None,
                    encryption_key: EncryptionKey::default(),
                    block_size: 1024,
                };
                let manager =
                    SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

                let metadata = SnapshotMeta {
                    last_log_id: None,
                    last_membership: StoredMembership::new(None, {
                        let mut config = std::collections::BTreeSet::new();
                        config.insert(NodeId::new(0));
                        let mut nodes = std::collections::BTreeMap::new();
                        nodes.insert(NodeId::new(0), NodeInfo::default());
                        openraft::Membership::new(vec![config], nodes)
                            .expect("Failed to create membership")
                    }),
                    snapshot_id: format!("integrity-{}", algo.as_u8()),
                };

                {
                    let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                    writer.write_kv(b"test", b"data").unwrap();
                    writer.finish().unwrap();
                }

                {
                    let (mut reader, read_metadata) =
                        manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
                    assert_eq!(read_metadata.integrity, algo);
                    let mut items = reader.next_block().unwrap();
                    let (k, v) = items.next().unwrap().unwrap();
                    assert_eq!(k, b"test");
                    assert_eq!(v, b"data");
                }
            }
        }

        #[test]
        fn test_snapshot_empty_blocks() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
            use crate::types::NodeInfo;
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let config = SnapshotConfig::default();
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: None,
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "empty-blocks".to_string(),
            };

            // Write snapshot with no data
            {
                let writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                writer.finish().unwrap();
            }

            // Read should return EOF immediately
            {
                let (mut reader, _) = manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
                let result = reader.next_block();
                assert!(result.is_err());
            }
        }

        #[test]
        fn test_snapshot_many_small_kvs() {
            use crate::storage::snapshot::{SnapshotConfig, SnapshotManager};
            use crate::types::NodeInfo;
            use nanograph_vfs::{FileSystem, MemoryFileSystem, Path};

            let fs = std::sync::Arc::new(MemoryFileSystem::new());
            fs.create_directory_all("/snapshots").unwrap();

            let config = SnapshotConfig {
                integrity: IntegrityAlgorithm::Crc32c,
                compression: CompressionAlgorithm::Lz4,
                encryption: EncryptionAlgorithm::None,
                encryption_key: EncryptionKey::default(),
                block_size: 256, // Small blocks
            };
            let manager =
                SnapshotManager::with_config(fs.clone(), Path::parse("/snapshots"), config);

            let metadata = SnapshotMeta {
                last_log_id: None,
                last_membership: StoredMembership::new(None, {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(0));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(0), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                }),
                snapshot_id: "many-small".to_string(),
            };

            // Write many small KVs
            let count = 100;
            {
                let mut writer = manager.create_snapshot_writer(metadata.clone()).unwrap();
                for i in 0..count {
                    let key = format!("k{:03}", i);
                    let value = format!("v{:03}", i);
                    writer.write_kv(key.as_bytes(), value.as_bytes()).unwrap();
                }
                writer.finish().unwrap();
            }

            // Read and verify all
            {
                let (mut reader, _) = manager.open_snapshot_reader(&metadata.snapshot_id).unwrap();
                let mut read_count = 0;
                while let Ok(items) = reader.next_block() {
                    for item in items {
                        let (k, v) = item.unwrap();
                        let key_str = String::from_utf8(k).unwrap();
                        let val_str = String::from_utf8(v).unwrap();
                        assert!(key_str.starts_with("k"));
                        assert!(val_str.starts_with("v"));
                        read_count += 1;
                    }
                }
                assert_eq!(read_count, count);
            }
        }
    }
}
