# nanograph-util Benchmarks

This directory contains comprehensive performance benchmarks for the nanograph-util crate.

## Benchmark Organization

### Compression Benchmarks

#### `bench_compression_algorithms`
Tests compression and decompression performance across different data sizes and patterns:

- **Data Sizes**: 1KB, 4KB, 16KB, 64KB, 256KB
- **Data Patterns**:
  - Zeros: All zero bytes (best case for compression)
  - Random: Pseudo-random data (worst case for compression)
  - Repetitive: Repeated text patterns (realistic case)
- **Algorithms**: None, LZ4, Zstd, Snappy
- **Operations**: Compress and decompress for each algorithm

#### `bench_compression_into_buffer`
Tests in-place compression performance with pre-allocated buffers:

- **Data Sizes**: 1KB, 16KB, 64KB
- **Algorithms**: LZ4, Zstd, Snappy
- **Focus**: Memory efficiency and performance with reusable buffers

#### `bench_compression_ratios`
Analyzes compression ratios across different data patterns:

- **Data Size**: 64KB
- **Patterns**: Zeros, Ones, Random, Repetitive, Sequential
- **Algorithms**: LZ4, Zstd, Snappy
- **Output**: Compression ratio percentages

### Encryption Benchmarks

#### `bench_encryption_algorithms`
Tests encryption and decryption performance:

- **Data Sizes**: 1KB, 4KB, 16KB, 64KB, 256KB
- **Algorithms**: AES-256-GCM, ChaCha20-Poly1305
- **Operations**: Encrypt and decrypt for each algorithm
- **Metrics**: Throughput in bytes/second

#### `bench_encryption_key_generation`
Measures key generation performance:

- **Algorithms**: AES-256-GCM, ChaCha20-Poly1305
- **Focus**: Random key generation overhead

#### `bench_encryption_nonce_generation`
Measures nonce generation performance:

- **Algorithms**: AES-256-GCM, ChaCha20-Poly1305
- **Focus**: Random nonce generation overhead

### Integrity Benchmarks

#### `bench_integrity_algorithms`
Tests hashing and verification performance:

- **Data Sizes**: 1KB, 4KB, 16KB, 64KB, 256KB, 1MB
- **Algorithms**: CRC32C, XXHash32
- **Operations**: Hash computation and verification
- **Metrics**: Throughput in bytes/second

#### `bench_integrity_incremental`
Tests incremental hashing performance:

- **Data Sizes**: 1KB, 16KB, 64KB, 256KB
- **Chunk Size**: 4KB
- **Algorithms**: CRC32C, XXHash32
- **Focus**: Multi-chunk hash computation efficiency

### Pipeline Benchmarks

#### `bench_compress_encrypt_pipeline`
Tests combined compression and encryption:

- **Data Sizes**: 4KB, 16KB, 64KB
- **Pipelines**:
  - LZ4 + AES-256-GCM
  - Zstd + ChaCha20-Poly1305
- **Focus**: Real-world usage patterns

#### `bench_full_pipeline`
Tests complete data processing pipeline:

- **Data Sizes**: 4KB, 16KB, 64KB
- **Operations**:
  - Compress → Hash → Encrypt (forward)
  - Decrypt → Verify → Decompress (reverse)
  - Full roundtrip
