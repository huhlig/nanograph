//
// Copyright 2019-2024 Hans W. Uhlig. All Rights Reserved.
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

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, RwLock};

use crate::filesystem::FileLockMode;

use super::{File, FileSystem, FileSystemError, FileSystemResult, Path};

/// A [`FileSystem`] implementation that stores all files and directories in-memory.
///
/// This implementation uses a `BTreeMap` to store the filesystem structure.
/// It is volatile and will be lost when the object is dropped.
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, File};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let fs = MemoryFileSystem::new();
/// fs.create_directory("/data")?;
/// let mut file = fs.create_file("/data/test.txt")?;
/// file.write_to_offset(0, b"In-memory data")?;
///
/// assert!(fs.exists("/data/test.txt")?);
/// assert_eq!(fs.filesize("/data/test.txt")?, 14);
/// # Ok(())
/// # }
/// ```
#[derive(Default, Clone)]
pub struct MemoryFileSystem(Arc<RwLock<BTreeMap<String, MemoryEntry>>>);

impl MemoryFileSystem {
    /// Creates a new, empty `MemoryFileSystem`.
    #[must_use]
    pub fn new() -> MemoryFileSystem {
        let tree = Arc::new(RwLock::new(BTreeMap::new()));
        tree.write().expect("Poisoned Lock").insert(
            "/".to_string(),
            MemoryEntry::Directory(MemoryDirectoryEntry(Arc::new(RwLock::new(
                MemoryDirectoryData::default(),
            )))),
        );
        MemoryFileSystem(tree)
    }
}

impl std::fmt::Debug for MemoryFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryFileSystem {{ files: {:?} }}", self.0)
    }
}

impl FileSystem for MemoryFileSystem {
    type File = MemoryFile;

