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

use nanograph_vfs::{File, FileSystem, LocalFilesystem, MemoryFileSystem};
use std::io::{Read, Seek, SeekFrom, Write};

#[test]
fn test_memory_filesystem_concurrent_access() {
    let fs = MemoryFileSystem::new();

    // Create multiple files concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let fs_clone = fs.clone();
            std::thread::spawn(move || {
                let path = format!("/file_{}.txt", i);
                let payload = format!("Content {}", i);
                let mut file = FileSystem::create_file(&fs_clone, &path).unwrap();
                file.write_all(payload.as_bytes()).unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all files exist
    for i in 0..10 {
        let path = format!("/file_{}.txt", i);
        assert!(FileSystem::exists(&fs, &path).unwrap());
        let mut file = FileSystem::open_file(&fs, &path).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(content, format!("Content {}", i));
    }
}

#[test]
fn test_large_file_operations() {
    let fs = MemoryFileSystem::new();
    let path = "/large_file.bin";

    // Create a 10MB file
    let size = 10 * 1024 * 1024;
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    {
        let mut file = FileSystem::create_file(&fs, path).unwrap();
        file.write_all(&data).unwrap();
    }

    // Verify size
    assert_eq!(FileSystem::filesize(&fs, path).unwrap(), size as u64);

    // Read back and verify
    {
        let mut file = FileSystem::open_file(&fs, path).unwrap();
        let mut read_data = vec![0u8; size];
        file.read_exact(&mut read_data).unwrap();
        assert_eq!(read_data, data);
    }
}

#[test]
fn test_directory_tree_operations() {
    let fs = MemoryFileSystem::new();

    // Create deep directory structure
    FileSystem::create_directory_all(&fs, "/a/b/c/d/e/f").unwrap();

    // Create files at various levels
    FileSystem::create_file(&fs, "/a/file1.txt").unwrap();
    FileSystem::create_file(&fs, "/a/b/file2.txt").unwrap();
    FileSystem::create_file(&fs, "/a/b/c/file3.txt").unwrap();

    // List directories
    let entries = FileSystem::list_directory(&fs, "/a/b").unwrap();
    assert_eq!(entries.len(), 2); // c and file2.txt
    assert!(entries.contains(&"c".to_string()));
    assert!(entries.contains(&"file2.txt".to_string()));

    // Remove entire tree
    FileSystem::remove_directory_all(&fs, "/a").unwrap();
    assert!(!FileSystem::exists(&fs, "/a").unwrap());
}

#[test]
fn test_file_seek_operations() {
    let fs = MemoryFileSystem::new();
    let path = "/seek_test.txt";

    {
        let mut file = FileSystem::create_file(&fs, path).unwrap();
        file.write_all(b"0123456789").unwrap();
    }

    {
        let mut file = FileSystem::open_file(&fs, path).unwrap();

        // Seek from start
        file.seek(SeekFrom::Start(5)).unwrap();
        let mut buf = [0u8; 5];
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"56789");

        // Seek from end
        file.seek(SeekFrom::End(-3)).unwrap();
        let mut buf = [0u8; 3];
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"789");

        // Seek from current
        file.seek(SeekFrom::Start(5)).unwrap();
        file.seek(SeekFrom::Current(2)).unwrap();
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"7");
    }
}

#[test]
fn test_file_truncate_and_resize() {
    let fs = MemoryFileSystem::new();
    let path = "/resize_test.txt";

    {
        let mut file = FileSystem::create_file(&fs, path).unwrap();
        file.write_all(b"Hello, World!").unwrap();
        assert_eq!(file.get_size().unwrap(), 13);

        // Truncate
        file.truncate().unwrap();
        assert_eq!(file.get_size().unwrap(), 0);

        // Seek to start and write again
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"New").unwrap();
        assert_eq!(file.get_size().unwrap(), 3);

        // Resize larger
        file.set_size(10).unwrap();
        assert_eq!(file.get_size().unwrap(), 10);
    }
}

#[test]
fn test_offset_read_write() {
    let fs = MemoryFileSystem::new();
    let path = "/offset_test.bin";

    {
        let mut file = FileSystem::create_file(&fs, path).unwrap();

        // Write at different offsets
        file.write_to_offset(0, b"AAAA").unwrap();
        file.write_to_offset(10, b"BBBB").unwrap();
        file.write_to_offset(5, b"CCCC").unwrap();

        // Read back
        let mut buf = [0u8; 4];
        file.read_at_offset(0, &mut buf).unwrap();
        assert_eq!(&buf, b"AAAA");

        file.read_at_offset(5, &mut buf).unwrap();
        assert_eq!(&buf, b"CCCC");

        file.read_at_offset(10, &mut buf).unwrap();
        assert_eq!(&buf, b"BBBB");
    }
}

#[test]
fn test_local_filesystem_integration() {
    let temp_dir = std::env::temp_dir().join(format!(
        "nanograph-vfs-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let fs = LocalFilesystem::new(&temp_dir);

    // Test basic operations
    FileSystem::create_directory(&fs, "/testdir").unwrap();
    assert!(FileSystem::is_directory(&fs, "/testdir").unwrap());

    let mut file = FileSystem::create_file(&fs, "/testdir/file.txt").unwrap();
    file.write_all(b"test content").unwrap();
    drop(file);

    assert!(FileSystem::exists(&fs, "/testdir/file.txt").unwrap());
    assert_eq!(FileSystem::filesize(&fs, "/testdir/file.txt").unwrap(), 12);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_error_conditions() {
    let fs = MemoryFileSystem::new();

    // Try to open non-existent file
    assert!(FileSystem::open_file(&fs, "/nonexistent.txt").is_err());

    // Try to create file in non-existent directory
    assert!(FileSystem::create_file(&fs, "/nonexistent/file.txt").is_err());

    // Try to remove non-existent file
    assert!(FileSystem::remove_file(&fs, "/nonexistent.txt").is_err());

    // Try to list non-existent directory
    assert!(FileSystem::list_directory(&fs, "/nonexistent").is_err());

    // Try to create duplicate file
    FileSystem::create_file(&fs, "/test.txt").unwrap();
    assert!(FileSystem::create_file(&fs, "/test.txt").is_err());
}

#[test]
fn test_file_sync_operations() {
    let fs = MemoryFileSystem::new();
    let path = "/sync_test.txt";

    let mut file = FileSystem::create_file(&fs, path).unwrap();
    file.write_all(b"test data").unwrap();

    // These should not fail
    file.sync_all().unwrap();
    file.sync_data().unwrap();
}

#[test]
fn test_file_lock_operations() {
    use nanograph_vfs::FileLockMode;

    let fs = MemoryFileSystem::new();
    let path = "/lock_test.txt";

    let mut file = FileSystem::create_file(&fs, path).unwrap();

    // Get initial lock status
    let initial_lock = file.get_lock_status().unwrap();
    assert_eq!(initial_lock, FileLockMode::Unlocked);

    // Set exclusive lock
    file.set_lock_status(FileLockMode::Exclusive).unwrap();
    let lock_status = file.get_lock_status().unwrap();
    assert_eq!(lock_status, FileLockMode::Exclusive);

    // Set shared lock
    file.set_lock_status(FileLockMode::Shared).unwrap();
    let lock_status = file.get_lock_status().unwrap();
    assert_eq!(lock_status, FileLockMode::Shared);

    // Unlock
    file.set_lock_status(FileLockMode::Unlocked).unwrap();
    let lock_status = file.get_lock_status().unwrap();
    assert_eq!(lock_status, FileLockMode::Unlocked);
}
