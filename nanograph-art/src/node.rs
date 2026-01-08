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

use std::sync::Arc;

/// Node types in the Adaptive Radix Tree
#[derive(Debug, Clone)]
pub enum Node<V> {
    /// Leaf node containing a value
    Leaf(LeafNode<V>),
    /// Inner node with 4 children
    Node4(Box<Node4<V>>),
    /// Inner node with 16 children
    Node16(Box<Node16<V>>),
    /// Inner node with 48 children
    Node48(Box<Node48<V>>),
    /// Inner node with 256 children
    Node256(Box<Node256<V>>),
}

/// Leaf node containing the actual value
#[derive(Debug, Clone)]
pub struct LeafNode<V> {
    /// The key stored in this leaf
    pub key: Vec<u8>,
    /// The value stored in this leaf
    pub value: V,
}

/// Base header for all inner nodes
#[derive(Debug, Clone)]
pub struct NodeHeader<V> {
    /// Number of children
    pub num_children: u16,
    /// Partial key for path compression
    pub partial: Vec<u8>,
    /// Optional value stored at this node (for keys that are prefixes of other keys)
    pub value: Option<V>,
}

/// Node with up to 4 children (sorted arrays)
#[derive(Debug, Clone)]
pub struct Node4<V> {
    pub header: NodeHeader<V>,
    /// Keys for children (sorted)
    pub keys: [u8; 4],
    /// Child pointers
    pub children: [Option<Arc<Node<V>>>; 4],
}

/// Node with up to 16 children (sorted arrays)
#[derive(Debug, Clone)]
pub struct Node16<V> {
    pub header: NodeHeader<V>,
    /// Keys for children (sorted)
    pub keys: [u8; 16],
    /// Child pointers
    pub children: [Option<Arc<Node<V>>>; 16],
}

/// Node with up to 48 children (index array + child array)
#[derive(Debug, Clone)]
pub struct Node48<V> {
    pub header: NodeHeader<V>,
    /// Index array: maps byte value to child index (255 = not present)
    pub child_index: [u8; 256],
    /// Child pointers (only first 48 used)
    pub children: [Option<Arc<Node<V>>>; 48],
}

/// Node with up to 256 children (direct array)
#[derive(Debug, Clone)]
pub struct Node256<V> {
    pub header: NodeHeader<V>,
    /// Direct child pointers indexed by byte value
    pub children: [Option<Arc<Node<V>>>; 256],
}

impl<V> Node<V> {
    /// Create a new leaf node
    pub fn new_leaf(key: Vec<u8>, value: V) -> Self {
        Node::Leaf(LeafNode { key, value })
    }

    /// Create a new Node4
    pub fn new_node4(partial: Vec<u8>) -> Self {
        Node::Node4(Box::new(Node4 {
            header: NodeHeader {
                num_children: 0,
                partial,
                value: None,
            },
            keys: [0; 4],
            children: Default::default(),
        }))
    }

    /// Create a new Node16
    pub fn new_node16(partial: Vec<u8>) -> Self {
        Node::Node16(Box::new(Node16 {
            header: NodeHeader {
                num_children: 0,
                partial,
                value: None,
            },
            keys: [0; 16],
            children: Default::default(),
        }))
    }

    /// Create a new Node48
    pub fn new_node48(partial: Vec<u8>) -> Self {
        Node::Node48(Box::new(Node48 {
            header: NodeHeader {
                num_children: 0,
                partial,
                value: None,
            },
            child_index: [255; 256],
            children: std::array::from_fn(|_| None),
        }))
    }

    /// Create a new Node256
    pub fn new_node256(partial: Vec<u8>) -> Self {
        Node::Node256(Box::new(Node256 {
            header: NodeHeader {
                num_children: 0,
                partial,
                value: None,
            },
            children: std::array::from_fn(|_| None),
        }))
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        matches!(self, Node::Leaf(_))
    }

    /// Get the partial key from the node header
    pub fn partial(&self) -> &[u8] {
        match self {
            Node::Leaf(_) => &[],
            Node::Node4(n) => &n.header.partial,
            Node::Node16(n) => &n.header.partial,
            Node::Node48(n) => &n.header.partial,
            Node::Node256(n) => &n.header.partial,
        }
    }

    /// Get the number of children
    pub fn num_children(&self) -> u16 {
        match self {
            Node::Leaf(_) => 0,
            Node::Node4(n) => n.header.num_children,
            Node::Node16(n) => n.header.num_children,
            Node::Node48(n) => n.header.num_children,
            Node::Node256(n) => n.header.num_children,
        }
    }

    /// Find a child by key byte
    pub fn find_child(&self, key_byte: u8) -> Option<&Arc<Node<V>>> {
        match self {
            Node::Leaf(_) => None,
            Node::Node4(n) => {
                for i in 0..n.header.num_children as usize {
                    if n.keys[i] == key_byte {
                        return n.children[i].as_ref();
                    }
                }
                None
            }
            Node::Node16(n) => {
                // Binary search for efficiency
                let mut left = 0;
                let mut right = n.header.num_children as usize;
                while left < right {
                    let mid = (left + right) / 2;
                    if n.keys[mid] < key_byte {
                        left = mid + 1;
                    } else if n.keys[mid] > key_byte {
                        right = mid;
                    } else {
                        return n.children[mid].as_ref();
                    }
                }
                None
            }
            Node::Node48(n) => {
                let idx = n.child_index[key_byte as usize];
                if idx != 255 {
                    n.children[idx as usize].as_ref()
                } else {
                    None
                }
            }
            Node::Node256(n) => n.children[key_byte as usize].as_ref(),
        }
    }

    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        match self {
            Node::Leaf(leaf) => std::mem::size_of::<LeafNode<V>>() + leaf.key.capacity(),
            Node::Node4(_) => std::mem::size_of::<Node4<V>>(),
            Node::Node16(_) => std::mem::size_of::<Node16<V>>(),
            Node::Node48(_) => std::mem::size_of::<Node48<V>>(),
            Node::Node256(_) => std::mem::size_of::<Node256<V>>(),
        }
    }
}

impl<V> LeafNode<V> {
    /// Check if the leaf matches the given key
    pub fn matches(&self, key: &[u8]) -> bool {
        self.key == key
    }
}

impl<V> NodeHeader<V> {
    /// Create a new node header
    pub fn new(partial: Vec<u8>) -> Self {
        Self {
            num_children: 0,
            partial,
            value: None,
        }
    }

    /// Create a new node header with a value
    pub fn with_value(partial: Vec<u8>, value: V) -> Self {
        Self {
            num_children: 0,
            partial,
            value: Some(value),
        }
    }
}

// Made with Bob
