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

//! # Object ID Allocator
//!
//! Provides unified ObjectId allocation for all database objects (Tables, Indexes, Functions, Namespaces)
//! within a database. This ensures no collisions when constructing ShardIds.
//!
//! ## Architecture
//!
//! - **Standalone Mode**: Uses local atomic counter for single-node deployments
//! - **Distributed Mode**: Uses Raft consensus for multi-node deployments
//!
//! ## Usage
//!
//! ```rust,ignore
//! use nanograph_kvm::ObjectAllocator;
//!
//! // Standalone mode
//! let allocator = ObjectAllocator::new_standalone();
//! let id = allocator.allocate_local().await?;
//!
//! // Distributed mode (requires Raft integration)
//! let allocator = ObjectAllocator::new_distributed(raft_group);
//! let id = allocator.allocate().await?;
//! ```

use nanograph_core::object::{ContainerId, ObjectId};
use nanograph_raft::ConsensusManager;
use nanograph_util::serialize;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Error types for object allocation
#[derive(Debug, Clone, thiserror::Error)]
pub enum AllocationError {
    /// Not the Raft leader, cannot allocate
    #[error("Not the Raft leader, cannot allocate IDs")]
    NotLeader,

    /// Allocation failed due to consensus error
    #[error("Consensus error: {message}")]
    ConsensusError { message: String },

    /// ID space exhausted
    #[error("ObjectId space exhausted")]
    Exhausted,

    /// Internal error
    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Result type for allocation operations
pub type AllocationResult<T> = Result<T, AllocationError>;

/// Commands for Raft-based allocation
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AllocationCommand {
    /// Allocate a single ObjectId
    AllocateObjectId {
        container_id: ContainerId,
        object_id: ObjectId,
    },

    /// Allocate a range of ObjectIds
    AllocateObjectIdRange {
        container_id: ContainerId,
        start: ObjectId,
        end: ObjectId,
    },
}

/// Object ID Allocator
///
/// Manages allocation of unique ObjectIds within a database. All object types
/// (Tables, Indexes, Functions, Namespaces) share the same ID space to prevent
/// ShardId collisions.
///
/// ## Modes
///
/// - **Standalone**: Direct atomic allocation for single-node deployments
/// - **Distributed**: Raft-based allocation for multi-node deployments
pub struct ObjectAllocator {
    /// Container ID this allocator is for
    container_id: ContainerId,

    /// Current next ID (replicated state in distributed mode)
    next_id: Arc<AtomicU32>,

    /// Raft consensus manager (None for standalone mode)
    raft_manager: Option<Arc<ConsensusManager>>,
}

impl ObjectAllocator {
    /// Create a new standalone allocator
    ///
    /// For single-node deployments. Uses local atomic counter.
    ///
    /// # Arguments
    /// * `container_id` - The container this allocator is for
    ///
    /// # Returns
    /// A new standalone ObjectAllocator starting from ID 1
    pub fn new_standalone(container_id: ContainerId) -> Self {
        Self {
            container_id,
            next_id: Arc::new(AtomicU32::new(1)),
            raft_manager: None,
        }
    }

    /// Create a new distributed allocator
    ///
    /// For multi-node deployments. Requires Raft consensus for allocation.
    ///
    /// # Arguments
    /// * `container_id` - The container this allocator is for
    /// * `raft_manager` - The Raft consensus manager
    ///
    /// # Returns
    /// A new distributed ObjectAllocator starting from ID 1
    pub fn new_distributed(container_id: ContainerId, raft_manager: Arc<ConsensusManager>) -> Self {
        Self {
            container_id,
            next_id: Arc::new(AtomicU32::new(1)),
            raft_manager: Some(raft_manager),
        }
    }

    /// Create allocator with specific starting ID
    ///
    /// Useful for recovery or migration scenarios.
    ///
    /// # Arguments
    /// * `container_id` - The container this allocator is for
    /// * `start_id` - The starting ObjectId
    /// * `raft_manager` - Optional Raft manager for distributed mode
    ///
    /// # Returns
    /// A new ObjectAllocator starting from the specified ID
    pub fn with_start_id(
        container_id: ContainerId,
        start_id: ObjectId,
        raft_manager: Option<Arc<ConsensusManager>>,
    ) -> Self {
        Self {
            container_id,
            next_id: Arc::new(AtomicU32::new(start_id.as_u32())),
            raft_manager,
        }
    }

