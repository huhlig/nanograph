//! Configuration for ART storage engine with tablespace support

use std::path::PathBuf;

/// Configuration for ART storage with tablespace-resolved paths
#[derive(Debug, Clone)]
pub struct ARTStorageConfig {
    /// Directory for ART data files (tablespace-resolved)
    pub data_dir: PathBuf,

    /// Directory for WAL files (tablespace-resolved)
    pub wal_dir: PathBuf,

    /// Maximum number of entries before triggering compaction
    pub max_entries: usize,

    /// Cache size in megabytes (optional)
    pub cache_size_mb: Option<usize>,
}

impl ARTStorageConfig {
    /// Create a new ART storage configuration
    pub fn new(data_dir: PathBuf, wal_dir: PathBuf) -> Self {
        Self {
            data_dir,
            wal_dir,
            max_entries: 1_000_000,
            cache_size_mb: None,
        }
    }

    /// Set the maximum number of entries
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Set the cache size in megabytes
    pub fn with_cache_size(mut self, cache_size_mb: usize) -> Self {
        self.cache_size_mb = Some(cache_size_mb);
        self
    }
}

impl Default for ARTStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/art"),
            wal_dir: PathBuf::from("data/wal"),
            max_entries: 1_000_000,
            cache_size_mb: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ARTStorageConfig::default();
        assert_eq!(config.data_dir, PathBuf::from("data/art"));
        assert_eq!(config.wal_dir, PathBuf::from("data/wal"));
        assert_eq!(config.max_entries, 1_000_000);
        assert_eq!(config.cache_size_mb, None);
    }

    #[test]
    fn test_custom_config() {
        let config = ARTStorageConfig::new(
            PathBuf::from("/mnt/ssd/art"),
            PathBuf::from("/mnt/nvme/wal"),
        )
        .with_max_entries(500_000)
        .with_cache_size(256);

        assert_eq!(config.data_dir, PathBuf::from("/mnt/ssd/art"));
        assert_eq!(config.wal_dir, PathBuf::from("/mnt/nvme/wal"));
        assert_eq!(config.max_entries, 500_000);
        assert_eq!(config.cache_size_mb, Some(256));
    }
}
