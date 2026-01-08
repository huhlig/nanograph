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

use crate::{DynamicFileSystem, FileSystem, FileSystemError, FileSystemResult, Path};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Unified FileSystem Type
pub type BoxedFileSystem = Arc<dyn DynamicFileSystem>;

/// A [`FileSystem`] implementation that manages and routes to other filesystems based on path schemes.
///
/// The manager allows registering multiple filesystem implementations under different schemes
/// (e.g., "mem", "file", "s3"). When an operation is performed, it parses the path,
/// identifies the scheme, and routes the request to the corresponding filesystem.
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, FileSystemManager, File};
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let manager = FileSystemManager::new();
/// let mem_fs = Arc::new(MemoryFileSystem::new());
///
/// // We need to wrap it to have File = Box<dyn File>
/// // In a real scenario, you'd use a wrapper or the DynamicFileSystem trait properly.
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct FileSystemManager {
    filesystems: RwLock<HashMap<String, BoxedFileSystem>>,
}

impl FileSystemManager {
    /// Create a new FileSystemManager
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a filesystem for a given scheme.
    pub fn register(&self, scheme: impl Into<String>, fs: BoxedFileSystem) {
        let mut filesystems = self.filesystems.write().unwrap();
        filesystems.insert(scheme.into(), fs);
    }

    /// Deregister a filesystem for a given scheme.
    pub fn deregister(&self, scheme: &str) {
        let mut filesystems = self.filesystems.write().unwrap();
        filesystems.remove(scheme);
    }

    /// List registered filesystem schemes
    pub fn list_schemes(&self) -> Vec<String> {
        self.filesystems.read().unwrap().keys().cloned().collect()
    }

    /// Set Default Scheme used for relative addresses
    pub fn set_default_scheme(&self, _scheme: &str) {
        todo!("TODO: Implement Default Scheme")
    }

    /// Resolve a filesystem based on a path.
    ///
    /// If the path has a scheme, it looks up the registered filesystem for that scheme.
    /// If no scheme is present, or the scheme is not registered, it returns None.
    pub fn resolve(&self, path: &str) -> Option<BoxedFileSystem> {
        let path = Path::parse(path);
        let scheme = path.scheme.as_deref().unwrap_or("default");
        let filesystems = self.filesystems.read().unwrap();
        filesystems.get(scheme).cloned()
    }
}

impl FileSystem for FileSystemManager {
    type File = Box<dyn crate::File>;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .exists(path)
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .is_file(path)
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .is_directory(path)
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .filesize(path)
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .create_directory(path)
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .create_directory_all(path)
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .list_directory(path)
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .remove_directory(path)
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .remove_directory_all(path)
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .create_file(path)
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .open_file(path)
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        self.resolve(path)
            .ok_or_else(|| FileSystemError::PathMissing)?
            .remove_file(path)
    }
}

impl std::fmt::Debug for FileSystemManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSystemManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryFileSystem;

    #[test]
    fn test_manager_registration() {
        let manager = FileSystemManager::new();
        let memfs = Arc::new(crate::test_suite::BoxedFileSystem {
            inner: Arc::new(MemoryFileSystem::new()),
        });
        let fs: BoxedFileSystem = memfs;
        manager.register("mem", fs);

        assert!(manager.resolve("mem://test").is_some());
        assert!(manager.resolve("other://test").is_none());

        manager.deregister("mem");
        assert!(manager.resolve("mem://test").is_none());
    }

    #[test]
    fn test_manager_default_resolution() {
        let manager = FileSystemManager::new();
        let memfs = Arc::new(crate::test_suite::BoxedFileSystem {
            inner: Arc::new(MemoryFileSystem::new()),
        });
        manager.register("default", memfs);

        assert!(manager.resolve("test").is_some());
        assert!(manager.resolve("/test").is_some());
    }
}
