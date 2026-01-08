//
// Copyright 2019-2026 Hans W. Uhlig. All Rights Reserved.
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

use super::{FileSystemError, FileSystemResult};
use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::sync::Arc;

/// API definition all KBase [`FileSystem`] implementations must adhere to.
///
/// This trait defines the core operations for a virtual filesystem, including
/// file and directory manipulation, metadata queries, and entry management.
pub trait FileSystem: Debug + Sync + Send + 'static {
    /// Type of File Returned by this Virtual File System
    type File: File;

    /// Check if an entry exists at the provided path.
    ///
    /// Returns `true` if an entry (file or directory) exists at the given path.
    fn exists(&self, path: &str) -> FileSystemResult<bool>;

    /// See if an entry at the path is a file.
    ///
    /// Returns `true` if the entry exists and is a file.
    fn is_file(&self, path: &str) -> FileSystemResult<bool>;

    /// See if an entry at the path is a folder.
    ///
    /// Returns `true` if the entry exists and is a directory.
    fn is_directory(&self, path: &str) -> FileSystemResult<bool>;

    /// Get file or directory size.
    ///
    /// For files, returns the size in bytes. For directories, behavior is implementation-dependent.
    fn filesize(&self, path: &str) -> FileSystemResult<u64>;

    /// Creates a new, empty folder entry at the provided path.
    ///
    /// Fails if the parent directory does not exist or if an entry already exists at the path.
    fn create_directory(&self, path: &str) -> FileSystemResult<()>;

    /// Creates a new, empty folder entry at the provided path, creating all parents as needed.
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()>;

    /// Returns an iterator over the names of entries within a Folder.
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>>;

    /// Removes the folder at this path.
    ///
    /// Typically fails if the directory is not empty.
    fn remove_directory(&self, path: &str) -> FileSystemResult<()>;

    /// Removes the folder at this path and all children.
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()>;

    /// Create or Open a new file for writing.
    ///
    /// If the file already exists, it is opened. If it doesn't exist, it is created.
    fn create_file(&self, path: &str) -> FileSystemResult<Self::File>;

    /// Open an existing file.
    ///
    /// Fails if the file does not exist.
    fn open_file(&self, path: &str) -> FileSystemResult<Self::File>;

    /// Removes the file at this path
    fn remove_file(&self, path: &str) -> FileSystemResult<()>;
}

/// A trait for interacting with a filesystem where the file type is erased.
///
/// This trait is automatically implemented for any type that implements [`FileSystem`].
/// It is useful when you need to store different filesystem implementations in the same collection.
pub trait DynamicFileSystem: Debug + Sync + Send + 'static {
    /// Check if an entry exists at the provided path.
    fn exists(&self, path: &str) -> FileSystemResult<bool>;
    /// See if an entry at the path is a file.
    fn is_file(&self, path: &str) -> FileSystemResult<bool>;
    /// See if an entry at the path is a folder.
    fn is_directory(&self, path: &str) -> FileSystemResult<bool>;
    /// Get file or directory size.
    fn filesize(&self, path: &str) -> FileSystemResult<u64>;
    /// Creates a new, empty folder entry at the provided path.
    fn create_directory(&self, path: &str) -> FileSystemResult<()>;
    /// Creates a new, empty folder entry at the provided path, creating all parents as needed.
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()>;
    /// Returns an iterator over the names of entries within a Folder.
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>>;
    /// Removes the folder at this path.
    fn remove_directory(&self, path: &str) -> FileSystemResult<()>;
    /// Removes the folder at this path and all children.
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()>;
    /// Create or Open a new append-only file for writing.
    fn create_file(&self, path: &str) -> FileSystemResult<Box<dyn File>>;
    /// Create or Open a new append only file for writing.
    fn open_file(&self, path: &str) -> FileSystemResult<Box<dyn File>>;
    /// Removes the file at this path
    fn remove_file(&self, path: &str) -> FileSystemResult<()>;
}

