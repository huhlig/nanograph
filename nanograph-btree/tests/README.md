# B+Tree Testing Documentation

This directory contains comprehensive tests for the nanograph-btree implementation.

## Test Structure

### Unit Tests (`unit_tests.rs`)
Tests individual components in isolation:

- **Node Tests**: Test leaf and internal node operations
  - Node creation and initialization
  - Insert, update, and delete operations
  - Node splitting logic
  - Sorted order maintenance
  - Parent/child relationships

- **Tree Structure Tests**: Test B+Tree operations
  - Tree creation and initialization
  - Single and multiple insertions
  - Node splitting and tree growth
  - Leaf finding algorithms
  - Delete operations
  - Update operations

- **Metrics Tests**: Test performance tracking
  - Metrics creation and initialization
  - Operation counting (reads, writes, deletes, scans)
  - Node operation tracking (splits, merges)

- **Configuration Tests**: Test tree configuration
  - Default configuration
  - Custom configuration
  - Configuration validation

- **Edge Cases**: Test boundary conditions
  - Empty keys and values
  - Very large keys and values
  - Binary key patterns
  - Sequential vs random insertion order

### Integration Tests (`integration_tests.rs`)
Tests the complete system working together:

- **Basic Operations**: CRUD operations through KeyValueStore interface
  - Put, get, delete operations
  - Empty key/value handling
  - Update existing keys

- **Batch Operations**: Bulk operations
  - Batch put, get, delete
  - Performance with large batches

- **Range Scans**: Iterator functionality
  - Full table scans
  - Bounded range scans
  - Limit and reverse scans
  - Forward and reverse iteration

- **Large Datasets**: Scalability testing
  - Sequential inserts (10,000+ keys)
  - Random inserts (1,000+ keys)
  - Tree splitting behavior

- **Statistics**: Metrics and monitoring
  - Key counting
  - Table statistics
  - B+Tree specific stats

- **Multiple Tables**: Multi-table operations
  - Table creation and isolation
  - Table listing
  - Table dropping

- **Concurrent Access**: Thread safety
  - Concurrent reads
  - Concurrent writes
  - Mixed read/write workloads

- **Error Handling**: Edge cases and errors
  - Very large keys/values
  - Duplicate table creation
  - Invalid operations

### Test Utilities (`test_utils.rs`)
Helper functions for testing:

- **Data Generation**:
  - `generate_random_kvs()`: Random key-value pairs with seed
  - `generate_sequential_keys()`: Sequential keys with prefix
  - `generate_sequential_kvs()`: Sequential key-value pairs
  - `generate_reverse_sequential_keys()`: Reverse order keys
  - `generate_edge_case_keys()`: Edge case patterns

- **Tree Utilities**:
  - `verify_tree_structure()`: Validate tree consistency
  - `create_test_tree()`: Create tree with custom config
  - `fill_tree_with_data()`: Populate tree with test data

- **Assertions**:
  - `assert_bytes_eq()`: Compare byte slices with better errors

## Running Tests

### Run All Tests
```bash
cargo test -p nanograph-btree
```

### Run Specific Test Suite
```bash
# Unit tests only
cargo test -p nanograph-btree --test unit_tests

# Integration tests only
cargo test -p nanograph-btree --test integration_tests
```

### Run Specific Test
```bash
cargo test -p nanograph-btree test_tree_forces_split
```

### Run with Output
```bash
cargo test -p nanograph-btree -- --nocapture
```

### Run with Specific Thread Count
```bash
cargo test -p nanograph-btree -- --test-threads=1
```

## Benchmarks

### Running Benchmarks
```bash
# Run all benchmarks
cargo bench -p nanograph-btree

# Run specific benchmark group
cargo bench -p nanograph-btree -- insert_sequential

# Generate HTML reports
cargo bench -p nanograph-btree -- --save-baseline my_baseline
```

### Benchmark Categories

1. **Insert Benchmarks**
   - Sequential insertion
   - Reverse insertion
   - Insertion with splits

2. **Get Benchmarks**
   - Existing key lookups
   - Missing key lookups
   - Random access patterns

3. **Delete Benchmarks**
   - Sequential deletion
   - Random deletion

4. **Range Scan Benchmarks**
   - Full table scans
   - Bounded range scans
   - Limited scans

5. **Batch Operation Benchmarks**
   - Batch put operations
   - Batch get operations