    #[tracing::instrument]
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        let tree = self.0.read().expect("Poisoned Lock");
        Ok(tree.contains_key(path))
    }

    #[tracing::instrument]
    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        let tree = self.0.read().expect("Poisoned Lock");
        if let Some(entry) = tree.get(path) {
            match entry {
                MemoryEntry::File(_) => Ok(true),
                MemoryEntry::Directory(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    #[tracing::instrument]
    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        let tree = self.0.read().expect("Poisoned Lock");
        if let Some(entry) = tree.get(path) {
            match entry {
                MemoryEntry::Directory(_) => Ok(true),
                MemoryEntry::File(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    #[tracing::instrument]
    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        let tree = self.0.read().expect("Poisoned Lock");
        if let Some(entry) = tree.get(path) {
            match entry {
                MemoryEntry::File(file) => {
                    let data = file.0.read().expect("Poisoned Lock");
                    Ok(data.buffer.len() as u64)
                }
                _ => Err(FileSystemError::InvalidOperation),
            }
        } else {
            Err(FileSystemError::PathMissing)
        }
    }

    #[tracing::instrument]
    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        let mut tree = self.0.write().expect("Poisoned Lock");
        if tree.contains_key(path) {
            return Err(FileSystemError::PathExists);
        }

        let p = Path::parse(path);
        let parent_str = p.parent().to_string();

        // Check if parent exists and is a directory
        if !p.segments().next().is_none() {
            if let Some(entry) = tree.get(&parent_str) {
                match entry {
                    MemoryEntry::Directory(dir_entry) => {
                        let mut dir_data = dir_entry.0.write().expect("Poisoned Lock");
                        dir_data.0.insert(p.segments().last().unwrap().to_string());
                    }
                    _ => return Err(FileSystemError::InvalidOperation),
                }
            } else {
                return Err(FileSystemError::PathMissing);
            }
        }

        tree.insert(
            path.to_string(),
            MemoryEntry::Directory(MemoryDirectoryEntry(Arc::new(RwLock::new(
                MemoryDirectoryData::default(),
            )))),
        );
        Ok(())
    }

    #[tracing::instrument]
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let mut tree = self.0.write().expect("Poisoned Lock");
        if tree.contains_key(path) {
            return Ok(());
        }

        let mut current_path = Path::parse(path);
        let mut to_create = Vec::new();

        // Collect all parent paths that don't exist
        while !current_path.segments().next().is_none() {
            let path_str = current_path.to_string();
            if tree.contains_key(&path_str) {
                break;
            }
            to_create.push((path_str, current_path.clone()));
            current_path = current_path.parent();
        }

        // Create them in order (from top to bottom)
        for (path_str, p) in to_create.into_iter().rev() {
            // Register in parent
            if !p.segments().next().is_none() {
                let parent_str = p.parent().to_string();
                if let Some(MemoryEntry::Directory(parent_dir)) = tree.get(&parent_str) {
                    let mut parent_data = parent_dir.0.write().expect("Poisoned Lock");
                    parent_data
                        .0
                        .insert(p.segments().last().unwrap().to_string());
                }
            }

            tree.insert(
                path_str,
                MemoryEntry::Directory(MemoryDirectoryEntry(Arc::new(RwLock::new(
                    MemoryDirectoryData::default(),
                )))),
            );
        }
        Ok(())
    }

    #[tracing::instrument]
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>> {
        let tree = self.0.read().expect("Poisoned Lock");
        if let Some(entry) = tree.get(path) {
            match entry {
                MemoryEntry::Directory(dir) => {
                    let dir = dir.0.read().expect("Poisoned Lock");
                    Ok(dir.0.iter().cloned().collect())
                }
                _ => Err(FileSystemError::InvalidOperation),
            }
        } else {
            Err(FileSystemError::PathMissing)
        }
    }

    #[tracing::instrument]
    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_directory_all(self, path)
    }

    #[tracing::instrument]
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        let mut tree = self.0.write().expect("Poisoned Lock");
        if !tree.contains_key(path) {
            return Err(FileSystemError::PathMissing);
        }

        let p = Path::parse(path);

        // Unregister from parent
        if !p.segments().next().is_none() {
            let parent_str = p.parent().to_string();
            if let Some(MemoryEntry::Directory(parent_dir)) = tree.get(&parent_str) {
                let mut parent_data = parent_dir.0.write().expect("Poisoned Lock");
                parent_data.0.remove(p.segments().last().unwrap());
            }
        }

        // Recursively remove children
        let mut to_remove = Vec::new();
        to_remove.push(path.to_string());

        let mut i = 0;
        while i < to_remove.len() {
            let current = to_remove[i].clone();
            if let Some(MemoryEntry::Directory(dir_entry)) = tree.get(&current) {
                let dir_data = dir_entry.0.read().expect("Poisoned Lock");
                let current_path = Path::parse(&current);
                for child in dir_data.0.iter() {
                    let mut child_path = current_path.clone();
                    child_path.push(child);
                    to_remove.push(child_path.to_string());
                }
            }
            i += 1;
        }

        for p_str in to_remove {
            tree.remove(&p_str);
        }

        Ok(())
    }

    #[tracing::instrument]
    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let mut tree = self.0.write().expect("Poisoned Lock");
        if tree.contains_key(path) {
            return Err(FileSystemError::PathExists);
        }

        let p = Path::parse(path);
        let parent_str = p.parent().to_string();

        // Check if parent exists and is a directory
        if !p.segments().next().is_none() {
            if let Some(entry) = tree.get(&parent_str) {
                match entry {
                    MemoryEntry::Directory(dir_entry) => {
                        let mut dir_data = dir_entry.0.write().expect("Poisoned Lock");
                        dir_data.0.insert(p.segments().last().unwrap().to_string());
                    }
                    MemoryEntry::File(_) => return Err(FileSystemError::InvalidOperation),
                }
            } else {
                return Err(FileSystemError::PathMissing);
            }
        }

        let inner = Arc::new(RwLock::new(MemoryFileData {
            buffer: Vec::default(),
            lock: FileLockMode::Unlocked,
        }));
        tree.insert(
            path.to_string(),
            MemoryEntry::File(MemoryFileEntry(inner.clone())),
        );
        Ok(MemoryFile {
            cursor: 0,
            name: path.to_string(),
            data: inner,
        })
    }

    #[tracing::instrument]
    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        if let Some(entry) = self.0.read().expect("Poisoned Lock").get(path) {
            match entry {
                MemoryEntry::File(file) => Ok(MemoryFile {
                    cursor: 0,
                    name: path.to_string(),
                    data: file.0.clone(),
                }),
                MemoryEntry::Directory(_) => Err(FileSystemError::InvalidOperation),
            }
        } else {
            Err(FileSystemError::PathMissing)
        }
    }

    #[tracing::instrument]
    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        let mut tree = self.0.write().expect("Poisoned Lock");
        if !tree.contains_key(path) {
            return Err(FileSystemError::PathMissing);
        }

        let p = Path::parse(path);

        // Unregister from parent
        if !p.segments().next().is_none() {
            let parent_str = p.parent().to_string();
            if let Some(MemoryEntry::Directory(parent_dir)) = tree.get(&parent_str) {
                let mut parent_data = parent_dir.0.write().expect("Poisoned Lock");
                parent_data.0.remove(p.segments().last().unwrap());
            }
        }

        tree.remove(path);
        Ok(())
    }
}

