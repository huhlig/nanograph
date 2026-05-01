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

use crate::error::LMDBError;
use futures_core::Stream;
use lmdb::{Cursor, Database, Environment, Transaction};
use nanograph_kvt::{KeyRange, KeyValueIterator, KeyValueResult};
use std::ops::Bound;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Chunk size for batched iteration (number of entries to fetch at once)
const CHUNK_SIZE: usize = 1000;

/// Streaming LMDB iterator that fetches data in chunks
///
/// This iterator holds an Arc to the Environment and Database, and creates
/// short-lived read transactions to fetch data in chunks. This avoids loading
/// all data into memory while working around LMDB's lifetime constraints.
///
/// The iterator maintains a cursor position (last key seen) and creates new
/// transactions as needed to continue iteration.
pub struct LMDBIterator {
    /// Reference to the LMDB environment
    env: Arc<Environment>,
    /// The database to iterate over
    db: Database,
    /// The key range to iterate over
    range: KeyRange,
    /// Current chunk of entries being iterated
    current_chunk: Vec<(Vec<u8>, Vec<u8>)>,
    /// Position within current chunk
    chunk_position: usize,
    /// Last key seen (for continuing iteration)
    last_key: Option<Vec<u8>>,
    /// Whether iteration has finished
    finished: bool,
    /// Number of items returned (for limit tracking)
    count: usize,
}

impl LMDBIterator {
    /// Create a new streaming iterator
    pub fn new(
        env: Arc<Environment>,
        db: Database,
        range: KeyRange,
    ) -> Self {
        Self {
            env,
            db,
            range,
            current_chunk: Vec::new(),
            chunk_position: 0,
            last_key: None,
            finished: false,
            count: 0,
        }
    }

    /// Check if a key is within the range bounds
    fn key_in_range(&self, key: &[u8]) -> bool {
        // Check start bound
        let after_start = match &self.range.start {
            Bound::Included(start) => key >= start.as_slice(),
            Bound::Excluded(start) => key > start.as_slice(),
            Bound::Unbounded => true,
        };

        // Check end bound
        let before_end = match &self.range.end {
            Bound::Included(end) => key <= end.as_slice(),
            Bound::Excluded(end) => key < end.as_slice(),
            Bound::Unbounded => true,
        };

        after_start && before_end
    }

    /// Check if we've reached the limit
    fn at_limit(&self) -> bool {
        if let Some(limit) = self.range.limit {
            self.count >= limit
        } else {
            false
        }
    }

    /// Fetch the next chunk of entries from LMDB
    fn fetch_next_chunk(&mut self) -> Result<(), LMDBError> {
        if self.finished || self.at_limit() {
            return Ok(());
        }

        // Calculate how many entries we can still fetch
        let remaining = if let Some(limit) = self.range.limit {
            limit.saturating_sub(self.count)
        } else {
            CHUNK_SIZE
        };
        let chunk_size = remaining.min(CHUNK_SIZE);

        // Create a new read transaction
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.db)?;

        // Clear current chunk
        self.current_chunk.clear();
        self.chunk_position = 0;

        // Determine starting point for iteration
        let iter = if let Some(ref last_key) = self.last_key {
            // Continue from where we left off
            // We need to skip past last_key since we already returned it
            cursor.iter_from(last_key.as_slice())
        } else {
            // First chunk - start based on range
            match &self.range.start {
                Bound::Included(start) => cursor.iter_from(start.as_slice()),
                Bound::Excluded(start) => cursor.iter_from(start.as_slice()),
                Bound::Unbounded => cursor.iter_start(),
            }
        };

        // Iterate and collect entries
        let mut skip_first = false;
        if let Some(ref last_key) = self.last_key {
            // Skip the first entry if it matches last_key (we already returned it)
            skip_first = true;
        } else if matches!(&self.range.start, Bound::Excluded(_)) {
            // Skip the first entry if start bound is excluded
            skip_first = true;
        }

        for (idx, result) in iter.enumerate() {
            // Skip first entry if needed
            if skip_first && idx == 0 {
                continue;
            }

            let (key, value) = result.map_err(LMDBError::from)?;
            let key: &[u8] = key;
            let value: &[u8] = value;

            // Check if in range
            if !self.key_in_range(key) {
                self.finished = true;
                break;
            }

            // Add to chunk
            self.current_chunk.push((key.to_vec(), value.to_vec()));
            self.last_key = Some(key.to_vec());

            // Check if we've filled the chunk
            if self.current_chunk.len() >= chunk_size {
                break;
            }
        }

        // If we didn't get any entries, we're done
        if self.current_chunk.is_empty() {
            self.finished = true;
        }

        // Handle reverse iteration
        if self.range.reverse {
            self.current_chunk.reverse();
        }

        Ok(())
    }

    /// Get the next entry
    fn next_entry(&mut self) -> Result<Option<(Vec<u8>, Vec<u8>)>, LMDBError> {
        // Check if we need to fetch a new chunk
        if self.chunk_position >= self.current_chunk.len() {
            if self.finished || self.at_limit() {
                return Ok(None);
            }
            self.fetch_next_chunk()?;
            if self.current_chunk.is_empty() {
                return Ok(None);
            }
        }

        // Return next entry from current chunk
        if self.chunk_position < self.current_chunk.len() {
            let entry = self.current_chunk[self.chunk_position].clone();
            self.chunk_position += 1;
            self.count += 1;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }
}

impl Stream for LMDBIterator {
    type Item = KeyValueResult<(Vec<u8>, Vec<u8>)>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.next_entry() {
            Ok(Some(entry)) => Poll::Ready(Some(Ok(entry))),
            Ok(None) => Poll::Ready(None),
            Err(e) => Poll::Ready(Some(Err(e.into()))),
        }
    }
}

impl KeyValueIterator for LMDBIterator {
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()> {
        // Reset state and set last_key to seek position
        self.current_chunk.clear();
        self.chunk_position = 0;
        self.last_key = Some(key.to_vec());
        self.finished = false;
        self.count = 0;

        // Fetch first chunk from new position
        self.fetch_next_chunk().map_err(|e| e.into())
    }

    fn position(&self) -> Option<Vec<u8>> {
        self.last_key.clone()
    }

    fn valid(&self) -> bool {
        !self.finished && (self.chunk_position < self.current_chunk.len() || !self.at_limit())
    }
}

/// Simple in-memory iterator for transaction scans
///
/// This is used by transactions which need to merge buffered writes
/// with base data. Since the merge requires collecting data anyway,
/// we use a simple Vec-based iterator.
pub struct LMDBMemoryIterator {
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    position: usize,
}

impl LMDBMemoryIterator {
    /// Create a new iterator from a vector of entries
    pub fn new(entries: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
        Self {
            entries,
            position: 0,
        }
    }
}

impl Stream for LMDBMemoryIterator {
    type Item = KeyValueResult<(Vec<u8>, Vec<u8>)>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.position < self.entries.len() {
            let entry = self.entries[self.position].clone();
            self.position += 1;
            Poll::Ready(Some(Ok(entry)))
        } else {
            Poll::Ready(None)
        }
    }
}

impl KeyValueIterator for LMDBMemoryIterator {
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k.as_slice() >= key) {
            self.position = pos;
        } else {
            self.position = self.entries.len();
        }
        Ok(())
    }

    fn position(&self) -> Option<Vec<u8>> {
        self.entries.get(self.position).map(|(k, _)| k.clone())
    }

    fn valid(&self) -> bool {
        self.position < self.entries.len()
    }
}

// Made with Bob
