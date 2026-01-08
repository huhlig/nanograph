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

use crate::error::BTreeResult;
use crate::node::NodeId;
use crate::tree::BPlusTree;
use futures_core::Stream;
use nanograph_kvt::KeyValueIterator;
use std::ops::Bound;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Cursor for resuming iteration
#[derive(Debug, Clone)]
pub struct IteratorCursor {
    pub leaf_id: Option<NodeId>,
    pub index: usize,
    pub count: usize,
}

/// Iterator over B+Tree entries
pub struct BPlusTreeIterator {
    tree: Arc<BPlusTree>,
    current_leaf: Option<NodeId>,
    current_entries: Vec<(Vec<u8>, Vec<u8>)>,
    current_index: usize,
    end_bound: Bound<Vec<u8>>,
    reverse: bool,
    limit: Option<usize>,
    count: usize,
}

impl BPlusTreeIterator {
    /// Create a new forward iterator starting from the leftmost leaf
    pub fn new(
        tree: Arc<BPlusTree>,
        start_bound: Bound<Vec<u8>>,
        end_bound: Bound<Vec<u8>>,
        reverse: bool,
        limit: Option<usize>,
    ) -> BTreeResult<Self> {
        let (current_leaf, current_entries, current_index) = if reverse {
            // For reverse iteration, start from the end
            Self::find_reverse_start(&tree, &end_bound)?
        } else {
            // For forward iteration, start from the beginning
            Self::find_forward_start(&tree, &start_bound)?
        };

        Ok(Self {
            tree,
            current_leaf,
            current_entries,
            current_index,
            end_bound,
            reverse,
            limit,
            count: 0,
        })
    }

    /// Find the starting point for forward iteration
    fn find_forward_start(
        tree: &BPlusTree,
        start_bound: &Bound<Vec<u8>>,
    ) -> BTreeResult<(Option<NodeId>, Vec<(Vec<u8>, Vec<u8>)>, usize)> {
        match start_bound {
            Bound::Unbounded => {
                // Start from leftmost leaf
                let leaf_id = tree.get_leftmost_leaf()?;
                let entries = tree.get_leaf_entries(leaf_id)?;
                Ok((Some(leaf_id), entries, 0))
            }
            Bound::Included(key) | Bound::Excluded(key) => {
                // Find the leaf containing or after this key
                let leaf_id = tree.find_leaf(*tree.root_id().read().unwrap(), key)?;
                let entries = tree.get_leaf_entries(leaf_id)?;

                // Find the starting index within this leaf
                let start_index = entries
                    .binary_search_by(|(k, _)| k.as_slice().cmp(key.as_slice()))
                    .unwrap_or_else(|idx| idx);

                // Adjust for Excluded bound
                let start_index = if matches!(start_bound, Bound::Excluded(_)) {
                    if start_index < entries.len() && &entries[start_index].0 == key {
                        start_index + 1
                    } else {
                        start_index
                    }
                } else {
                    start_index
                };

                Ok((Some(leaf_id), entries, start_index))
            }
        }
    }

    /// Find the starting point for reverse iteration
    fn find_reverse_start(
        tree: &BPlusTree,
        end_bound: &Bound<Vec<u8>>,
    ) -> BTreeResult<(Option<NodeId>, Vec<(Vec<u8>, Vec<u8>)>, usize)> {
        match end_bound {
            Bound::Unbounded => {
                // Start from rightmost leaf - traverse to find it
                let mut leaf_id = tree.get_leftmost_leaf()?;
                let mut next = tree.get_next_leaf(leaf_id)?;

                while let Some(next_id) = next {
                    leaf_id = next_id;
                    next = tree.get_next_leaf(leaf_id)?;
                }

                let entries = tree.get_leaf_entries(leaf_id)?;
                let start_index = if entries.is_empty() {
                    0
                } else {
                    entries.len() - 1
                };
                Ok((Some(leaf_id), entries, start_index))
            }
            Bound::Included(key) | Bound::Excluded(key) => {
                // Find the leaf containing or before this key
                let leaf_id = tree.find_leaf(*tree.root_id().read().unwrap(), key)?;
                let entries = tree.get_leaf_entries(leaf_id)?;

                // Find the starting index within this leaf
                let start_index =
                    match entries.binary_search_by(|(k, _)| k.as_slice().cmp(key.as_slice())) {
                        Ok(idx) => {
                            // Exact match
                            if matches!(end_bound, Bound::Excluded(_)) {
                                if idx == 0 {
                                    // Need to go to previous leaf
                                    if let Some(prev_id) = tree.get_prev_leaf(leaf_id)? {
                                        let prev_entries = tree.get_leaf_entries(prev_id)?;
                                        let prev_index = if prev_entries.is_empty() {
                                            0
                                        } else {
                                            prev_entries.len() - 1
                                        };
                                        return Ok((Some(prev_id), prev_entries, prev_index));
                                    } else {
                                        return Ok((None, Vec::new(), 0));
                                    }
                                } else {
                                    idx - 1
                                }
                            } else {
                                idx
                            }
                        }
                        Err(idx) => {
                            // Not found, start from the entry before insertion point
                            if idx == 0 {
                                // Need to go to previous leaf
                                if let Some(prev_id) = tree.get_prev_leaf(leaf_id)? {
                                    let prev_entries = tree.get_leaf_entries(prev_id)?;
                                    let prev_index = if prev_entries.is_empty() {
                                        0
                                    } else {
                                        prev_entries.len() - 1
                                    };
                                    return Ok((Some(prev_id), prev_entries, prev_index));
                                } else {
                                    return Ok((None, Vec::new(), 0));
                                }
                            } else {
                                idx - 1
                            }
                        }
                    };

                Ok((Some(leaf_id), entries, start_index))
            }
        }
    }