/// An entry in the memory filesystem (either a file or a directory).
#[derive(Clone, Debug)]
enum MemoryEntry {
    Directory(MemoryDirectoryEntry),
    File(MemoryFileEntry),
}

/// A directory entry in the memory filesystem.
#[derive(Clone, Debug)]
struct MemoryDirectoryEntry(Arc<RwLock<MemoryDirectoryData>>);

/// The data stored for a directory in the memory filesystem.
#[derive(Default, Clone, Debug)]
struct MemoryDirectoryData(BTreeSet<String>);

/// A file entry in the memory filesystem.
#[derive(Clone, Debug)]
pub struct MemoryFileEntry(Arc<RwLock<MemoryFileData>>);

/// The data stored for a file in the memory filesystem.
#[derive(Clone)]
struct MemoryFileData {
    buffer: Vec<u8>,
    lock: FileLockMode,
}

impl std::fmt::Debug for MemoryFileData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryFileData {{ data: {} bytes }}", self.buffer.len())
    }
}

/// A handle to an in-memory file.
#[derive(Clone)]
pub struct MemoryFile {
    cursor: usize,
    name: String,
    data: Arc<RwLock<MemoryFileData>>,
}

impl std::fmt::Debug for MemoryFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MemoryFileHandle {{ cursor: {}, data: {:?} }}",
            self.cursor, self.data
        )
    }
}

impl Read for MemoryFile {
    #[tracing::instrument]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let data = self.data.read().unwrap();
        let len = std::cmp::min(buf.len(), data.buffer.len() - self.cursor);
        buf[..len].copy_from_slice(&data.buffer[self.cursor..self.cursor + len]);
        self.cursor += len;
        Ok(len)
    }
}

impl Write for MemoryFile {
    #[tracing::instrument]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut data = self.data.write().unwrap();
        if self.cursor + buf.len() > data.buffer.len() {
            data.buffer.resize(self.cursor + buf.len(), 0);
        }
        data.buffer[self.cursor..self.cursor + buf.len()].copy_from_slice(buf);
        self.cursor += buf.len();
        Ok(buf.len())
    }

    #[tracing::instrument]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for MemoryFile {
    #[tracing::instrument]
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let data = self.data.read().expect("Poisoned Lock");
        match pos {
            SeekFrom::Start(offset) => {
                self.cursor = offset as usize;
            }
            SeekFrom::End(offset) => {
                self.cursor = (data.buffer.len() as i64 + offset) as usize;
            }
            SeekFrom::Current(offset) => {
                self.cursor = (self.cursor as i64 + offset) as usize;
            }
        }
        Ok(self.cursor as u64)
    }
}

impl File for MemoryFile {
    #[tracing::instrument]
    fn path(&self) -> &str {
        &self.name.as_str()
    }

    #[tracing::instrument]
    fn get_size(&self) -> FileSystemResult<u64> {
        let file = self.data.read().expect("Poisoned Lock");
        Ok(file.buffer.len() as u64)
    }

    #[tracing::instrument]
    fn set_size(&mut self, new_length: u64) -> FileSystemResult<()> {
        let mut file = self.data.write().expect("Poisoned Lock");
        file.buffer.resize(new_length as usize, 0);
        Ok(())
    }

    #[tracing::instrument]
    fn sync_all(&mut self) -> FileSystemResult<()> {
        Ok(())
    }

    #[tracing::instrument]
    fn sync_data(&mut self) -> FileSystemResult<()> {
        Ok(())
    }

    #[tracing::instrument]
    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        let file = self.data.read().expect("Poisoned Lock");
        Ok(file.lock)
    }

    #[tracing::instrument]
    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()> {
        let mut file = self.data.write().expect("Poisoned Lock");
        file.lock = mode;
        Ok(())
    }

    #[tracing::instrument]
    fn read_at_offset(&mut self, pos: u64, buf: &mut [u8]) -> FileSystemResult<usize> {
        let data = self.data.read().expect("Poisoned Lock");

        // Calculate Slice Bounds
        let off = pos as usize; // Lower Slice Bound
        let end = std::cmp::min(off + buf.len(), data.buffer.len()); // Upper Slice Bound
        let len = end - off;

        // Read
        buf[..len].copy_from_slice(&data.buffer[off..end]);

        Ok(len)
    }
    #[tracing::instrument]
    fn write_to_offset(&mut self, pos: u64, buf: &[u8]) -> FileSystemResult<usize> {
        let mut data = self.data.write().unwrap();

        // Calculate Slice Bounds
        let off = usize::try_from(pos).expect("Position Too Large"); // Lower Slice Bound
        let end = off + buf.len(); // Upper Slice Bound

        // Resize if array capacity too small
        if end > data.buffer.len() {
            data.buffer.resize(end, 0);
        }

        // Write data to buffer
        data.buffer[off..end].copy_from_slice(buf);

        Ok(buf.len())
    }
}

