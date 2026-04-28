# Nanograph Index Implementation Plan

## Overview

This document provides a detailed implementation plan to address critical gaps in the nanograph-idx crate, focusing on persistence, replication, distribution, and production readiness.

**Status**: Planning Phase  
**Target Completion**: 12 weeks  
**Last Updated**: 2026-01-27

---

## Current State Assessment

### ✅ Completed
- Trait hierarchy design (IndexStore, OrderedIndex, TextSearchIndex, etc.)
- Basic trait definitions for 11 index types
- Stub implementations for B-Tree and Hash indexes
- Comprehensive design documentation

### ❌ Critical Gaps
- No persistence layer integration
- No Raft-based replication
- No distributed query execution
- Incomplete index building logic
- Minimal test coverage

---

## Implementation Priorities

### Phase 1: Persistence Integration (Weeks 1-3)
### Phase 2: Raft Integration & Replication (Weeks 4-6)
### Phase 3: Query Execution (Weeks 7-8)
### Phase 4: Index Building & Maintenance (Weeks 9-10)
### Phase 5: Testing & Validation (Weeks 11-12)

---

# Phase 1: Persistence Integration (Weeks 1-3)

## Objective
Connect indexes to the underlying storage layer (nanograph-kvt) and ensure durability through WAL integration.

## Week 1: Storage Layer Integration

### Task 1.1: Add Storage Dependencies
**File**: `nanograph-idx/Cargo.toml`

Add required dependencies for storage integration.

### Task 1.2: Create Persistence Module
**File**: `nanograph-idx/src/persistence.rs`

Implement `PersistentIndexStore` that:
- Wraps KeyValueShardStore for persistent storage
- Integrates WriteAheadLogManager for durability
- Provides write_entry(), read_entry(), delete_entry() methods
- Implements scan_range() for range queries
- Handles serialization/deserialization

### Task 1.3: Update B-Tree Index Implementation
**File**: `nanograph-idx/src/index/ordered/btree.rs`

Replace in-memory BTreeMap with PersistentIndexStore:
- Add storage field with PersistentIndexStore
- Add LRU cache for hot entries
- Update insert/delete/query methods to use persistent storage
- Implement proper flush() method

## Week 2: Serialization & Recovery

### Task 2.1: Implement Index Entry Serialization
**File**: `nanograph-idx/src/serialization.rs`

Create serialization layer:
- Define SerializedIndexEntry with versioning
- Implement serialize_entry() and deserialize_entry()
- Use bincode for efficient binary serialization
- Handle backward compatibility

### Task 2.2: Implement Index Recovery
**File**: `nanograph-idx/src/recovery.rs`

Implement IndexRecovery for crash recovery:
- recover_from_wal() - replay WAL entries
- create_snapshot() - checkpoint index state
- recover_from_snapshot() - restore from checkpoint
- Handle partial recovery scenarios

## Week 3: Index Metadata & Statistics

### Task 3.1: Implement Index Metadata Persistence
**File**: `nanograph-idx/src/metadata.rs`

Create IndexMetadataStore:
- save_metadata() - persist IndexRecord
- load_metadata() - restore IndexRecord
- save_stats() - persist IndexStats
- load_stats() - restore IndexStats

### Task 3.2: Add Statistics Tracking
**File**: `nanograph-idx/src/stats.rs`

Implement IndexStatsTracker:
- Track entry_count, size_bytes atomically
- Record insert/delete/query operations
- Calculate avg_entry_size
- Provide get_stats() for current statistics

**Deliverables**:
- ✅ Persistent storage integration
- ✅ WAL-based durability
- ✅ Crash recovery mechanism
- ✅ Metadata persistence
- ✅ Statistics tracking

---

# Phase 2: Raft Integration & Replication (Weeks 4-6)

## Objective
Integrate indexes with Raft consensus for replication, high availability, and strong consistency.

## Week 4: Raft Wrapper Implementation

### Task 4.1: Create Distributed Index Wrapper
**File**: `nanograph-idx/src/distributed.rs`

