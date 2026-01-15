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
use crate::{File, FileLockMode, FileSystemResult, Path};
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing_timing::HashMap;

/// A [`FileSystem`] wrapper that monitors and records metrics for all operations.
///
/// This filesystem acts as a wrapper around another filesystem,
/// logging all operations using the `tracing` crate and recording
/// metrics using the `metrics` crate. It tracks:
/// - Operation counts (success/failure)
/// - Bytes read/written
/// - Operation durations
/// - Currently open files
/// - File sizes
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, MonitoredFilesystem, File};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mem_fs = MemoryFileSystem::new();
/// let fs = MonitoredFilesystem::new(mem_fs, "memory");
///
/// let mut file = fs.create_file("/test.bin")?;
/// file.write_to_offset(0, &[0u8; 100])?;
///
/// assert_eq!(fs.bytes_written(), 100);
/// # Ok(())
/// # }
/// ```
/// TODO: Implement Operation Counting
#[derive(Clone)]
pub struct MonitoredFilesystem {
    inner: Arc<dyn DynamicFileSystem>,
    filesystem_type: &'static str,
    stats: Arc<RwLock<FileSystemStats>>,
    files: Arc<RwLock<HashMap<Path, Arc<RwLock<FileStats>>>>>,
}

impl MonitoredFilesystem {
    /// Creates a new `MonitoredFilesystem` wrapping the provided `inner` filesystem.
    ///
    /// # Arguments
    /// * `inner` - The filesystem to wrap
    /// * `filesystem_type` - A static string identifying the filesystem type (e.g., "memory", "local")
    pub fn new<DFS: DynamicFileSystem + 'static>(
        inner: DFS,
        filesystem_type: &'static str,
    ) -> Self {
        Self {
            inner: Arc::new(inner),
            filesystem_type,
            stats: Arc::new(RwLock::new(FileSystemStats::default())),
            files: Arc::new(RwLock::default()),
        }
    }

    /// Returns the total number of bytes written to this filesystem.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.stats.read().unwrap().bytes_written
    }

    /// Returns the total number of bytes read from this filesystem.
    #[must_use]
    pub fn bytes_read(&self) -> u64 {
        self.stats.read().unwrap().bytes_read
    }

    /// Count of Failed Operations
    #[must_use]
    pub fn failed_operations(&self) -> usize {
        self.stats.read().unwrap().failed_operations
    }

    /// Count of Successful Operations
    #[must_use]
    pub fn successful_operations(&self) -> usize {
        self.stats.read().unwrap().success_operations
    }

    /// Count of Total Operations
    #[must_use]
    pub fn total_operations(&self) -> usize {
        self.stats.read().unwrap().failed_operations + self.stats.read().unwrap().success_operations
    }

    /// Get Count of Specific Operations
    #[must_use]
    pub fn operation_count(&self, operation: &str) -> usize {
        match operation {
            "create_directory" => self.stats.read().unwrap().create_directory_calls,
            "list_directory" => self.stats.read().unwrap().list_directory_calls,
            "remove_directory" => self.stats.read().unwrap().remove_directory_calls,
            "create_file" => self.stats.read().unwrap().create_file_calls,
            "open_file" => self.stats.read().unwrap().open_file_calls,
            "remove_file" => self.stats.read().unwrap().remove_file_calls,
            _ => 0,
        }
    }

    /// Reset metrics to zero
    pub fn reset_metrics(&self) {
        {
            let mut lock = self.stats.write().unwrap();
            lock.open_files.clear();
            lock.bytes_written = 0;
            lock.bytes_read = 0;
            lock.create_directory_calls = 0;
            lock.list_directory_calls = 0;
            lock.remove_directory_calls = 0;
            lock.create_file_calls = 0;
            lock.open_file_calls = 0;
            lock.remove_file_calls = 0;
            lock.failed_operations = 0;
            lock.success_operations = 0;
        }
        {
            let mut outer_lock = self.files.write().unwrap();
            for (_, stats) in outer_lock.iter_mut() {
                let mut inner_lock = stats.write().unwrap();
                inner_lock.bytes_read = 0;
                inner_lock.bytes_written = 0;
                inner_lock.write_calls = 0;
                inner_lock.read_calls = 0;
            }
        }
    }

    /// Returns a list of currently open files.
    #[must_use]
    pub fn open_files(&self) -> Vec<Path> {
        self.stats
            .read()
            .unwrap()
            .open_files
            .iter()
            .cloned()
            .collect()
    }

    /// Returns the filesystem type identifier.
    #[must_use]
    pub fn filesystem_type(&self) -> &'static str {
        self.filesystem_type
    }
}

impl std::fmt::Debug for MonitoredFilesystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorFilesystem").finish()
    }
}