    /// Check if a key is within the end bound
    fn is_within_end_bound(&self, key: &[u8]) -> bool {
        match &self.end_bound {
            Bound::Unbounded => true,
            Bound::Included(end_key) => key <= end_key.as_slice(),
            Bound::Excluded(end_key) => key < end_key.as_slice(),
        }
    }
}

// Implement Stream trait for BPlusTreeIterator
impl Stream for BPlusTreeIterator {
    type Item = nanograph_kvt::KeyValueResult<(Vec<u8>, Vec<u8>)>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Check limit first
            if let Some(limit) = self.limit {
                if self.count >= limit {
                    return Poll::Ready(None);
                }
            }

            // Ensure we have a valid current position
            if self.current_leaf.is_none() {
                return Poll::Ready(None);
            }

            // Check if current entries is empty
            if self.current_entries.is_empty() {
                self.current_leaf = None;
                return Poll::Ready(None);
            }

            // Check if we need to move to next/prev leaf
            let need_next_leaf = if self.reverse {
                // For reverse, check if we've gone past the beginning
                self.current_index >= self.current_entries.len()
            } else {
                // For forward, we've exhausted when index >= length
                self.current_index >= self.current_entries.len()
            };

            if need_next_leaf {
                let next_leaf = if self.reverse {
                    match self.tree.get_prev_leaf(self.current_leaf.unwrap()) {
                        Ok(leaf) => leaf,
                        Err(e) => {
                            return Poll::Ready(Some(Err(nanograph_kvt::KeyValueError::from(e))));
                        }
                    }
                } else {
                    match self.tree.get_next_leaf(self.current_leaf.unwrap()) {
                        Ok(leaf) => leaf,
                        Err(e) => {
                            return Poll::Ready(Some(Err(nanograph_kvt::KeyValueError::from(e))));
                        }
                    }
                };

                match next_leaf {
                    Some(leaf_id) => {
                        // Load next leaf
                        match self.tree.get_leaf_entries(leaf_id) {
                            Ok(entries) => {
                                if entries.is_empty() {
                                    // Skip empty leaves
                                    self.current_leaf = Some(leaf_id);
                                    self.current_entries = entries;
                                    continue;
                                }
                                self.current_leaf = Some(leaf_id);
                                self.current_entries = entries;
                                self.current_index = if self.reverse {
                                    self.current_entries.len() - 1
                                } else {
                                    0
                                };
                                // Continue loop to return first entry from this leaf
                                continue;
                            }
                            Err(e) => {
                                return Poll::Ready(Some(Err(nanograph_kvt::KeyValueError::from(
                                    e,
                                ))));
                            }
                        }
                    }
                    None => {
                        // No more leaves
                        self.current_leaf = None;
                        return Poll::Ready(None);
                    }
                }
            }

            // We have a valid entry at current_index
            let (key, value) = &self.current_entries[self.current_index];

            // Check bounds (for forward iteration)
            if !self.reverse && !self.is_within_end_bound(key) {
                self.current_leaf = None;
                return Poll::Ready(None);
            }

            // Return the entry and advance
            let entry = (key.clone(), value.clone());
            self.count += 1;

            // Advance index based on direction
            if self.reverse {
                if self.current_index == 0 {
                    // Set index to trigger leaf change on next call
                    self.current_index = usize::MAX;
                } else {
                    self.current_index -= 1;
                }
            } else {
                self.current_index += 1;
            }

            return Poll::Ready(Some(Ok(entry)));
        }
    }
}

impl KeyValueIterator for BPlusTreeIterator {
    fn seek(&mut self, key: &[u8]) -> nanograph_kvt::KeyValueResult<()> {
        // Find the leaf containing the key
        let root_id = *self.tree.root_id().read().unwrap();
        let leaf_id = self
            .tree
            .find_leaf(root_id, key)
            .map_err(|e| nanograph_kvt::KeyValueError::from(e))?;

        // Get leaf entries through public method
        let entries = self
            .tree
            .get_leaf_entries(leaf_id)
            .map_err(|e| nanograph_kvt::KeyValueError::from(e))?;

        self.current_leaf = Some(leaf_id);
        self.current_entries = entries;

        // Find the position of the key or where it would be inserted
        let idx = self
            .current_entries
            .binary_search_by(|(k, _)| k.as_slice().cmp(key))
            .unwrap_or_else(|idx| idx);
        self.current_index = idx;

        Ok(())
    }