Implement DistributedIndex:
- Wrap local IndexStore implementation
- Integrate with ShardGroup (Raft consensus)
- Define IndexCommand enum (Insert, Update, Delete, Flush)
- Implement propose_command() for Raft proposals
- Implement apply_command() for local application
- Check is_leader() before write operations

Key features:
- All writes go through Raft consensus
- Reads can be served locally (eventual consistency)
- Leader election and failover support
- Strong consistency guarantees

## Week 5: Raft State Machine Integration

### Task 5.1: Implement Index State Machine
**File**: `nanograph-idx/src/state_machine.rs`

Create IndexStateMachine implementing RaftStateMachine:
- apply() - apply committed log entries
- snapshot() - create state machine snapshot
- restore_snapshot() - restore from snapshot
- Track last_applied log index

### Task 5.2: Add Snapshot Support
**File**: `nanograph-idx/src/snapshot.rs`

Implement IndexSnapshotManager:
- create_snapshot() - serialize all index entries
- restore_snapshot() - rebuild index from snapshot
- list_snapshots() - enumerate available snapshots
- cleanup_old_snapshots() - remove old snapshots

Snapshot format:
- Binary format with entry length prefix
- Supports incremental reading
- Compressed for large indexes

## Week 6: Replication Testing & Failover

### Task 6.1: Add Replication Tests
**File**: `nanograph-idx/tests/replication_tests.rs`

Test scenarios:
- test_replicated_insert() - verify write replication
- test_leader_failover() - verify failover works
- test_split_brain_prevention() - verify no split brain
- test_network_partition() - verify partition handling

### Task 6.2: Add Consistency Tests
**File**: `nanograph-idx/tests/consistency_tests.rs`

Test scenarios:
- test_linearizable_reads() - verify read consistency
- test_snapshot_isolation() - verify snapshot consistency
- test_concurrent_writes() - verify write serialization
- test_read_your_writes() - verify session consistency

**Deliverables**:
- ✅ Raft-based replication
- ✅ Leader election and failover
- ✅ Snapshot-based recovery
- ✅ Strong consistency guarantees
- ✅ Comprehensive replication tests

---

# Phase 3: Query Execution (Weeks 7-8)

## Objective
Implement efficient query execution for all index types with proper optimization.

## Week 7: Query Implementation

### Task 7.1: Implement B-Tree Range Queries
**File**: `nanograph-idx/src/index/ordered/btree.rs`

Implement OrderedIndex trait:
- range_scan() - efficient range queries with bounds
- min_key() - get minimum key
- max_key() - get maximum key
- prefix_scan() - prefix matching
- count_range() - count entries in range

Optimizations:
- Use storage layer's native range scan
- Apply bounds filtering efficiently
- Support reverse scans
- Implement limit pushdown

### Task 7.2: Implement Hash Unique Lookups
**File**: `nanograph-idx/src/index/ordered/hash.rs`

Implement UniqueIndex trait:
- lookup_unique() - O(1) unique value lookup
- validate_unique() - check uniqueness before insert
- Use hash function for key distribution

### Task 7.3: Implement Query Optimization
**File**: `nanograph-idx/src/query_optimizer.rs`

Create QueryOptimizer:
- register_index() - register available indexes
- select_index() - choose best index for query
- estimate_cost() - estimate query cost
- Support multiple index types

Cost model:
- Equality: O(1) for hash indexes
- Range: O(log n + k) for B-tree indexes
- Full-text: O(k) for inverted indexes
- Spatial: O(log n + k) for R-tree indexes

## Week 8: Distributed Query Execution

### Task 8.1: Implement Query Router
**File**: `nanograph-idx/src/query_router.rs`

Create QueryRouter for distributed queries:
- route_query() - route to appropriate nodes
- aggregate_results() - combine results from multiple nodes
- handle_node_failure() - retry on failure
- Support query parallelization

### Task 8.2: Implement Query Execution Engine
**File**: `nanograph-idx/src/query_executor.rs`

Create QueryExecutor:
- execute_local() - execute on local node
- execute_distributed() - execute across cluster
- apply_filters() - post-query filtering
- apply_projections() - column selection
- apply_sorting() - result ordering
- apply_pagination() - limit/offset

