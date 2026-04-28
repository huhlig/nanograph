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
use nanograph_util::{CompressionAlgorithm, IntegrityAlgorithm, IntegrityHash};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Seek, SeekFrom, Write};

const MAGIC_NUMBER: u64 = 0x4E414E4F4C534D54; // "NANOLSMT"
const VERSION: u32 = 1;
const RESTART_INTERVAL: usize = 16;

/// SSTable metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSTableMetadata {
    pub file_number: u64,
    pub level: usize,
    pub min_key: Vec<u8>,
    pub max_key: Vec<u8>,
    pub min_sequence: u64,
    pub max_sequence: u64,
    pub entry_count: u64,
    pub file_size: u64,
    pub created_at: u64,
}

/// Block handle - points to a block in the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHandle {
    pub offset: u64,
    pub size: u64,
}

/// SSTable footer (fixed size at end of file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Footer {
    pub meta_index_handle: BlockHandle,
    pub index_handle: BlockHandle,
    pub magic: u64,
    pub version: u32,
    pub checksum: u32,
}

impl Footer {
    pub const SIZE: usize = 48; // 8+8+8+8+8+4+4

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);
        buf.extend_from_slice(&self.meta_index_handle.offset.to_le_bytes());
        buf.extend_from_slice(&self.meta_index_handle.size.to_le_bytes());
        buf.extend_from_slice(&self.index_handle.offset.to_le_bytes());
        buf.extend_from_slice(&self.index_handle.size.to_le_bytes());
        buf.extend_from_slice(&self.magic.to_le_bytes());
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() != Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid footer size",
            ));
        }

        let meta_index_offset = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let meta_index_size = u64::from_le_bytes(data[8..16].try_into().unwrap());
        let index_offset = u64::from_le_bytes(data[16..24].try_into().unwrap());
        let index_size = u64::from_le_bytes(data[24..32].try_into().unwrap());
        let magic = u64::from_le_bytes(data[32..40].try_into().unwrap());
        let version = u32::from_le_bytes(data[40..44].try_into().unwrap());
        let checksum = u32::from_le_bytes(data[44..48].try_into().unwrap());

        if magic != MAGIC_NUMBER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number",
            ));
        }

        Ok(Self {
            meta_index_handle: BlockHandle {
                offset: meta_index_offset,
                size: meta_index_size,
            },
            index_handle: BlockHandle {
                offset: index_offset,
                size: index_size,
            },
            magic,
            version,
            checksum,
        })
    }
}

/// Data block containing key-value entries
#[derive(Debug, Clone)]
pub struct DataBlock {
    pub entries: Vec<Entry>,
    pub restart_points: Vec<u32>,
}

