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

//! WAL record types for B+Tree operations

use crate::error::{BTreeError, BTreeResult};

/// WAL record types for B+Tree operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum WalRecordKind {
    /// Put operation: key-value pair
    Put = 1,
    /// Delete operation: key only
    Delete = 2,
    /// Checkpoint: marks a consistent state
    Checkpoint = 3,
}

impl WalRecordKind {
    /// Convert from u16
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Put),
            2 => Some(Self::Delete),
            3 => Some(Self::Checkpoint),
            _ => None,
        }
    }

    /// Convert to u16
    pub fn to_u16(self) -> u16 {
        self as u16
    }
}

/// Encode a Put operation into WAL payload format
/// Format: [key_len: u32][key: bytes][value_len: u32][value: bytes]
pub fn encode_put(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + key.len() + value.len());

    // Write key length and key
    payload.extend_from_slice(&(key.len() as u32).to_le_bytes());
    payload.extend_from_slice(key);

    // Write value length and value
    payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
    payload.extend_from_slice(value);

    payload
}

/// Decode a Put operation from WAL payload
/// Returns (key, value)
pub fn decode_put(payload: &[u8]) -> BTreeResult<(Vec<u8>, Vec<u8>)> {
    if payload.len() < 8 {
        return Err(BTreeError::Internal("WAL Put record too short".to_string()));
    }

    let mut offset = 0;

    // Read key length
    let key_len = u32::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ]) as usize;
    offset += 4;

    if offset + key_len > payload.len() {
        return Err(BTreeError::Internal(
            "WAL Put record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[offset..offset + key_len].to_vec();
    offset += key_len;

    if offset + 4 > payload.len() {
        return Err(BTreeError::Internal(
            "WAL Put record missing value length".to_string(),
        ));
    }

    // Read value length
    let value_len = u32::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ]) as usize;
    offset += 4;

    if offset + value_len > payload.len() {
        return Err(BTreeError::Internal(
            "WAL Put record value length exceeds payload".to_string(),
        ));
    }

    // Read value
    let value = payload[offset..offset + value_len].to_vec();

    Ok((key, value))
}

/// Encode a Delete operation into WAL payload format
/// Format: [key_len: u32][key: bytes]
pub fn encode_delete(key: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + key.len());

    // Write key length and key
    payload.extend_from_slice(&(key.len() as u32).to_le_bytes());
    payload.extend_from_slice(key);

    payload
}

/// Decode a Delete operation from WAL payload
/// Returns key
pub fn decode_delete(payload: &[u8]) -> BTreeResult<Vec<u8>> {
    if payload.len() < 4 {
        return Err(BTreeError::Internal(
            "WAL Delete record too short".to_string(),
        ));
    }

    // Read key length
    let key_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if 4 + key_len > payload.len() {
        return Err(BTreeError::Internal(
            "WAL Delete record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[4..4 + key_len].to_vec();

    Ok(key)
}

/// Encode a Checkpoint operation into WAL payload format
///
/// Checkpoint records have no payload, just a marker
pub fn encode_checkpoint() -> Vec<u8> {
    Vec::new()
}

/// Decode a Checkpoint operation from WAL payload format
///
/// Checkpoint records have no payload
pub fn decode_checkpoint(_payload: &[u8]) -> BTreeResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_put() {
        let key = b"test_key";
        let value = b"test_value";

        let payload = encode_put(key, value);
        let (decoded_key, decoded_value) = decode_put(&payload).unwrap();

        assert_eq!(decoded_key, key);
        assert_eq!(decoded_value, value);
    }

    #[test]
    fn test_encode_decode_delete() {
        let key = b"test_key";

        let payload = encode_delete(key);
        let decoded_key = decode_delete(&payload).unwrap();

        assert_eq!(decoded_key, key);
    }

    #[test]
    fn test_decode_put_invalid() {
        // Too short
        assert!(decode_put(&[1, 2, 3]).is_err());

        // Key length exceeds payload
        let mut payload = vec![0, 0, 0, 100]; // key_len = 100
        payload.extend_from_slice(b"short");
        assert!(decode_put(&payload).is_err());
    }

    #[test]
    fn test_decode_delete_invalid() {
        // Too short
        assert!(decode_delete(&[1, 2]).is_err());

        // Key length exceeds payload
        let mut payload = vec![0, 0, 0, 100]; // key_len = 100
        payload.extend_from_slice(b"short");
        assert!(decode_delete(&payload).is_err());
    }

    #[test]
    fn test_record_kind_conversion() {
        assert_eq!(WalRecordKind::from_u16(1), Some(WalRecordKind::Put));
        assert_eq!(WalRecordKind::from_u16(2), Some(WalRecordKind::Delete));
        assert_eq!(WalRecordKind::from_u16(3), Some(WalRecordKind::Checkpoint));
        assert_eq!(WalRecordKind::from_u16(99), None);

        assert_eq!(WalRecordKind::Put.to_u16(), 1);
        assert_eq!(WalRecordKind::Delete.to_u16(), 2);
        assert_eq!(WalRecordKind::Checkpoint.to_u16(), 3);
    }

    #[test]
    fn test_checkpoint_encoding() {
        let payload = encode_checkpoint();
        assert!(payload.is_empty());
        assert!(decode_checkpoint(&payload).is_ok());
    }
}