impl FileSystem for MonitoredFilesystem {
    type File = MonitoredFile;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        let start = Instant::now();
        tracing::debug!(path, "Checking existence");
        let result = self.inner.exists(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("exists", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("exists", self.filesystem_type, duration);
        result
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        let start = Instant::now();
        tracing::debug!(path, "Checking if file");
        let result = self.inner.is_file(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("is_file", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("is_file", self.filesystem_type, duration);
        result
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        let start = Instant::now();
        tracing::debug!(path, "Checking if directory");
        let result = self.inner.is_directory(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("is_directory", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("is_directory", self.filesystem_type, duration);
        result
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        let start = Instant::now();
        tracing::debug!(path, "Getting filesize");
        let result = self.inner.filesize(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("filesize", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("filesize", self.filesystem_type, duration);
        if let Ok(size) = result {
            crate::metrics::record_file_size(self.filesystem_type, size);
        }
        result
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        let start = Instant::now();
        tracing::info!(path, "Creating directory");
        let result = self.inner.create_directory(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("create_directory", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration(
            "create_directory",
            self.filesystem_type,
            duration,
        );
        result
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let start = Instant::now();
        tracing::info!(path, "Creating all directories");
        let result = self.inner.create_directory_all(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation(
            "create_directory_all",
            self.filesystem_type,
            result.is_ok(),
        );
        crate::metrics::record_operation_duration(
            "create_directory_all",
            self.filesystem_type,
            duration,
        );
        result
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        let start = Instant::now();
        tracing::debug!(path, "Listing directory");
        let result = self.inner.list_directory(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("list_directory", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("list_directory", self.filesystem_type, duration);
        result
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        let start = Instant::now();
        tracing::warn!(path, "Removing directory");
        let result = self.inner.remove_directory(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("remove_directory", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration(
            "remove_directory",
            self.filesystem_type,
            duration,
        );
        result
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let start = Instant::now();
        tracing::warn!(path, "Removing all directories");
        let result = self.inner.remove_directory_all(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation(
            "remove_directory_all",
            self.filesystem_type,
            result.is_ok(),
        );
        crate::metrics::record_operation_duration(
            "remove_directory_all",
            self.filesystem_type,
            duration,
        );
        result
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let start = Instant::now();
        tracing::info!(path, "Creating file");
        let inner_file = self.inner.create_file(path)?;
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("create_file", self.filesystem_type, true);
        crate::metrics::record_operation_duration("create_file", self.filesystem_type, duration);

        let path_obj = Path::parse(path);
        let file_stats = {
            let mut files = self.files.write().unwrap();
            files
                .entry(path_obj.clone())
                .or_insert_with(|| Arc::new(RwLock::new(FileStats::default())))
                .clone()
        };
        {
            let mut stats = self.stats.write().unwrap();
            stats.open_files.insert(path_obj.clone());
            crate::metrics::record_open_files(self.filesystem_type, stats.open_files.len() as i64);
        }
        Ok(MonitoredFile {
            filesystem_type: self.filesystem_type,
            filesystem_stats: self.stats.clone(),
            file_stats,
            inner: Arc::from(inner_file),
            path: path_obj,
        })
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let start = Instant::now();
        tracing::debug!(path, "Opening file");
        let inner_file = self.inner.open_file(path)?;
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("open_file", self.filesystem_type, true);
        crate::metrics::record_operation_duration("open_file", self.filesystem_type, duration);

        let path_obj = Path::parse(path);
        let file_stats = {
            let mut files = self.files.write().unwrap();
            files
                .entry(path_obj.clone())
                .or_insert_with(|| Arc::new(RwLock::new(FileStats::default())))
                .clone()
        };
        {
            let mut stats = self.stats.write().unwrap();
            stats.open_files.insert(path_obj.clone());
            crate::metrics::record_open_files(self.filesystem_type, stats.open_files.len() as i64);
        }
        Ok(MonitoredFile {
            filesystem_type: self.filesystem_type,
            filesystem_stats: self.stats.clone(),
            file_stats,
            inner: Arc::from(inner_file),
            path: path_obj,
        })
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        let start = Instant::now();
        tracing::warn!(path, "Removing file");
        let path_obj = Path::parse(path);
        {
            let mut stats = self.stats.write().unwrap();
            stats.open_files.remove(&path_obj);
            crate::metrics::record_open_files(self.filesystem_type, stats.open_files.len() as i64);
        }
        let result = self.inner.remove_file(path);
        let duration = start.elapsed().as_micros() as u64;
        crate::metrics::record_operation("remove_file", self.filesystem_type, result.is_ok());
        crate::metrics::record_operation_duration("remove_file", self.filesystem_type, duration);
        result
    }
}

/// A [`File`] handle that monitors and records metrics for all operations.
pub struct MonitoredFile {
    filesystem_type: &'static str,
    filesystem_stats: Arc<RwLock<FileSystemStats>>,
    file_stats: Arc<RwLock<FileStats>>,
    inner: Arc<dyn File>,
    path: Path,
}

impl MonitoredFile {
    /// Number of bytes written to the file
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.file_stats.read().unwrap().bytes_written
    }
    /// Number of bytes read from the file
    #[must_use]
    pub fn bytes_read(&self) -> u64 {
        self.file_stats.read().unwrap().bytes_read
    }
    /// Number of read calls made to the file
    #[must_use]
    pub fn read_calls(&self) -> usize {
        self.file_stats.read().unwrap().read_calls
    }
    /// Number of write calls made to the file
    #[must_use]
    pub fn write_calls(&self) -> usize {
        self.file_stats.write().unwrap().write_calls
    }
}

impl Drop for MonitoredFile {
    fn drop(&mut self) {
        let mut stats = self.filesystem_stats.write().unwrap();
        stats.open_files.remove(&self.path);
        crate::metrics::record_open_files(self.filesystem_type, stats.open_files.len() as i64);
    }
}

impl Read for MonitoredFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = Arc::get_mut(&mut self.inner).unwrap().read(buf)?;
        {
            let mut fs_stats = self.filesystem_stats.write().unwrap();
            fs_stats.bytes_read += n as u64;
        }
        {
            let mut f_stats = self.file_stats.write().unwrap();
            f_stats.bytes_read += n as u64;
        }
        crate::metrics::record_bytes_read(self.filesystem_type, n as u64);
        Ok(n)
    }
}

impl Write for MonitoredFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = Arc::get_mut(&mut self.inner).unwrap().write(buf)?;
        {
            let mut fs_stats = self.filesystem_stats.write().unwrap();
            fs_stats.bytes_written += n as u64;
        }
        {
            let mut f_stats = self.file_stats.write().unwrap();
            f_stats.bytes_written += n as u64;
        }
        crate::metrics::record_bytes_written(self.filesystem_type, n as u64);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Arc::get_mut(&mut self.inner).unwrap().flush()
    }
}

impl Seek for MonitoredFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        Arc::get_mut(&mut self.inner).unwrap().seek(pos)
    }
}

impl File for MonitoredFile {
    fn path(&self) -> &str {
        self.inner.path()
    }

    fn get_size(&self) -> FileSystemResult<u64> {
        self.inner.get_size()
    }

    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.inner).unwrap().set_size(new_size)
    }

    fn sync_all(&mut self) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.inner).unwrap().sync_all()
    }

    fn sync_data(&mut self) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.inner).unwrap().sync_data()
    }

    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        self.inner.get_lock_status()
    }

    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.inner).unwrap().set_lock_status(mode)
    }
}