impl DataBlock {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            restart_points: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: Entry) {
        if self.entries.len() % RESTART_INTERVAL == 0 {
            self.restart_points.push(self.entries.len() as u32);
        }
        self.entries.push(entry);
    }

    pub fn encode(&self, compression: CompressionAlgorithm) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Encode entries with prefix compression
        let mut prev_key: Vec<u8> = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            let shared_len = if i % RESTART_INTERVAL == 0 {
                0
            } else {
                common_prefix_len(&prev_key, &entry.key)
            };

            let unshared_len = entry.key.len() - shared_len;
            let value_len = entry.value.as_ref().map_or(0, |v| v.len());

            // Write lengths
            write_varint(&mut buf, shared_len as u64)?;
            write_varint(&mut buf, unshared_len as u64)?;
            write_varint(&mut buf, value_len as u64)?;
            write_varint(&mut buf, entry.sequence)?;

            // Write unshared key
            buf.extend_from_slice(&entry.key[shared_len..]);

            // Write value (or tombstone marker)
            if let Some(ref value) = entry.value {
                buf.write_all(&[1])?; // Value present
                buf.write_all(value)?;
            } else {
                buf.write_all(&[0])?; // Tombstone
            }

            prev_key = entry.key.clone();
        }

        // Write restart points
        for point in &self.restart_points {
            buf.extend_from_slice(&point.to_le_bytes());
        }
        buf.extend_from_slice(&(self.restart_points.len() as u32).to_le_bytes());

        // Compress if needed
        let compressed = match compression {
            CompressionAlgorithm::None => buf,
            _ => compression
                .compress(&buf)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?,
        };

        // Build block with compression type and data
        let mut result = Vec::new();
        result.push(compression as u8);
        result.extend_from_slice(&compressed);

        // Calculate and append CRC32 checksum over compression type + data
        let checksum = IntegrityAlgorithm::Crc32c.hash(&result);
        if let nanograph_util::IntegrityHash::Hash32(crc) = checksum {
            result.extend_from_slice(&crc.to_le_bytes());
        }

        Ok(result)
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            // Minimum: 1 byte compression + 4 bytes CRC32
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Block data too short",
            ));
        }

        // Split off CRC32 from end
        let (block_data, crc_bytes) = data.split_at(data.len() - 4);
        let stored_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        // Verify CRC32 checksum
        let calculated_checksum = IntegrityAlgorithm::Crc32c.hash(block_data);
        if let nanograph_util::IntegrityHash::Hash32(calculated_crc) = calculated_checksum {
            if calculated_crc != stored_crc {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Block CRC32 mismatch: expected 0x{:08x}, got 0x{:08x}",
                        stored_crc, calculated_crc
                    ),
                ));
            }
        }

        let compression_byte = block_data[0];
        let compression = CompressionAlgorithm::from_u8(compression_byte).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid compression algorithm: {}", compression_byte),
            )
        })?;
        let compressed_data = &block_data[1..];

        // Decompress if needed
        let decompressed = match compression {
            CompressionAlgorithm::None => compressed_data.to_vec(),
            _ => compression
                .decompress(compressed_data, None)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?,
        };
        let decompressed = decompressed.as_slice();

        // Read restart points count
        if decompressed.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid block data",
            ));
        }

        let restart_count =
            u32::from_le_bytes(decompressed[decompressed.len() - 4..].try_into().unwrap()) as usize;
        let restart_offset = decompressed.len() - 4 - (restart_count * 4);

        // Read restart points
        let mut restart_points = Vec::with_capacity(restart_count);
        for i in 0..restart_count {
            let offset = restart_offset + i * 4;
            let point = u32::from_le_bytes(decompressed[offset..offset + 4].try_into().unwrap());
            restart_points.push(point);
        }

        // Decode entries
        let mut entries = Vec::new();
        let mut cursor = 0;
        let mut prev_key = Vec::new();

        while cursor < restart_offset {
            let (shared_len, n1) = read_varint(&decompressed[cursor..])?;
            cursor += n1;
            let (unshared_len, n2) = read_varint(&decompressed[cursor..])?;
            cursor += n2;
            let (value_len, n3) = read_varint(&decompressed[cursor..])?;
            cursor += n3;
            let (sequence, n4) = read_varint(&decompressed[cursor..])?;
            cursor += n4;

            // Reconstruct key
            let mut key = prev_key[..shared_len as usize].to_vec();
            key.extend_from_slice(&decompressed[cursor..cursor + unshared_len as usize]);
            cursor += unshared_len as usize;

            // Read value or tombstone
            let has_value = decompressed[cursor];
            cursor += 1;

            let value = if has_value == 1 {
                let v = decompressed[cursor..cursor + value_len as usize].to_vec();
                cursor += value_len as usize;
                Some(v)
            } else {
                None
            };

            entries.push(Entry::new(key.clone(), value, sequence));
            prev_key = key;
        }

        Ok(Self {
            entries,
            restart_points,
        })
    }
}

/// Index block - maps last key in each data block to its location
#[derive(Debug, Clone)]
pub struct IndexBlock {
    pub entries: Vec<(Vec<u8>, BlockHandle)>,
}

