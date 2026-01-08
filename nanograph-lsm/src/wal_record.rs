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

//! WAL record types for LSM Tree operations

use nanograph_kvt::KeyValueResult;

/// WAL record types for LSM Tree operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum WalRecordKind {
    /// Put operation: key-value pair (uncommitted)
    Put = 1,
    /// Delete operation: key only (uncommitted)
    Delete = 2,
    /// Put operation with commit timestamp (committed)
    PutCommitted = 3,
    /// Delete operation with commit timestamp (committed)
    DeleteCommitted = 4,
    /// Transaction commit marker
    Commit = 5,
    /// Checkpoint marker with memtable sequence number
    Checkpoint = 6,
    /// Memtable flush completion marker
    FlushComplete = 7,
}

impl WalRecordKind {
    /// Convert from u16
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Put),
            2 => Some(Self::Delete),
            3 => Some(Self::PutCommitted),
            4 => Some(Self::DeleteCommitted),
            5 => Some(Self::Commit),
            6 => Some(Self::Checkpoint),
            7 => Some(Self::FlushComplete),
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
pub fn decode_put(payload: &[u8]) -> KeyValueResult<(Vec<u8>, Vec<u8>)> {
    if payload.len() < 8 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Put record too short".to_string(),
        ));
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
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Put record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[offset..offset + key_len].to_vec();
    offset += key_len;

    if offset + 4 > payload.len() {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
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
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
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
pub fn decode_delete(payload: &[u8]) -> KeyValueResult<Vec<u8>> {
    if payload.len() < 4 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Delete record too short".to_string(),
        ));
    }

    // Read key length
    let key_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if 4 + key_len > payload.len() {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Delete record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[4..4 + key_len].to_vec();

    Ok(key)
}

/// Encode a PutCommitted operation into WAL payload format
/// Format: [key_len: u32][key: bytes][value_len: u32][value: bytes][commit_ts: i64]
pub fn encode_put_committed(key: &[u8], value: &[u8], commit_ts: i64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + key.len() + value.len());

    // Write key length and key
    payload.extend_from_slice(&(key.len() as u32).to_le_bytes());
    payload.extend_from_slice(key);

    // Write value length and value
    payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
    payload.extend_from_slice(value);

    // Write commit timestamp
    payload.extend_from_slice(&commit_ts.to_le_bytes());

    payload
}

/// Decode a PutCommitted operation from WAL payload
/// Returns (key, value, commit_ts)
pub fn decode_put_committed(payload: &[u8]) -> KeyValueResult<(Vec<u8>, Vec<u8>, i64)> {
    if payload.len() < 16 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL PutCommitted record too short".to_string(),
        ));
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
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL PutCommitted record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[offset..offset + key_len].to_vec();
    offset += key_len;

    if offset + 4 > payload.len() {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL PutCommitted record missing value length".to_string(),
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

    if offset + value_len + 8 > payload.len() {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL PutCommitted record value length exceeds payload".to_string(),
        ));
    }

    // Read value
    let value = payload[offset..offset + value_len].to_vec();
    offset += value_len;

    // Read commit timestamp
    let commit_ts = i64::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
        payload[offset + 4],
        payload[offset + 5],
        payload[offset + 6],
        payload[offset + 7],
    ]);

    Ok((key, value, commit_ts))
}

/// Encode a DeleteCommitted operation into WAL payload format
/// Format: [key_len: u32][key: bytes][commit_ts: i64]
pub fn encode_delete_committed(key: &[u8], commit_ts: i64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(12 + key.len());

    // Write key length and key
    payload.extend_from_slice(&(key.len() as u32).to_le_bytes());
    payload.extend_from_slice(key);

    // Write commit timestamp
    payload.extend_from_slice(&commit_ts.to_le_bytes());

    payload
}

/// Decode a DeleteCommitted operation from WAL payload
/// Returns (key, commit_ts)
pub fn decode_delete_committed(payload: &[u8]) -> KeyValueResult<(Vec<u8>, i64)> {
    if payload.len() < 12 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL DeleteCommitted record too short".to_string(),
        ));
    }

    // Read key length
    let key_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if 4 + key_len + 8 > payload.len() {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL DeleteCommitted record key length exceeds payload".to_string(),
        ));
    }

    // Read key
    let key = payload[4..4 + key_len].to_vec();

    // Read commit timestamp
    let offset = 4 + key_len;
    let commit_ts = i64::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
        payload[offset + 4],
        payload[offset + 5],
        payload[offset + 6],
        payload[offset + 7],
    ]);

    Ok((key, commit_ts))
}

/// Encode a Commit operation into WAL payload format
/// Format: [commit_ts: i64]
pub fn encode_commit(commit_ts: i64) -> Vec<u8> {
    commit_ts.to_le_bytes().to_vec()
}

/// Decode a Commit operation from WAL payload
/// Returns commit_ts
pub fn decode_commit(payload: &[u8]) -> KeyValueResult<i64> {
    if payload.len() < 8 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Commit record too short".to_string(),
        ));
    }

    let commit_ts = i64::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
        payload[7],
    ]);

    Ok(commit_ts)
}

