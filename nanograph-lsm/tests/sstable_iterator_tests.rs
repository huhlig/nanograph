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

use nanograph_lsm::{Entry, SSTable};
use nanograph_util::{CompressionAlgorithm, IntegrityAlgorithm};
use std::io::Cursor;

#[test]
fn test_sstable_iterator_basic() {
    // Create test entries
    let entries = vec![
        Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
        Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
        Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        Entry::new(b"key4".to_vec(), Some(b"value4".to_vec()), 4),
        Entry::new(b"key5".to_vec(), Some(b"value5".to_vec()), 5),
    ];

    // Write SSTable to memory buffer
    let mut buffer = Cursor::new(Vec::new());
    let _metadata = SSTable::create(
        &mut buffer,
        entries.clone(),
        1,
        0,
        4096,
        CompressionAlgorithm::None,
        IntegrityAlgorithm::None,
    )
    .unwrap();

    // Create iterator
    buffer.set_position(0);
    let iter = SSTable::iter(buffer).unwrap();

    // Collect all entries
    let read_entries: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();

    // Verify count
    assert_eq!(read_entries.len(), 5);

    // Verify each entry
    for (i, entry) in read_entries.iter().enumerate() {
        assert_eq!(entry.key, entries[i].key);
        assert_eq!(entry.value, entries[i].value);
        assert_eq!(entry.sequence, entries[i].sequence);
    }
}

#[test]
fn test_sstable_iterator_large_dataset() {
    // Create 1000 entries
    let entries: Vec<_> = (0..1000)
        .map(|i| {
            Entry::new(
                format!("key{:04}", i).into_bytes(),
                Some(format!("value{:04}", i).into_bytes()),
                i as u64,
            )
        })
        .collect();

    // Write SSTable
    let mut buffer = Cursor::new(Vec::new());
    let _metadata = SSTable::create(
        &mut buffer,
        entries.clone(),
        1,
        0,
        4096,
        CompressionAlgorithm::None,
        IntegrityAlgorithm::None,
    )
    .unwrap();

    // Iterate and verify
    buffer.set_position(0);
    let iter = SSTable::iter(buffer).unwrap();
    let read_entries: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(read_entries.len(), 1000);

    // Spot check some entries
    assert_eq!(read_entries[0].key, b"key0000");
    assert_eq!(read_entries[500].key, b"key0500");
    assert_eq!(read_entries[999].key, b"key0999");
}

#[test]
fn test_sstable_iterator_with_compression() {
    let entries = vec![
        Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
        Entry::new(b"key2".to_vec(), Some(b"value2".to_vec()), 2),
        Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
    ];

    // Write with compression
    let mut buffer = Cursor::new(Vec::new());
    let _metadata = SSTable::create(
        &mut buffer,
        entries.clone(),
        1,
        0,
        4096,
        CompressionAlgorithm::Lz4,
        IntegrityAlgorithm::None,
    )
    .unwrap();

    // Read back
    buffer.set_position(0);
    let iter = SSTable::iter(buffer).unwrap();
    let read_entries: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(read_entries.len(), 3);
    for (i, entry) in read_entries.iter().enumerate() {
        assert_eq!(entry.key, entries[i].key);
        assert_eq!(entry.value, entries[i].value);
    }
}

#[test]
fn test_sstable_iterator_with_tombstones() {
    let entries = vec![
        Entry::new(b"key1".to_vec(), Some(b"value1".to_vec()), 1),
        Entry::new(b"key2".to_vec(), None, 2), // Tombstone
        Entry::new(b"key3".to_vec(), Some(b"value3".to_vec()), 3),
        Entry::new(b"key4".to_vec(), None, 4), // Tombstone
    ];

    let mut buffer = Cursor::new(Vec::new());
    let _metadata = SSTable::create(
        &mut buffer,
        entries.clone(),
        1,
        0,
        4096,
        CompressionAlgorithm::None,
        IntegrityAlgorithm::None,
    )
    .unwrap();

    buffer.set_position(0);
    let iter = SSTable::iter(buffer).unwrap();
    let read_entries: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(read_entries.len(), 4);
    assert!(read_entries[0].value.is_some());
    assert!(read_entries[1].value.is_none()); // Tombstone
    assert!(read_entries[2].value.is_some());
    assert!(read_entries[3].value.is_none()); // Tombstone
}
