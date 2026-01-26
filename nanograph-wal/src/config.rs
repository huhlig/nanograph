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

// Re-export types from nanograph-util for convenience
pub use nanograph_util::{
    CompressionAlgorithm, EncryptionAlgorithm, EncryptionKey, IntegrityAlgorithm,
};

/// Configuration for the Write Ahead Log Service
///
/// # Examples
///
/// ```rust
/// use nanograph_wal::WriteAheadLogConfig;
///
/// let config = WriteAheadLogConfig::new(1)
///     .with_max_segment_size(1024 * 1024);
///
/// assert_eq!(config.shard_id, 1);
/// assert_eq!(config.max_segment_size, 1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct WriteAheadLogConfig {
    /// Unique identifier for this shard's WAL
    pub shard_id: u128,
    /// Maximum size of a single WAL segment before rotation
    pub max_segment_size: u64,
    /// Whether to force fsync when rotating segments
    pub sync_on_rotate: bool,
    /// Integrity/Checksum algorithm to use
    pub checksum: IntegrityAlgorithm,
    /// Compression algorithm to use
    pub compression: CompressionAlgorithm,
    /// Encryption algorithm to use
    pub encryption: EncryptionAlgorithm,
    /// Encryption key (if encryption is enabled)
    pub encryption_key: Option<EncryptionKey>,
}

impl Default for WriteAheadLogConfig {
    fn default() -> Self {
        Self {
            shard_id: 0,
            max_segment_size: 64 * 1024 * 1024, // 64 MB
            sync_on_rotate: true,
            checksum: IntegrityAlgorithm::Crc32c,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key: None,
        }
    }
}

impl WriteAheadLogConfig {
    /// Create a new configuration with the given shard ID
    pub fn new(shard_id: u128) -> Self {
        Self {
            shard_id,
            ..Default::default()
        }
    }

    /// Set the maximum segment size
    pub fn with_max_segment_size(mut self, size: u64) -> Self {
        self.max_segment_size = size;
        self
    }

    /// Set whether to sync on rotate
    pub fn with_sync_on_rotate(mut self, sync: bool) -> Self {
        self.sync_on_rotate = sync;
        self
    }

    /// Set the integrity algorithm
    pub fn with_integrity(mut self, integrity: IntegrityAlgorithm) -> Self {
        self.checksum = integrity;
        self
    }

    /// Set the compression algorithm
    pub fn with_compression(mut self, compression: CompressionAlgorithm) -> Self {
        self.compression = compression;
        self
    }

    /// Set the encryption algorithm and key
    pub fn with_encryption(
        mut self,
        encryption: EncryptionAlgorithm,
        key: Option<EncryptionKey>,
    ) -> Self {
        self.encryption = encryption;
        self.encryption_key = key;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_segment_size == 0 {
            return Err("max_segment_size must be greater than 0".to_string());
        }

        // If encryption is enabled, ensure we have a key
        if self.encryption != EncryptionAlgorithm::None && self.encryption_key.is_none() {
            return Err("encryption_key must be provided when encryption is enabled".to_string());
        }

        // If encryption key is provided, ensure it matches the algorithm
        if let Some(ref key) = self.encryption_key {
            if key.algorithm != self.encryption {
                return Err(format!(
                    "encryption_key algorithm ({:?}) does not match config encryption ({:?})",
                    key.algorithm, self.encryption
                ));
            }
        }

        Ok(())
    }
}

/// Durability level for writes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Buffered in memory only, lost on crash or power failure.
    Memory,

    /// Written to OS buffers, persisted on power failure if OS survives, lost on crash.
    Flush,

    /// Fully persisted to stable storage via fsync, survives crash and power failure.
    Sync,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = WriteAheadLogConfig::default();
        assert_eq!(config.shard_id, 0);
        assert_eq!(config.max_segment_size, 64 * 1024 * 1024);
        assert!(config.sync_on_rotate);
        assert_eq!(config.checksum, IntegrityAlgorithm::Crc32c);
        assert_eq!(config.compression, CompressionAlgorithm::None);
        assert_eq!(config.encryption, EncryptionAlgorithm::None);
        assert!(config.encryption_key.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = WriteAheadLogConfig::new(42)
            .with_max_segment_size(128 * 1024 * 1024)
            .with_sync_on_rotate(false)
            .with_integrity(IntegrityAlgorithm::XXHash32)
            .with_compression(CompressionAlgorithm::Lz4);

        assert_eq!(config.shard_id, 42);
        assert_eq!(config.max_segment_size, 128 * 1024 * 1024);
        assert!(!config.sync_on_rotate);
        assert_eq!(config.checksum, IntegrityAlgorithm::XXHash32);
        assert_eq!(config.compression, CompressionAlgorithm::Lz4);
    }

    #[test]
    fn test_config_validation() {
        let mut config = WriteAheadLogConfig::default();
        assert!(config.validate().is_ok());

        config.max_segment_size = 0;
        assert!(config.validate().is_err());

        config.max_segment_size = 1024;
        config.encryption = EncryptionAlgorithm::Aes256Gcm;
        assert!(config.validate().is_err()); // No key provided

        let key = EncryptionAlgorithm::Aes256Gcm.generate_key();
        config.encryption_key = Some(key);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_derives() {
        let config = WriteAheadLogConfig::default();
        let _ = config.clone();
        let _ = format!("{:?}", config);
    }

    #[test]
    fn test_durability_derives() {
        assert_eq!(Durability::Sync, Durability::Sync.clone());
        assert_eq!(Durability::Sync, Durability::Sync);
        assert_ne!(Durability::Sync, Durability::Memory);
    }
}
