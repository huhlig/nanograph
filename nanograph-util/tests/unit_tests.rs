//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! Comprehensive unit tests for nanograph-util

use nanograph_util::*;

// ============================================================================
// Error Module Tests
// ============================================================================

#[test]
fn test_error_display() {
    let err = Error::Compression("test error".to_string());
    assert_eq!(err.to_string(), "Compression error: test error");

    let err = Error::Decompression("test error".to_string());
    assert_eq!(err.to_string(), "Decompression error: test error");

    let err = Error::Encryption("test error".to_string());
    assert_eq!(err.to_string(), "Encryption error: test error");

    let err = Error::Decryption("test error".to_string());
    assert_eq!(err.to_string(), "Decryption error: test error");
}

#[test]
fn test_error_invalid_algorithm() {
    let err = Error::InvalidAlgorithm(99);
    assert_eq!(err.to_string(), "Invalid algorithm identifier: 99");
}

#[test]
fn test_error_buffer_too_small() {
    let err = Error::BufferTooSmall {
        required: 100,
        available: 50,
    };
    assert_eq!(
        err.to_string(),
        "Buffer too small: required 100, available 50"
    );
}

#[test]
fn test_error_invalid_key_size() {
    let err = Error::InvalidKeySize {
        expected: 32,
        actual: 16,
    };
    assert_eq!(err.to_string(), "Invalid key size: expected 32, got 16");
}

#[test]
fn test_error_invalid_nonce_size() {
    let err = Error::InvalidNonceSize {
        expected: 12,
        actual: 8,
    };
    assert_eq!(err.to_string(), "Invalid nonce size: expected 12, got 8");
}

#[test]
fn test_error_integrity_check_failed() {
    let err = Error::IntegrityCheckFailed {
        expected: 0x12345678,
        actual: 0x87654321,
    };
    assert_eq!(
        err.to_string(),
        "Integrity check failed: expected 12345678, got 87654321"
    );
}

// ============================================================================
// Compression Module Tests
// ============================================================================

#[test]
fn test_compression_algorithm_serialization() {
    let algorithms = [
        CompressionAlgorithm::None,
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ];

    for algo in algorithms {
        let byte = algo.as_u8();
        let restored = CompressionAlgorithm::from_u8(byte).unwrap();
        assert_eq!(algo, restored);
    }

    // Test invalid value
    assert!(CompressionAlgorithm::from_u8(255).is_none());
}

#[test]
fn test_compression_algorithm_names() {
    assert_eq!(CompressionAlgorithm::None.name(), "none");
    assert_eq!(CompressionAlgorithm::Lz4.name(), "lz4");
    assert_eq!(CompressionAlgorithm::Zstd.name(), "zstd");
    assert_eq!(CompressionAlgorithm::Snappy.name(), "snappy");
}

#[test]
fn test_compression_empty_data() {
    let empty: &[u8] = &[];
    for algo in [
        CompressionAlgorithm::None,
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ] {
        let compressed = algo.compress(empty).unwrap();
        let decompressed = algo.decompress(&compressed, Some(0)).unwrap();
        assert_eq!(decompressed, empty);
    }
}

#[test]
fn test_compression_single_byte() {
    let data = &[42u8];
    for algo in [
        CompressionAlgorithm::None,
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ] {
        let compressed = algo.compress(data).unwrap();
        let decompressed = algo.decompress(&compressed, Some(1)).unwrap();
        assert_eq!(decompressed, data);
    }
}

#[test]
fn test_compression_large_data() {
    // Create 1MB of data with patterns
    let mut data = Vec::with_capacity(1024 * 1024);
    for i in 0..1024 * 1024 {
        data.push((i % 256) as u8);
    }

    for algo in [
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ] {
        let compressed = algo.compress(&data).unwrap();
        assert!(
            compressed.len() < data.len(),
            "Algorithm {} should compress repetitive data",
            algo.name()
        );
        let decompressed = algo.decompress(&compressed, Some(data.len())).unwrap();
        assert_eq!(decompressed, data);
    }
}

