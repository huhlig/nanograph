# Adaptive Radix Tree Benchmarks

This directory contains comprehensive benchmarks for the Adaptive Radix Tree (ART) implementation.

## Benchmark Suites

### art_benchmarks.rs

Core ART data structure benchmarks covering:

#### Insert Benchmarks
- **insert_sequential**: Sequential key insertion (100, 1K, 10K elements)
- **insert_random**: Random key insertion
- **insert_reverse**: Reverse order insertion
- **insert_with_common_prefix**: Keys with common prefixes

#### Lookup Benchmarks
- **lookup_sequential**: Sequential key lookups
- **lookup_random**: Random key lookups
- **lookup_missing**: Lookups for non-existent keys
- **contains_key**: Key existence checks

#### Delete Benchmarks
- **delete_sequential**: Sequential key deletion
- **delete_random**: Random key deletion
- **delete_alternating**: Delete every other key

#### Iterator Benchmarks
- **iterator_full_scan**: Full tree iteration
- **iterator_collect**: Collect all items into a vector
- **keys_iterator**: Iterate over keys only
- **values_iterator**: Iterate over values only

#### Memory Benchmarks
- **memory_usage**: Memory usage tracking
- **clone**: Tree cloning performance

#### Mixed Workload Benchmarks
- **mixed_read_write**: 50% reads, 50% writes
- **mixed_operations**: 33% insert, 33% read, 33% delete

#### Node Type Transition Benchmarks
- **node_growth**: Node4 → Node16 → Node48 → Node256 transitions

### persistence_benchmarks.rs

Persistence and I/O benchmarks covering:

#### KVStore Operation Benchmarks
- **kvstore_put**: Batch put operations
- **kvstore_get**: Batch get operations
- **kvstore_delete**: Batch delete operations

#### Range Query Benchmarks
- **kvstore_range_scan**: Full range scans
- **kvstore_bounded_range**: Bounded range queries

#### Concurrent Operation Benchmarks
- **concurrent_reads**: Parallel read operations
- **concurrent_writes**: Parallel write operations
- **concurrent_mixed**: Mixed parallel reads and writes

#### Batch Operation Benchmarks
- **batch_insert**: Batch insertion performance
- **batch_get**: Batch retrieval performance

#### Memory and Size Benchmarks
- **tree_memory_usage**: Memory usage measurement
- **tree_size**: Tree size tracking

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench --package nanograph-art
```

### Run Specific Benchmark Suite
```bash
# Core ART benchmarks
cargo bench --package nanograph-art --bench art_benchmarks

# Persistence benchmarks
cargo bench --package nanograph-art --bench persistence_benchmarks
```

### Run Specific Benchmark Group
```bash
# Insert benchmarks only
cargo bench --package nanograph-art --bench art_benchmarks insert

# Lookup benchmarks only
cargo bench --package nanograph-art --bench art_benchmarks lookup

# Concurrent benchmarks only
cargo bench --package nanograph-art --bench persistence_benchmarks concurrent
```

### Run Specific Benchmark
```bash
cargo bench --package nanograph-art --bench art_benchmarks -- insert_sequential
```

## Benchmark Results

Results are saved to `target/criterion/` with:
- HTML reports for visualization
- Statistical analysis
- Historical comparison data

View results by opening `target/criterion/report/index.html` in a browser.

## Benchmark Configuration

Benchmarks use Criterion.rs with:
- Warm-up iterations
- Multiple samples for statistical significance
- Throughput measurements (elements/second)
- Comparison with previous runs

## Performance Targets

Expected performance characteristics:

### Insert Operations
- Sequential: ~1-2M ops/sec
- Random: ~500K-1M ops/sec
- With common prefix: ~800K-1.5M ops/sec

### Lookup Operations
- Sequential: ~2-5M ops/sec
- Random: ~1-3M ops/sec
- Missing keys: ~2-4M ops/sec

### Delete Operations
- Sequential: ~1-2M ops/sec
- Random: ~500K-1M ops/sec

### Iterator Operations
- Full scan: ~5-10M elements/sec
- Collect: ~3-8M elements/sec

### Memory Usage
- Node4: ~48 bytes
- Node16: ~144 bytes
- Node48: ~384 bytes
- Node256: ~2KB

## Interpreting Results

### Throughput
Higher is better. Measured in elements/second or operations/second.

### Time
Lower is better. Measured in nanoseconds, microseconds, or milliseconds.

### Memory
Lower is better for memory usage. Measured in bytes.

### Variance
Lower variance indicates more consistent performance.

## Contributing

When adding new benchmarks:
1. Follow existing naming conventions
2. Use appropriate sample sizes (100, 1K, 10K)
3. Include throughput measurements
4. Document expected performance
5. Test with both sequential and random data

## Made with Bob