impl<FS> DynamicFileSystem for FS
where
    FS: FileSystem,
    FS::File: File + 'static,
{
    /// Check if an entry exists at the provided path.
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::exists(self, path)
    }
    /// See if an entry at the path is a file.
    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::is_file(self, path)
    }
    /// See if an entry at the path is a folder.
    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::is_directory(self, path)
    }
    /// Get file or directory size.
    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        FileSystem::filesize(self, path)
    }
    /// Creates a new, empty folder entry at the provided path.
    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::create_directory(self, path)
    }
    /// Creates a new, empty folder entry at the provided path, creating all parents as needed.
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::create_directory_all(self, path)
    }
    /// Returns an iterator over the names of entries within a Folder.
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>> {
        FileSystem::list_directory(self, path)
    }
    /// Removes the folder at this path.
    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_directory(self, path)
    }
    /// Removes the folder at this path and all children.
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_directory_all(self, path)
    }
    /// Create or Open a new append-only file for writing.
    fn create_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        Ok(Box::new(FileSystem::create_file(self, path)?))
    }
    /// Create or Open a new append only file for writing.
    fn open_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        Ok(Box::new(FileSystem::open_file(self, path)?))
    }
    /// Removes the file at this path
    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_file(self, path)
    }
}

impl<FS> DynamicFileSystem for Arc<FS>
where
    FS: FileSystem,
    FS::File: File + 'static,
{
    /// Check if an entry exists at the provided path.
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::exists(self.deref(), path)
    }
    /// See if an entry at the path is a file.
    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::is_file(self.deref(), path)
    }
    /// See if an entry at the path is a folder.
    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        FileSystem::is_directory(self.deref(), path)
    }
    /// Get file or directory size.
    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        FileSystem::filesize(self.deref(), path)
    }
    /// Creates a new, empty folder entry at the provided path.
    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::create_directory(self.deref(), path)
    }
    /// Creates a new, empty folder entry at the provided path, creating all parents as needed.
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::create_directory_all(self.deref(), path)
    }
    /// Returns an iterator over the names of entries within a Folder.
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>> {
        FileSystem::list_directory(self.deref(), path)
    }
    /// Removes the folder at this path.
    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_directory(self.deref(), path)
    }
    /// Removes the folder at this path and all children.
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_directory_all(self.deref(), path)
    }
    /// Create or Open a new append-only file for writing.
    fn create_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        Ok(Box::new(FileSystem::create_file(self.deref(), path)?))
    }
    /// Create or Open a new append only file for writing.
    fn open_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        Ok(Box::new(FileSystem::open_file(self.deref(), path)?))
    }
    /// Removes the file at this path
    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        FileSystem::remove_file(self.deref(), path)
    }
}

impl DynamicFileSystem for Arc<dyn DynamicFileSystem> {
    /// Check if an entry exists at the provided path.
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        self.deref().exists(path)
    }
    /// See if an entry at the path is a file.
    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        self.deref().is_file(path)
    }
    /// See if an entry at the path is a folder.
    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        self.deref().is_directory(path)
    }
    /// Get file or directory size.
    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        self.deref().filesize(path)
    }
    /// Creates a new, empty folder entry at the provided path.
    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.deref().create_directory(path)
    }
    /// Creates a new, empty folder entry at the provided path, creating all parents as needed.
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.deref().create_directory_all(path)
    }
    /// Returns an iterator over the names of entries within a Folder.
    fn list_directory<'a>(&self, path: &str) -> FileSystemResult<Vec<String>> {
        self.deref().list_directory(path)
    }
    /// Removes the folder at this path.
    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        self.deref().remove_directory(path)
    }
    /// Removes the folder at this path and all children.
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.deref().remove_directory_all(path)
    }
    /// Create or Open a new append-only file for writing.
    fn create_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        self.deref().create_file(path)
    }
    /// Create or Open a new append only file for writing.
    fn open_file(&self, path: &str) -> FileSystemResult<Box<dyn File>> {
        self.deref().open_file(path)
    }
    /// Removes the file at this path
    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        self.deref().remove_file(path)
    }
}