#[test]
fn test_compression_random_data() {
    // Random data doesn't compress well
    let data: Vec<u8> = (0..1000).map(|i| ((i * 7919) % 256) as u8).collect();

    for algo in [
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ] {
        let compressed = algo.compress(&data).unwrap();
        let decompressed = algo.decompress(&compressed, Some(data.len())).unwrap();
        assert_eq!(decompressed, data);
    }
}

#[test]
fn test_compression_into_buffer_too_small() {
    let data = b"Hello, World!";
    let mut buffer = vec![0u8; 5]; // Too small

    let result = CompressionAlgorithm::None.compress_into(data, &mut buffer);
    assert!(result.is_err());
    match result {
        Err(Error::BufferTooSmall {
            required,
            available,
        }) => {
            assert_eq!(required, data.len());
            assert_eq!(available, 5);
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

#[test]
fn test_compression_max_compressed_size() {
    let data = vec![0u8; 1000];
    for algo in [
        CompressionAlgorithm::None,
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
    ] {
        let max_size = algo.max_compressed_size(data.len());
        assert!(max_size >= data.len());

        let mut buffer = vec![0u8; max_size];
        let size = algo.compress_into(&data, &mut buffer).unwrap();
        assert!(size <= max_size);
    }
}

#[test]
fn test_compression_roundtrip_all_algorithms() {
    let test_cases = vec![
        b"".to_vec(),
        b"a".to_vec(),
        b"Hello, World!".to_vec(),
        vec![0u8; 100],
        vec![255u8; 100],
        (0..256).map(|i| i as u8).collect::<Vec<_>>(),
    ];

    for data in test_cases {
        for algo in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ] {
            let compressed = algo.compress(&data).unwrap();
            let decompressed = algo.decompress(&compressed, Some(data.len())).unwrap();
            assert_eq!(decompressed, data, "Failed for algorithm: {}", algo.name());
        }
    }
}

// ============================================================================
// Encryption Module Tests
// ============================================================================

#[test]
fn test_encryption_algorithm_serialization() {
    let algorithms = [
        EncryptionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ];

    for algo in algorithms {
        let byte = algo.as_u8();
        let restored = EncryptionAlgorithm::from_u8(byte).unwrap();
        assert_eq!(algo, restored);
    }

    // Test invalid value
    assert!(EncryptionAlgorithm::from_u8(255).is_none());
}

#[test]
fn test_encryption_algorithm_names() {
    assert_eq!(EncryptionAlgorithm::None.name(), "none");
    assert_eq!(EncryptionAlgorithm::Aes256Gcm.name(), "aes-256-gcm");
    assert_eq!(
        EncryptionAlgorithm::ChaCha20Poly1305.name(),
        "chacha20-poly1305"
    );
}

#[test]
fn test_encryption_key_sizes() {
    assert_eq!(EncryptionAlgorithm::None.key_size(), 0);
    assert_eq!(EncryptionAlgorithm::Aes256Gcm.key_size(), 32);
    assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.key_size(), 32);
}

#[test]
fn test_encryption_nonce_sizes() {
    assert_eq!(EncryptionAlgorithm::None.nonce_size(), 0);
    assert_eq!(EncryptionAlgorithm::Aes256Gcm.nonce_size(), 12);
    assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.nonce_size(), 12);
}

#[test]
fn test_encryption_tag_sizes() {
    assert_eq!(EncryptionAlgorithm::None.tag_size(), 0);
    assert_eq!(EncryptionAlgorithm::Aes256Gcm.tag_size(), 16);
    assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.tag_size(), 16);
}

#[test]
fn test_encryption_key_generation() {
    for algo in [
        EncryptionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key = algo.generate_key();
        assert_eq!(key.algorithm, algo);
        assert_eq!(key.key.len(), algo.key_size());
    }
}

#[test]
fn test_encryption_nonce_generation() {
    for algo in [
        EncryptionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let nonce = algo.generate_nonce();
        assert_eq!(nonce.algorithm, algo);
        assert_eq!(nonce.nonce.len(), algo.nonce_size());
    }
}

