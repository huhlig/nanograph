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

//! # Nanograph Virtual File System (VFS)
//!
//! This crate provides a unified interface for interacting with various filesystem backends.
//! It supports local filesystems, in-memory filesystems, and composite filesystems
//! like mounting and overlay layers.
//!
//! ## Examples
//!
//! ### Basic MemoryFileSystem Usage
//!
//! ```
//! use nanograph_vfs::{FileSystem, MemoryFileSystem, File};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = MemoryFileSystem::new();
//! fs.create_directory("/test")?;
//! let mut file = fs.create_file("/test/hello.txt")?;
//! file.write_to_offset(0, b"Hello, VFS!")?;
//!
//! assert!(fs.exists("/test/hello.txt")?);
//! # Ok(())
//! # }
//! ```
//!
//! ### Reading and Writing Files
//!
//! ```
//! use nanograph_vfs::{FileSystem, MemoryFileSystem, File};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = MemoryFileSystem::new();
//!
//! // Create and write to a file
//! let mut file = fs.create_file("/data.txt")?;
//! file.write_to_offset(0, b"Line 1\n")?;
//! file.write_to_offset(7, b"Line 2\n")?;
//!
//! // Read the file
//! let mut buffer = vec![0u8; 14];
//! file.read_at_offset(0, &mut buffer)?;
//! assert_eq!(&buffer, b"Line 1\nLine 2\n");
//!
//! // Get file size
//! let size = file.get_size()?;
//! assert_eq!(size, 14);
//! # Ok(())
//! # }
//! ```
//!
//! ### Working with Directories
//!
//! ```
//! use nanograph_vfs::{FileSystem, MemoryFileSystem};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = MemoryFileSystem::new();
//!
//! // Create nested directories
//! fs.create_directory_all("/app/data/logs")?;
//!
//! // List directory contents
//! fs.create_file("/app/data/file1.txt")?;
//! fs.create_file("/app/data/file2.txt")?;
//!
//! let entries = fs.list_directory("/app/data")?;
//! assert_eq!(entries.len(), 3); // file1.txt, file2.txt, logs/
//! # Ok(())
//! # }
//! ```
//!
//! ### Using LocalFilesystem
//!
//! ```no_run
//! use nanograph_vfs::{FileSystem, LocalFilesystem, File};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = LocalFilesystem::new("/tmp/vfs_root");
//!
//! // Create a file on the local filesystem
//! let mut file = fs.create_file("/test.txt")?;
//! file.write_to_offset(0, b"Hello, local filesystem!")?;
//!
//! // Check if file exists
//! assert!(fs.exists("/test.txt")?);
//!
//! // Get file size
//! let size = fs.filesize("/test.txt")?;
//! println!("File size: {} bytes", size);
//! # Ok(())
//! # }
//! ```
//!
//! ### File Locking
//!
//! ```
//! use nanograph_vfs::{FileSystem, MemoryFileSystem, File, FileLockMode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = MemoryFileSystem::new();
//! let mut file = fs.create_file("/locked.txt")?;
//!
//! // Acquire exclusive lock
//! file.set_lock_status(FileLockMode::Exclusive)?;
//! file.write_to_offset(0, b"Protected data")?;
//! file.set_lock_status(FileLockMode::Unlocked)?;
//!
//! // Acquire shared lock for reading
//! file.set_lock_status(FileLockMode::Shared)?;
//! let mut buffer = vec![0u8; 14];
//! file.read_at_offset(0, &mut buffer)?;
//! file.set_lock_status(FileLockMode::Unlocked)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Using FileSystemManager
//!
//! ```
//! use nanograph_vfs::{FileSystemManager, MemoryFileSystem, FileSystem, File};
//! use std::sync::Arc;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let manager = FileSystemManager::new();
//!
//! // Register a memory filesystem
//! let mem_fs = Arc::new(MemoryFileSystem::new());
//! manager.register("mem", mem_fs.clone());
//!
//! // Access the registered filesystem directly
//! let mut file = mem_fs.create_file("/test.txt")?;
//! file.write_to_offset(0, b"Hello from manager!")?;
//!
//! // Verify it's registered
//! assert!(manager.list_schemes().contains(&"mem".to_string()));
//! # Ok(())
//! # }
//! ```
//!
//! ### Copying and Moving Files
//!
//! ```
//! use nanograph_vfs::{FileSystem, MemoryFileSystem, File};
//! use std::io::Read;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = MemoryFileSystem::new();
//!
//! // Create source file
//! let mut file = fs.create_file("/source.txt")?;
//! file.write_to_offset(0, b"Original content")?;
//! drop(file);
//!
//! // Copy file manually (copy operation)
//! let mut source = fs.open_file("/source.txt")?;
//! let mut buffer = Vec::new();
//! source.read_to_end(&mut buffer)?;
//! drop(source);
//! let mut dest = fs.create_file("/copy.txt")?;
//! dest.write_to_offset(0, &buffer)?;
//! drop(dest);
//! assert!(fs.exists("/copy.txt")?);
//!
//! // Move file manually (remove old after copy)
//! let mut source = fs.open_file("/copy.txt")?;
//! let mut buffer = Vec::new();
//! source.read_to_end(&mut buffer)?;
//! drop(source);
//! let mut dest = fs.create_file("/moved.txt")?;
//! dest.write_to_offset(0, &buffer)?;
//! drop(dest);
//! fs.remove_file("/copy.txt")?;
//! assert!(fs.exists("/moved.txt")?);
//! assert!(!fs.exists("/copy.txt")?);
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(
    clippy::cargo,
    missing_docs,
    clippy::pedantic,
    future_incompatible,
    rust_2018_idioms
)]
#![allow(
    clippy::option_if_let_else,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc
)]

mod filesystem;
mod localfs;
mod manager;
mod memoryfs;
mod monitoredfs;
mod mountingfs;
mod overlayfs;
mod result;
mod virtualfs;

/// Metrics collection for filesystem operations
pub mod metrics;

#[cfg(test)]
mod test_suite;

pub use self::filesystem::{DynamicFileSystem, File, FileLockMode, FileSystem, Path};
pub use self::localfs::{LocalFile, LocalFilesystem};
pub use self::manager::FileSystemManager;
pub use self::memoryfs::{MemoryFile, MemoryFileSystem};
pub use self::monitoredfs::{MonitoredFile, MonitoredFilesystem};
pub use self::mountingfs::{MountableFilesystem, MountedFile};
pub use self::overlayfs::{OverlayFile, OverlayFilesystem};
pub use self::result::{FileSystemError, FileSystemResult};
pub use self::virtualfs::VirtualFileSystem;
