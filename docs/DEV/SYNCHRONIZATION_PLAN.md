# Synchronization Implementation Plan

## Overview

This document outlines the implementation plan for ensuring synchronization between `KeyValueDatabaseManager`, `KeyValueShardManager`, `MetadataCache`, and network consensus (Raft) in the Nanograph distributed key-value store.

## Problem Statement

Currently, there are synchronization gaps between components:

1. **MetadataCache** (`nanograph-kvm/src/metacache.rs`) is in-memory only and not backed by persistent storage or consensus
2. **Table creation** updates the metadata cache locally without ensuring Raft consensus for metadata changes
3. **No version tracking** or conflict resolution mechanisms exist
4. **Missing synchronization barriers** between storage, cache, and consensus layers

## Architecture Goals

### Consistency Guarantees

- **Linearizability**: All metadata operations must go through Raft consensus
- **Durability**: WAL ensures operations survive crashes
- **Consistency**: Version tracking prevents stale updates
- **Availability**: Quorum-based replication ensures availability during failures

### Component Synchronization Flow

```
Client Request
    ↓
KeyValueDatabaseManager
    ↓
[Distributed Mode?]
    ↓ Yes                    ↓ No
ConsensusRouter         Direct ShardManager
    ↓                           ↓
Raft Consensus                  ↓
    ↓                           ↓
Replicate to Quorum             ↓
    ↓                           ↓
Apply to Storage ←──────────────┘
    ↓
Update MetadataCache
    ↓
Persist Metadata
    ↓
Response to Client
```

## Implementation Phases

### Phase 1: Consensus-Backed Metadata Storage

**Goal**: Make MetadataCache persistent and consensus-backed

**Files to Modify**:
- `nanograph-kvm/src/metacache.rs`
- `nanograph-kvm/src/database.rs`
- `nanograph-raft/src/metadata.rs`

**Changes**:

1. **Add metadata shard backing to MetadataCache**:
```rust
pub struct MetadataCache {
    // In-memory cache (existing)
    cluster: ClusterMetadata,
    regions: HashMap<RegionId, RegionMetadata>,
    servers: HashMap<ServerId, ServerMetadata>,
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    tables: HashMap<TableId, TableMetadata>,
    shards: HashMap<ShardId, ShardMetadata>,
    
    // NEW: Backing storage
    metadata_shard: Option<ShardId>,
    consensus_router: Option<Arc<ConsensusRouter>>,
    
    // Existing resolver fields...
}
```

2. **Add persistence methods**:
```rust
impl MetadataCache {
    /// Persist metadata to backing shard
    pub async fn persist(&self) -> Result<(), MetadataError> {
        if let Some(router) = &self.consensus_router {
            // Serialize and persist via Raft
        }
    }
    
    /// Load metadata from backing shard
    pub async fn load(&mut self) -> Result<(), MetadataError> {
        if let Some(router) = &self.consensus_router {
            // Load and deserialize from Raft
        }
    }
}
```

**Acceptance Criteria**:
- [ ] MetadataCache can be persisted to a dedicated Raft shard
- [ ] MetadataCache can be loaded from persistent storage on startup
- [ ] All metadata mutations are written through consensus

### Phase 2: Version Tracking and Conflict Resolution

**Goal**: Add version tracking to prevent stale updates

**Files to Modify**:
- `nanograph-kvt/src/types.rs`
- `nanograph-kvm/src/metacache.rs`

**Changes**:

1. **Add version fields to metadata structures**:
```rust
pub struct TableMetadata {
    pub id: TableId,
    pub name: String,
    pub path: String,
    pub created_at: Timestamp,
    pub engine_type: StorageEngineType,
    pub last_modified: Timestamp,
    pub sharding: TableSharding,
    
    // NEW: Version tracking
    pub version: u64,
    pub last_modified_by: Option<NodeId>,
}

pub struct ShardMetadata {
    pub id: ShardId,
    pub table_id: TableId,
    pub key_range: (Vec<u8>, Vec<u8>),
    pub replicas: Vec<NodeId>,
    pub created_at: Timestamp,
    
    // NEW: Version tracking
    pub version: u64,
    pub last_modified: Timestamp,
}
```

