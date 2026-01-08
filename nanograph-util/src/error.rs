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

use thiserror::Error;

/// Result type for nanograph-util operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for nanograph-util operations
///
/// This enum defines all possible errors that can occur when using the
/// compression, encryption, and integrity utilities.
#[derive(Debug, Error)]
pub enum Error {
    /// Compression error with a descriptive message
    #[error("Compression error: {0}")]
    Compression(String),

    /// Decompression error with a descriptive message
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Encryption error with a descriptive message
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Decryption error with a descriptive message
    #[error("Decryption error: {0}")]
    Decryption(String),

    /// Invalid algorithm identifier (u8 value not recognized)
    #[error("Invalid algorithm identifier: {0}")]
    InvalidAlgorithm(u8),

    /// Provided buffer is too small for the operation
    #[error("Buffer too small: required {required}, available {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes actually available in the buffer
        available: usize,
    },

    /// Provided encryption key has the wrong size for the chosen algorithm
    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize {
        /// Expected size in bytes
        expected: usize,
        /// Actual size provided
        actual: usize,
    },

    /// Provided nonce has the wrong size for the chosen algorithm
    #[error("Invalid nonce size: expected {expected}, got {actual}")]
    InvalidNonceSize {
        /// Expected size in bytes
        expected: usize,
        /// Actual size provided
        actual: usize,
    },

    /// Integrity check (checksum/hash) failed
    #[error("Integrity check failed: expected {expected:08x}, got {actual:08x}")]
    IntegrityCheckFailed {
        /// Expected hash value
        expected: u32,
        /// Actual hash value calculated from data
        actual: u32,
    },

    /// Underlying IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
