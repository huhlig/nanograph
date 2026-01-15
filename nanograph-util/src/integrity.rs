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

use crate::error::{Error, Result};

const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);

/// Integrity Algorithm for checksums and hashing
///
/// This enum defines the supported integrity algorithms for the Nanograph system.
///
/// # Examples
///
/// ```
/// use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
///
/// let data = b"hello world";
/// let algorithm = IntegrityAlgorithm::Crc32c;
/// let hash = algorithm.hash(data);
///
/// assert!(algorithm.verify(data, &hash).is_ok());
/// ```
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum IntegrityAlgorithm {
    /// No integrity check
    #[default]
    None = 0,
    /// CRC32C Checksum - Hardware accelerated on modern CPUs, good for error detection
    Crc32c = 1,
    /// XXHash32 - Very fast non-cryptographic hash
    XXHash32 = 2,
}

impl IntegrityAlgorithm {
    /// Convert the integrity algorithm to a u8 value for serialization
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityAlgorithm;
    ///
    /// assert_eq!(IntegrityAlgorithm::None.as_u8(), 0);
    /// assert_eq!(IntegrityAlgorithm::Crc32c.as_u8(), 1);
    /// ```
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert a u8 value to an integrity algorithm, returning None if invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityAlgorithm;
    ///
    /// assert_eq!(IntegrityAlgorithm::from_u8(1), Some(IntegrityAlgorithm::Crc32c));
    /// assert_eq!(IntegrityAlgorithm::from_u8(255), None);
    /// ```
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(IntegrityAlgorithm::None),
            1 => Some(IntegrityAlgorithm::Crc32c),
            2 => Some(IntegrityAlgorithm::XXHash32),
            _ => None,
        }
    }

    /// Get the name of the integrity algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityAlgorithm;
    ///
    /// assert_eq!(IntegrityAlgorithm::Crc32c.name(), "crc32c");
    /// ```
    pub const fn name(self) -> &'static str {
        match self {
            IntegrityAlgorithm::None => "none",
            IntegrityAlgorithm::Crc32c => "crc32c",
            IntegrityAlgorithm::XXHash32 => "xxhash32",
        }
    }

    /// Get the size of the hash output in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityAlgorithm;
    ///
    /// assert_eq!(IntegrityAlgorithm::Crc32c.hash_size(), 4);
    /// ```
    pub const fn hash_size(self) -> usize {
        match self {
            IntegrityAlgorithm::None => 0,
            IntegrityAlgorithm::Crc32c => 4,
            IntegrityAlgorithm::XXHash32 => 4,
        }
    }

    /// Calculate the integrity hash for the given data
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
    ///
    /// let hash = IntegrityAlgorithm::Crc32c.hash(b"test");
    /// match hash {
    ///     IntegrityHash::Hash32(v) => println!("Hash: {:08x}", v),
    ///     _ => panic!("Expected 32-bit hash"),
    /// }
    /// ```
    pub fn hash(self, data: &[u8]) -> IntegrityHash {
        match self {
            IntegrityAlgorithm::None => IntegrityHash::None,
            IntegrityAlgorithm::Crc32c => IntegrityHash::Hash32(CRC.checksum(data)),
            IntegrityAlgorithm::XXHash32 => {
                IntegrityHash::Hash32(xxhash_rust::xxh32::xxh32(data, 0))
            }
        }
    }

    /// Verify that the data matches the expected hash
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
    ///
    /// let algorithm = IntegrityAlgorithm::Crc32c;
    /// let data = b"test data";
    /// let hash = algorithm.hash(data);
    ///
    /// assert!(algorithm.verify(data, &hash).is_ok());
    /// ```
    pub fn verify(self, data: &[u8], expected: &IntegrityHash) -> Result<()> {
        let actual = self.hash(data);
        if actual == *expected {
            Ok(())
        } else {
            match (expected, actual) {
                (IntegrityHash::Hash32(exp), IntegrityHash::Hash32(act)) => {
                    Err(Error::IntegrityCheckFailed {
                        expected: *exp,
                        actual: act,
                    })
                }
                _ => Err(Error::IntegrityCheckFailed {
                    expected: 0,
                    actual: 0,
                }),
            }
        }
    }

    /// Create a new hasher for incremental hashing
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher, IntegrityHash};
    ///
    /// let mut hasher = IntegrityAlgorithm::Crc32c.hasher();
    /// hasher.update(b"hello");
    /// hasher.update(b" world");
    /// let hash = hasher.finalize();
    /// ```
    pub fn hasher(self) -> IntegrityHasher {
        IntegrityHasher::new(self)
    }
}

