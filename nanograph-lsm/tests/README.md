# Integration Tests for nanograph-lsm

This directory contains integration tests for the LSM Tree implementation.

## Test Coverage

### Basic Operations
- `test_basic_put_get` - Basic put and get operations
- `test_put_update` - Updating existing keys
- `test_delete` - Deletion operations
- `test_nonexistent_key` - Handling non-existent keys
- `test_empty_key_value` - Edge cases with empty keys/values

### Data Integrity
- `test_multiple_keys` - Multiple key operations
- `test_wal_recovery` - WAL recovery after restart
- `test_memtable_flush` - Memtable flushing to SSTables

### Performance Patterns
- `test_sequential_writes` - Sequential write patterns
- `test_random_access` - Random access patterns
- `test_overwrite_pattern` - Overwrite patterns
- `test_mixed_operations` - Mixed read/write/delete operations

### Scalability
- `test_large_values` - Large value handling (1MB+)
- `test_stats` - Statistics tracking

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific test
cargo test --test integration_tests test_basic_put_get

# Run with output
cargo test --test integration_tests -- --nocapture

# Run with multiple threads
cargo test --test integration_tests -- --test-threads=4
```

## Test Environment

Tests use:
- `MemoryFileSystem` for WAL storage
- `TempDir` for SSTable storage
- Default LSM Tree options

## Adding New Tests

When adding new integration tests:

1. Use the `create_test_engine()` helper function
2. Clean up resources properly (TempDir handles this automatically)
3. Test both success and failure cases
4. Document what the test validates