impl IndexBlock {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, last_key: Vec<u8>, handle: BlockHandle) {
        self.entries.push((last_key, handle));
    }

    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();

        for (key, handle) in &self.entries {
            write_varint(&mut buf, key.len() as u64)?;
            buf.write_all(key)?;
            write_varint(&mut buf, handle.offset)?;
            write_varint(&mut buf, handle.size)?;
        }

        // Calculate and append CRC32 checksum
        let checksum = IntegrityAlgorithm::Crc32c.hash(&buf);
        if let nanograph_util::IntegrityHash::Hash32(crc) = checksum {
            buf.extend_from_slice(&crc.to_le_bytes());
        }

        Ok(buf)
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Index block data too short",
            ));
        }

        // Split off CRC32 from end
        let (index_data, crc_bytes) = data.split_at(data.len() - 4);
        let stored_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        // Verify CRC32 checksum
        let calculated_checksum = IntegrityAlgorithm::Crc32c.hash(index_data);
        if let nanograph_util::IntegrityHash::Hash32(calculated_crc) = calculated_checksum {
            if calculated_crc != stored_crc {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Index block CRC32 mismatch: expected 0x{:08x}, got 0x{:08x}",
                        stored_crc, calculated_crc
                    ),
                ));
            }
        }

        let mut entries = Vec::new();
        let mut cursor = 0;

        while cursor < index_data.len() {
            let (key_len, n1) = read_varint(&index_data[cursor..])?;
            cursor += n1;

            let key = index_data[cursor..cursor + key_len as usize].to_vec();
            cursor += key_len as usize;

            let (offset, n2) = read_varint(&index_data[cursor..])?;
            cursor += n2;

            let (size, n3) = read_varint(&index_data[cursor..])?;
            cursor += n3;

            entries.push((key, BlockHandle { offset, size }));
        }

        Ok(Self { entries })
    }

    pub fn find_block(&self, key: &[u8]) -> Option<&BlockHandle> {
        // Binary search for the block containing the key
        let idx = self
            .entries
            .binary_search_by(|(k, _)| k.as_slice().cmp(key))
            .unwrap_or_else(|i| i);

        if idx < self.entries.len() {
            Some(&self.entries[idx].1)
        } else {
            None
        }
    }
}

/// Bloom filter for fast negative lookups
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u8>,
    num_hash_functions: usize,
}

impl BloomFilter {
    pub fn new(num_keys: usize, bits_per_key: usize) -> Self {
        let num_bits = num_keys * bits_per_key;
        let num_bytes = (num_bits + 7) / 8;
        let num_hash_functions = ((bits_per_key as f64 * 0.69) as usize).max(1).min(30);

        Self {
            bits: vec![0; num_bytes],
            num_hash_functions,
        }
    }

    pub fn add(&mut self, key: &[u8]) {
        let hash = hash_key(key);
        let delta = (hash >> 17) | (hash << 15);

        for i in 0..self.num_hash_functions {
            let bit_pos =
                hash.wrapping_add((i as u64).wrapping_mul(delta)) % (self.bits.len() as u64 * 8);
            self.bits[bit_pos as usize / 8] |= 1 << (bit_pos % 8);
        }
    }

    pub fn may_contain(&self, key: &[u8]) -> bool {
        let hash = hash_key(key);
        let delta = (hash >> 17) | (hash << 15);

        for i in 0..self.num_hash_functions {
            let bit_pos =
                hash.wrapping_add((i as u64).wrapping_mul(delta)) % (self.bits.len() as u64 * 8);
            if (self.bits[bit_pos as usize / 8] & (1 << (bit_pos % 8))) == 0 {
                return false;
            }
        }

        true
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.num_hash_functions as u8);
        buf.extend_from_slice(&self.bits);

        // Calculate and append CRC32 checksum
        let checksum = IntegrityAlgorithm::Crc32c.hash(&buf);
        if let nanograph_util::IntegrityHash::Hash32(crc) = checksum {
            buf.extend_from_slice(&crc.to_le_bytes());
        }