/// Handle for File Access
///
/// This trait provides a common interface for file operations like reading,
/// writing, seeking, and metadata management.
pub trait File: Debug + Read + Write + Seek + Sync + Send + 'static {
    /// Path to this File
    fn path(&self) -> &str;

    /// Get File Size in bytes
    fn get_size(&self) -> FileSystemResult<u64>;

    /// Set File Length (Resize)
    ///
    /// If the new size is larger than the current size, the file is extended.
    /// If the new size is smaller, the file is truncated.
    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()>;

    /// Flushes all data and metadata to storage.
    fn sync_all(&mut self) -> FileSystemResult<()>;

    /// Flush all data to storage.
    fn sync_data(&mut self) -> FileSystemResult<()>;

    /// Get Advisory Lock Status of this file
    fn get_lock_status(&self) -> FileSystemResult<FileLockMode>;

    /// Apply or Clear Advisory Lock of this File
    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()>;

    /// Read directly from a location without modifying the internal cursor.
    ///
    /// This method saves the current cursor position, seeks to `offset`, reads into `buffer`,
    /// and then restores the original cursor position.
    fn read_at_offset(&mut self, offset: u64, buffer: &mut [u8]) -> FileSystemResult<usize> {
        let pos = self.stream_position().map_err(FileSystemError::io_error)?;
        self.seek(SeekFrom::Start(offset))
            .map_err(FileSystemError::io_error)?;
        let rv = self.read(buffer).map_err(FileSystemError::io_error)?;
        self.seek(SeekFrom::Start(pos))
            .map_err(FileSystemError::io_error)?;
        Ok(rv)
    }

    /// Write directly to a location without modifying the internal cursor.
    ///
    /// This method saves the current cursor position, seeks to `offset`, writes from `buffer`,
    /// and then restores the original cursor position.
    fn write_to_offset(&mut self, offset: u64, buffer: &[u8]) -> FileSystemResult<usize> {
        let pos = self.stream_position().map_err(FileSystemError::io_error)?;
        self.seek(SeekFrom::Start(offset))
            .map_err(FileSystemError::io_error)?;
        let rv = self.write(buffer).map_err(FileSystemError::io_error)?;
        self.seek(SeekFrom::Start(pos))
            .map_err(FileSystemError::io_error)?;
        Ok(rv)
    }

    /// Truncate a file to zero length.
    fn truncate(&mut self) -> FileSystemResult<()> {
        self.set_size(0)
    }
}

impl File for Box<dyn File> {
    fn path(&self) -> &str {
        self.as_ref().path()
    }

    fn get_size(&self) -> FileSystemResult<u64> {
        self.as_ref().get_size()
    }

    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()> {
        self.as_mut().set_size(new_size)
    }

    fn sync_all(&mut self) -> FileSystemResult<()> {
        self.as_mut().sync_all()
    }

    fn sync_data(&mut self) -> FileSystemResult<()> {
        self.as_mut().sync_data()
    }

    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        self.as_ref().get_lock_status()
    }

    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()> {
        self.as_mut().set_lock_status(mode)
    }
}

/// An enumeration of types which represents the state of an advisory lock.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileLockMode {
    /// ## UNLOCKED
    Unlocked,
    /// ## SHARED
    Shared,
    /// ## EXCLUSIVE
    Exclusive,
}

/// Represents a path in the virtual file system.
///
/// A [`Path`] can be absolute or relative, and may optionally include a scheme (e.g., `memory://`).
/// It handles path normalization by resolving `.` and `..` segments during manipulation.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Path {
    /// Optional scheme, e.g., "memory", "file", "http"
    pub scheme: Option<String>,

    /// Whether the path is absolute (starts with `/`) **within the scheme**
    pub absolute: bool,

    /// Number of leading `..` segments for relative paths.
    /// This is only used if `absolute` is `false` and there are no segments to pop.
    pub ups: usize,

    /// Normalized segments of the path.
    /// These segments never contain `.` or `..`.
    pub segments: Vec<String>,
}

impl Path {
    /// Creates a new relative path.
    pub fn new() -> Self {
        Self {
            scheme: None,
            absolute: false,
            ups: 0,
            segments: Vec::new(),
        }
    }

    /// Parses a path string into a [`Path`] object.
    ///
    /// The string can optionally include a scheme (e.g., `mem://path/to/file`).
    /// If no scheme is present, the path is parsed as a relative or absolute path.
    pub fn parse(input: &str) -> Self {
        let (scheme, rest) = if let Some(pos) = input.find("://") {
            (Some(input[..pos].to_string()), &input[pos + 3..])
        } else {
            (None, input)
        };

        let absolute = rest.starts_with('/');
        let mut path = if absolute {
            Path {
                scheme,
                absolute: true,
                ups: 0,
                segments: Vec::new(),
            }
        } else {
            Path {
                scheme,
                absolute: false,
                ups: 0,
                segments: Vec::new(),
            }
        };

        for seg in rest.split('/') {
            path.push(seg);
        }

        path
    }

    /// Creates a new absolute root path.
    pub fn root() -> Self {
        Self {
            scheme: None,
            absolute: true,
            ups: 0,
            segments: Vec::new(),
        }
    }

