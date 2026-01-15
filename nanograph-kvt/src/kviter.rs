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

use crate::error::KeyValueResult;
use futures_core::Stream;

/// Key-Value Iterator
///
/// Provides streaming access to key-value pairs with seeking capability.
/// TODO: Replace with [`std::async_iter::AsyncIterator`] when stabilized
pub trait KeyValueIterator: Stream<Item = KeyValueResult<(Vec<u8>, Vec<u8>)>> + Unpin {
    /// Seek to a specific key
    ///
    /// Positions the iterator at the first key >= the given key.
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()>;

    /// Get current position
    ///
    /// Returns the key at the current iterator position, or None if exhausted.
    fn position(&self) -> Option<Vec<u8>>;

    /// Check if iterator is valid (not exhausted)
    fn valid(&self) -> bool;
}
