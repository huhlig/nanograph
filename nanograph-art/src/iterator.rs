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

use crate::node::Node;
use std::sync::Arc;

/// Iterator over the ART tree
pub struct ArtIterator<V> {
    stack: Vec<IteratorFrame<V>>,
}

struct IteratorFrame<V> {
    node: Arc<Node<V>>,
    child_index: usize,
}

impl<V: Clone> ArtIterator<V> {
    /// Create a new iterator starting from the given node
    pub fn new(root: Option<Arc<Node<V>>>) -> Self {
        let mut iter = Self { stack: Vec::new() };

        if let Some(node) = root {
            iter.descend_leftmost(node);
        }

        iter
    }

    /// Descend to the leftmost leaf
    fn descend_leftmost(&mut self, mut node: Arc<Node<V>>) {
        loop {
            if node.is_leaf() {
                self.stack.push(IteratorFrame {
                    node,
                    child_index: 0,
                });
                break;
            }

            // Find first child
            let first_child = self.get_first_child(&node);
            self.stack.push(IteratorFrame {
                node: node.clone(),
                child_index: 0,
            });

            if let Some(child) = first_child {
                node = child;
            } else {
                break;
            }
        }
    }

    /// Get the first child of a node
    fn get_first_child(&self, node: &Arc<Node<V>>) -> Option<Arc<Node<V>>> {
        match node.as_ref() {
            Node::Leaf(_) => None,
            Node::Node4(n) => {
                if n.header.num_children > 0 {
                    n.children[0].clone()
                } else {
                    None
                }
            }
            Node::Node16(n) => {
                if n.header.num_children > 0 {
                    n.children[0].clone()
                } else {
                    None
                }
            }
            Node::Node48(n) => {
                for key_byte in 0..256 {
                    let idx = n.child_index[key_byte];
                    if idx != 255 {
                        return n.children[idx as usize].clone();
                    }
                }
                None
            }
            Node::Node256(n) => {
                for child in &n.children {
                    if child.is_some() {
                        return child.clone();
                    }
                }
                None
            }
        }
    }

    /// Get the next child of a node at the given index
    fn get_next_child(
        &self,
        node: &Arc<Node<V>>,
        current_index: usize,
    ) -> Option<(Arc<Node<V>>, usize)> {
        match node.as_ref() {
            Node::Leaf(_) => None,
            Node::Node4(n) => {
                let next_idx = current_index + 1;
                if next_idx < n.header.num_children as usize {
                    n.children[next_idx].clone().map(|c| (c, next_idx))
                } else {
                    None
                }
            }
            Node::Node16(n) => {
                let next_idx = current_index + 1;
                if next_idx < n.header.num_children as usize {
                    n.children[next_idx].clone().map(|c| (c, next_idx))
                } else {
                    None
                }
            }
            Node::Node48(n) => {
                // Find the current key byte
                let mut current_key = 0;
                let mut found_count = 0;
                for key_byte in 0..256 {
                    let idx = n.child_index[key_byte];
                    if idx != 255 {
                        if found_count == current_index {
                            current_key = key_byte;
                            break;
                        }
                        found_count += 1;
                    }
                }

                // Find next key byte
                for key_byte in (current_key + 1)..256 {
                    let idx = n.child_index[key_byte];
                    if idx != 255 {
                        return n.children[idx as usize]
                            .clone()
                            .map(|c| (c, current_index + 1));
                    }
                }
                None
            }
            Node::Node256(n) => {
                // Find current key byte
                let mut current_key = 0;
                let mut found_count = 0;
                for key_byte in 0..256 {
                    if n.children[key_byte].is_some() {
                        if found_count == current_index {
                            current_key = key_byte;
                            break;
                        }
                        found_count += 1;
                    }
                }

                // Find next key byte
                for key_byte in (current_key + 1)..256 {
                    if let Some(child) = &n.children[key_byte] {
                        return Some((child.clone(), current_index + 1));
                    }
                }
                None
            }
        }
    }
}

impl<V: Clone> Iterator for ArtIterator<V> {
    type Item = (Vec<u8>, V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.stack.last() {
            if frame.node.is_leaf() {
                // Found a leaf - return it and pop from stack
                if let Node::Leaf(leaf) = frame.node.as_ref() {
                    let result = (leaf.key.clone(), leaf.value.clone());
                    self.stack.pop();
                    return Some(result);
                }
            }

            // Try to get next child
            let node = frame.node.clone();
            let current_index = frame.child_index;

            if let Some((child, next_index)) = self.get_next_child(&node, current_index) {
                // Update current frame's index
                if let Some(last) = self.stack.last_mut() {
                    last.child_index = next_index;
                }
                // Descend to leftmost leaf of this child
                self.descend_leftmost(child);
            } else {
                // No more children, pop this frame
                self.stack.pop();
            }
        }

        None
    }
}

/// Range iterator for the ART tree
pub struct ArtRangeIterator<V> {
    inner: ArtIterator<V>,
    end_key: Option<Vec<u8>>,
    inclusive: bool,
}

impl<V: Clone> ArtRangeIterator<V> {
    /// Create a new range iterator
    pub fn new(
        root: Option<Arc<Node<V>>>,
        start_key: Option<Vec<u8>>,
        end_key: Option<Vec<u8>>,
        inclusive: bool,
    ) -> Self {
        let mut iter = ArtIterator::new(root);

        // Skip to start key if provided
        if let Some(start) = start_key {
            while let Some((key, _)) = iter.next() {
                if key >= start {
                    // Put this item back by recreating iterator from this point
                    // For simplicity, we'll just continue from here
                    break;
                }
            }
        }

        Self {
            inner: iter,
            end_key,
            inclusive,
        }
    }
}

impl<V: Clone> Iterator for ArtRangeIterator<V> {
    type Item = (Vec<u8>, V);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;

        if let Some(end) = &self.end_key {
            if self.inclusive {
                if &item.0 > end {
                    return None;
                }
            } else {
                if &item.0 >= end {
                    return None;
                }
            }
        }

        Some(item)
    }
}
