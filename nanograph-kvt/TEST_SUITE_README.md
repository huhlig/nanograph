# KeyValueShardStore Test Suite

This document describes the common test suite for `KeyValueShardStore` implementations in nanograph-kvt.

## Overview

The test suite provides comprehensive testing for any implementation of the `KeyValueShardStore` trait, similar to how nanograph-vfs provides `run_generic_test_suite()` for `FileSystem` implementations.

## Location

- **Test Suite**: `nanograph-kvt/src/test_suite.rs`
- **Function**: `run_kvstore_test_suite<S: KeyValueShardStore>(store: &S)`

## Usage

### In Your Implementation Tests

Add the test-utils feature to your dev-dependencies:

```toml
[dev-dependencies]
nanograph-kvt = { workspace = true, features = ["test-utils"] }
```

Then create a test that calls the suite:

```rust
use nanograph_kvt::test_suite::run_kvstore_test_suite;
use your_crate::YourKeyValueStore;

#[tokio::test]
async fn test_with_common_suite() {
    let store = YourKeyValueStore::new();
    run_kvstore_test_suite(&store).await;
}
```

### Examples

- **Memory Store**: `nanograph-kvt/src/memory.rs` (test_common_test_suite)
- **LMDB Store**: `nanograph-lmdb/tests/common_test_suite.rs`
- **LSM Store**: `nanograph-lsm/tests/common_test_suite.rs`

## Test Coverage

The test suite comprehensively tests all trait methods:

### 1. Basic Operations
- `get()` - Retrieve values by key
- `put()` - Store key-value pairs
- `delete()` - Remove keys
- `exists()` - Check key existence
- Update existing keys
- Non-existent key handling

### 2. Batch Operations
- `batch_get()` - Retrieve multiple keys
- `batch_put()` - Store multiple pairs atomically
- `batch_delete()` - Remove multiple keys

### 3. Range Scanning
- `scan()` - Range queries with bounds
- `scan_prefix()` - Prefix-based queries
- Forward and reverse iteration
- Limit enforcement
- Iterator seeking and positioning

### 4. Transactions
- `begin_transaction()` - Start transactions
- Transaction isolation (reads within transaction)
- `commit()` - Atomic commits
- `rollback()` - Transaction rollback
- Transaction deletes

### 5. Shard Management
- `create_shard()` - Create new shards
- `drop_shard()` - Remove shards
- `clear()` - Clear shard data
- `list_shards()` - Enumerate shards
- `shard_exists()` - Check shard existence
- Shard isolation verification

### 6. Metadata Operations
- `key_count()` - Count keys in shard
- `shard_stats()` - Get comprehensive statistics

### 7. Maintenance Operations
- `flush()` - Persist pending writes
- `compact()` - Trigger compaction (shard-specific and global)

### 8. Edge Cases
- Empty keys and values
- Large values (1MB+)
- Many small keys (1000+)
- Concurrent access patterns

### 9. Iterator Operations
- Seeking to specific keys
- Position tracking
- Validity checking
- Continued iteration after seek

### 10. Concurrent Access
- Multiple concurrent readers
- Data integrity verification

## Known Implementation Limitations

### LMDB
- **Transaction Limitation**: LMDB transactions are shard-specific. The test suite's transaction tests may fail because LMDB requires environment-level transactions rather than cross-shard transactions.
- **Workaround**: Implementations may need to skip or adapt transaction tests based on their architecture.

### LSM
- **Initialization**: LSM stores require `init_tx_manager()` to be called after wrapping in `Arc`.

## Benefits

1. **Consistency**: Ensures all implementations behave consistently
2. **Completeness**: Tests all trait methods comprehensively
3. **Regression Prevention**: Catches breaking changes early
4. **Documentation**: Serves as executable specification
5. **Time Savings**: Reduces test duplication across implementations

## Adding New Tests

When adding new methods to `KeyValueShardStore`:

1. Add corresponding tests to `test_suite.rs`
2. Update this documentation
3. Run the suite against all implementations
4. Document any implementation-specific limitations

## Test Organization

Tests are organized into logical sections:
- Each section tests related functionality
- Tests use incrementing shard IDs to avoid conflicts
- Helper function `create_test_shard()` generates unique shard IDs
- All tests are async and use tokio runtime

## Future Enhancements

Potential improvements to the test suite:

- [ ] Configurable test parameters (value sizes, iteration counts)
- [ ] Performance benchmarking integration
- [ ] Stress testing modes
- [ ] Concurrent write testing
- [ ] Transaction conflict testing
- [ ] Recovery and crash testing
- [ ] Memory leak detection