        buf
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            // Minimum: 1 byte num_hash_functions + 4 bytes CRC32
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Bloom filter data too short",
            ));
        }

        // Split off CRC32 from end
        let (bloom_data, crc_bytes) = data.split_at(data.len() - 4);
        let stored_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        // Verify CRC32 checksum
        let calculated_checksum = IntegrityAlgorithm::Crc32c.hash(bloom_data);
        if let nanograph_util::IntegrityHash::Hash32(calculated_crc) = calculated_checksum {
            if calculated_crc != stored_crc {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Bloom filter CRC32 mismatch: expected 0x{:08x}, got 0x{:08x}",
                        stored_crc, calculated_crc
                    ),
                ));
            }
        }

        let num_hash_functions = bloom_data[0] as usize;
        let bits = bloom_data[1..].to_vec();

        Ok(Self {
            bits,
            num_hash_functions,
        })
    }
}

/// SSTable - Sorted String Table
///
/// This struct provides static methods for creating and reading SSTables.
/// Paths are managed by the LSM engine using VFS, not stored here.
#[derive(Debug, Clone)]
pub struct SSTable {
    pub metadata: SSTableMetadata,
}

impl SSTable {
    /// Create a new SSTable from memtable entries
    pub fn create<W: Write + Seek>(
        writer: &mut W,
        entries: Vec<Entry>,
        file_number: u64,
        level: usize,
        block_size: usize,
        compression: CompressionAlgorithm,
        _integrity: IntegrityAlgorithm,
    ) -> io::Result<SSTableMetadata> {
        if entries.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot create SSTable from empty entries",
            ));
        }

        let mut current_offset = 0u64;

        // Build bloom filter
        let mut bloom_filter = BloomFilter::new(entries.len(), 10);
        for entry in &entries {
            bloom_filter.add(&entry.key);
        }

        // Write data blocks
        let mut index = IndexBlock::new();
        let mut current_block = DataBlock::new();
        let mut current_block_size = 0;

        for entry in &entries {
            let entry_size = entry.size();

            if current_block_size + entry_size > block_size && !current_block.entries.is_empty() {
                // Write current block
                let block_data = current_block.encode(compression)?;
                let block_handle = BlockHandle {
                    offset: current_offset,
                    size: block_data.len() as u64,
                };

                writer.write_all(&block_data)?;
                current_offset += block_data.len() as u64;

                // Add to index
                let last_key = current_block.entries.last().unwrap().key.clone();
                index.add_entry(last_key, block_handle);

                // Start new block
                current_block = DataBlock::new();
                current_block_size = 0;
            }

            current_block.add_entry(entry.clone());
            current_block_size += entry_size;
        }

        // Write last block
        if !current_block.entries.is_empty() {
            let block_data = current_block.encode(compression)?;
            let block_handle = BlockHandle {
                offset: current_offset,
                size: block_data.len() as u64,
            };

            writer.write_all(&block_data)?;
            current_offset += block_data.len() as u64;

            let last_key = current_block.entries.last().unwrap().key.clone();
            index.add_entry(last_key, block_handle);
        }

        // Write meta block (bloom filter)
        let bloom_data = bloom_filter.encode();
        let meta_index_handle = BlockHandle {
            offset: current_offset,
            size: bloom_data.len() as u64,
        };
        writer.write_all(&bloom_data)?;
        current_offset += bloom_data.len() as u64;

        // Write index block
        let index_data = index.encode()?;
        let index_handle = BlockHandle {
            offset: current_offset,
            size: index_data.len() as u64,
        };
        writer.write_all(&index_data)?;
        current_offset += index_data.len() as u64;

        // Note: Checksum calculation would require reading back the file,
        // which is not possible with a Write-only stream. For now, we set
        // checksum to 0. A proper implementation would need to either:
        // 1. Calculate checksum incrementally during writes
        // 2. Use a Read+Write+Seek stream
        // 3. Store checksums per-block instead of per-file
        let checksum = 0u32;

        // Write footer with checksum
        let footer = Footer {
            meta_index_handle,
            index_handle,
            magic: MAGIC_NUMBER,
            version: VERSION,
            checksum,
        };
        let footer_data = footer.encode();
        writer.write_all(&footer_data)?;
        current_offset += footer_data.len() as u64;

        let metadata = SSTableMetadata {
            file_number,
            level,
            min_key: entries.first().unwrap().key.clone(),
            max_key: entries.last().unwrap().key.clone(),
            min_sequence: entries.iter().map(|e| e.sequence).min().unwrap(),
            max_sequence: entries.iter().map(|e| e.sequence).max().unwrap(),
            entry_count: entries.len() as u64,
            file_size: current_offset,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        Ok(metadata)
    }

    /// Read an entry from an SSTable file
    pub fn get<R: Read + Seek>(
        reader: &mut R,
        key: &[u8],
        integrity: IntegrityAlgorithm,
    ) -> io::Result<Option<Entry>> {
        // Read footer
        reader.seek(SeekFrom::End(-(Footer::SIZE as i64)))?;
        let mut footer_data = vec![0u8; Footer::SIZE];
        reader.read_exact(&mut footer_data)?;
        let footer = Footer::decode(&footer_data)?;

        // Verify checksum if integrity checking is enabled
        if integrity != IntegrityAlgorithm::None && footer.checksum != 0 {
            // Calculate file size without footer
            let file_size = reader.seek(SeekFrom::End(0))? - Footer::SIZE as u64;

            // Read and hash all data before footer
            reader.seek(SeekFrom::Start(0))?;
            let mut hasher = integrity.hasher();
            let mut buffer = vec![0u8; 8192];
            let mut remaining = file_size;

            while remaining > 0 {
                let to_read = std::cmp::min(remaining as usize, buffer.len());
                let bytes_read = reader.read(&mut buffer[..to_read])?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
                remaining -= bytes_read as u64;
            }

            let calculated_checksum = match hasher.finalize() {
                IntegrityHash::Hash32(v) => v,
                IntegrityHash::None => 0,
            };

            if calculated_checksum != footer.checksum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Checksum mismatch: expected {}, got {}",
                        footer.checksum, calculated_checksum
                    ),
                ));
            }
        }

        // Read bloom filter
        reader.seek(SeekFrom::Start(footer.meta_index_handle.offset))?;
        let mut bloom_data = vec![0u8; footer.meta_index_handle.size as usize];
        reader.read_exact(&mut bloom_data)?;
        let bloom_filter = BloomFilter::decode(&bloom_data)?;

        // Check bloom filter first
        if !bloom_filter.may_contain(key) {
            return Ok(None);
        }

        // Read index
        reader.seek(SeekFrom::Start(footer.index_handle.offset))?;
        let mut index_data = vec![0u8; footer.index_handle.size as usize];
        reader.read_exact(&mut index_data)?;
        let index = IndexBlock::decode(&index_data)?;

        // Find the block containing the key
        let block_handle = match index.find_block(key) {
            Some(handle) => handle,
            None => return Ok(None),
        };

        // Read and decode the block
        reader.seek(SeekFrom::Start(block_handle.offset))?;
        let mut block_data = vec![0u8; block_handle.size as usize];
        reader.read_exact(&mut block_data)?;

        let block = DataBlock::decode(&block_data)?;

        // Binary search within the block
        Ok(block
            .entries
            .binary_search_by(|e| e.key.as_slice().cmp(key))
            .ok()
            .map(|idx| block.entries[idx].clone()))
    }
    /// Create an iterator over all entries in an SSTable
    /// Takes ownership of the reader
    pub fn iter<R: Read + Seek>(reader: R) -> io::Result<SSTableIterator<R>> {
        SSTableIterator::new(reader)
    }
}

