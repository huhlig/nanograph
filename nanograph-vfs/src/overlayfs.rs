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

use crate::filesystem::FileSystem;
use crate::{DynamicFileSystem, File, FileLockMode, FileSystemResult};
use std::fmt::{Debug, Formatter};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

/// A [`FileSystem`] implementation that provides a unified view of multiple layered filesystems.
///
/// The layers are ordered from top to bottom.
/// - Read operations check layers in order and use the first one that has the path.
/// - Write operations (create, remove, etc.) always target the first (top-most) layer.
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, OverlayFilesystem, File};
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let upper = Arc::new(MemoryFileSystem::new());
/// let lower = Arc::new(MemoryFileSystem::new());
///
/// lower.create_file("/readonly.txt")?;
///
/// let fs = OverlayFilesystem::new(vec![upper.clone() as Arc<_>, lower.clone() as Arc<_>].into_iter());
///
/// // Can read from lower layer
/// assert!(fs.exists("/readonly.txt")?);
///
/// // Writes go to upper layer
/// fs.create_file("/new.txt")?;
/// assert!(upper.exists("/new.txt")?);
/// assert!(!lower.exists("/new.txt")?);
/// # Ok(())
/// # }
/// ```
pub struct OverlayFilesystem {
    layers: Vec<Arc<dyn DynamicFileSystem>>,
}

impl OverlayFilesystem {
    /// Creates a new `OverlayFilesystem` with the given layers.
    ///
    /// # Arguments
    /// layers - Iterator of layers, each implementing `DynamicFileSystem`.
    ///          The first item is the top-most (writable) layer.
    pub fn new(layers: impl Iterator<Item = Arc<dyn DynamicFileSystem>>) -> OverlayFilesystem {
        OverlayFilesystem {
            layers: layers.collect(),
        }
    }

    /// Returns the layers of this overlay filesystem.
    #[must_use]
    pub fn layers(&self) -> &[Arc<dyn DynamicFileSystem>] {
        &self.layers
    }

    /// Adds a new layer to the bottom of the overlay.
    pub fn add_layer(&mut self, layer: Arc<dyn DynamicFileSystem>) {
        self.layers.push(layer);
    }

    fn find_layer(&self, path: &str) -> Option<&Arc<dyn DynamicFileSystem>> {
        self.layers
            .iter()
            .find(|layer| layer.exists(path).unwrap_or(false))
    }

    fn upper(&self) -> FileSystemResult<&Arc<dyn DynamicFileSystem>> {
        self.layers.first().ok_or_else(|| {
            crate::FileSystemError::internal_error("OverlayFilesystem must have at least one layer")
        })
    }
}

impl Debug for OverlayFilesystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayFilesystem")
            .field("layers", &self.layers)
            .finish()
    }
}

impl FileSystem for OverlayFilesystem {
    type File = OverlayFile;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        for layer in &self.layers {
            if layer.exists(path)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        if let Option::Some(layer) = self.find_layer(path) {
            layer.is_file(path)
        } else {
            Ok(false)
        }
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        if let Option::Some(layer) = self.find_layer(path) {
            layer.is_directory(path)
        } else {
            Ok(false)
        }
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        if let Option::Some(layer) = self.find_layer(path) {
            layer.filesize(path)
        } else {
            Err(crate::FileSystemError::PathMissing)
        }
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.upper()?.create_directory(path)
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.upper()?.create_directory_all(path)
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        let mut entries = std::collections::BTreeSet::new();
        let mut found = false;
        for layer in &self.layers {
            if layer.exists(path)? && layer.is_directory(path)? {
                found = true;
                for entry in layer.list_directory(path)? {
                    entries.insert(entry);
                }
            }
        }
        if found {
            Ok(entries.into_iter().collect())
        } else {
            Err(crate::FileSystemError::PathMissing)
        }
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        self.upper()?.remove_directory(path)
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.upper()?.remove_directory_all(path)
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        let file = self.upper()?.create_file(path)?;
        Ok(OverlayFile {
            file: Arc::new(file),
            path: path.to_string(),
        })
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        if let Option::Some(layer) = self.find_layer(path) {
            let file = layer.open_file(path)?;
            Ok(OverlayFile {
                file: Arc::from(file),
                path: path.to_string(),
            })
        } else {
            Err(crate::FileSystemError::PathMissing)
        }
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        self.upper()?.remove_file(path)
    }
}