2. **Add conflict detection**:
```rust
#[derive(Debug, Clone)]
pub enum ConflictError {
    StaleVersion { expected: u64, got: u64 },
    ConcurrentModification,
}

impl MetadataCache {
    pub fn update_table_metadata(&mut self, metadata: TableMetadata) 
        -> Result<(), ConflictError> {
        
        if let Some(existing) = self.tables.get(&metadata.id) {
            if existing.version >= metadata.version {
                return Err(ConflictError::StaleVersion {
                    expected: existing.version + 1,
                    got: metadata.version,
                });
            }
        }
        self.tables.insert(metadata.id, metadata);
        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] All metadata structures have version fields
- [ ] Stale updates are detected and rejected
- [ ] Version increments are atomic with updates

### Phase 3: Two-Phase Metadata Updates

**Goal**: Ensure metadata changes go through consensus before local application

**Files to Modify**:
- `nanograph-kvm/src/database.rs`
- `nanograph-raft/src/router.rs`
- `nanograph-raft/src/metadata.rs`

**Changes**:

1. **Refactor create_table to use two-phase commit**:
```rust
impl KeyValueDatabaseManager {
    pub async fn create_table(
        &self,
        path: &str,
        name: String,
        config: TableConfig,
    ) -> KeyValueResult<TableId> {
        if let Some(router) = &self.raft_router {
            // Phase 1: Propose metadata change via Raft
            let table_id = self.allocate_table_id().await?;
            let metadata_op = MetadataOperation::CreateTable {
                table_id,
                path: path.to_string(),
                name: name.clone(),
                config: config.clone(),
            };
            
            router.metadata()
                .propose_metadata_change(metadata_op)
                .await?;
            
            // Phase 2: Create shards via Raft (existing code)
            match config.sharding_config {
                TableSharding::Single => {
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                    let replicas = self.select_replicas(1)?;
                    
                    router.metadata()
                        .create_shard(shard_id, (vec![], vec![0xFF; 32]), replicas)
                        .await?;
                }
                TableSharding::Multiple { shard_count, replication_factor, .. } => {
                    for shard_index in 0..shard_count {
                        let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));
                        let replicas = self.select_replicas(replication_factor)?;
                        
                        router.metadata()
                            .create_shard(shard_id, (vec![], vec![0xFF; 32]), replicas)
                            .await?;
                    }
                }
            }
            
            // Phase 3: Update local cache after consensus
            let mut metadata = self.metadata_manager.write().unwrap();
            metadata.add_table(path, config);
            
            Ok(table_id)
        } else {
            // Single-node mode: direct update
            let table_id = self.allocate_table_id().await?;
            let mut metadata = self.metadata_manager.write().unwrap();
            metadata.add_table(path, config);
            Ok(table_id)
        }
    }
}
```

2. **Add metadata operation types**:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MetadataOperation {
    CreateTable {
        table_id: TableId,
        path: String,
        name: String,
        config: TableConfig,
    },
    DropTable {
        table_id: TableId,
    },
    CreateShard {
        shard_id: ShardId,
        key_range: (Vec<u8>, Vec<u8>),
        replicas: Vec<NodeId>,
    },
    UpdateShardReplicas {
        shard_id: ShardId,
        replicas: Vec<NodeId>,
    },
}
```

**Acceptance Criteria**:
- [ ] All metadata mutations go through Raft consensus
- [ ] Local cache is updated only after consensus is achieved
- [ ] Rollback mechanisms exist for failed operations

### Phase 4: WAL Integration and Ordering

**Goal**: Ensure all operations flow through WAL with proper ordering

**Files to Modify**:
- `nanograph-raft/src/storage.rs`
- `nanograph-raft/src/shard_group.rs`

**Changes**:

1. **Enhance RaftStorageAdapter with ordering guarantees**:
```rust
impl RaftStorageAdapter {
    /// Apply operation with ordering guarantees
    pub async fn apply_operation_ordered(
        &self,
        operation: &Operation,
        expected_index: u64,
    ) -> ConsensusResult<OperationResponse> {
        // Verify ordering
        let last_applied = self.raft_state.read().await.last_applied;
        if expected_index != last_applied + 1 {
            return Err(ConsensusError::OutOfOrder {
                expected: last_applied + 1,
                got: expected_index,
            });
        }
        
        // Apply operation
        self.apply_operation(operation).await
    }
}
```

