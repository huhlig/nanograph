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

//! Serialization layer for index entries
//!
//! This module provides efficient serialization and deserialization of index entries
//! with versioning support for backward compatibility.

use crate::{IndexError, IndexResult};
use serde::{Deserialize, Serialize};

/// Current serialization format version
pub const CURRENT_VERSION: u8 = 1;

/// Serialized index entry with versioning
///
/// Format: [version:1][data_len:4][data:N]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedIndexEntry {
    /// Format version for backward compatibility
    pub version: u8,
    /// The indexed value (e.g., column value being indexed)
    pub indexed_value: Vec<u8>,
    /// The primary key of the table row
    pub primary_key: Vec<u8>,
    /// Optional included columns for covering indexes
    pub included_columns: Option<Vec<u8>>,
    /// Timestamp when entry was created (for MVCC)
    pub created_at: u64,
    /// Timestamp when entry was last modified
    pub modified_at: u64,
}

impl SerializedIndexEntry {
    /// Create a new serialized entry
    pub fn new(
        indexed_value: Vec<u8>,
        primary_key: Vec<u8>,
        included_columns: Option<Vec<u8>>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            version: CURRENT_VERSION,
            indexed_value,
            primary_key,
            included_columns,
            created_at: now,
            modified_at: now,
        }
    }

    /// Update the modification timestamp
    pub fn touch(&mut self) {
        self.modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}

/// Serialize an index entry to bytes
///
/// Uses bincode for efficient binary serialization with a version prefix.
///
/// # Arguments
/// * `indexed_value` - The value being indexed
/// * `primary_key` - The primary key of the row
/// * `included_columns` - Optional included columns for covering indexes
///
/// # Returns
/// * `Ok(Vec<u8>)` - Serialized bytes
/// * `Err(IndexError)` - Serialization error
pub fn serialize_entry(
    indexed_value: &[u8],
    primary_key: &[u8],
    included_columns: Option<&[u8]>,
) -> IndexResult<Vec<u8>> {
    let entry = SerializedIndexEntry::new(
        indexed_value.to_vec(),
        primary_key.to_vec(),
        included_columns.map(|c| c.to_vec()),
    );

    bincode::serialize(&entry).map_err(|e| IndexError::Serialization(e.to_string()))
}

/// Deserialize an index entry from bytes
///
/// Handles version checking and backward compatibility.
///
/// # Arguments
/// * `bytes` - Serialized entry bytes
///
/// # Returns
/// * `Ok(SerializedIndexEntry)` - Deserialized entry
/// * `Err(IndexError)` - Deserialization or version error
pub fn deserialize_entry(bytes: &[u8]) -> IndexResult<SerializedIndexEntry> {
    let entry: SerializedIndexEntry =
        bincode::deserialize(bytes).map_err(|e| IndexError::Serialization(e.to_string()))?;

    // Version compatibility check
    if entry.version > CURRENT_VERSION {
        return Err(IndexError::Serialization(format!(
            "Unsupported entry version: {} (current: {})",
            entry.version, CURRENT_VERSION
        )));
    }

    Ok(entry)
}

/// Serialize index metadata
///
/// Serializes index configuration and statistics for persistence.
///
/// # Arguments
/// * `metadata` - Index metadata to serialize
///
/// # Returns
/// * `Ok(Vec<u8>)` - Serialized bytes
/// * `Err(IndexError)` - Serialization error
pub fn serialize_metadata<T: Serialize>(metadata: &T) -> IndexResult<Vec<u8>> {
    bincode::serialize(metadata).map_err(|e| IndexError::Serialization(e.to_string()))
}

/// Deserialize index metadata
///
/// # Arguments
/// * `bytes` - Serialized metadata bytes
///
/// # Returns
/// * `Ok(T)` - Deserialized metadata
/// * `Err(IndexError)` - Deserialization error
pub fn deserialize_metadata<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> IndexResult<T> {
    bincode::deserialize(bytes).map_err(|e| IndexError::Serialization(e.to_string()))
}

