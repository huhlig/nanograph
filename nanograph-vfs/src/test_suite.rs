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

use crate::{File, FileLockMode, FileSystem, FileSystemResult};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

#[cfg(test)]
pub fn run_generic_test_suite<F: FileSystem>(fs: &F) {
    // 1. Basic File Operations
    {
        let path = "/test_file.txt";
        assert!(!fs.exists(path).unwrap());

        // Create
        let mut file = fs.create_file(path).expect("Failed to create file");
        assert!(fs.exists(path).unwrap());
        assert!(fs.is_file(path).unwrap());
        assert!(!fs.is_directory(path).unwrap());
        assert_eq!(file.get_size().unwrap(), 0);

        // Write
        file.write_all(b"Hello VFS")
            .expect("Failed to write to file");
        assert_eq!(file.get_size().unwrap(), 9);
        assert_eq!(fs.filesize(path).unwrap(), 9);

        // Sync (should not crash)
        file.sync_all().unwrap();
        file.sync_data().unwrap();

        // Read
        let mut buf = [0u8; 9];
        file.seek(SeekFrom::Start(0)).expect("Seek failed");
        file.read_exact(&mut buf).expect("Read exact failed");
        assert_eq!(&buf, b"Hello VFS");

        // Open
        let mut file2 = fs.open_file(path).expect("Failed to open file");
        let mut buf2 = Vec::new();
        file2.read_to_end(&mut buf2).unwrap();
        assert_eq!(buf2, b"Hello VFS");

        // Path
        assert_eq!(file.path(), path);

        // Resize
        file.set_size(5).unwrap();
        assert_eq!(file.get_size().unwrap(), 5);

        // Truncate (via set_size(0) usually, but check trait)
        // Some implementations might not have truncate() if it's not in trait but we can use set_size(0)
        file.set_size(0).unwrap();
        assert_eq!(file.get_size().unwrap(), 0);

        // Close/Drop and Remove
        drop(file);
        drop(file2);
        fs.remove_file(path).expect("Failed to remove file");
        assert!(!fs.exists(path).unwrap());
    }

    // 2. Directory Operations
    {
        let dir_path = "/test_dir";
        let sub_dir_path = "/test_dir/sub";
        let file_in_dir = "/test_dir/file.txt";

        fs.create_directory(dir_path)
            .expect("Failed to create directory");
        assert!(fs.exists(dir_path).unwrap());
        assert!(fs.is_directory(dir_path).unwrap());

        fs.create_directory_all(sub_dir_path)
            .expect("Failed to create_directory_all");
        assert!(fs.is_directory(sub_dir_path).unwrap());

        fs.create_file(file_in_dir)
            .expect("Failed to create file in dir");

        let entries = fs
            .list_directory(dir_path)
            .expect("Failed to list directory");
        assert!(entries.contains(&"sub".to_string()));
        assert!(entries.contains(&"file.txt".to_string()));
        assert_eq!(entries.len(), 2);

        // Remove
        fs.remove_file(file_in_dir).unwrap();
        fs.remove_directory(sub_dir_path).unwrap();
        fs.remove_directory(dir_path).unwrap();
        assert!(!fs.exists(dir_path).unwrap());
    }

    // 3. Recursive Removal
    {
        let root = "/remove_all";
        fs.create_directory_all("/remove_all/a/b/c").unwrap();
        fs.create_file("/remove_all/a/file1.txt").unwrap();
        fs.create_file("/remove_all/a/b/file2.txt").unwrap();

        fs.remove_directory_all(root)
            .expect("Failed to remove_directory_all");
        assert!(!fs.exists(root).unwrap());
    }

    // 4. Offset Reads/Writes
    {
        let path = "/offset_test.bin";
        let mut file = fs.create_file(path).unwrap();

        let data1 = b"ABCDE";
        let data2 = b"123";

        file.write_to_offset(0, data1).unwrap();
        file.write_to_offset(2, data2).unwrap(); // AB123

        let mut buf = [0u8; 5];
        file.read_at_offset(0, &mut buf).unwrap();
        assert_eq!(&buf, b"AB123");

        fs.remove_file(path).unwrap();
    }

    // 5. Lock Status (Basic check)
    {
        let path = "/lock_test.txt";
        let mut file = fs.create_file(path).unwrap();

        // Most implementations might just return Unlocked or Ok(())
        let _initial_lock = file.get_lock_status().unwrap();
        file.set_lock_status(FileLockMode::Exclusive).unwrap();
        // We don't strictly assert change because some FS might not support it and just return Ok

        drop(file);
        fs.remove_file(path).unwrap();
    }
}

/// A wrapper that converts any `FileSystem` to one that returns Box<dyn File>
#[derive(Debug)]
pub struct BoxedFileSystem<F: FileSystem> {
    pub inner: Arc<F>,
}

impl<F: FileSystem + 'static> FileSystem for BoxedFileSystem<F> {
    type File = Box<dyn File>;

    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        self.inner.exists(path)
    }

    fn is_file(&self, path: &str) -> FileSystemResult<bool> {
        self.inner.is_file(path)
    }

    fn is_directory(&self, path: &str) -> FileSystemResult<bool> {
        self.inner.is_directory(path)
    }

    fn filesize(&self, path: &str) -> FileSystemResult<u64> {
        self.inner.filesize(path)
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.inner.create_directory(path)
    }

    fn create_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.inner.create_directory_all(path)
    }

    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>> {
        self.inner.list_directory(path)
    }

    fn remove_directory(&self, path: &str) -> FileSystemResult<()> {
        self.inner.remove_directory(path)
    }

    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()> {
        self.inner.remove_directory_all(path)
    }

    fn create_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.inner
            .create_file(path)
            .map(|f| Box::new(f) as Box<dyn File>)
    }

    fn open_file(&self, path: &str) -> FileSystemResult<Self::File> {
        self.inner
            .open_file(path)
            .map(|f| Box::new(f) as Box<dyn File>)
    }

    fn remove_file(&self, path: &str) -> FileSystemResult<()> {
        self.inner.remove_file(path)
    }
}