    /// Pushes a new segment onto the path.
    /// - `.` and empty segments are ignored.
    /// - `..` removes the last segment or increments `ups` for relative paths.
    /// - Any other string is added as a new segment.
    ///
    /// # Examples
    /// ```
    /// use nanograph_vfs::Path;
    /// let mut path = Path::parse("/a/b");
    /// path.push("c");
    /// assert_eq!(path.to_string(), "/a/b/c");
    /// path.push("..");
    /// assert_eq!(path.to_string(), "/a/b");
    /// ```
    pub fn push<S: AsRef<str>>(&mut self, segment: S) {
        match segment.as_ref() {
            "" | "." => {}

            ".." => {
                if let Some(_) = self.segments.pop() {
                    // consumed a segment
                } else if !self.absolute {
                    self.ups += 1;
                }
                // absolute path clamps at root
            }

            s => self.segments.push(s.to_owned()),
        }
    }

    /// Pops the last segment from the path.
    /// If there are no segments, it may decrement `ups` for relative paths.
    /// Returns `true` if a segment or `ups` was removed.
    pub fn pop(&mut self) -> bool {
        if self.segments.pop().is_some() {
            true
        } else if self.ups > 0 {
            self.ups -= 1;
            true
        } else {
            false
        }
    }

    /// Joins this path with another path.
    /// - If `other` is absolute, it returns `other`.
    /// - If `other` is relative, it appends `other`'s segments to this path,
    ///   handling `ups` correctly.
    /// - Panics if the schemes are different.
    ///
    /// # Examples
    /// ```
    /// use nanograph_vfs::Path;
    /// let p1 = Path::parse("/a/b");
    /// let p2 = Path::parse("c/d");
    /// assert_eq!(p1.join(&p2).to_string(), "/a/b/c/d");
    ///
    /// let p3 = Path::parse("../e");
    /// assert_eq!(p1.join(&p3).to_string(), "/a/e");
    /// ```
    pub fn join(&self, other: &Path) -> Path {
        if other.scheme.is_some() && other.scheme != self.scheme {
            panic!("Cannot join paths with different schemes");
        }

        if other.absolute {
            return other.clone();
        }

        let mut out = self.clone();

        for _ in 0..other.ups {
            out.push("..");
        }

        for seg in &other.segments {
            out.push(seg);
        }

        out
    }

    /// Returns a new `Path` with all segments fully normalized,
    /// collapsing `.` and `..`, and removing redundant `ups` where possible.
    pub fn resolve(&self) -> Path {
        let mut resolved = if self.absolute {
            Path::root()
        } else {
            Path::new()
        };

        resolved.scheme = self.scheme.clone();

        for _ in 0..self.ups {
            resolved.push("..");
        }

        for seg in &self.segments {
            resolved.push(seg);
        }

        resolved
    }

