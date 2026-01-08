# nanograph-util Test Suite

This directory contains comprehensive tests for the nanograph-util crate.

## Test Organization

### Unit Tests (`unit_tests.rs`)

Comprehensive unit tests covering all modules:

#### Error Module Tests
- Error display formatting
- Error variant creation
- All error types (Compression, Decompression, Encryption, Decryption, InvalidAlgorithm, BufferTooSmall, InvalidKeySize, InvalidNonceSize, IntegrityCheckFailed)

#### Compression Module Tests
- **Algorithm Serialization**: Converting algorithms to/from u8 values
- **Algorithm Names**: Verifying algorithm name strings
- **Empty Data**: Handling zero-length inputs
- **Single Byte**: Minimal data compression
- **Large Data**: 1MB compression tests
- **Random Data**: Non-compressible data handling
- **Buffer Operations**: `compress_into` and `decompress_into` with pre-allocated buffers
- **Buffer Size Validation**: Error handling for undersized buffers
- **Max Compressed Size**: Calculating worst-case output sizes
- **Roundtrip Tests**: Compress/decompress cycles for all algorithms

Algorithms tested:
- None (no compression)
- LZ4 (fast compression)
- Zstd (high compression ratio)
- Snappy (very fast)

#### Encryption Module Tests
- **Algorithm Serialization**: Converting algorithms to/from u8 values
- **Algorithm Names**: Verifying algorithm name strings
- **Key/Nonce Sizes**: Validating size requirements
- **Tag Sizes**: Authentication tag size verification
- **Key Generation**: Random key generation
- **Nonce Generation**: Random nonce generation
- **Key/Nonce Validation**: Size validation and error handling
- **Empty Data**: Encrypting zero-length inputs
- **Large Data**: 1MB encryption tests
- **Wrong Key Detection**: Ensuring decryption fails with incorrect keys
- **Wrong Nonce Detection**: Ensuring decryption fails with incorrect nonces
- **Tamper Detection**: Ensuring modified ciphertext is rejected
- **Algorithm Mismatch**: Detecting key/nonce algorithm mismatches
- **Serialization**: Converting keys/nonces to/from bytes

Algorithms tested:
- None (no encryption)
- AES-256-GCM (hardware accelerated)
- ChaCha20-Poly1305 (software optimized)

#### Integrity Module Tests
- **Algorithm Serialization**: Converting algorithms to/from u8 values
- **Algorithm Names**: Verifying algorithm name strings
- **Hash Sizes**: Validating output sizes
- **Empty Data**: Hashing zero-length inputs
- **Single Byte**: Minimal data hashing
- **Large Data**: 1MB hashing tests
- **Collision Resistance**: Different data produces different hashes
- **Verification Failure**: Detecting data modifications
- **Hash Serialization**: Converting hashes to/from bytes
- **Hash Extraction**: Getting u32 values from hashes
- **Incremental Hashing**: Multi-chunk hash computation
- **Hasher Algorithm**: Verifying hasher algorithm tracking
- **Invalid Input Handling**: Error handling for malformed data
- **Determinism**: Same input always produces same hash

Algorithms tested:
- None (no integrity check)
- CRC32C (hardware accelerated)
- XXHash32 (very fast)

#### Integration Tests
- **Compress then Encrypt**: Pipeline testing
- **Compress with Integrity**: Compression + hash verification
- **Full Pipeline**: Complete workflow (compress → hash → encrypt → decrypt → verify → decompress)

## Running Tests

### Run All Tests
```bash
cargo test
```

### Run Specific Test Module
```bash
cargo test --test unit_tests
```

### Run Tests with Output
```bash
cargo test -- --nocapture
```

### Run Specific Test
```bash
cargo test test_compression_large_data
```

### Run Tests in Release Mode
```bash
cargo test --release
```

## Test Coverage

The test suite provides comprehensive coverage:

- **69 total tests** across all modules
- **Error handling**: All error variants tested
- **Edge cases**: Empty data, single byte, large data
- **Algorithm coverage**: All compression, encryption, and integrity algorithms
- **Integration**: Cross-module functionality
- **Validation**: Input validation and error conditions
- **Serialization**: All serialization/deserialization paths

## Benchmarks

See `../benches/README.md` for benchmark documentation.

## Test Data Patterns

Tests use various data patterns to ensure comprehensive coverage:

- **Empty**: Zero-length data
- **Single byte**: Minimal data
- **Small**: Few bytes (< 100)
- **Medium**: Kilobytes (1-64 KB)
- **Large**: Megabytes (1+ MB)
- **Zeros**: All zero bytes (highly compressible)
- **Random**: Pseudo-random data (incompressible)
- **Repetitive**: Repeated patterns (compressible)
- **Sequential**: Sequential byte values

## Continuous Integration

These tests are designed to run in CI/CD pipelines:

- Fast execution (< 1 second for all unit tests)
- No external dependencies
- Deterministic results
- Clear failure messages
- Comprehensive coverage

## Adding New Tests

When adding new functionality:

1. Add unit tests for the new feature
2. Add edge case tests (empty, large, invalid input)
3. Add integration tests if it interacts with other modules
4. Update this README with test descriptions
5. Ensure all tests pass before committing

## Test Maintenance

- Keep tests focused and independent
- Use descriptive test names
- Document complex test scenarios
- Update tests when APIs change
- Remove obsolete tests