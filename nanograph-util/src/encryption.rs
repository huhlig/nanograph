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
use aes_gcm::{
    Aes256Gcm, Key as AesKey, Nonce as AesNonce,
    aead::{Aead, KeyInit, OsRng},
};
use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};
use rand::RngCore;

/// Encryption Algorithm
///
/// This enum defines the supported encryption algorithms for the Nanograph system.
///
/// # Examples
///
/// ```
/// use nanograph_util::{EncryptionAlgorithm, EncryptionKey, Nonce};
///
/// let algorithm = EncryptionAlgorithm::Aes256Gcm;
/// let key = algorithm.generate_key();
/// let nonce = algorithm.generate_nonce();
/// let data = b"secret message";
///
/// let ciphertext = algorithm.encrypt(&key, &nonce, data).unwrap();
/// let decrypted = algorithm.decrypt(&key, &nonce, &ciphertext).unwrap();
///
/// assert_eq!(data, decrypted.as_slice());
/// ```
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum EncryptionAlgorithm {
    /// No encryption
    #[default]
    None = 0,
    /// AES-256-GCM - Industry standard, hardware accelerated on modern CPUs
    Aes256Gcm = 1,
    /// ChaCha20-Poly1305 - Fast software implementation, good for systems without AES-NI
    ChaCha20Poly1305 = 2,
}

