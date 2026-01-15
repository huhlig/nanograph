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

use crate::filesystem::{FileLockMode, FileSystem};
use crate::{File, FileSystemError, FileSystemResult, Path};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// A [`FileSystem`] implementation that uses the local OS filesystem.
///
/// All paths are resolved relative to a `root` directory provided during construction.
///
/// # Examples
/// ```no_run
/// use nanograph_vfs::{FileSystem, LocalFilesystem, File};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let fs = LocalFilesystem::new("/tmp/vfs_root");
/// fs.create_directory_all("/data")?;
/// let mut file = fs.create_file("/data/config.toml")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct LocalFilesystem {
    root: PathBuf,
}

impl LocalFilesystem {
    /// Creates a new `LocalFilesystem` rooted at the given path.
    ///
    /// # Arguments
    ///
    /// * `root` - The base directory for this filesystem.
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }

    /// Resolves a virtual path to a physical `PathBuf` on the local filesystem.
    fn resolve(&self, path: &str) -> PathBuf {
        let mut full_path = self.root.clone();
        let p = Path::parse(path);
        for segment in p.segments() {
            full_path.push(segment);
        }
        full_path
    }
}

impl FileSystem for LocalFilesystem {
    type File = LocalFile;

    #[tracing::instrument]
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        Ok(self.resolve(path).exists())
    }

    #[tracing::instrument]
    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        Ok(self.resolve(path).is_file())
    }

    #[tracing::instrument]
    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        Ok(self.resolve(path).is_dir())
    }

    #[tracing::instrument]
    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        let metadata =
            fs::metadata(self.resolve(path)).map_err(|_| FileSystemError::PathMissing)?;
        Ok(metadata.len())
    }

    #[tracing::instrument]
    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        fs::create_dir(self.resolve(path)).map_err(|_| FileSystemError::InvalidOperation)
    }

    #[tracing::instrument]
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        fs::create_dir_all(self.resolve(path)).map_err(|_| FileSystemError::InvalidOperation)
    }

    #[tracing::instrument]
    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        let entries = fs::read_dir(self.resolve(path)).map_err(|_| FileSystemError::PathMissing)?;
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if let Some(name) = file_name.to_str() {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    #[tracing::instrument]
    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        fs::remove_dir(self.resolve(path)).map_err(|_| FileSystemError::InvalidOperation)
    }

    #[tracing::instrument]
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        fs::remove_dir_all(self.resolve(path)).map_err(|_| FileSystemError::InvalidOperation)
    }

    #[tracing::instrument]
    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(self.resolve(path))
            .map_err(|_| FileSystemError::InvalidOperation)?;
        Ok(LocalFile {
            inner: file,
            path: path.to_string(),
        })
    }

    #[tracing::instrument]
    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.resolve(path))
            .map_err(|_| FileSystemError::PathMissing)?;
        Ok(LocalFile {
            inner: file,
            path: path.to_string(),
        })
    }

    #[tracing::instrument]
    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        fs::remove_file(self.resolve(path)).map_err(|_| FileSystemError::PathMissing)
    }
}

/// A handle to a file on the local filesystem.
#[derive(Debug)]
pub struct LocalFile {
    inner: fs::File,
    path: String,
}

impl Read for LocalFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for LocalFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Seek for LocalFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl File for LocalFile {
    fn path(&self) -> &str {
        &self.path
    }

    fn get_size(&self) -> FileSystemResult<u64> {
        let metadata = self
            .inner
            .metadata()
            .map_err(|_| FileSystemError::InvalidOperation)?;
        Ok(metadata.len())
    }

    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()> {
        self.inner
            .set_len(new_size)
            .map_err(|_| FileSystemError::InvalidOperation)
    }

    fn sync_all(&mut self) -> FileSystemResult<()> {
        self.inner
            .sync_all()
            .map_err(|_| FileSystemError::InvalidOperation)
    }

    fn sync_data(&mut self) -> FileSystemResult<()> {
        self.inner
            .sync_data()
            .map_err(|_| FileSystemError::InvalidOperation)
    }

    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        Ok(FileLockMode::Unlocked)
    }

    fn set_lock_status(&mut self, _mode: FileLockMode) -> FileSystemResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::LocalFilesystem;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!(
            "nanograph-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let fs = LocalFilesystem::new(temp_dir.clone());
        crate::test_suite::run_generic_test_suite(&fs);

        // Clean up: on Windows, we might need to make sure all handles are closed before removing the dir.
        // Although the generic test suite drops files, there might be a delay or some other issue.
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