    fn position(&self) -> Option<Vec<u8>> {
        if self.current_index < self.current_entries.len() {
            Some(self.current_entries[self.current_index].0.clone())
        } else {
            None
        }
    }

    fn valid(&self) -> bool {
        self.current_leaf.is_some() && self.current_index < self.current_entries.len()
    }
}

impl BPlusTreeIterator {
    /// Synchronous next for testing and non-async contexts
    ///
    /// This is a convenience method that polls the Stream synchronously.
    /// Get current cursor position for resuming iteration later
    pub fn get_cursor(&self) -> IteratorCursor {
        IteratorCursor {
            leaf_id: self.current_leaf,
            index: self.current_index,
            count: self.count,
        }
    }

    /// Create iterator from a saved cursor position
    pub fn from_cursor(
        tree: Arc<BPlusTree>,
        cursor: IteratorCursor,
        end_bound: Bound<Vec<u8>>,
        reverse: bool,
        limit: Option<usize>,
    ) -> BTreeResult<Self> {
        let (current_leaf, current_entries, current_index) = if let Some(leaf_id) = cursor.leaf_id {
            let entries = tree.get_leaf_entries(leaf_id)?;
            (Some(leaf_id), entries, cursor.index)
        } else {
            (None, Vec::new(), 0)
        };

        Ok(Self {
            tree,
            current_leaf,
            current_entries,
            current_index,
            end_bound,
            reverse,
            limit,
            count: cursor.count,
        })
    }

    /// For production async code, use the Stream trait directly.
    pub fn next_sync(&mut self) -> Option<nanograph_kvt::KeyValueResult<(Vec<u8>, Vec<u8>)>> {
        use futures::task::{Context, Poll};
        use std::pin::Pin;

        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(self).poll_next(&mut cx) {
            Poll::Ready(item) => item,
            Poll::Pending => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{BPlusTree, BPlusTreeConfig};

    #[test]
    fn test_forward_iteration() {
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));

        // Insert test data
        for i in 0..10 {
            let key = format!("key{:02}", i);
            let value = format!("value{}", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Iterate forward
        let mut iter = BPlusTreeIterator::new(
            tree.clone(),
            Bound::Unbounded,
            Bound::Unbounded,
            false,
            None,
        )
        .unwrap();

        let mut count = 0;
        while let Some(Ok((key, value))) = iter.next_sync() {
            let expected_key = format!("key{:02}", count);
            let expected_value = format!("value{}", count);
            assert_eq!(key, expected_key.into_bytes());
            assert_eq!(value, expected_value.into_bytes());
            count += 1;
        }

        assert_eq!(count, 10);
    }

    #[test]
    fn test_reverse_iteration() {
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));

        // Insert test data
        for i in 0..10 {
            let key = format!("key{:02}", i);
            let value = format!("value{}", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Iterate reverse
        let mut iter =
            BPlusTreeIterator::new(tree.clone(), Bound::Unbounded, Bound::Unbounded, true, None)
                .unwrap();

        let mut count = 9;
        while let Some(Ok((key, value))) = iter.next_sync() {
            let expected_key = format!("key{:02}", count);
            let expected_value = format!("value{}", count);
            assert_eq!(key, expected_key.into_bytes());
            assert_eq!(value, expected_value.into_bytes());
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }

    #[test]
    fn test_bounded_iteration() {
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));

        // Insert test data
        for i in 0..10 {
            let key = format!("key{:02}", i);
            let value = format!("value{}", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Iterate from key03 to key07
        let mut iter = BPlusTreeIterator::new(
            tree.clone(),
            Bound::Included(b"key03".to_vec()),
            Bound::Excluded(b"key07".to_vec()),
            false,
            None,
        )
        .unwrap();

        let mut keys = Vec::new();
        while let Some(Ok((key, _))) = iter.next_sync() {
            keys.push(String::from_utf8(key).unwrap());
        }

        assert_eq!(keys, vec!["key03", "key04", "key05", "key06"]);
    }

    #[test]
    fn test_limited_iteration() {
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));

        // Insert test data
        for i in 0..10 {
            let key = format!("key{:02}", i);
            let value = format!("value{}", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Iterate with limit
        let mut iter = BPlusTreeIterator::new(
            tree.clone(),
            Bound::Unbounded,
            Bound::Unbounded,
            false,
            Some(5),
        )
        .unwrap();

        let mut count = 0;
        while iter.next_sync().is_some() {
            count += 1;
        }

        assert_eq!(count, 5);
    }
}