impl EncryptionAlgorithm {
    /// Convert the encryption algorithm to a u8 value for serialization
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::None.as_u8(), 0);
    /// assert_eq!(EncryptionAlgorithm::Aes256Gcm.as_u8(), 1);
    /// ```
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert a u8 value to an encryption algorithm, returning None if invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::from_u8(1), Some(EncryptionAlgorithm::Aes256Gcm));
    /// assert_eq!(EncryptionAlgorithm::from_u8(255), None);
    /// ```
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(EncryptionAlgorithm::None),
            1 => Some(EncryptionAlgorithm::Aes256Gcm),
            2 => Some(EncryptionAlgorithm::ChaCha20Poly1305),
            _ => None,
        }
    }

    /// Get the name of the encryption algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::Aes256Gcm.name(), "aes-256-gcm");
    /// ```
    pub const fn name(self) -> &'static str {
        match self {
            EncryptionAlgorithm::None => "none",
            EncryptionAlgorithm::Aes256Gcm => "aes-256-gcm",
            EncryptionAlgorithm::ChaCha20Poly1305 => "chacha20-poly1305",
        }
    }

    /// Get the required key size in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::Aes256Gcm.key_size(), 32);
    /// ```
    pub const fn key_size(self) -> usize {
        match self {
            EncryptionAlgorithm::None => 0,
            EncryptionAlgorithm::Aes256Gcm => 32, // 256 bits
            EncryptionAlgorithm::ChaCha20Poly1305 => 32, // 256 bits
        }
    }

    /// Get the required nonce size in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::Aes256Gcm.nonce_size(), 12);
    /// ```
    pub const fn nonce_size(self) -> usize {
        match self {
            EncryptionAlgorithm::None => 0,
            EncryptionAlgorithm::Aes256Gcm => 12, // 96 bits
            EncryptionAlgorithm::ChaCha20Poly1305 => 12, // 96 bits
        }
    }

    /// Get the authentication tag size in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// assert_eq!(EncryptionAlgorithm::Aes256Gcm.tag_size(), 16);
    /// ```
    pub const fn tag_size(self) -> usize {
        match self {
            EncryptionAlgorithm::None => 0,
            EncryptionAlgorithm::Aes256Gcm => 16, // 128 bits
            EncryptionAlgorithm::ChaCha20Poly1305 => 16, // 128 bits
        }
    }

    /// Generate a new random encryption key
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let key = algorithm.generate_key();
    /// assert_eq!(key.as_bytes().len(), 32);
    /// ```
    pub fn generate_key(self) -> EncryptionKey {
        let size = self.key_size();
        let mut key = vec![0u8; size];
        if size > 0 {
            OsRng.fill_bytes(&mut key);
        }
        EncryptionKey {
            algorithm: self,
            key,
        }
    }

    /// Generate a new random nonce
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::EncryptionAlgorithm;
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let nonce = algorithm.generate_nonce();
    /// assert_eq!(nonce.as_bytes().len(), 12);
    /// ```
    pub fn generate_nonce(self) -> Nonce {
        let size = self.nonce_size();
        let mut nonce = vec![0u8; size];
        if size > 0 {
            OsRng.fill_bytes(&mut nonce);
        }
        Nonce {
            algorithm: self,
            nonce,
        }
    }

    /// Encrypt data with the given key and nonce
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, EncryptionKey, Nonce};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let key = algorithm.generate_key();
    /// let nonce = algorithm.generate_nonce();
    /// let ciphertext = algorithm.encrypt(&key, &nonce, b"data").unwrap();
    /// ```
    pub fn encrypt(self, key: &EncryptionKey, nonce: &Nonce, plaintext: &[u8]) -> Result<Vec<u8>> {
        if key.algorithm != self {
            return Err(Error::Encryption(format!(
                "Key algorithm mismatch: expected {}, got {}",
                self.name(),
                key.algorithm.name()
            )));
        }
        if nonce.algorithm != self {
            return Err(Error::Encryption(format!(
                "Nonce algorithm mismatch: expected {}, got {}",
                self.name(),
                nonce.algorithm.name()
            )));
        }

        match self {
            EncryptionAlgorithm::None => Ok(plaintext.to_vec()),
            EncryptionAlgorithm::Aes256Gcm => {
                let key_array = AesKey::<Aes256Gcm>::from_slice(&key.key);
                let cipher = Aes256Gcm::new(key_array);
                let nonce_array = AesNonce::from_slice(&nonce.nonce);

                cipher
                    .encrypt(nonce_array, plaintext)
                    .map_err(|e| Error::Encryption(format!("AES-256-GCM encryption failed: {}", e)))
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let key_array = ChaChaKey::from_slice(&key.key);
                let cipher = ChaCha20Poly1305::new(key_array);
                let nonce_array = ChaChaNonce::from_slice(&nonce.nonce);

                cipher.encrypt(nonce_array, plaintext).map_err(|e| {
                    Error::Encryption(format!("ChaCha20-Poly1305 encryption failed: {}", e))
                })
            }
        }
    }

    /// Decrypt data with the given key and nonce
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, EncryptionKey, Nonce};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let key = algorithm.generate_key();
    /// let nonce = algorithm.generate_nonce();
    /// let plaintext = b"secret message";
    /// let ciphertext = algorithm.encrypt(&key, &nonce, plaintext).unwrap();
    /// let decrypted = algorithm.decrypt(&key, &nonce, &ciphertext).unwrap();
    /// assert_eq!(decrypted, plaintext);
    /// ```
    pub fn decrypt(self, key: &EncryptionKey, nonce: &Nonce, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if key.algorithm != self {
            return Err(Error::Decryption(format!(
                "Key algorithm mismatch: expected {}, got {}",
                self.name(),
                key.algorithm.name()
            )));
        }
        if nonce.algorithm != self {
            return Err(Error::Decryption(format!(
                "Nonce algorithm mismatch: expected {}, got {}",
                self.name(),
                nonce.algorithm.name()
            )));
        }

        match self {
            EncryptionAlgorithm::None => Ok(ciphertext.to_vec()),
            EncryptionAlgorithm::Aes256Gcm => {
                let key_array = AesKey::<Aes256Gcm>::from_slice(&key.key);
                let cipher = Aes256Gcm::new(key_array);
                let nonce_array = AesNonce::from_slice(&nonce.nonce);

                cipher
                    .decrypt(nonce_array, ciphertext)
                    .map_err(|e| Error::Decryption(format!("AES-256-GCM decryption failed: {}", e)))
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let key_array = ChaChaKey::from_slice(&key.key);
                let cipher = ChaCha20Poly1305::new(key_array);
                let nonce_array = ChaChaNonce::from_slice(&nonce.nonce);

                cipher.decrypt(nonce_array, ciphertext).map_err(|e| {
                    Error::Decryption(format!("ChaCha20-Poly1305 decryption failed: {}", e))
                })
            }
        }
    }
}

/// Encryption key with associated algorithm
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptionKey {
    pub algorithm: EncryptionAlgorithm,
    pub key: Vec<u8>,
}

impl EncryptionKey {
    /// Create a new encryption key from raw bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, EncryptionKey};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let key_bytes = vec![0u8; 32];
    /// let key = EncryptionKey::from_bytes(algorithm, key_bytes).unwrap();
    /// ```
    pub fn from_bytes(algorithm: EncryptionAlgorithm, key: Vec<u8>) -> Result<Self> {
        let expected_size = algorithm.key_size();
        if key.len() != expected_size {
            return Err(Error::InvalidKeySize {
                expected: expected_size,
                actual: key.len(),
            });
        }
        Ok(Self { algorithm, key })
    }