impl std::fmt::Debug for MonitoredFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitoredFile").finish()
    }
}

/// `FileSystem` Statistics
#[derive(Default)]
struct FileSystemStats {
    open_files: HashSet<Path>,
    bytes_written: u64,
    bytes_read: u64,
    create_directory_calls: usize,
    list_directory_calls: usize,
    remove_directory_calls: usize,
    create_file_calls: usize,
    open_file_calls: usize,
    remove_file_calls: usize,
    failed_operations: usize,
    success_operations: usize,
}

#[derive(Default)]
struct FileStats {
    bytes_written: u64,
    bytes_read: u64,
    read_calls: usize,
    write_calls: usize,
}

#[cfg(test)]
mod test {
    use super::MonitoredFilesystem;
    use crate::filesystem::FileSystem;
    use crate::memoryfs::MemoryFileSystem;
    use crate::test_suite::run_generic_test_suite;
    use crate::{File, Path};
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        let fs = MonitoredFilesystem::new(MemoryFileSystem::new(), "memory");
        run_generic_test_suite(&fs);
    }

    #[test]
    fn test_stats() {
        use std::io::{Read, Write};
        let fs = MonitoredFilesystem::new(MemoryFileSystem::new(), "memory");

        let mut file = fs.create_file("/test.txt").unwrap();
        file.write_all(b"hello").unwrap();
        file.sync_all().unwrap();
        drop(file);

        assert_eq!(fs.bytes_written(), 5);
        assert_eq!(fs.bytes_read(), 0);

        let mut file = fs.open_file("/test.txt").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"hello");
        drop(file);

        assert_eq!(fs.bytes_written(), 5);
        assert_eq!(fs.bytes_read(), 5);
    }

    #[test]
    fn test_file_stats_persistence() {
        use std::io::Write;
        let fs = MonitoredFilesystem::new(MemoryFileSystem::new(), "memory");
        let path = "/test.txt";

        {
            let mut file = fs.create_file(path).unwrap();
            file.write_all(b"hello").unwrap();
        }

        {
            let stats = fs.files.read().unwrap();
            let file_stats = stats.get(&Path::parse(path)).expect("Stats should exist");
            assert_eq!(file_stats.read().unwrap().bytes_written, 5);
        }

        {
            let mut file = fs.open_file(path).unwrap();
            file.write_all(b" world").unwrap();
        }

        {
            let stats = fs.files.read().unwrap();
            let file_stats = stats.get(&Path::parse(path)).expect("Stats should exist");
            assert_eq!(file_stats.read().unwrap().bytes_written, 11);
        }
    }
}
