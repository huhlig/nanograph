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

use futures_core::Stream;
use nanograph_kvt::{KeyValueIterator, KeyValueResult};
use std::pin::Pin;
use std::task::{Context, Poll};

/// LMDB iterator implementation
///
/// This is a simple in-memory iterator that holds all entries.
/// For large result sets, this could be optimized to stream from LMDB.
pub struct LMDBIterator {
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    position: usize,
}

impl LMDBIterator {
    /// Create a new iterator from a vector of entries
    pub fn new(entries: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
        Self {
            entries,
            position: 0,
        }
    }
}

impl Stream for LMDBIterator {
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

impl KeyValueIterator for LMDBIterator {
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