### Task 8.3: Add Query Caching
**File**: `nanograph-idx/src/query_cache.rs`

Implement QueryCache:
- Cache frequently executed queries
- Invalidate on index updates
- LRU eviction policy
- Configurable cache size

**Deliverables**:
- ✅ Efficient range queries
- ✅ Unique constraint lookups
- ✅ Query optimization
- ✅ Distributed query execution
- ✅ Query result caching

---

# Phase 4: Index Building & Maintenance (Weeks 9-10)

## Objective
Implement robust index building from table data and ongoing maintenance operations.

## Week 9: Index Building

### Task 9.1: Implement Bulk Index Building
**File**: `nanograph-idx/src/builder.rs`

Create IndexBuilder:
- build_from_table() - build index from table scan
- build_in_batches() - batch processing for large tables
- track_progress() - report build progress
- handle_errors() - error recovery during build

Building strategies:
- **Online building**: Build while table is accessible
- **Offline building**: Build with table locked
- **Incremental building**: Build in chunks
- **Parallel building**: Use multiple threads

### Task 9.2: Implement Index Status Management
**File**: `nanograph-idx/src/status.rs`

Track index lifecycle:
- Building - index is being built
- Active - index is ready for queries
- Rebuilding - index is being rebuilt
- Disabled - index is temporarily disabled
- Failed - index build failed

### Task 9.3: Add Progress Tracking
**File**: `nanograph-idx/src/progress.rs`

Implement BuildProgress:
- Track rows processed
- Estimate time remaining
- Report percentage complete
- Handle cancellation

### Task 9.4: Implement Incremental Updates
**File**: `nanograph-idx/src/incremental.rs`

Support incremental index updates:
- extract_indexed_values() - extract values from row
- update_on_insert() - update index on row insert
- update_on_update() - update index on row update
- update_on_delete() - update index on row delete

## Week 10: Index Maintenance

### Task 10.1: Implement Index Optimization
**File**: `nanograph-idx/src/optimizer.rs`

Create IndexOptimizer:
- compact() - remove deleted entries
- rebalance() - rebalance tree structures
- rebuild() - full index rebuild
- analyze() - collect statistics

Optimization triggers:
- Scheduled maintenance windows
- Fragmentation threshold exceeded
- Performance degradation detected
- Manual trigger

### Task 10.2: Implement Index Validation
**File**: `nanograph-idx/src/validator.rs`

Create IndexValidator:
- validate_structure() - check index structure integrity
- validate_consistency() - verify index matches table
- validate_uniqueness() - check unique constraints
- validate_statistics() - verify statistics accuracy

### Task 10.3: Add Index Monitoring
**File**: `nanograph-idx/src/monitoring.rs`

Implement IndexMonitor:
- Track query performance metrics
- Monitor index size growth
- Detect performance degradation
- Alert on anomalies

Metrics to track:
- Query latency (p50, p95, p99)
- Query throughput (queries/sec)
- Index size (bytes)
- Fragmentation percentage
- Cache hit rate

### Task 10.4: Implement Index Repair
**File**: `nanograph-idx/src/repair.rs`

Create IndexRepair:
- detect_corruption() - find corrupted entries
- repair_corruption() - fix corrupted entries
- rebuild_if_needed() - rebuild on severe corruption
- verify_repair() - verify repair success

**Deliverables**:
- ✅ Bulk index building
- ✅ Incremental updates
- ✅ Index optimization
- ✅ Index validation
- ✅ Monitoring and alerting
- ✅ Corruption repair

---

# Phase 5: Testing & Validation (Weeks 11-12)

## Objective
Comprehensive testing to ensure production readiness and correctness.

## Week 11: Comprehensive Testing

### Task 11.1: Unit Tests
**File**: `nanograph-idx/tests/unit/`

Test coverage for all modules:
- **persistence_tests.rs** - storage layer tests
- **serialization_tests.rs** - serialization tests
- **recovery_tests.rs** - recovery tests
- **metadata_tests.rs** - metadata tests
- **stats_tests.rs** - statistics tests