    /// Allocate a single ObjectId
    ///
    /// In standalone mode, allocates locally. In distributed mode, uses Raft consensus.
    ///
    /// # Returns
    /// * `Ok(ObjectId)` - The allocated ID
    /// * `Err(AllocationError)` - Allocation failed
    pub async fn allocate(&self) -> AllocationResult<ObjectId> {
        match &self.raft_manager {
            None => self.allocate_local(),
            Some(raft) => self.allocate_distributed(raft).await,
        }
    }

    /// Allocate a single ObjectId (standalone mode only)
    ///
    /// Fast local allocation without consensus. Only use in single-node deployments.
    ///
    /// # Returns
    /// * `Ok(ObjectId)` - The allocated ID
    /// * `Err(AllocationError::Exhausted)` - ID space exhausted
    fn allocate_local(&self) -> AllocationResult<ObjectId> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if id == u32::MAX {
            return Err(AllocationError::Exhausted);
        }
        Ok(ObjectId::new(id))
    }

    /// Allocate a single ObjectId through Raft consensus
    async fn allocate_distributed(
        &self,
        raft: &Arc<ConsensusManager>,
    ) -> AllocationResult<ObjectId> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if id == u32::MAX {
            return Err(AllocationError::Exhausted);
        }

        // Serialize the allocation command
        let command = AllocationCommand::AllocateObjectId {
            container_id: self.container_id,
            object_id: ObjectId::new(id),
        };
        let command_bytes = serialize(&command).map_err(|e| AllocationError::Internal {
            message: format!("Failed to serialize command: {}", e),
        })?;

        // Store allocation state through Raft consensus
        let key = format!("allocator:{}:next", self.container_id).into_bytes();
        raft.put(key, command_bytes)
            .await
            .map_err(|e| AllocationError::ConsensusError {
                message: format!("Failed to propose allocation: {}", e),
            })?;

        Ok(ObjectId::new(id))
    }

    /// Allocate a range of ObjectIds
    ///
    /// Pre-allocates multiple IDs for batch operations.
    ///
    /// # Arguments
    /// * `count` - Number of IDs to allocate
    ///
    /// # Returns
    /// * `Ok(Range<ObjectId>)` - The allocated ID range
    /// * `Err(AllocationError)` - Allocation failed
    pub async fn allocate_range(&self, count: u32) -> AllocationResult<Range<ObjectId>> {
        match &self.raft_manager {
            None => self.allocate_range_local(count),
            Some(raft) => self.allocate_range_distributed(raft, count).await,
        }
    }

    /// Allocate a range of ObjectIds (standalone mode only)
    fn allocate_range_local(&self, count: u32) -> AllocationResult<Range<ObjectId>> {
        let start = self.next_id.fetch_add(count, Ordering::SeqCst);
        let end = start.checked_add(count).ok_or(AllocationError::Exhausted)?;
        Ok(ObjectId::new(start)..ObjectId::new(end))
    }

    /// Allocate a range of ObjectIds through Raft consensus
    async fn allocate_range_distributed(
        &self,
        raft: &Arc<ConsensusManager>,
        count: u32,
    ) -> AllocationResult<Range<ObjectId>> {
        let start = self.next_id.fetch_add(count, Ordering::SeqCst);
        let end = start.checked_add(count).ok_or(AllocationError::Exhausted)?;

        // Serialize the allocation command
        let command = AllocationCommand::AllocateObjectIdRange {
            container_id: self.container_id,
            start: ObjectId::new(start),
            end: ObjectId::new(end),
        };
        let command_bytes = serialize(&command).map_err(|e| AllocationError::Internal {
            message: format!("Failed to serialize command: {}", e),
        })?;

        // Store allocation state through Raft consensus
        let key = format!("allocator:{}:next", self.container_id).into_bytes();
        raft.put(key, command_bytes)
            .await
            .map_err(|e| AllocationError::ConsensusError {
                message: format!("Failed to propose allocation: {}", e),
            })?;

        Ok(ObjectId::new(start)..ObjectId::new(end))
    }

    /// Apply a committed allocation (called on all nodes after Raft commit)
    ///
    /// Updates the local state to match the committed allocation. This ensures
    /// all replicas have consistent state.
    ///
    /// # Arguments
    /// * `command` - The committed allocation command
    pub fn apply_allocation(&self, command: &AllocationCommand) {
        match command {
            AllocationCommand::AllocateObjectId { object_id, .. } => {
                // Update to next available ID
                let next = object_id.as_u32().saturating_add(1);
                self.next_id.store(next, Ordering::SeqCst);
            }
            AllocationCommand::AllocateObjectIdRange { end, .. } => {
                // Update to next available ID after range
                self.next_id.store(end.as_u32(), Ordering::SeqCst);
            }
        }
    }

    /// Get the current next ID (for debugging/monitoring)
    ///
    /// # Returns
    /// The next ObjectId that will be allocated
    pub fn current_next_id(&self) -> ObjectId {
        ObjectId::new(self.next_id.load(Ordering::SeqCst))
    }

    /// Check if allocator is in distributed mode
    ///
    /// # Returns
    /// `true` if distributed mode, `false` if standalone
    pub fn is_distributed(&self) -> bool {
        self.raft_manager.is_some()
    }

    /// Reset allocator to a specific ID (dangerous - use with caution)
    ///
    /// Only use during recovery or migration. Can cause ID collisions if misused.
    ///
    /// # Arguments
    /// * `id` - The ID to reset to
    pub fn reset_to(&self, id: ObjectId) {
        self.next_id.store(id.as_u32(), Ordering::SeqCst);
    }

    /// Get the container ID this allocator is for
    pub fn container_id(&self) -> ContainerId {
        self.container_id
    }
}