- **Focus**: End-to-end performance

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench
```

### Run Specific Benchmark Group
```bash
cargo bench --bench util_benchmarks compression
cargo bench --bench util_benchmarks encryption
cargo bench --bench util_benchmarks integrity
cargo bench --bench util_benchmarks pipeline
```

### Run Specific Benchmark
```bash
cargo bench --bench util_benchmarks -- compression/repetitive/16384
```

### Save Baseline
```bash
cargo bench --bench util_benchmarks -- --save-baseline main
```

### Compare Against Baseline
```bash
cargo bench --bench util_benchmarks -- --baseline main
```

### Generate HTML Report
```bash
cargo bench --bench util_benchmarks
# Open target/criterion/report/index.html
```

## Benchmark Results Interpretation

### Compression Performance

**Expected Results:**
- **LZ4**: Fastest compression/decompression, moderate ratio
- **Zstd**: Best compression ratio, moderate speed
- **Snappy**: Very fast, lower compression ratio
- **Pattern Impact**: Zeros/Repetitive compress best, Random compresses poorly

**Key Metrics:**
- Throughput (MB/s)
- Compression ratio (%)
- Latency (µs)

### Encryption Performance

**Expected Results:**
- **AES-256-GCM**: Fastest on CPUs with AES-NI
- **ChaCha20-Poly1305**: Consistent performance, good without AES-NI
- **Size Impact**: Linear scaling with data size

**Key Metrics:**
- Throughput (MB/s)
- Latency (µs)
- Key/nonce generation time (ns)

### Integrity Performance

**Expected Results:**
- **CRC32C**: Very fast, hardware accelerated on modern CPUs
- **XXHash32**: Extremely fast, consistent performance
- **Size Impact**: Linear scaling with data size

**Key Metrics:**
- Throughput (MB/s)
- Hash computation time (µs)
- Verification overhead (ns)

### Pipeline Performance

**Expected Results:**
- Combined operations show cumulative overhead
- Compression before encryption improves throughput
- Integrity checking adds minimal overhead

**Key Metrics:**
- End-to-end latency (µs)
- Throughput (MB/s)
- Operation breakdown

## Performance Optimization Tips

### Compression
1. Use LZ4 for speed-critical paths
2. Use Zstd for storage optimization
3. Use Snappy for balanced performance
4. Pre-allocate buffers with `max_compressed_size()`
5. Reuse buffers with `compress_into()`

### Encryption
1. Use AES-256-GCM on Intel/AMD CPUs with AES-NI
2. Use ChaCha20-Poly1305 on ARM or older CPUs
3. Generate keys/nonces once and reuse when safe
4. Batch operations when possible

### Integrity
1. Use CRC32C for general-purpose checksums
2. Use XXHash32 for maximum speed
3. Use incremental hashing for large data
4. Compute hashes in parallel when possible

### Pipelines
1. Compress before encrypting (smaller ciphertext)
2. Hash compressed data (faster)
3. Batch operations to amortize overhead
4. Use appropriate buffer sizes (4KB-64KB chunks)

## Benchmark Configuration

Benchmarks use Criterion.rs with the following settings:

- **Warm-up time**: 3 seconds
- **Measurement time**: 5 seconds
- **Sample size**: 100 iterations
- **Confidence level**: 95%
- **Noise threshold**: 5%

## Hardware Considerations

Performance varies significantly based on:

- **CPU Architecture**: x86_64 vs ARM vs other
- **CPU Features**: AES-NI, SSE4.2, NEON
- **CPU Speed**: Clock frequency and turbo boost
- **Cache Size**: L1/L2/L3 cache affects large data
- **Memory Speed**: DDR4/DDR5 bandwidth
- **Thermal Throttling**: Sustained load performance

## Continuous Integration

Benchmarks can be run in CI to detect performance regressions:

```bash
# Run benchmarks and save baseline
cargo bench --bench util_benchmarks -- --save-baseline ci

# Compare against baseline (fails if regression > 10%)
cargo bench --bench util_benchmarks -- --baseline ci
```

## Adding New Benchmarks

When adding new functionality:

1. Add benchmark function to appropriate group
2. Use representative data sizes and patterns
3. Include both best-case and worst-case scenarios
4. Document expected performance characteristics
5. Update this README with benchmark descriptions

## Benchmark Maintenance

- Run benchmarks regularly to detect regressions
- Update baselines after intentional changes
- Document performance characteristics
- Remove obsolete benchmarks
- Keep benchmark code simple and focused