/// Encode a Checkpoint operation into WAL payload format
/// Format: [sequence: u64][file_number: u64]
pub fn encode_checkpoint(sequence: u64, file_number: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&sequence.to_le_bytes());
    payload.extend_from_slice(&file_number.to_le_bytes());
    payload
}

/// Decode a Checkpoint operation from WAL payload
/// Returns (sequence, file_number)
pub fn decode_checkpoint(payload: &[u8]) -> KeyValueResult<(u64, u64)> {
    if payload.len() < 16 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL Checkpoint record too short".to_string(),
        ));
    }

    let sequence = u64::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
        payload[7],
    ]);

    let file_number = u64::from_le_bytes([
        payload[8],
        payload[9],
        payload[10],
        payload[11],
        payload[12],
        payload[13],
        payload[14],
        payload[15],
    ]);

    Ok((sequence, file_number))
}

/// Encode a FlushComplete operation into WAL payload format
/// Format: [file_number: u64][level: u32]
pub fn encode_flush_complete(file_number: u64, level: u32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&file_number.to_le_bytes());
    payload.extend_from_slice(&level.to_le_bytes());
    payload
}

/// Decode a FlushComplete operation from WAL payload
/// Returns (file_number, level)
pub fn decode_flush_complete(payload: &[u8]) -> KeyValueResult<(u64, u32)> {
    if payload.len() < 12 {
        return Err(nanograph_kvt::KeyValueError::StorageCorruption(
            "WAL FlushComplete record too short".to_string(),
        ));
    }

    let file_number = u64::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
        payload[7],
    ]);

    let level = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);

    Ok((file_number, level))
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
        assert_eq!(
            WalRecordKind::from_u16(3),
            Some(WalRecordKind::PutCommitted)
        );
        assert_eq!(
            WalRecordKind::from_u16(4),
            Some(WalRecordKind::DeleteCommitted)
        );
        assert_eq!(WalRecordKind::from_u16(5), Some(WalRecordKind::Commit));
        assert_eq!(WalRecordKind::from_u16(6), Some(WalRecordKind::Checkpoint));
        assert_eq!(
            WalRecordKind::from_u16(7),
            Some(WalRecordKind::FlushComplete)
        );
        assert_eq!(WalRecordKind::from_u16(99), None);

        assert_eq!(WalRecordKind::Put.to_u16(), 1);
        assert_eq!(WalRecordKind::Delete.to_u16(), 2);
        assert_eq!(WalRecordKind::PutCommitted.to_u16(), 3);
        assert_eq!(WalRecordKind::DeleteCommitted.to_u16(), 4);
        assert_eq!(WalRecordKind::Commit.to_u16(), 5);
        assert_eq!(WalRecordKind::Checkpoint.to_u16(), 6);
        assert_eq!(WalRecordKind::FlushComplete.to_u16(), 7);
    }

    #[test]
    fn test_encode_decode_put_committed() {
        let key = b"test_key";
        let value = b"test_value";
        let commit_ts = 12345i64;

        let payload = encode_put_committed(key, value, commit_ts);
        let (decoded_key, decoded_value, decoded_ts) = decode_put_committed(&payload).unwrap();

        assert_eq!(decoded_key, key);
        assert_eq!(decoded_value, value);
        assert_eq!(decoded_ts, commit_ts);
    }

    #[test]
    fn test_encode_decode_delete_committed() {
        let key = b"test_key";
        let commit_ts = 12345i64;

        let payload = encode_delete_committed(key, commit_ts);
        let (decoded_key, decoded_ts) = decode_delete_committed(&payload).unwrap();

        assert_eq!(decoded_key, key);
        assert_eq!(decoded_ts, commit_ts);
    }

    #[test]
    fn test_encode_decode_commit() {
        let commit_ts = 12345i64;

        let payload = encode_commit(commit_ts);
        let decoded_ts = decode_commit(&payload).unwrap();

        assert_eq!(decoded_ts, commit_ts);
    }

    #[test]
    fn test_encode_decode_checkpoint() {
        let sequence = 100u64;
        let file_number = 42u64;

        let payload = encode_checkpoint(sequence, file_number);
        let (decoded_seq, decoded_file) = decode_checkpoint(&payload).unwrap();

        assert_eq!(decoded_seq, sequence);
        assert_eq!(decoded_file, file_number);
    }

    #[test]
    fn test_encode_decode_flush_complete() {
        let file_number = 42u64;
        let level = 1u32;

        let payload = encode_flush_complete(file_number, level);
        let (decoded_file, decoded_level) = decode_flush_complete(&payload).unwrap();

        assert_eq!(decoded_file, file_number);
        assert_eq!(decoded_level, level);
    }

    #[test]
    fn test_decode_put_committed_invalid() {
        // Too short
        assert!(decode_put_committed(&[1, 2, 3]).is_err());
    }

    #[test]
    fn test_decode_delete_committed_invalid() {
        // Too short
        assert!(decode_delete_committed(&[1, 2]).is_err());
    }
}