    /// Get the key bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, EncryptionKey};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let key = algorithm.generate_key();
    /// let bytes = key.as_bytes();
    /// assert_eq!(bytes.len(), 32);
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.key
    }
}

/// Nonce (number used once) for encryption
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nonce {
    pub algorithm: EncryptionAlgorithm,
    pub nonce: Vec<u8>,
}

impl Nonce {
    /// Create a new nonce from raw bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, Nonce};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let nonce_bytes = vec![0u8; 12];
    /// let nonce = Nonce::from_bytes(algorithm, nonce_bytes).unwrap();
    /// ```
    pub fn from_bytes(algorithm: EncryptionAlgorithm, nonce: Vec<u8>) -> Result<Self> {
        let expected_size = algorithm.nonce_size();
        if nonce.len() != expected_size {
            return Err(Error::InvalidNonceSize {
                expected: expected_size,
                actual: nonce.len(),
            });
        }
        Ok(Self { algorithm, nonce })
    }

    /// Get the nonce bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::{EncryptionAlgorithm, Nonce};
    ///
    /// let algorithm = EncryptionAlgorithm::Aes256Gcm;
    /// let nonce = algorithm.generate_nonce();
    /// let bytes = nonce.as_bytes();
    /// assert_eq!(bytes.len(), 12);
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"Hello, World! This is a secret message.";

    #[test]
    fn test_none_encryption() {
        let key = EncryptionAlgorithm::None.generate_key();
        let nonce = EncryptionAlgorithm::None.generate_nonce();

        let encrypted = EncryptionAlgorithm::None
            .encrypt(&key, &nonce, TEST_DATA)
            .unwrap();
        assert_eq!(encrypted, TEST_DATA);

        let decrypted = EncryptionAlgorithm::None
            .decrypt(&key, &nonce, &encrypted)
            .unwrap();
        assert_eq!(decrypted, TEST_DATA);
    }

    #[test]
    fn test_aes256gcm_encryption() {
        let key = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let nonce = EncryptionAlgorithm::Aes256Gcm.generate_nonce();

        let encrypted = EncryptionAlgorithm::Aes256Gcm
            .encrypt(&key, &nonce, TEST_DATA)
            .unwrap();
        assert_ne!(encrypted, TEST_DATA);
        assert!(encrypted.len() > TEST_DATA.len()); // Includes auth tag

        let decrypted = EncryptionAlgorithm::Aes256Gcm
            .decrypt(&key, &nonce, &encrypted)
            .unwrap();
        assert_eq!(decrypted, TEST_DATA);
    }

    #[test]
    fn test_chacha20poly1305_encryption() {
        let key = EncryptionAlgorithm::ChaCha20Poly1305.generate_key();
        let nonce = EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce();

        let encrypted = EncryptionAlgorithm::ChaCha20Poly1305
            .encrypt(&key, &nonce, TEST_DATA)
            .unwrap();
        assert_ne!(encrypted, TEST_DATA);
        assert!(encrypted.len() > TEST_DATA.len()); // Includes auth tag

        let decrypted = EncryptionAlgorithm::ChaCha20Poly1305
            .decrypt(&key, &nonce, &encrypted)
            .unwrap();
        assert_eq!(decrypted, TEST_DATA);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let key2 = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let nonce = EncryptionAlgorithm::Aes256Gcm.generate_nonce();

        let encrypted = EncryptionAlgorithm::Aes256Gcm
            .encrypt(&key1, &nonce, TEST_DATA)
            .unwrap();
        let result = EncryptionAlgorithm::Aes256Gcm.decrypt(&key2, &nonce, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_algorithm_serialization() {
        for algo in [
            EncryptionAlgorithm::None,
            EncryptionAlgorithm::Aes256Gcm,
            EncryptionAlgorithm::ChaCha20Poly1305,
        ] {
            let byte = algo.as_u8();
            let restored = EncryptionAlgorithm::from_u8(byte).unwrap();
            assert_eq!(algo, restored);
        }
    }

    #[test]
    fn test_key_sizes() {
        assert_eq!(EncryptionAlgorithm::None.key_size(), 0);
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.key_size(), 32);
        assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.key_size(), 32);
    }

    #[test]
    fn test_nonce_sizes() {
        assert_eq!(EncryptionAlgorithm::None.nonce_size(), 0);
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.nonce_size(), 12);
        assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.nonce_size(), 12);
    }
}

// Made with Bob