Test scenarios:
- Normal operations
- Edge cases (empty index, single entry, etc.)
- Error conditions
- Boundary values

### Task 11.2: Integration Tests
**File**: `nanograph-idx/tests/integration/`

End-to-end test scenarios:
- **btree_integration_tests.rs** - B-tree index tests
- **hash_integration_tests.rs** - hash index tests
- **distributed_integration_tests.rs** - distributed tests
- **replication_integration_tests.rs** - replication tests
- **query_integration_tests.rs** - query execution tests

Test scenarios:
- Full index lifecycle (create, build, query, maintain, delete)
- Multi-node operations
- Failover scenarios
- Concurrent operations

### Task 11.3: Performance Tests
**File**: `nanograph-idx/benches/`

Benchmark critical operations:
- **insert_benchmarks.rs** - insert performance
- **query_benchmarks.rs** - query performance
- **build_benchmarks.rs** - build performance
- **replication_benchmarks.rs** - replication overhead

Metrics to measure:
- Throughput (ops/sec)
- Latency (p50, p95, p99)
- Memory usage
- CPU usage
- Network bandwidth (for distributed)

### Task 11.4: Correctness Tests
**File**: `nanograph-idx/tests/correctness/`

Verify correctness:
- **uniqueness_tests.rs** - unique constraint enforcement
- **consistency_tests.rs** - data consistency
- **isolation_tests.rs** - transaction isolation
- **durability_tests.rs** - crash recovery

Test scenarios:
- Verify unique indexes reject duplicates
- Verify index matches table data
- Verify concurrent operations don't corrupt data
- Verify data survives crashes

## Week 12: Production Readiness

### Task 12.1: Stress Testing
**File**: `nanograph-idx/tests/stress/`

Stress test scenarios:
- **high_load_tests.rs** - sustained high load
- **spike_tests.rs** - sudden load spikes
- **endurance_tests.rs** - long-running tests
- **chaos_tests.rs** - random failures

Test conditions:
- 10,000+ concurrent operations
- 1M+ index entries
- Network partitions
- Node failures
- Disk full scenarios

### Task 12.2: Documentation
**Files**: Various documentation files

Complete documentation:
- **API_REFERENCE.md** - complete API documentation
- **OPERATIONS_GUIDE.md** - operational procedures
- **TROUBLESHOOTING.md** - common issues and solutions
- **PERFORMANCE_TUNING.md** - performance optimization guide

### Task 12.3: Examples
**File**: `nanograph-idx/examples/`

Practical examples:
- **basic_usage.rs** - simple index usage
- **distributed_setup.rs** - multi-node setup
- **query_optimization.rs** - query optimization
- **maintenance.rs** - index maintenance
- **monitoring.rs** - monitoring setup

### Task 12.4: Migration Guide
**File**: `nanograph-idx/MIGRATION_GUIDE.md`

Migration documentation:
- Upgrading from in-memory to persistent indexes
- Migrating between index types
- Zero-downtime migration strategies
- Rollback procedures

**Deliverables**:
- ✅ Comprehensive unit tests (>80% coverage)
- ✅ Integration tests for all scenarios
- ✅ Performance benchmarks
- ✅ Correctness validation
- ✅ Stress testing
- ✅ Complete documentation
- ✅ Practical examples
- ✅ Migration guide

---

# Success Criteria

## Phase 1: Persistence Integration
- [ ] All indexes use persistent storage
- [ ] WAL integration complete
- [ ] Crash recovery works
- [ ] Metadata persisted correctly
- [ ] Statistics tracked accurately

## Phase 2: Raft Integration
- [ ] Indexes replicate via Raft
- [ ] Leader election works
- [ ] Failover is automatic
- [ ] Snapshots work correctly
- [ ] Strong consistency guaranteed

## Phase 3: Query Execution
- [ ] Range queries work efficiently
- [ ] Unique lookups are O(1)
- [ ] Query optimizer selects best index
- [ ] Distributed queries work
- [ ] Query cache improves performance