#[test]
fn test_encryption_key_from_bytes_invalid_size() {
    let result = EncryptionKey::from_bytes(
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionKeyId::new(1),
        vec![0u8; 16],
    );
    assert!(result.is_err());
    match result {
        Err(Error::InvalidKeySize { expected, actual }) => {
            assert_eq!(expected, 32);
            assert_eq!(actual, 16);
        }
        _ => panic!("Expected InvalidKeySize error"),
    }
}

#[test]
fn test_encryption_nonce_from_bytes_invalid_size() {
    let result = Nonce::from_bytes(EncryptionAlgorithm::Aes256Gcm, vec![0u8; 8]);
    assert!(result.is_err());
    match result {
        Err(Error::InvalidNonceSize { expected, actual }) => {
            assert_eq!(expected, 12);
            assert_eq!(actual, 8);
        }
        _ => panic!("Expected InvalidNonceSize error"),
    }
}

#[test]
fn test_encryption_empty_data() {
    let empty: &[u8] = &[];
    for algo in [
        EncryptionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key = algo.generate_key();
        let nonce = algo.generate_nonce();
        let encrypted = algo.encrypt(&key, &nonce, empty).unwrap();
        let decrypted = algo.decrypt(&key, &nonce, &encrypted).unwrap();
        assert_eq!(decrypted, empty);
    }
}

#[test]
fn test_encryption_large_data() {
    let data = vec![42u8; 1024 * 1024]; // 1MB
    for algo in [
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key = algo.generate_key();
        let nonce = algo.generate_nonce();
        let encrypted = algo.encrypt(&key, &nonce, &data).unwrap();
        assert_ne!(encrypted, data);
        let decrypted = algo.decrypt(&key, &nonce, &encrypted).unwrap();
        assert_eq!(decrypted, data);
    }
}

#[test]
fn test_encryption_wrong_key_fails() {
    let data = b"Secret message";
    for algo in [
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key1 = algo.generate_key();
        let key2 = algo.generate_key();
        let nonce = algo.generate_nonce();

        let encrypted = algo.encrypt(&key1, &nonce, data).unwrap();
        let result = algo.decrypt(&key2, &nonce, &encrypted);
        assert!(result.is_err());
    }
}

#[test]
fn test_encryption_wrong_nonce_fails() {
    let data = b"Secret message";
    for algo in [
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key = algo.generate_key();
        let nonce1 = algo.generate_nonce();
        let nonce2 = algo.generate_nonce();

        let encrypted = algo.encrypt(&key, &nonce1, data).unwrap();
        let result = algo.decrypt(&key, &nonce2, &encrypted);
        assert!(result.is_err());
    }
}

#[test]
fn test_encryption_tampered_ciphertext_fails() {
    let data = b"Secret message";
    for algo in [
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ] {
        let key = algo.generate_key();
        let nonce = algo.generate_nonce();

        let mut encrypted = algo.encrypt(&key, &nonce, data).unwrap();
        // Tamper with the ciphertext
        if !encrypted.is_empty() {
            encrypted[0] ^= 1;
        }
        let result = algo.decrypt(&key, &nonce, &encrypted);
        assert!(result.is_err());
    }
}

#[test]
fn test_encryption_algorithm_mismatch() {
    let data = b"Test data";
    let key_aes = EncryptionAlgorithm::Aes256Gcm.generate_key();
    let nonce_chacha = EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce();

    let result = EncryptionAlgorithm::Aes256Gcm.encrypt(&key_aes, &nonce_chacha, data);
    assert!(result.is_err());
}

#[test]
fn test_encryption_key_as_bytes() {
    let algo = EncryptionAlgorithm::Aes256Gcm;
    let key_id = EncryptionKeyId::new(1);
    let key = algo.generate_key();
    let bytes = key.as_bytes();
    assert_eq!(bytes.len(), 32);

    let restored = EncryptionKey::from_bytes(algo, key_id, bytes.to_vec()).unwrap();
    assert_eq!(key, restored);
}

#[test]
fn test_encryption_nonce_as_bytes() {
    let algo = EncryptionAlgorithm::Aes256Gcm;
    let nonce = algo.generate_nonce();
    let bytes = nonce.as_bytes();
    assert_eq!(bytes.len(), 12);

    let restored = Nonce::from_bytes(algo, bytes.to_vec()).unwrap();
    assert_eq!(nonce, restored);
}

