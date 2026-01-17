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

//! # nanograph-util
//!
//! Utility crate providing compression, encryption, and integrity checking
//! functionality for the Nanograph database system.
//!
//! This crate provides a unified interface for various algorithms used to ensure
//! data efficiency, security, and correctness.
//!
//! ## Features
//!
//! - **Compression**: Multiple algorithms including LZ4, Zstd, and Snappy.
//! - **Encryption**: Authenticated encryption using AES-256-GCM and ChaCha20-Poly1305.
//! - **Integrity**: Checksumming and hashing using CRC32C and XXHash32.
//!
//! ## Examples
//!
//! ### Basic Compression
//!
//! ```
//! use nanograph_util::CompressionAlgorithm;
//!
//! let data = b"some repetitive data repetitive data repetitive data";
//! let algorithm = CompressionAlgorithm::Zstd;
//!
//! let compressed = algorithm.compress(data).unwrap();
//! let decompressed = algorithm.decompress(&compressed, None).unwrap();
//!
//! assert_eq!(data, decompressed.as_slice());
//! println!("Original: {} bytes, Compressed: {} bytes", data.len(), compressed.len());
//! ```
//!
//! ### Comparing Compression Algorithms
//!
//! ```
//! use nanograph_util::CompressionAlgorithm;
//!
//! let data = b"test data ".repeat(100);
//!
//! // Try different algorithms
//! let lz4 = CompressionAlgorithm::Lz4;
//! let zstd = CompressionAlgorithm::Zstd;
//! let snappy = CompressionAlgorithm::Snappy;
//!
//! let lz4_compressed = lz4.compress(&data).unwrap();
//! let zstd_compressed = zstd.compress(&data).unwrap();
//! let snappy_compressed = snappy.compress(&data).unwrap();
//!
//! println!("LZ4: {} bytes", lz4_compressed.len());
//! println!("Zstd: {} bytes", zstd_compressed.len());
//! println!("Snappy: {} bytes", snappy_compressed.len());
//! ```
//!
//! ### Encryption with AES-256-GCM
//!
//! ```
//! use nanograph_util::EncryptionAlgorithm;
//!
//! let algorithm = EncryptionAlgorithm::Aes256Gcm;
//! let key = algorithm.generate_key();
//! let nonce = algorithm.generate_nonce();
//! let data = b"confidential information";
//!
//! let ciphertext = algorithm.encrypt(&key, &nonce, data).unwrap();
//! let decrypted = algorithm.decrypt(&key, &nonce, &ciphertext).unwrap();
//!
//! assert_eq!(data, decrypted.as_slice());
//! assert_ne!(data.as_slice(), ciphertext.as_slice());
//! ```
//!
//! ### Encryption with ChaCha20-Poly1305
//!
//! ```
//! use nanograph_util::EncryptionAlgorithm;
//!
//! let algorithm = EncryptionAlgorithm::ChaCha20Poly1305;
//! let key = algorithm.generate_key();
//! let nonce = algorithm.generate_nonce();
//! let data = b"top secret information";
//!
//! let ciphertext = algorithm.encrypt(&key, &nonce, data).unwrap();
//! let decrypted = algorithm.decrypt(&key, &nonce, &ciphertext).unwrap();
//!
//! assert_eq!(data, decrypted.as_slice());
//! ```
//!
//! ### Integrity Checking with CRC32C
//!
//! ```
//! use nanograph_util::IntegrityAlgorithm;
//!
//! let data = b"important data";
//! let algorithm = IntegrityAlgorithm::Crc32c;
//!
//! let hash = algorithm.hash(data);
//! assert!(algorithm.verify(data, &hash).is_ok());
//!
//! // Verify fails with corrupted data
//! let corrupted = b"corrupted data";
//! assert!(algorithm.verify(corrupted, &hash).is_err());
//! ```
//!
//! ### Integrity Checking with XXHash
//!
//! ```
//! use nanograph_util::IntegrityAlgorithm;
//!
//! let data = b"data to hash";
//! let algorithm = IntegrityAlgorithm::XXHash32;
//!
//! let hash = algorithm.hash(data);
//! assert!(algorithm.verify(data, &hash).is_ok());
//! ```
//!
//! ### Combined Usage: Compress, Encrypt, and Hash
//!
//! ```
//! use nanograph_util::{CompressionAlgorithm, EncryptionAlgorithm, IntegrityAlgorithm};
//!
//! let original_data = b"sensitive data ".repeat(50);
//!
//! // 1. Compress
//! let compression = CompressionAlgorithm::Zstd;
//! let compressed = compression.compress(&original_data).unwrap();
//!
//! // 2. Encrypt
//! let encryption = EncryptionAlgorithm::Aes256Gcm;
//! let key = encryption.generate_key();
//! let nonce = encryption.generate_nonce();
//! let encrypted = encryption.encrypt(&key, &nonce, &compressed).unwrap();
//!
//! // 3. Hash for integrity
//! let integrity = IntegrityAlgorithm::Crc32c;
//! let hash = integrity.hash(&encrypted);
//!
//! // Verify and decrypt
//! assert!(integrity.verify(&encrypted, &hash).is_ok());
//! let decrypted = encryption.decrypt(&key, &nonce, &encrypted).unwrap();
//! let decompressed = compression.decompress(&decrypted, None).unwrap();
//!
//! assert_eq!(original_data.as_slice(), decompressed.as_slice());
//! ```

mod compression;
mod encryption;
mod error;
mod integrity;
mod cache;

pub use compression::CompressionAlgorithm;
pub use encryption::{EncryptionAlgorithm, EncryptionKey, Nonce};
pub use error::{Error, Result};
pub use integrity::{IntegrityAlgorithm, IntegrityHash, IntegrityHasher};
pub use cache::CacheMap;
