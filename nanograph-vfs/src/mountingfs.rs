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

use crate::filesystem::{DynamicFileSystem, FileSystem};
use crate::{File, FileLockMode, FileSystemError, FileSystemResult};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

/// A [`FileSystem`] implementation that allows mounting other filesystems at specific paths.
///
/// This filesystem allows other filesystems to be "mounted" at specific paths.
/// When an operation is performed, it finds the longest matching mount point
/// and delegates the operation to the corresponding filesystem with a relative path.
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, MountableFilesystem, File};
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut fs = MountableFilesystem::new();
/// let mem_fs = Arc::new(MemoryFileSystem::new());
///
/// fs.mount("/mnt/mem", mem_fs);
///
/// fs.create_directory_all("/mnt/mem/data")?;
/// assert!(fs.is_directory("/mnt/mem/data")?);
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct MountableFilesystem {
    mounts: BTreeMap<String, Arc<dyn DynamicFileSystem>>,
}

impl MountableFilesystem {
    /// Creates a new, empty `MountingFilesystem`.
    pub fn new() -> Self {
        Self {
            mounts: BTreeMap::new(),
        }
    }

    /// Mounts a filesystem at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path where the filesystem should be mounted.
    /// * `fs` - The filesystem to mount.
    pub fn mount(&mut self, path: &str, fs: Arc<dyn DynamicFileSystem>) {
        self.mounts.insert(path.to_string(), fs);
    }

    /// Unmounts a filesystem from the specified path
    ///
    /// # Arguments
    ///
    /// * `path` - The path where the filesystem should be unmounted.
    pub fn unmount(&mut self, path: &str) {
        self.mounts.remove(path);
    }

    /// Lists all mounted filesystems with their mount points
    pub fn list_mounts(&self) -> Vec<String> {
        self.mounts
            .iter()
            .map(|(k, v)| format!("{} - {:?}", k, v))
            .collect()
    }

    /// Finds the appropriate mount point for a given path.
    /// Returns the filesystem and the path relative to its mount point.
    fn find_mount(&self, path: &str) -> FileSystemResult<(&Arc<dyn DynamicFileSystem>, String)> {
        for (mount_path, fs) in self.mounts.iter().rev() {
            if path == mount_path {
                return Ok((fs, "/".to_string()));
            }
            if let Some(rel) = path.strip_prefix(mount_path) {
                if mount_path.is_empty()
                    || mount_path == "/"
                    || mount_path.ends_with('/')
                    || rel.starts_with('/')
                {
                    let rel = if rel.is_empty() {
                        "/".to_string()
                    } else if !rel.starts_with('/') {
                        format!("/{}", rel)
                    } else {
                        rel.to_string()
                    };
                    return Ok((fs, rel));
                }
            }
        }
        Err(FileSystemError::PathMissing)
    }
}

impl std::fmt::Debug for MountableFilesystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountingFilesystem")
            .field("mounts", &self.mounts.keys())
            .finish()
    }
}

impl FileSystem for MountableFilesystem {
    type File = MountedFile;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        let (fs, rel) = self.find_mount(path)?;
        fs.exists(&rel)
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        let (fs, rel) = self.find_mount(path)?;
        fs.is_file(&rel)
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        let (fs, rel) = self.find_mount(path)?;
        fs.is_directory(&rel)
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        let (fs, rel) = self.find_mount(path)?;
        fs.filesize(&rel)
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        let (fs, rel) = self.find_mount(path)?;
        fs.create_directory(&rel)
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let (fs, rel) = self.find_mount(path)?;
        fs.create_directory_all(&rel)
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        let (fs, rel) = self.find_mount(path)?;
        fs.list_directory(&rel)
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        let (fs, rel) = self.find_mount(path)?;
        fs.remove_directory(&rel)
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let (fs, rel) = self.find_mount(path)?;
        fs.remove_directory_all(&rel)
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let (fs, rel) = self.find_mount(path)?;
        Ok(MountedFile {
            path: path.to_string(),
            inner: fs.create_file(&rel)?,
        })
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let (fs, rel) = self.find_mount(path)?;
        Ok(MountedFile {
            path: path.to_string(),
            inner: fs.open_file(&rel)?,
        })
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        let (fs, rel) = self.find_mount(path)?;
        fs.remove_file(&rel)
    }
}

/// A [`File`] handle for a file on a mounted filesystem.
pub struct MountedFile {
    inner: Box<dyn File>,
    path: String,
}

impl File for MountedFile {
    fn path(&self) -> &str {
        &self.path
    }

    fn get_size(&self) -> FileSystemResult<u64> {
        self.inner.get_size()
    }

    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()> {
        self.inner.set_size(new_size)
    }

    fn sync_all(&mut self) -> FileSystemResult<()> {
        self.inner.sync_all()
    }

    fn sync_data(&mut self) -> FileSystemResult<()> {
        self.inner.sync_data()
    }

    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        self.inner.get_lock_status()
    }

    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()> {
        self.inner.set_lock_status(mode)
    }
}

impl std::fmt::Debug for MountedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountedFile")
            .field("path", &self.path)
            .finish()
    }
}

impl Write for MountedFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Read for MountedFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for MountedFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[cfg(test)]
mod test {
    use super::MountableFilesystem;
    use crate::FileSystem;
    use crate::memoryfs::MemoryFileSystem;
    use crate::test_suite::{BoxedFileSystem, run_generic_test_suite};
    use std::sync::Arc;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        let mut fs = MountableFilesystem::new();
        fs.mount(
            "/",
            Arc::new(BoxedFileSystem {
                inner: Arc::new(MemoryFileSystem::new()),
            }),
        );
        run_generic_test_suite(fs);
    }

    #[test]
    #[traced_test]
    fn test_mounting_logic() {
        let mut fs = MountableFilesystem::new();
        let root_inner = Arc::new(MemoryFileSystem::new());
        let data_inner = Arc::new(MemoryFileSystem::new());

        fs.mount(
            "/",
            Arc::new(BoxedFileSystem {
                inner: root_inner.clone(),
            }),
        );
        fs.mount(
            "/data",
            Arc::new(BoxedFileSystem {
                inner: data_inner.clone(),
            }),
        );

        // Create file in root
        fs.create_file("/root.txt").unwrap();
        assert!(root_inner.exists("/root.txt").unwrap());
        assert!(!data_inner.exists("/root.txt").unwrap());

        // Create file in data
        fs.create_file("/data/test.bin").unwrap();
        assert!(data_inner.exists("/test.bin").unwrap());
        assert!(!root_inner.exists("/data/test.bin").unwrap());
    }
}