## Phase 4: Index Building
- [ ] Bulk building works for large tables
- [ ] Incremental updates work
- [ ] Index optimization reduces fragmentation
- [ ] Validation detects corruption
- [ ] Monitoring provides visibility

## Phase 5: Testing & Validation
- [ ] >80% test coverage
- [ ] All integration tests pass
- [ ] Performance meets targets
- [ ] Correctness verified
- [ ] Stress tests pass
- [ ] Documentation complete

---

# Performance Targets

## Latency Targets
- Point lookup: <1ms (p99)
- Range scan (100 entries): <10ms (p99)
- Insert: <5ms (p99)
- Delete: <5ms (p99)

## Throughput Targets
- Reads: >100,000 ops/sec (single node)
- Writes: >10,000 ops/sec (single node)
- Distributed reads: >500,000 ops/sec (5 nodes)
- Distributed writes: >50,000 ops/sec (5 nodes)

## Scalability Targets
- Support 1B+ entries per index
- Support 1000+ concurrent queries
- Support 100+ nodes in cluster
- Linear scalability up to 10 nodes

## Availability Targets
- 99.99% uptime
- <30s failover time
- Zero data loss on single node failure
- Survive 2 node failures in 5-node cluster

---

# Risk Mitigation

## Technical Risks

### Risk 1: Performance Degradation
**Mitigation**:
- Continuous benchmarking
- Performance regression tests
- Profiling and optimization
- Caching strategies

### Risk 2: Data Corruption
**Mitigation**:
- Checksums on all data
- Regular validation
- Automated repair
- Backup and recovery procedures

### Risk 3: Replication Lag
**Mitigation**:
- Monitor replication lag
- Tune Raft parameters
- Implement read-your-writes consistency
- Add replication metrics

### Risk 4: Complexity
**Mitigation**:
- Modular design
- Comprehensive documentation
- Code reviews
- Incremental rollout

## Operational Risks

### Risk 1: Difficult Debugging
**Mitigation**:
- Extensive logging
- Distributed tracing
- Debug tools
- Runbooks

### Risk 2: Upgrade Challenges
**Mitigation**:
- Backward compatibility
- Rolling upgrades
- Canary deployments
- Rollback procedures

### Risk 3: Resource Exhaustion
**Mitigation**:
- Resource limits
- Backpressure mechanisms
- Monitoring and alerting
- Auto-scaling

---

# Dependencies

## Internal Dependencies
- nanograph-kvt (storage layer)
- nanograph-wal (write-ahead log)
- nanograph-raft (consensus)
- nanograph-vfs (virtual filesystem)
- nanograph-serde (serialization)
- nanograph-core (core types)

## External Dependencies
- openraft (Raft implementation)
- tokio (async runtime)
- bincode (serialization)
- serde (serialization framework)
- lru (LRU cache)

---

# Timeline Summary

| Phase | Duration | Key Deliverables |
|-------|----------|------------------|
| Phase 1 | Weeks 1-3 | Persistence integration, WAL, recovery |
| Phase 2 | Weeks 4-6 | Raft integration, replication, snapshots |
| Phase 3 | Weeks 7-8 | Query execution, optimization, distribution |
| Phase 4 | Weeks 9-10 | Index building, maintenance, monitoring |
| Phase 5 | Weeks 11-12 | Testing, documentation, production readiness |

**Total Duration**: 12 weeks  
**Team Size**: 2-3 engineers  
**Estimated Effort**: 6-9 person-months

---

# Next Steps

## Immediate Actions (Week 1)
1. Review and approve implementation plan
2. Set up development environment
3. Create feature branches
4. Begin Task 1.1: Add storage dependencies
5. Schedule weekly progress reviews

## Weekly Checkpoints
- Monday: Sprint planning
- Wednesday: Mid-week sync
- Friday: Demo and retrospective

## Monthly Milestones
- End of Month 1: Phase 1 complete
- End of Month 2: Phase 2 complete
- End of Month 3: Phases 3-5 complete

---

**Document Version**: 1.0  
**Status**: Ready for Review  
**Next Review**: After Phase 1 completion  
**Owner**: Nanograph Index Team