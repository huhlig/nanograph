# Tablespace Implementation - Remaining Work

## Completed ✅

- [x] Phase 1: Add TablespaceId to core types
- [x] Phase 2: Modify LSM storage engine with tablespace support
- [x] Phase 3: Modify B+Tree storage engine with tablespace support
- [x] Phase 4: Modify ART storage engine with tablespace support
- [x] Phase 5: Add StoragePathResolver to shard manager
- [x] Phase 6: Update KeyValueShardManager infrastructure
- [x] Phase 7: Add tablespace management to database manager
- [x] Phase 8: Update configuration structures (TablespaceCreate, TablespaceUpdate, TablespaceMetadata)
- [x] Phase 9: Create comprehensive test suite
- [x] Phase 10: Add create_shard_with_tablespace trait method
- [x] Documentation: ADR-0028, Implementation Guide, Architecture docs

## TODO - High Priority 🔴

### 1. Fix Pre-existing Database Manager Issues
**File**: `nanograph-kvm/src/database.rs`

- [ ] Fix missing `else` clause in `get_cluster()` method (line 128)
- [ ] Fix missing `else` clause in `create_table()` method (line 386)
- [ ] Remove or fix `metadata_manager` field references (lines 411, 455, 587)
- [ ] Fix `TableCreate` field references (shard_count, replication_factor, partitioner)
- [ ] Add semicolons after `unimplemented!()` macros

### 2. Raft Persistence Integration
**Files**: `nanograph-kvm/src/database.rs`

- [ ] Implement Raft persistence in `create_tablespace()`
  - Serialize tablespace metadata
  - Submit to Raft log via router
  - Wait for consensus confirmation
  
- [ ] Implement Raft persistence in `update_tablespace()`
  - Serialize update operation
  - Submit to Raft log
  - Update local cache after consensus
  
- [ ] Implement Raft persistence in `delete_tablespace()`
  - Submit deletion to Raft log
  - Remove from cache after consensus

### 3. Path Resolver Dynamic Updates
**Files**: `nanograph-kvm/src/database.rs`, `nanograph-kvm/src/shardmgr.rs`

- [ ] Add method to update path resolver with new tablespace config
- [ ] Call path resolver update in `create_tablespace()`
- [ ] Call path resolver update in `update_tablespace()`
- [ ] Call path resolver update in `delete_tablespace()`
- [ ] Add method to reload tablespace configs from storage

### 4. Tablespace Safety Checks
**File**: `nanograph-kvm/src/database.rs`

- [ ] Implement check for tables using tablespace before deletion
- [ ] Add validation for tablespace path existence
- [ ] Add validation for tablespace path permissions
- [ ] Prevent deletion of default tablespace
- [ ] Add cascade delete option with confirmation

## TODO - Medium Priority 🟡

### 5. Configuration File Loading
**Files**: `nanograph-kvm/src/config.rs`, new config loader

- [ ] Define tablespace configuration file format (TOML/YAML)
- [ ] Implement configuration file parser
- [ ] Load tablespace configs on startup
- [ ] Support hot-reload of tablespace configurations
- [ ] Add validation for configuration consistency

### 6. Integration Testing
**Files**: `nanograph-kvm/tests/`

- [ ] Test tablespace creation with actual storage engines
- [ ] Test shard creation with tablespace-aware paths
- [ ] Test path resolution with different tiers
- [ ] Test concurrent tablespace operations
- [ ] Test tablespace operations in distributed mode
- [ ] Test failover scenarios with tablespaces

### 7. Monitoring and Metrics
**Files**: `nanograph-kvm/src/database.rs`, metrics module

- [ ] Add metrics for tablespace operations (create/update/delete)
- [ ] Track tablespace usage (number of tables per tablespace)
- [ ] Monitor storage capacity per tablespace
- [ ] Add alerts for tablespace capacity thresholds
- [ ] Track I/O performance per storage tier

## TODO - Low Priority 🟢

### 8. Advanced Features

- [ ] Implement tablespace quotas
- [ ] Add tablespace compression settings
- [ ] Support tablespace encryption configuration
- [ ] Implement tablespace backup/restore
- [ ] Add tablespace migration tools
- [ ] Support tablespace rebalancing

### 9. Documentation Updates

- [ ] Add API documentation for tablespace methods
- [ ] Create user guide for tablespace management
- [ ] Add examples for common tablespace scenarios
- [ ] Document best practices for storage tiering
- [ ] Create troubleshooting guide

### 10. Performance Optimization

- [ ] Benchmark tablespace operations
- [ ] Optimize path resolution caching
- [ ] Implement lazy loading for tablespace metadata
- [ ] Add connection pooling per tablespace
- [ ] Optimize concurrent access patterns

## Notes

### Current TODOs in Code

The following TODO comments are already in the codebase:

**`nanograph-kvm/src/database.rs`**:
- Line 262: "TODO: Persist to system shard via Raft if in distributed mode"
- Line 263: "TODO: Update shard manager's path resolver with new tablespace config"
- Line 287: "TODO: Persist to system shard via Raft if in distributed mode"
- Line 288: "TODO: Update shard manager's path resolver with updated tablespace config"
- Line 304: "TODO: Check if any tables are using this tablespace"
- Line 305: "TODO: Prevent deletion if tables exist in this tablespace"
- Line 310: "TODO: Persist deletion to system shard via Raft if in distributed mode"
- Line 311: "TODO: Update shard manager's path resolver to remove tablespace config"

**`nanograph-kvm/src/shardmgr.rs`**:
- Line 217: "TODO: Persist to system shard via Raft if in distributed mode"
- Line 218: "TODO: Update shard manager's path resolver with new tablespace config"

### Testing Status

All 8 tablespace tests are written and syntactically correct:
- ✅ test_create_tablespace
- ✅ test_list_tablespaces
- ✅ test_update_tablespace
- ✅ test_update_nonexistent_tablespace
- ✅ test_delete_tablespace
- ✅ test_tablespace_with_options_and_metadata
- ✅ test_tablespace_storage_tiers
- ✅ test_get_nonexistent_tablespace

Tests cannot run until pre-existing database.rs compilation errors are fixed.

### Dependencies

- Raft integration depends on fixing database manager issues
- Integration tests depend on Raft persistence
- Configuration loading can be done independently
- Monitoring can be added incrementally