/// Batch serialize multiple entries
///
/// More efficient than serializing entries individually.
///
/// # Arguments
/// * `entries` - Iterator of (indexed_value, primary_key, included_columns) tuples
///
/// # Returns
/// * `Ok(Vec<Vec<u8>>)` - Vector of serialized entries
/// * `Err(IndexError)` - Serialization error
pub fn batch_serialize_entries<'a, I>(entries: I) -> IndexResult<Vec<Vec<u8>>>
where
    I: Iterator<Item = (&'a [u8], &'a [u8], Option<&'a [u8]>)>,
{
    entries
        .map(|(indexed_value, primary_key, included_columns)| {
            serialize_entry(indexed_value, primary_key, included_columns)
        })
        .collect()
}

/// Batch deserialize multiple entries
///
/// # Arguments
/// * `bytes_vec` - Vector of serialized entry bytes
///
/// # Returns
/// * `Ok(Vec<SerializedIndexEntry>)` - Vector of deserialized entries
/// * `Err(IndexError)` - Deserialization error
pub fn batch_deserialize_entries(bytes_vec: &[Vec<u8>]) -> IndexResult<Vec<SerializedIndexEntry>> {
    bytes_vec.iter().map(|bytes| deserialize_entry(bytes)).collect()
}

/// Estimate serialized size of an entry
///
/// Useful for capacity planning and memory management.
///
/// # Arguments
/// * `indexed_value_len` - Length of indexed value
/// * `primary_key_len` - Length of primary key
/// * `included_columns_len` - Optional length of included columns
///
/// # Returns
/// * Estimated size in bytes
pub fn estimate_entry_size(
    indexed_value_len: usize,
    primary_key_len: usize,
    included_columns_len: Option<usize>,
) -> usize {
    // Version (1) + timestamps (16) + lengths (12) + data
    let overhead = 1 + 16 + 12;
    let data_size = indexed_value_len + primary_key_len + included_columns_len.unwrap_or(0);
    overhead + data_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_entry() {
        let indexed_value = b"test_value";
        let primary_key = b"pk_123";
        let included = Some(b"extra_data".as_slice());

        let serialized = serialize_entry(indexed_value, primary_key, included).unwrap();
        let deserialized = deserialize_entry(&serialized).unwrap();

        assert_eq!(deserialized.version, CURRENT_VERSION);
        assert_eq!(deserialized.indexed_value, indexed_value);
        assert_eq!(deserialized.primary_key, primary_key);
        assert_eq!(
            deserialized.included_columns.as_deref(),
            Some(b"extra_data".as_slice())
        );
    }

    #[test]
    fn test_serialize_without_included_columns() {
        let indexed_value = b"value";
        let primary_key = b"key";

        let serialized = serialize_entry(indexed_value, primary_key, None).unwrap();
        let deserialized = deserialize_entry(&serialized).unwrap();

        assert_eq!(deserialized.indexed_value, indexed_value);
        assert_eq!(deserialized.primary_key, primary_key);
        assert!(deserialized.included_columns.is_none());
    }

    #[test]
    fn test_batch_serialize_deserialize() {
        let entries = vec![
            (b"val1".as_slice(), b"pk1".as_slice(), None),
            (b"val2".as_slice(), b"pk2".as_slice(), Some(b"inc2".as_slice())),
            (b"val3".as_slice(), b"pk3".as_slice(), None),
        ];

        let serialized = batch_serialize_entries(entries.iter().copied()).unwrap();
        assert_eq!(serialized.len(), 3);

        let deserialized = batch_deserialize_entries(&serialized).unwrap();
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].indexed_value, b"val1");
        assert_eq!(deserialized[1].indexed_value, b"val2");
        assert_eq!(deserialized[2].indexed_value, b"val3");
    }

    #[test]
    fn test_estimate_entry_size() {
        let size = estimate_entry_size(10, 5, Some(20));
        assert!(size > 35); // At least the data size
        assert!(size < 100); // Reasonable overhead
    }

    #[test]
    fn test_version_compatibility() {
        let mut entry = SerializedIndexEntry::new(b"value".to_vec(), b"key".to_vec(), None);
        entry.version = CURRENT_VERSION + 1; // Future version

        let serialized = bincode::serialize(&entry).unwrap();
        let result = deserialize_entry(&serialized);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported entry version"));
    }

    #[test]
    fn test_touch_updates_timestamp() {
        let mut entry = SerializedIndexEntry::new(b"value".to_vec(), b"key".to_vec(), None);
        let original_modified = entry.modified_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        entry.touch();

        assert!(entry.modified_at > original_modified);
    }
}

// Made with Bob