2. **Add WAL replay on recovery**:
```rust
impl RaftStorageAdapter {
    /// Replay WAL entries on recovery
    pub async fn replay_wal(&self, from_index: u64) -> ConsensusResult<()> {
        let entries = self.get_log_entries(from_index, u64::MAX).await?;
        
        for entry in entries {
            if entry.index > self.raft_state.read().await.last_applied {
                self.apply_operation(&entry.operation).await?;
            }
        }
        
        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] All operations are logged before application
- [ ] Operations are applied in order
- [ ] WAL can be replayed on recovery

### Phase 5: Synchronization Barriers and Verification

**Goal**: Add explicit sync points and consistency verification

**Files to Modify**:
- `nanograph-kvm/src/database.rs`
- `nanograph-kvm/src/shardmgr.rs`

**Changes**:

1. **Add sync method**:
```rust
impl KeyValueDatabaseManager {
    /// Ensure all components are synchronized
    pub async fn sync(&self) -> KeyValueResult<()> {
        // 1. Flush storage engines
        self.shard_manager.read().unwrap().flush().await?;
        
        // 2. Ensure Raft commits are applied
        if let Some(router) = &self.raft_router {
            router.wait_for_applied().await?;
        }
        
        // 3. Persist metadata cache
        self.metadata_manager.read().unwrap().persist().await
            .map_err(|e| KeyValueError::Internal(e.to_string()))?;
        
        Ok(())
    }
}
```

2. **Add consistency verification**:
```rust
#[derive(Debug, Clone)]
pub enum InconsistencyReport {
    MissingShard { table_id: TableId, shard_id: ShardId },
    OrphanedShard { shard_id: ShardId },
    VersionMismatch { object_id: ObjectId, cache_version: u64, storage_version: u64 },
    ReplicaMismatch { shard_id: ShardId, expected: Vec<NodeId>, actual: Vec<NodeId> },
}