/// Iterator over SSTable entries
pub struct SSTableIterator<R: Read + Seek> {
    reader: R,
    index: IndexBlock,
    current_block_idx: usize,
    current_block: Option<DataBlock>,
    current_entry_idx: usize,
}

impl<R: Read + Seek> SSTableIterator<R> {
    fn new(mut reader: R) -> io::Result<Self> {
        // Read footer
        reader.seek(SeekFrom::End(-(Footer::SIZE as i64)))?;
        let mut footer_data = vec![0u8; Footer::SIZE];
        reader.read_exact(&mut footer_data)?;
        let footer = Footer::decode(&footer_data)?;

        // Read index block
        reader.seek(SeekFrom::Start(footer.index_handle.offset))?;
        let mut index_data = vec![0u8; footer.index_handle.size as usize];
        reader.read_exact(&mut index_data)?;
        let index = IndexBlock::decode(&index_data)?;

        Ok(Self {
            reader,
            index,
            current_block_idx: 0,
            current_block: None,
            current_entry_idx: 0,
        })
    }

    fn load_next_block(&mut self) -> io::Result<bool> {
        if self.current_block_idx >= self.index.entries.len() {
            return Ok(false);
        }

        let block_handle = &self.index.entries[self.current_block_idx].1;

        // Read block data
        self.reader.seek(SeekFrom::Start(block_handle.offset))?;
        let mut block_data = vec![0u8; block_handle.size as usize];
        self.reader.read_exact(&mut block_data)?;

        // Decode block
        let block = DataBlock::decode(&block_data)?;
        self.current_block = Some(block);
        self.current_entry_idx = 0;
        self.current_block_idx += 1;

        Ok(true)
    }
}

