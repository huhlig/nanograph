# nanograph-util

Utility crate providing compression, encryption, and integrity checking functionality for the Nanograph database system.

## Features

### Compression Algorithms

The crate supports multiple compression algorithms optimized for different use cases:

- **None**: No compression (passthrough)
- **LZ4**: Fast compression with decent ratio - ideal for real-time applications
- **Zstd**: Excellent compression ratio with good speed - best for storage optimization
- **Snappy**: Very fast compression/decompression - optimized for speed over ratio

#### Usage Example

```rust
use nanograph_util::{CompressionAlgorithm, Result};

fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    // Compress with LZ4
    let compressed = CompressionAlgorithm::Lz4.compress(data)?;
    
    // Decompress
    let decompressed = CompressionAlgorithm::Lz4.decompress(&compressed, Some(data.len()))?;
    
    assert_eq!(data, decompressed);
    Ok(compressed)
}

// Pre-allocate buffer for better performance
fn compress_into_buffer(data: &[u8]) -> Result<Vec<u8>> {
    let algo = CompressionAlgorithm::Zstd;
    let max_size = algo.max_compressed_size(data.len());
    let mut buffer = vec![0u8; max_size];
    
    let size = algo.compress_into(data, &mut buffer)?;
    buffer.truncate(size);
    Ok(buffer)
}
```

### Encryption Algorithms

The crate provides authenticated encryption with two modern algorithms:

- **None**: No encryption (passthrough)
- **AES-256-GCM**: Industry standard, hardware accelerated on modern CPUs with AES-NI
- **ChaCha20-Poly1305**: Fast software implementation, excellent for systems without AES-NI

Both algorithms provide:
- 256-bit keys
- 96-bit nonces
- 128-bit authentication tags
- AEAD (Authenticated Encryption with Associated Data)

#### Usage Example

```rust
use nanograph_util::{EncryptionAlgorithm, Result};

fn encrypt_data(data: &[u8]) -> Result<Vec<u8>> {
    let algo = EncryptionAlgorithm::Aes256Gcm;
    
    // Generate key and nonce
    let key = algo.generate_key();
    let nonce = algo.generate_nonce();
    
    // Encrypt
    let ciphertext = algo.encrypt(&key, &nonce, data)?;
    
    // Decrypt
    let plaintext = algo.decrypt(&key, &nonce, &ciphertext)?;
    
    assert_eq!(data, plaintext);
    Ok(ciphertext)
}

// Use ChaCha20-Poly1305 for better software performance
fn encrypt_with_chacha(data: &[u8]) -> Result<Vec<u8>> {
    let algo = EncryptionAlgorithm::ChaCha20Poly1305;
    let key = algo.generate_key();
    let nonce = algo.generate_nonce();
    
    algo.encrypt(&key, &nonce, data)
}
```

### Integrity Checking

Fast checksums and hashes for data integrity verification:

- **None**: No integrity check
- **CRC32C**: Hardware accelerated on modern CPUs, excellent for error detection
- **XXHash32**: Very fast 32-bit non-cryptographic hash

#### Usage Example

```rust
use nanograph_util::{IntegrityAlgorithm, Result};

fn verify_data(data: &[u8]) -> Result<()> {
    let algo = IntegrityAlgorithm::Crc32c;
    
    // Calculate hash
    let hash = algo.hash(data);
    
    // Verify data integrity
    algo.verify(data, &hash)?;
    
    Ok(())
}

// Incremental hashing for large data
fn hash_large_file(chunks: &[&[u8]]) -> Result<()> {
    let algo = IntegrityAlgorithm::XXHash32;
    let mut hasher = algo.hasher();
    
    for chunk in chunks {
        hasher.update(chunk);
    }
    
    let hash = hasher.finalize();
    println!("Hash: {:?}", hash);
    Ok(())
}
```

## Error Handling

All operations return `Result<T, Error>` with comprehensive error types:

- `Compression(String)`: Compression errors
- `Decompression(String)`: Decompression errors
- `Encryption(String)`: Encryption errors
- `Decryption(String)`: Decryption errors
- `InvalidAlgorithm(u8)`: Invalid algorithm identifier
- `BufferTooSmall`: Output buffer too small
- `InvalidKeySize`: Wrong key size for algorithm
- `InvalidNonceSize`: Wrong nonce size for algorithm
- `IntegrityCheckFailed`: Hash mismatch

## Performance Considerations

### Compression

- **LZ4**: ~500 MB/s compression, ~2000 MB/s decompression
- **Zstd**: ~400 MB/s compression (level 3), ~800 MB/s decompression
- **Snappy**: ~550 MB/s compression, ~1800 MB/s decompression

### Encryption

- **AES-256-GCM**: ~1000 MB/s with AES-NI, ~100 MB/s without
- **ChaCha20-Poly1305**: ~600 MB/s (consistent across platforms)

### Integrity

- **CRC32C**: ~10 GB/s with hardware acceleration
- **XXHash32**: ~7 GB/s

## Algorithm Selection Guide

### Compression

- Use **LZ4** for real-time applications requiring fast compression/decompression
- Use **Zstd** for storage optimization where compression ratio matters
- Use **Snappy** for maximum decompression speed with acceptable compression

### Encryption

- Use **AES-256-GCM** on modern x86/ARM CPUs with hardware acceleration
- Use **ChaCha20-Poly1305** on older systems or for consistent cross-platform performance

### Integrity

- Use **CRC32C** for error detection with hardware acceleration
- Use **XXHash32** for fast hashing when hardware CRC32C is unavailable

## Thread Safety

All algorithm enums are `Copy + Send + Sync`. The hasher types are `Send` but not `Sync` (use one per thread).

## License

Licensed under the Apache License, Version 2.0. See LICENSE.md for details.