#[cfg(test)]
mod test {
    use crate::File;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        use super::MemoryFileSystem;
        crate::test_suite::run_generic_test_suite(&MemoryFileSystem::new());
    }

    #[test]
    #[traced_test]
    fn test_memory_filesystem() {
        use super::MemoryFileSystem;
        use crate::FileSystem;
        use std::io::{Read, Seek, SeekFrom, Write};

        let fs = MemoryFileSystem::new();
        let filename = "/test.tst";
        {
            // Create new File
            let mut file = fs.create_file(filename).expect("Error Creating File");
            assert_eq!(file.get_size().unwrap(), 0, "File size wasn't zero");

            // Write to File
            file.write_all(b"Hello, World!").unwrap();
            assert_eq!(file.get_size().unwrap(), 13, "File size wasn't 13");

            // Read full File Contents and compare
            let mut buf = Vec::new();
            file.seek(SeekFrom::Start(0))
                .expect("Error Seeking to beginning of file");
            file.read_to_end(&mut buf).expect("Error Reading File");
            assert_eq!(buf, b"Hello, World!");

            // Shrink file to size 5 and test
            file.set_size(5).expect("Error Setting File Size");
            assert_eq!(file.get_size().unwrap(), 5);

            // Seek to start and read full file
            let mut buf = Vec::new();
            file.seek(SeekFrom::Start(0)).expect("Error Seeking File");
            file.read_to_end(&mut buf).expect("Error Reading File");
            assert_eq!(buf, b"Hello");

            // Set file size to zero and test
            file.set_size(0).unwrap();
            assert_eq!(file.get_size().expect("Unable to get file size"), 0);

            // Write new data to file and test
            file.seek(SeekFrom::Start(0))
                .expect("Error Seeking to beginning of file");
            file.write_all(b"Goodbye!").expect("Error Writing File");
            assert_eq!(file.get_size().expect("Unable to get file size"), 8);

            // Seek to start and read full file
            let mut buf = Vec::new();
            file.seek(SeekFrom::Start(0)).expect("Error Seeking File");
            file.read_to_end(&mut buf).expect("Error Reading File");
            assert_eq!(buf, b"Goodbye!");
        }
        {
            // Open existing file and test
            let mut file = fs.open_file(filename).unwrap();
            assert_eq!(file.get_size().unwrap(), 8);

            // Seek to start and read full file
            let mut buf = Vec::new();
            file.seek(SeekFrom::Start(0)).expect("Error Seeking File");
            file.read_to_end(&mut buf).expect("Error Reading File");
            assert_eq!(buf, b"Goodbye!");
        }

        // Remove file and test
        fs.remove_file(filename).expect("Error Removing File");
        assert!(!fs.exists(filename).expect("Error Checking File Existence"));
    }

    #[test]
    #[traced_test]
    fn test_memory_directory_operations() {
        use super::MemoryFileSystem;
        use crate::FileSystem;

        let fs = MemoryFileSystem::new();

        // Create directory hierarchy
        fs.create_directory_all("/a/b/c")
            .expect("Failed to create directories");
        assert!(fs.is_directory("/a").unwrap());
        assert!(fs.is_directory("/a/b").unwrap());
        assert!(fs.is_directory("/a/b/c").unwrap());

        // List directory
        let entries = fs.list_directory("/a/b").expect("Failed to list directory");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "c");

        // Create file in directory
        fs.create_file("/a/b/file.txt")
            .expect("Failed to create file");
        let entries = fs.list_directory("/a/b").expect("Failed to list directory");
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&"c".to_string()));
        assert!(entries.contains(&"file.txt".to_string()));

        // Remove directory all
        fs.remove_directory_all("/a/b")
            .expect("Failed to remove directory all");
        assert!(fs.exists("/a").unwrap());
        assert!(!fs.exists("/a/b").unwrap());
        assert!(!fs.exists("/a/b/c").unwrap());
        assert!(!fs.exists("/a/b/file.txt").unwrap());

        let entries = fs.list_directory("/a").unwrap();
        assert_eq!(entries.len(), 0);
    }
}