    /// Returns an iterator over normalized segments
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(|s| s.as_str())
    }

    /// Returns a new [`Path`] representing the parent path
    pub fn parent(&self) -> Path {
        if !self.segments.is_empty() {
            // Remove last segment
            let mut p = self.clone();
            p.segments.pop();
            p
        } else if !self.absolute {
            // Relative path: add a leading ".."
            let mut p = self.clone();
            p.ups += 1;
            p
        } else {
            // Absolute root: parent is still root
            self.clone()
        }
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(scheme) = &self.scheme {
            write!(f, "{}://", scheme)?;
        }

        if self.absolute {
            write!(f, "/")?;
        }

        for i in 0..self.ups {
            write!(f, "..")?;
            if i < self.ups - 1 || !self.segments.is_empty() {
                write!(f, "/")?;
            }
        }

        write!(f, "{}", self.segments.join("/"))
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_new() {
        let p = Path::new();
        assert_eq!(p.scheme, None);
        assert!(!p.absolute);
        assert_eq!(p.ups, 0);
        assert!(p.segments.is_empty());
    }

    #[test]
    fn test_path_root() {
        let p = Path::root();
        assert_eq!(p.scheme, None);
        assert!(p.absolute);
        assert_eq!(p.ups, 0);
        assert!(p.segments.is_empty());
    }

    #[test]
    fn test_path_parse() {
        let cases = vec![
            ("", None, false, 0, vec![]),
            ("/", None, true, 0, vec![]),
            ("abc", None, false, 0, vec!["abc"]),
            ("/abc", None, true, 0, vec!["abc"]),
            ("abc/def", None, false, 0, vec!["abc", "def"]),
            ("/abc/def", None, true, 0, vec!["abc", "def"]),
            ("./abc", None, false, 0, vec!["abc"]),
            ("../abc", None, false, 1, vec!["abc"]),
            ("../../abc", None, false, 2, vec!["abc"]),
            ("/../abc", None, true, 0, vec!["abc"]), // Absolute path clamps at root
            ("mem://", Some("mem"), false, 0, vec![]),
            ("mem:///", Some("mem"), true, 0, vec![]),
            ("mem:///abc", Some("mem"), true, 0, vec!["abc"]),
        ];

        for (input, scheme, absolute, ups, segments) in cases {
            let p = Path::parse(input);
            assert_eq!(p.scheme, scheme.map(|s| s.to_string()), "Input: {}", input);
            assert_eq!(p.absolute, absolute, "Input: {}", input);
            assert_eq!(p.ups, ups, "Input: {}", input);
            assert_eq!(p.segments, segments, "Input: {}", input);
        }
    }

    #[test]
    fn test_path_push_pop() {
        let mut p = Path::new();
        p.push("abc");
        assert_eq!(p.segments, vec!["abc"]);
        p.push("def");
        assert_eq!(p.segments, vec!["abc", "def"]);
        assert!(p.pop());
        assert_eq!(p.segments, vec!["abc"]);
        assert!(p.pop());
        assert!(p.segments.is_empty());
        assert!(!p.pop());

        let mut p = Path::new();
        p.push("..");
        assert_eq!(p.ups, 1);
        assert!(p.pop());
        assert_eq!(p.ups, 0);

        let mut p = Path::root();
        p.push(".."); // Absolute clamps
        assert_eq!(p.ups, 0);
        assert!(p.segments.is_empty());
        assert!(!p.pop());
    }

    #[test]
    fn test_path_join() {
        let p1 = Path::parse("/abc");
        let p2 = Path::parse("def");
        let joined = p1.join(&p2);
        assert_eq!(joined.to_string(), "/abc/def");

        let p1 = Path::parse("/abc/def");
        let p2 = Path::parse("..");
        let joined = p1.join(&p2);
        assert_eq!(joined.to_string(), "/abc");

        let p1 = Path::parse("abc");
        let p2 = Path::parse("../def");
        let joined = p1.join(&p2);
        assert_eq!(joined.to_string(), "def");

        let p1 = Path::parse("abc");
        let p2 = Path::parse("../../def");
        let joined = p1.join(&p2);
        assert_eq!(joined.to_string(), "../def");

        let p1 = Path::parse("mem:///abc");
        let p2 = Path::parse("def");
        let joined = p1.join(&p2);
        assert_eq!(joined.to_string(), "mem:///abc/def");
    }

    #[test]
    #[should_panic(expected = "Cannot join paths with different schemes")]
    fn test_path_join_mismatched_schemes() {
        let p1 = Path::parse("mem:///abc");
        let p2 = Path::parse("file:///def");
        p1.join(&p2);
    }

    #[test]
    fn test_path_resolve() {
        let p = Path::parse("abc/../def");
        let r = p.resolve();
        assert_eq!(r.to_string(), "def");

        let p = Path::parse("/abc/../../def");
        let r = p.resolve();
        assert_eq!(r.to_string(), "/def");
    }

    #[test]
    fn test_path_parent() {
        assert_eq!(Path::parse("/abc/def").parent().to_string(), "/abc");
        assert_eq!(Path::parse("/abc").parent().to_string(), "/");
        assert_eq!(Path::parse("/").parent().to_string(), "/");
        assert_eq!(Path::parse("abc/def").parent().to_string(), "abc");
        assert_eq!(Path::parse("abc").parent().to_string(), "");
        assert_eq!(Path::parse("").parent().to_string(), "..");
        assert_eq!(Path::parse("..").parent().to_string(), "../..");
    }

    #[test]
    fn test_path_display() {
        assert_eq!(Path::parse("/abc/def").to_string(), "/abc/def");
        assert_eq!(Path::parse("abc/def").to_string(), "abc/def");
        assert_eq!(Path::parse("../abc").to_string(), "../abc");
        assert_eq!(Path::parse("mem:///abc").to_string(), "mem:///abc");
        assert_eq!(Path::parse("mem://abc").to_string(), "mem://abc");
    }
}