/// Hash value produced by an integrity algorithm
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntegrityHash {
    /// No hash
    None,
    /// 32-bit hash value
    Hash32(u32),
}

impl IntegrityHash {
    /// Convert the hash to bytes for serialization
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityHash;
    ///
    /// let hash = IntegrityHash::Hash32(0x12345678);
    /// // Note: uses little-endian by default in current implementation
    /// let bytes = hash.to_bytes();
    /// ```
    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            IntegrityHash::None => Vec::new(),
            IntegrityHash::Hash32(v) => v.to_le_bytes().to_vec(),
        }
    }

    /// Create a hash from bytes and algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHash};
    ///
    /// let bytes = [0x78, 0x56, 0x34, 0x12]; // 0x12345678 in little-endian
    /// let hash = IntegrityHash::from_bytes(IntegrityAlgorithm::Crc32c, &bytes).unwrap();
    /// assert_eq!(hash, IntegrityHash::Hash32(0x12345678));
    /// ```
    pub fn from_bytes(algorithm: IntegrityAlgorithm, bytes: &[u8]) -> Result<Self> {
        match algorithm {
            IntegrityAlgorithm::None => {
                if !bytes.is_empty() {
                    return Err(Error::InvalidAlgorithm(algorithm.as_u8()));
                }
                Ok(IntegrityHash::None)
            }
            IntegrityAlgorithm::Crc32c | IntegrityAlgorithm::XXHash32 => {
                if bytes.len() != 4 {
                    return Err(Error::InvalidAlgorithm(algorithm.as_u8()));
                }
                let mut array = [0u8; 4];
                array.copy_from_slice(bytes);
                Ok(IntegrityHash::Hash32(u32::from_le_bytes(array)))
            }
        }
    }

    /// Get the hash as a u32 (for 32-bit hashes)
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::IntegrityHash;
    ///
    /// let hash = IntegrityHash::Hash32(42);
    /// assert_eq!(hash.as_u32(), Some(42));
    /// ```
    pub fn as_u32(self) -> Option<u32> {
        match self {
            IntegrityHash::Hash32(v) => Some(v),
            _ => None,
        }
    }
}

/// Incremental hasher for computing integrity hashes over multiple chunks
///
/// # Examples
///
/// ```
/// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher, IntegrityHash};
///
/// let mut hasher = IntegrityHasher::new(IntegrityAlgorithm::Crc32c);
/// hasher.update(b"test");
/// let hash = hasher.finalize();
/// ```
pub struct IntegrityHasher {
    algorithm: IntegrityAlgorithm,
    state: HasherState,
}

enum HasherState {
    None,
    Crc32c(crc::Digest<'static, u32>),
    XXHash32(xxhash_rust::xxh32::Xxh32),
}

impl IntegrityHasher {
    /// Create a new hasher for the given algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher};
    ///
    /// let hasher = IntegrityHasher::new(IntegrityAlgorithm::Crc32c);
    /// ```
    pub fn new(algorithm: IntegrityAlgorithm) -> Self {
        let state = match algorithm {
            IntegrityAlgorithm::None => HasherState::None,
            IntegrityAlgorithm::Crc32c => HasherState::Crc32c(CRC.digest()),
            IntegrityAlgorithm::XXHash32 => {
                HasherState::XXHash32(xxhash_rust::xxh32::Xxh32::new(0))
            }
        };
        Self { algorithm, state }
    }

    /// Update the hasher with more data
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher};
    ///
    /// let mut hasher = IntegrityHasher::new(IntegrityAlgorithm::Crc32c);
    /// hasher.update(b"more data");
    /// ```
    pub fn update(&mut self, data: &[u8]) {
        match &mut self.state {
            HasherState::None => {}
            HasherState::Crc32c(crc) => {
                crc.update(data);
            }
            HasherState::XXHash32(hasher) => {
                hasher.update(data);
            }
        }
    }