6. **Mixed Workload Benchmarks**
   - 90% read / 10% write
   - 50% read / 50% write

7. **Tree Structure Benchmarks**
   - Impact of node size on performance
   - Tree height effects

### Benchmark Results Location
Results are saved in `target/criterion/` with HTML reports.

## Test Coverage

### Current Coverage Areas

✅ **Node Operations**
- Leaf node CRUD
- Internal node operations
- Node splitting
- Sorted order maintenance

✅ **Tree Operations**
- Insert, get, delete
- Tree growth and splitting
- Multi-level trees
- Leaf finding

✅ **KeyValueStore Interface**
- All CRUD operations
- Batch operations
- Range scans
- Table management

✅ **Concurrency**
- Concurrent reads
- Concurrent writes
- Thread safety

✅ **Edge Cases**
- Empty keys/values
- Large keys/values
- Binary patterns
- Various insertion orders

### Areas for Future Testing

🔲 **Advanced Concurrency**
- Read-write conflicts
- Deadlock scenarios
- High contention workloads

🔲 **Failure Scenarios**
- Out of memory conditions
- Corrupted data recovery
- Partial write handling

🔲 **Performance Regression**
- Automated performance tracking
- Comparison with baseline
- Performance alerts

🔲 **Stress Testing**
- Very large datasets (millions of keys)
- Long-running operations
- Memory pressure scenarios

🔲 **Property-Based Testing**
- Using proptest for invariant checking
- Randomized operation sequences
- Fuzzing inputs

## Writing New Tests

### Unit Test Template
```rust
#[test]
fn test_feature_name() {
    // Setup
    let tree = create_test_tree(128);
    
    // Execute
    tree.insert(b"key".to_vec(), b"value".to_vec()).unwrap();
    
    // Verify
    assert_eq!(tree.get(b"key").unwrap(), Some(b"value".to_vec()));
}
```

### Integration Test Template
```rust
#[tokio::test]
async fn test_feature_name() {
    // Setup
    let store = BTreeKeyValueStore::default();
    let table = store.create_table("test").await.unwrap();
    
    // Execute
    store.put(table, b"key", b"value").await.unwrap();
    
    // Verify
    let value = store.get(table, b"key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}
```

### Benchmark Template
```rust
fn bench_feature_name(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_group");
    
    group.bench_function("feature_name", |b| {
        b.iter(|| {
            // Code to benchmark
            black_box(operation());
        });
    });
    
    group.finish();
}
```

## Test Best Practices

1. **Isolation**: Each test should be independent
2. **Clarity**: Test names should describe what they test
3. **Coverage**: Test both success and failure paths
4. **Performance**: Keep unit tests fast (<100ms)
5. **Determinism**: Tests should be reproducible
6. **Documentation**: Complex tests should have comments

## Continuous Integration

Tests are automatically run on:
- Every commit
- Pull requests
- Before releases

CI configuration ensures:
- All tests pass
- No performance regressions
- Code coverage meets threshold

## Debugging Failed Tests

### Enable Logging
```bash
RUST_LOG=debug cargo test -p nanograph-btree -- --nocapture
```

### Run Single Test with Backtrace
```bash
RUST_BACKTRACE=1 cargo test -p nanograph-btree test_name -- --exact
```

### Use Test Utilities
```rust
// Add detailed assertions
assert_bytes_eq(actual, expected, "Context about what failed");

// Verify tree structure
verify_tree_structure(&tree).unwrap();
```

## Performance Testing

### Baseline Creation
```bash
cargo bench -p nanograph-btree -- --save-baseline main
```

### Compare Against Baseline
```bash
cargo bench -p nanograph-btree -- --baseline main
```

### Profile Tests
```bash
cargo test -p nanograph-btree --release -- --nocapture
```

## Contributing Tests

When adding new features:
1. Write unit tests for new components
2. Add integration tests for user-facing functionality
3. Include benchmarks for performance-critical code
4. Update this README with new test categories
5. Ensure all tests pass before submitting PR

## Test Maintenance

- Review and update tests when APIs change
- Remove obsolete tests
- Refactor duplicated test code into utilities
- Keep test data generation functions up to date
- Monitor test execution time and optimize slow tests

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [Property-Based Testing with Proptest](https://altsysrq.github.io/proptest-book/)

## License

Licensed under the Apache License, Version 2.0. See LICENSE for details.