// ============================================================================
// Integrity Module Tests
// ============================================================================

#[test]
fn test_integrity_algorithm_serialization() {
    let algorithms = [
        IntegrityAlgorithm::None,
        IntegrityAlgorithm::Crc32c,
        IntegrityAlgorithm::XXHash32,
    ];

    for algo in algorithms {
        let byte = algo.as_u8();
        let restored = IntegrityAlgorithm::from_u8(byte).unwrap();
        assert_eq!(algo, restored);
    }

    // Test invalid value
    assert!(IntegrityAlgorithm::from_u8(255).is_none());
}

#[test]
fn test_integrity_algorithm_names() {
    assert_eq!(IntegrityAlgorithm::None.name(), "none");
    assert_eq!(IntegrityAlgorithm::Crc32c.name(), "crc32c");
    assert_eq!(IntegrityAlgorithm::XXHash32.name(), "xxhash32");
}

#[test]
fn test_integrity_hash_sizes() {
    assert_eq!(IntegrityAlgorithm::None.hash_size(), 0);
    assert_eq!(IntegrityAlgorithm::Crc32c.hash_size(), 4);
    assert_eq!(IntegrityAlgorithm::XXHash32.hash_size(), 4);
}

#[test]
fn test_integrity_empty_data() {
    let empty: &[u8] = &[];
    for algo in [
        IntegrityAlgorithm::None,
        IntegrityAlgorithm::Crc32c,
        IntegrityAlgorithm::XXHash32,
    ] {
        let hash = algo.hash(empty);
        assert!(algo.verify(empty, &hash).is_ok());
    }
}

#[test]
fn test_integrity_single_byte() {
    let data = &[42u8];
    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        let hash = algo.hash(data);
        assert!(algo.verify(data, &hash).is_ok());
    }
}

#[test]
fn test_integrity_large_data() {
    let data = vec![42u8; 1024 * 1024]; // 1MB
    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        let hash = algo.hash(&data);
        assert!(algo.verify(&data, &hash).is_ok());
    }
}

#[test]
fn test_integrity_different_data_different_hash() {
    let data1 = b"Hello, World!";
    let data2 = b"Hello, World?";

    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        let hash1 = algo.hash(data1);
        let hash2 = algo.hash(data2);
        assert_ne!(hash1, hash2);
    }
}

#[test]
fn test_integrity_verification_fails_on_wrong_data() {
    let data = b"Original data";
    let wrong_data = b"Modified data";

    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        let hash = algo.hash(data);
        let result = algo.verify(wrong_data, &hash);
        assert!(result.is_err());
        match result {
            Err(Error::IntegrityCheckFailed { .. }) => {}
            _ => panic!("Expected IntegrityCheckFailed error"),
        }
    }
}

#[test]
fn test_integrity_hash_serialization() {
    let data = b"Test data for serialization";
    for algo in [
        IntegrityAlgorithm::None,
        IntegrityAlgorithm::Crc32c,
        IntegrityAlgorithm::XXHash32,
    ] {
        let hash = algo.hash(data);
        let bytes = hash.to_bytes();
        assert_eq!(bytes.len(), algo.hash_size());

        let restored = IntegrityHash::from_bytes(algo, &bytes).unwrap();
        assert_eq!(hash, restored);
    }
}

#[test]
fn test_integrity_hash_as_u32() {
    let data = b"Test data";

    let hash_none = IntegrityAlgorithm::None.hash(data);
    assert_eq!(hash_none.as_u32(), None);

    let hash_crc = IntegrityAlgorithm::Crc32c.hash(data);
    assert!(hash_crc.as_u32().is_some());

    let hash_xx = IntegrityAlgorithm::XXHash32.hash(data);
    assert!(hash_xx.as_u32().is_some());
}

#[test]
fn test_integrity_incremental_hashing() {
    let data = b"Hello, World! This is a test string for incremental hashing.";

    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        // Hash all at once
        let direct_hash = algo.hash(data);

        // Hash incrementally
        let mut hasher = algo.hasher();
        hasher.update(&data[..20]);
        hasher.update(&data[20..40]);
        hasher.update(&data[40..]);
        let incremental_hash = hasher.finalize();

        assert_eq!(direct_hash, incremental_hash);
    }
}