    /// Finalize the hash and return the result
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher, IntegrityHash};
    ///
    /// let mut hasher = IntegrityHasher::new(IntegrityAlgorithm::Crc32c);
    /// hasher.update(b"data");
    /// let hash = hasher.finalize();
    /// assert!(matches!(hash, IntegrityHash::Hash32(_)));
    /// ```
    pub fn finalize(self) -> IntegrityHash {
        match self.state {
            HasherState::None => IntegrityHash::None,
            HasherState::Crc32c(crc) => IntegrityHash::Hash32(crc.finalize()),
            HasherState::XXHash32(hasher) => IntegrityHash::Hash32(hasher.digest()),
        }
    }

    /// Get the algorithm being used
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{IntegrityAlgorithm, IntegrityHasher};
    ///
    /// let hasher = IntegrityHasher::new(IntegrityAlgorithm::Crc32c);
    /// assert_eq!(hasher.algorithm(), IntegrityAlgorithm::Crc32c);
    /// ```
    pub fn algorithm(&self) -> IntegrityAlgorithm {
        self.algorithm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"Hello, World! This is a test string for integrity checking.";

    #[test]
    fn test_none_integrity() {
        let hash = IntegrityAlgorithm::None.hash(TEST_DATA);
        assert_eq!(hash, IntegrityHash::None);
        assert!(IntegrityAlgorithm::None.verify(TEST_DATA, &hash).is_ok());
    }

    #[test]
    fn test_crc32c_integrity() {
        let hash = IntegrityAlgorithm::Crc32c.hash(TEST_DATA);
        assert!(matches!(hash, IntegrityHash::Hash32(_)));
        assert!(IntegrityAlgorithm::Crc32c.verify(TEST_DATA, &hash).is_ok());

        // Verify fails with wrong data
        let wrong_data = b"Wrong data";
        assert!(
            IntegrityAlgorithm::Crc32c
                .verify(wrong_data, &hash)
                .is_err()
        );
    }

    #[test]
    fn test_xxhash32_integrity() {
        let hash = IntegrityAlgorithm::XXHash32.hash(TEST_DATA);
        assert!(matches!(hash, IntegrityHash::Hash32(_)));
        assert!(
            IntegrityAlgorithm::XXHash32
                .verify(TEST_DATA, &hash)
                .is_ok()
        );
    }

    #[test]
    fn test_incremental_hashing() {
        for algo in [IntegrityAlgorithm::Crc32c, IntegrityAlgorithm::XXHash32] {
            let mut hasher = algo.hasher();
            hasher.update(&TEST_DATA[..30]);
            hasher.update(&TEST_DATA[30..]);
            let incremental_hash = hasher.finalize();

            let direct_hash = algo.hash(TEST_DATA);
            assert_eq!(incremental_hash, direct_hash);
        }
    }

    #[test]
    fn test_hash_serialization() {
        for algo in [
            IntegrityAlgorithm::None,
            IntegrityAlgorithm::Crc32c,
            IntegrityAlgorithm::XXHash32,
        ] {
            let hash = algo.hash(TEST_DATA);
            let bytes = hash.to_bytes();
            let restored = IntegrityHash::from_bytes(algo, &bytes).unwrap();
            assert_eq!(hash, restored);
        }
    }

    #[test]
    fn test_algorithm_serialization() {
        for algo in [
            IntegrityAlgorithm::None,
            IntegrityAlgorithm::Crc32c,
            IntegrityAlgorithm::XXHash32,
        ] {
            let byte = algo.as_u8();
            let restored = IntegrityAlgorithm::from_u8(byte).unwrap();
            assert_eq!(algo, restored);
        }
    }

    #[test]
    fn test_hash_sizes() {
        assert_eq!(IntegrityAlgorithm::None.hash_size(), 0);
        assert_eq!(IntegrityAlgorithm::Crc32c.hash_size(), 4);
        assert_eq!(IntegrityAlgorithm::XXHash32.hash_size(), 4);
    }
}