impl<R: Read + Seek> Iterator for SSTableIterator<R> {
    type Item = io::Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Check if we have a current block
            if let Some(ref block) = self.current_block {
                // Check if there are more entries in current block
                if self.current_entry_idx < block.entries.len() {
                    let entry = block.entries[self.current_entry_idx].clone();
                    self.current_entry_idx += 1;
                    return Some(Ok(entry));
                }
            }

            // Need to load next block
            match self.load_next_block() {
                Ok(true) => continue,          // Successfully loaded next block
                Ok(false) => return None,      // No more blocks
                Err(e) => return Some(Err(e)), // Error loading block
            }
        }
    }
}

// Helper functions

fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    let min_len = a.len().min(b.len());
    for i in 0..min_len {
        if a[i] != b[i] {
            return i;
        }
    }
    min_len
}

fn write_varint<W: Write>(writer: &mut W, mut value: u64) -> io::Result<()> {
    while value >= 0x80 {
        writer.write_all(&[(value as u8) | 0x80])?;
        value >>= 7;
    }
    writer.write_all(&[value as u8])?;
    Ok(())
}

fn read_varint(data: &[u8]) -> io::Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut bytes_read = 0;

    for &byte in data {
        bytes_read += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, bytes_read));
        }
        shift += 7;
        if shift >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Varint too large",
            ));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "Incomplete varint",
    ))
}