impl KeyValueDatabaseManager {
    /// Verify consistency between components
    pub async fn verify_consistency(&self) -> KeyValueResult<Vec<InconsistencyReport>> {
        let mut issues = Vec::new();
        
        let metadata = self.metadata_manager.read().unwrap();
        let shard_mgr = self.shard_manager.read().unwrap();
        
        // Check metadata cache vs storage
        for table in metadata.get_tables() {
            let shard_ids = self.get_shard_ids_for_table(table.id)?;
            
            for shard_id in shard_ids {
                if !shard_mgr.shard_exists(shard_id) {
                    issues.push(InconsistencyReport::MissingShard {
                        table_id: table.id,
                        shard_id,
                    });
                }
            }
        }
        
        // Check for orphaned shards
        for shard_state in shard_mgr.list_shards()? {
            let table_id = shard_state.id.table_id();
            if metadata.get_table_metadata(&table_id).is_none() {
                issues.push(InconsistencyReport::OrphanedShard {
                    shard_id: shard_state.id,
                });
            }
        }
        
        // Check Raft consensus state
        if let Some(router) = &self.raft_router {
            let consensus_metadata = router.metadata().get_all_tables().await?;
            
            for (table_id, consensus_table) in consensus_metadata {
                if let Some(cache_table) = metadata.get_table_metadata(&table_id) {
                    if cache_table.version != consensus_table.version {
                        issues.push(InconsistencyReport::VersionMismatch {
                            object_id: ObjectId(table_id.0),
                            cache_version: cache_table.version,
                            storage_version: consensus_table.version,
                        });
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    /// Repair detected inconsistencies
    pub async fn repair_inconsistencies(
        &self,
        issues: Vec<InconsistencyReport>,
    ) -> KeyValueResult<()> {
        for issue in issues {
            match issue {
                InconsistencyReport::MissingShard { table_id, shard_id } => {
                    // Recreate missing shard
                    self.recreate_shard(table_id, shard_id).await?;
                }
                InconsistencyReport::OrphanedShard { shard_id } => {
                    // Remove orphaned shard
                    self.shard_manager.read().unwrap().drop_shard(shard_id).await?;
                }
                InconsistencyReport::VersionMismatch { .. } => {
                    // Reload metadata from consensus
                    self.reload_metadata_from_consensus().await?;
                }
                InconsistencyReport::ReplicaMismatch { shard_id, expected, .. } => {
                    // Update replica set
                    self.update_shard_replicas(shard_id, expected).await?;
                }
            }
        }
        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] Sync method ensures all components are consistent
- [ ] Verification detects all types of inconsistencies
- [ ] Repair mechanisms can fix detected issues

### Phase 6: Recovery and Fault Tolerance

**Goal**: Implement recovery procedures for various failure scenarios

**Files to Create**:
- `nanograph-kvm/src/recovery.rs`

**Changes**:

1. **Add recovery coordinator**:
```rust
pub struct RecoveryCoordinator {
    database_manager: Arc<KeyValueDatabaseManager>,
}

impl RecoveryCoordinator {
    /// Recover from node crash
    pub async fn recover_from_crash(&self) -> KeyValueResult<()> {
        // 1. Replay WAL
        self.replay_wal().await?;
        
        // 2. Reload metadata from consensus
        self.reload_metadata().await?;
        
        // 3. Verify consistency
        let issues = self.database_manager.verify_consistency().await?;
        
        // 4. Repair if needed
        if !issues.is_empty() {
            self.database_manager.repair_inconsistencies(issues).await?;
        }
        
        // 5. Rejoin Raft groups
        self.rejoin_raft_groups().await?;
        
        Ok(())
    }
    
    /// Recover from network partition
    pub async fn recover_from_partition(&self) -> KeyValueResult<()> {
        // 1. Detect stale data
        let issues = self.database_manager.verify_consistency().await?;
        
        // 2. Sync with leader
        if let Some(router) = self.database_manager.consensus_router() {
            router.sync_with_leader().await?;
        }
        
        // 3. Repair inconsistencies
        self.database_manager.repair_inconsistencies(issues).await?;
        
        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] System can recover from node crashes
- [ ] System can recover from network partitions
- [ ] No data loss during recovery

## Testing Strategy

### Unit Tests

1. **Metadata versioning tests**:
   - Test version increment on updates
   - Test stale update rejection
   - Test concurrent update handling

2. **Synchronization tests**:
   - Test sync method flushes all components
   - Test consistency verification detects issues
   - Test repair mechanisms fix issues

### Integration Tests

1. **End-to-end synchronization tests**:
   - Create table → verify metadata in cache and consensus
   - Update table → verify version increments
   - Drop table → verify cleanup in all components

2. **Failure scenario tests**:
   - Node crash during table creation
   - Network partition during metadata update
   - Leader election during write operation

### Performance Tests

1. **Synchronization overhead**:
   - Measure latency of consensus-backed metadata operations
   - Compare single-node vs distributed mode performance

2. **Recovery time**:
   - Measure time to recover from crash
   - Measure time to recover from partition

## Implementation Timeline

| Phase | Estimated Duration | Dependencies |
|-------|-------------------|--------------|
| Phase 1: Consensus-Backed Metadata | 2 weeks | None |
| Phase 2: Version Tracking | 1 week | Phase 1 |
| Phase 3: Two-Phase Updates | 2 weeks | Phase 1, 2 |
| Phase 4: WAL Integration | 1 week | Phase 3 |
| Phase 5: Sync & Verification | 2 weeks | Phase 1-4 |
| Phase 6: Recovery | 2 weeks | Phase 1-5 |
| **Total** | **10 weeks** | |

## Success Metrics

- [ ] All metadata operations are linearizable
- [ ] Zero data loss during failures
- [ ] Consistency verification passes 100% of the time
- [ ] Recovery time < 10 seconds for typical workloads
- [ ] Synchronization overhead < 10% compared to single-node mode

## References

- `nanograph-kvm/src/database.rs` - KeyValueDatabaseManager
- `nanograph-kvm/src/shardmgr.rs` - KeyValueShardManager
- `nanograph-kvm/src/metacache.rs` - MetadataCache
- `nanograph-raft/src/storage.rs` - RaftStorageAdapter
- `nanograph-raft/src/shard_group.rs` - ShardRaftGroup
- `nanograph-raft/src/router.rs` - ConsensusRouter