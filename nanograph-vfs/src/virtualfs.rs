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

use crate::FileSystemResult;
use crate::filesystem::FileSystem;
use std::fmt::Debug;
use std::sync::Arc;

/// A [`FileSystem`] implementation that serves as an aggregator for various backends.
///
/// This serves as a primary entry point or wrapper for various filesystem backends.
/// It uses dynamic dispatch to interact with an underlying `FileSystem` where the
/// file type has been boxed.
///
/// # Examples
/// ```
/// use nanograph_vfs::{FileSystem, MemoryFileSystem, VirtualFileSystem, File};
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // In practice, you might wrap a MemoryFileSystem to match the expected signature
/// // let inner = Arc::new(BoxedFileSystem { inner: Arc::new(MemoryFileSystem::new()) });
/// // let vfs = VirtualFileSystem::new(inner);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct VirtualFileSystem {
    backend: Arc<dyn FileSystem<File = Box<dyn crate::File>>>,
}

impl VirtualFileSystem {
    /// Creates a new `VirtualFileSystem` using the provided backend.
    pub fn new(backend: Arc<dyn FileSystem<File = Box<dyn crate::File>>>) -> Self {
        Self { backend }
    }
}

impl Debug for VirtualFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualFileSystem").finish()
    }
}

impl FileSystem for VirtualFileSystem {
    type File = Box<dyn crate::File>;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        self.backend.exists(path)
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        self.backend.is_file(path)
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        self.backend.is_directory(path)
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        self.backend.filesize(path)
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.backend.create_directory(path)
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.backend.create_directory_all(path)
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        self.backend.list_directory(path)
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        self.backend.remove_directory(path)
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.backend.remove_directory_all(path)
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.backend.create_file(path)
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.backend.open_file(path)
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        self.backend.remove_file(path)
    }
}

#[cfg(test)]
mod test {
    use super::VirtualFileSystem;
    use crate::memoryfs::MemoryFileSystem;
    use crate::test_suite::{BoxedFileSystem, run_generic_test_suite};
    use std::sync::Arc;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_generic() {
        let inner = Arc::new(BoxedFileSystem {
            inner: Arc::new(MemoryFileSystem::new()),
        });
        let fs = VirtualFileSystem::new(inner);
        run_generic_test_suite(fs);
    }
}