/// A [`File`] handle for a file on an overlay filesystem.
pub struct OverlayFile {
    file: Arc<dyn File>,
    path: String,
}

impl Debug for OverlayFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayFile")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl File for OverlayFile {
    fn path(&self) -> &str {
        &self.path
    }

    fn get_size(&self) -> FileSystemResult<u64> {
        self.file.get_size()
    }

    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| {
                crate::FileSystemError::internal_error("Cannot get mutable access to OverlayFile")
            })?
            .set_size(new_size)
    }

    fn sync_all(&mut self) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| {
                crate::FileSystemError::internal_error("Cannot get mutable access to OverlayFile")
            })?
            .sync_all()
    }

    fn sync_data(&mut self) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| {
                crate::FileSystemError::internal_error("Cannot get mutable access to OverlayFile")
            })?
            .sync_data()
    }

    fn get_lock_status(&self) -> FileSystemResult<FileLockMode> {
        self.file.get_lock_status()
    }

    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| {
                crate::FileSystemError::internal_error("Cannot get mutable access to OverlayFile")
            })?
            .set_lock_status(mode)
    }
}

impl Write for OverlayFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| std::io::Error::other("Cannot get mutable access to OverlayFile"))?
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| std::io::Error::other("Cannot get mutable access to OverlayFile"))?
            .flush()
    }
}

impl Read for OverlayFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| std::io::Error::other("Cannot get mutable access to OverlayFile"))?
            .read(buf)
    }
}

impl Seek for OverlayFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        Arc::get_mut(&mut self.file)
            .ok_or_else(|| std::io::Error::other("Cannot get mutable access to OverlayFile"))?
            .seek(pos)
    }
}

#[cfg(test)]
mod test {
    use super::OverlayFilesystem;
    use crate::FileSystem;
    use crate::memoryfs::MemoryFileSystem;
    use crate::test_suite::{BoxedFileSystem, run_generic_test_suite};
    use std::io::Write;
    use std::sync::Arc;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        let upper = Arc::new(BoxedFileSystem {
            inner: Arc::new(MemoryFileSystem::new()),
        });
        let lower = Arc::new(BoxedFileSystem {
            inner: Arc::new(MemoryFileSystem::new()),
        });
        let fs = OverlayFilesystem::new(
            vec![
                upper as Arc<dyn crate::DynamicFileSystem>,
                lower as Arc<dyn crate::DynamicFileSystem>,
            ]
            .into_iter(),
        );
        run_generic_test_suite(&fs);
    }

    #[test]
    #[traced_test]
    fn test_overlay_logic() {
        let upper_inner = Arc::new(MemoryFileSystem::new());
        let lower_inner = Arc::new(MemoryFileSystem::new());

        // Setup lower with a file
        {
            let mut f = lower_inner.create_file("/lower.txt").unwrap();
            f.write_all(b"lower content").unwrap();
        }

        let upper = Arc::new(BoxedFileSystem {
            inner: upper_inner.clone(),
        });
        let lower = Arc::new(BoxedFileSystem {
            inner: lower_inner.clone(),
        });
        let fs = OverlayFilesystem::new(
            vec![
                upper as Arc<dyn crate::DynamicFileSystem>,
                lower as Arc<dyn crate::DynamicFileSystem>,
            ]
            .into_iter(),
        );

        // Check if we can see lower file
        assert!(fs.exists("/lower.txt").unwrap());
        assert_eq!(fs.filesize("/lower.txt").unwrap(), 13);

        // Overwrite in upper
        {
            let mut f = fs.create_file("/lower.txt").unwrap();
            f.write_all(b"upper override").unwrap();
        }

        // Check if we see upper content
        assert_eq!(fs.filesize("/lower.txt").unwrap(), 14);

        // Verify original lower file is untouched
        assert_eq!(lower_inner.filesize("/lower.txt").unwrap(), 13);
    }
}