#[test]
fn test_integrity_hasher_algorithm() {
    for algo in [
        IntegrityAlgorithm::None,
        IntegrityAlgorithm::Crc32c,
        IntegrityAlgorithm::XXHash32,
    ] {
        let hasher = algo.hasher();
        assert_eq!(hasher.algorithm(), algo);
    }
}

#[test]
fn test_integrity_hash_from_bytes_invalid() {
    // Wrong size for Crc32c
    let result = IntegrityHash::from_bytes(IntegrityAlgorithm::Crc32c, &[1, 2, 3]);
    assert!(result.is_err());

    // Non-empty for None
    let result = IntegrityHash::from_bytes(IntegrityAlgorithm::None, &[1]);
    assert!(result.is_err());
}

#[test]
fn test_integrity_deterministic() {
    let data = b"Deterministic test data";

    for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
        let hash1 = algo.hash(data);
        let hash2 = algo.hash(data);
        assert_eq!(
            hash1,
            hash2,
            "Hash should be deterministic for {}",
            algo.name()
        );
    }
}

// ============================================================================
// Cross-Module Integration Tests
// ============================================================================

#[test]
fn test_compress_then_encrypt() {
    let data = b"Hello, World! This is a test message that will be compressed and encrypted.";

    // Compress
    let compressed = CompressionAlgorithm::Zstd.compress(data).unwrap();

    // Encrypt
    let key = EncryptionAlgorithm::Aes256Gcm.generate_key();
    let nonce = EncryptionAlgorithm::Aes256Gcm.generate_nonce();
    let encrypted = EncryptionAlgorithm::Aes256Gcm
        .encrypt(&key, &nonce, &compressed)
        .unwrap();

    // Decrypt
    let decrypted = EncryptionAlgorithm::Aes256Gcm
        .decrypt(&key, &nonce, &encrypted)
        .unwrap();

    // Decompress
    let decompressed = CompressionAlgorithm::Zstd
        .decompress(&decrypted, Some(data.len()))
        .unwrap();

    assert_eq!(decompressed, data);
}

#[test]
fn test_compress_with_integrity() {
    let data = b"Test data for compression with integrity checking";

    // Compress
    let compressed = CompressionAlgorithm::Lz4.compress(data).unwrap();

    // Calculate integrity hash
    let hash = IntegrityAlgorithm::Crc32c.hash(&compressed);

    // Verify integrity
    assert!(
        IntegrityAlgorithm::Crc32c
            .verify(&compressed, &hash)
            .is_ok()
    );

    // Decompress
    let decompressed = CompressionAlgorithm::Lz4
        .decompress(&compressed, Some(data.len()))
        .unwrap();

    assert_eq!(decompressed, data);
}

#[test]
fn test_full_pipeline() {
    let original_data = b"This is the original data that will go through the full pipeline: compress, hash, encrypt, decrypt, verify, decompress.";

    // Step 1: Compress
    let compressed = CompressionAlgorithm::Zstd.compress(original_data).unwrap();

    // Step 2: Calculate integrity hash
    let hash = IntegrityAlgorithm::XXHash32.hash(&compressed);

    // Step 3: Encrypt
    let key = EncryptionAlgorithm::ChaCha20Poly1305.generate_key();
    let nonce = EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce();
    let encrypted = EncryptionAlgorithm::ChaCha20Poly1305
        .encrypt(&key, &nonce, &compressed)
        .unwrap();

    // Step 4: Decrypt
    let decrypted = EncryptionAlgorithm::ChaCha20Poly1305
        .decrypt(&key, &nonce, &encrypted)
        .unwrap();

    // Step 5: Verify integrity
    assert!(
        IntegrityAlgorithm::XXHash32
            .verify(&decrypted, &hash)
            .is_ok()
    );

    // Step 6: Decompress
    let decompressed = CompressionAlgorithm::Zstd
        .decompress(&decrypted, Some(original_data.len()))
        .unwrap();

    assert_eq!(decompressed, original_data);
}