impl Clone for ObjectAllocator {
    fn clone(&self) -> Self {
        Self {
            container_id: self.container_id,
            next_id: Arc::clone(&self.next_id),
            raft_manager: self.raft_manager.clone(),
        }
    }
}

impl std::fmt::Debug for ObjectAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectAllocator")
            .field("container_id", &self.container_id)
            .field("next_id", &self.next_id.load(Ordering::SeqCst))
            .field("distributed", &self.is_distributed())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::{DatabaseId, TenantId};

    fn test_container_id() -> ContainerId {
        ContainerId::from_parts(TenantId::new(1), DatabaseId::new(1))
    }

    #[tokio::test]
    async fn test_standalone_allocation() {
        let allocator = ObjectAllocator::new_standalone(test_container_id());

        let id1 = allocator.allocate().await.unwrap();
        let id2 = allocator.allocate().await.unwrap();
        let id3 = allocator.allocate().await.unwrap();

        assert_eq!(id1, ObjectId::new(1));
        assert_eq!(id2, ObjectId::new(2));
        assert_eq!(id3, ObjectId::new(3));
    }

    #[tokio::test]
    async fn test_range_allocation() {
        let allocator = ObjectAllocator::new_standalone(test_container_id());

        let range = allocator.allocate_range(10).await.unwrap();
        assert_eq!(range, ObjectId::new(1)..ObjectId::new(11));

        let next_id = allocator.allocate().await.unwrap();
        assert_eq!(next_id, ObjectId::new(11));
    }

    #[test]
    fn test_apply_allocation() {
        let allocator = ObjectAllocator::new_standalone(test_container_id());

        let cmd = AllocationCommand::AllocateObjectId {
            container_id: test_container_id(),
            object_id: ObjectId::new(5),
        };
        allocator.apply_allocation(&cmd);

        assert_eq!(allocator.current_next_id(), ObjectId::new(6));
    }

    #[test]
    fn test_apply_range_allocation() {
        let allocator = ObjectAllocator::new_standalone(test_container_id());

        let cmd = AllocationCommand::AllocateObjectIdRange {
            container_id: test_container_id(),
            start: ObjectId::new(1),
            end: ObjectId::new(11),
        };
        allocator.apply_allocation(&cmd);

        assert_eq!(allocator.current_next_id(), ObjectId::new(11));
    }

    #[test]
    fn test_with_start_id() {
        let allocator =
            ObjectAllocator::with_start_id(test_container_id(), ObjectId::new(100), None);

        let id = allocator.allocate_local().unwrap();
        assert_eq!(id, ObjectId::new(100));
    }

    #[test]
    fn test_reset_to() {
        let allocator = ObjectAllocator::new_standalone(test_container_id());

        allocator.allocate_local().unwrap();
        allocator.allocate_local().unwrap();

        allocator.reset_to(ObjectId::new(50));
        let id = allocator.allocate_local().unwrap();
        assert_eq!(id, ObjectId::new(50));
    }
}