fn hash_key(key: &[u8]) -> u64 {
    // Simple FNV-1a hash
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in key {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encoding() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 0).unwrap();
        assert_eq!(buf, vec![0]);

        buf.clear();
        write_varint(&mut buf, 127).unwrap();
        assert_eq!(buf, vec![127]);

        buf.clear();
        write_varint(&mut buf, 128).unwrap();
        assert_eq!(buf, vec![0x80, 0x01]);

        buf.clear();
        write_varint(&mut buf, 16384).unwrap();
        assert_eq!(buf, vec![0x80, 0x80, 0x01]);
    }

    #[test]
    fn test_varint_decoding() {
        assert_eq!(read_varint(&[0]).unwrap(), (0, 1));
        assert_eq!(read_varint(&[127]).unwrap(), (127, 1));
        assert_eq!(read_varint(&[0x80, 0x01]).unwrap(), (128, 2));
        assert_eq!(read_varint(&[0x80, 0x80, 0x01]).unwrap(), (16384, 3));
    }

    #[test]
    fn test_bloom_filter() {
        let mut bloom = BloomFilter::new(100, 10);

        bloom.add(b"key1");
        bloom.add(b"key2");
        bloom.add(b"key3");

        assert!(bloom.may_contain(b"key1"));
        assert!(bloom.may_contain(b"key2"));
        assert!(bloom.may_contain(b"key3"));

        // May have false positives, but should mostly be negative
        let false_positives = (0..100)
            .filter(|i| bloom.may_contain(format!("nonexistent{}", i).as_bytes()))
            .count();

        assert!(false_positives < 10); // Should be around 1% with 10 bits per key
    }

    #[test]
    fn test_sstable_create_and_read() {
        use std::io::Cursor;

        let entries = vec![
            Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
            Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
            Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        ];

        let mut buffer = Cursor::new(Vec::new());
        let metadata = SSTable::create(
            &mut buffer,
            entries,
            1,
            0,
            4096,
            CompressionAlgorithm::None,
            IntegrityAlgorithm::None,
        )
        .unwrap();

        assert_eq!(metadata.entry_count, 3);
        assert_eq!(metadata.min_key, b"key1");
        assert_eq!(metadata.max_key, b"key3");

        // Test reading
        buffer.set_position(0);
        let entry = SSTable::get(&mut buffer, b"key2", IntegrityAlgorithm::None)
            .unwrap()
            .unwrap();
        assert_eq!(entry.key, b"key2");
        assert_eq!(entry.value.as_ref().unwrap(), b"value2");

        // Test non-existent key
        buffer.set_position(0);
        let entry = SSTable::get(&mut buffer, b"key4", IntegrityAlgorithm::None).unwrap();
        assert!(entry.is_none());
    #[test]
    fn test_block_crc32_verification() {
        // Test that blocks with valid CRC32 checksums are accepted
        let mut block = DataBlock::new();
        block.add_entry(Entry::new(b"test_key".to_vec(), Some(b"test_value".to_vec()), 1));
        
        let encoded = block.encode(CompressionAlgorithm::None).unwrap();
        let decoded = DataBlock::decode(&encoded).unwrap();
        
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].key, b"test_key");
        assert_eq!(decoded.entries[0].value.as_ref().unwrap(), b"test_value");
    }

    #[test]
    fn test_corrupted_data_block_detection() {
        // Create a valid block
        let mut block = DataBlock::new();
        block.add_entry(Entry::new(b"key".to_vec(), Some(b"value".to_vec()), 1));
        
        let mut encoded = block.encode(CompressionAlgorithm::None).unwrap();
        
        // Corrupt a byte in the middle of the block (but not the CRC32 at the end)
        if encoded.len() > 10 {
            encoded[5] ^= 0xFF; // Flip all bits in one byte
        }
        
        // Decoding should fail due to CRC32 mismatch
        let result = DataBlock::decode(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC32 mismatch"));
    }

    #[test]
    fn test_index_block_crc32_verification() {
        let mut index = IndexBlock::new();
        index.add_entry(b"key1".to_vec(), BlockHandle { offset: 0, size: 100 });
        index.add_entry(b"key2".to_vec(), BlockHandle { offset: 100, size: 150 });
        
        let encoded = index.encode().unwrap();
        let decoded = IndexBlock::decode(&encoded).unwrap();
        
        assert_eq!(decoded.entries.len(), 2);
        assert_eq!(decoded.entries[0].0, b"key1");
        assert_eq!(decoded.entries[1].0, b"key2");
    }

    #[test]
    fn test_corrupted_index_block_detection() {
        let mut index = IndexBlock::new();
        index.add_entry(b"key".to_vec(), BlockHandle { offset: 0, size: 100 });
        
        let mut encoded = index.encode().unwrap();
        
        // Corrupt a byte (but not the CRC32 at the end)
        if encoded.len() > 10 {
            encoded[3] ^= 0xFF;
        }
        
        let result = IndexBlock::decode(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC32 mismatch"));
    }

    #[test]
    fn test_bloom_filter_crc32_verification() {
        let mut bloom = BloomFilter::new(10, 10);
        bloom.add(b"test_key");
        
        let encoded = bloom.encode();
        let decoded = BloomFilter::decode(&encoded).unwrap();
        
        assert!(decoded.may_contain(b"test_key"));
    }

    #[test]
    fn test_corrupted_bloom_filter_detection() {
        let mut bloom = BloomFilter::new(10, 10);
        bloom.add(b"key");
        
        let mut encoded = bloom.encode();
        
        // Corrupt a byte (but not the CRC32 at the end)
        if encoded.len() > 10 {
            encoded[2] ^= 0xFF;
        }
        
        let result = BloomFilter::decode(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC32 mismatch"));
    }
    }
}
