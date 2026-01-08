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
//! ## Usage Examples
//!
//! ### Compression
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
//! ```
//!
//! ### Encryption
//!
//! ```
//! use nanograph_util::{EncryptionAlgorithm, EncryptionKey, Nonce};
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
//! ### Integrity
//!
//! ```
//! use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
//!
//! let data = b"important data";
//! let algorithm = IntegrityAlgorithm::Crc32c;
//!
//! let hash = algorithm.hash(data);
//! assert!(algorithm.verify(data, &hash).is_ok());
//! ```

mod compression;
mod encryption;
mod error;
mod integrity;

pub use compression::CompressionAlgorithm;
pub use encryption::{EncryptionAlgorithm, EncryptionKey, Nonce};
pub use error::{Error, Result};
pub use integrity::{IntegrityAlgorithm, IntegrityHash, IntegrityHasher};
