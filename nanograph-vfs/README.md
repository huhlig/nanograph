# Nanograph Virtual File System (VFS)

A pluggable, metrics-enabled virtual file system abstraction for Nanograph.

## Overview

The `nanograph-vfs` crate provides a unified interface for interacting with various filesystem backends. It supports:

- **Multiple Backends**: Local filesystem, in-memory, overlay, and mounting filesystems
- **Pluggable Architecture**: Easy to add custom filesystem implementations
- **Metrics Integration**: Built-in metrics collection using the `metrics` crate
- **Monitoring**: Transparent operation tracking and performance measurement
- **Type Safety**: Strong typing with trait-based design

## Features

### Core Traits

#### `FileSystem` Trait
The main trait that all filesystem implementations must implement:

```rust
pub trait FileSystem: Debug + Sync + Send + 'static {
    type File: File;
    
    // Directory operations
    fn exists(&self, path: &str) -> FileSystemResult<bool>;
    fn is_file(&self, path: &str) -> FileSystemResult<bool>;
    fn is_directory(&self, path: &str) -> FileSystemResult<bool>;
    fn create_directory(&self, path: &str) -> FileSystemResult<()>;
    fn create_directory_all(&self, path: &str) -> FileSystemResult<()>;
    fn list_directory(&self, path: &str) -> FileSystemResult<Vec<String>>;
    fn remove_directory(&self, path: &str) -> FileSystemResult<()>;
    fn remove_directory_all(&self, path: &str) -> FileSystemResult<()>;
    
    // File operations
    fn create_file(&self, path: &str) -> FileSystemResult<Self::File>;
    fn open_file(&self, path: &str) -> FileSystemResult<Self::File>;
    fn remove_file(&self, path: &str) -> FileSystemResult<()>;
    fn filesize(&self, path: &str) -> FileSystemResult<u64>;
}
```

#### `File` Trait
Trait for file handles with read/write/seek capabilities:

```rust
pub trait File: Debug + Read + Write + Seek + Sync + Send + 'static {
    fn path(&self) -> &str;
    fn get_size(&self) -> FileSystemResult<u64>;
    fn set_size(&mut self, new_size: u64) -> FileSystemResult<()>;
    fn sync_all(&mut self) -> FileSystemResult<()>;
    fn sync_data(&mut self) -> FileSystemResult<()>;
    fn get_lock_status(&self) -> FileSystemResult<FileLockMode>;
    fn set_lock_status(&mut self, mode: FileLockMode) -> FileSystemResult<()>;
}
```

#### `DynamicFileSystem` Trait
Type-erased version of `FileSystem` for dynamic dispatch:

```rust
pub trait DynamicFileSystem: Debug + Sync + Send + 'static {
    fn create_file(&self, path: &str) -> FileSystemResult<Box<dyn File>>;
    fn open_file(&self, path: &str) -> FileSystemResult<Box<dyn File>>;
    // ... other methods
}
```

## Filesystem Implementations

### MemoryFileSystem
In-memory filesystem for testing and caching:

```rust
use nanograph_vfs::{FileSystem, MemoryFileSystem, File};

let fs = MemoryFileSystem::new();
fs.create_directory("/data")?;
let mut file = fs.create_file("/data/test.txt")?;
file.write_to_offset(0, b"Hello, VFS!")?;
```

### LocalFilesystem
OS filesystem access with a root directory:

```rust
use nanograph_vfs::{FileSystem, LocalFilesystem};

let fs = LocalFilesystem::new("/tmp/my_app");
fs.create_directory_all("/logs")?;
```

### MonitoredFilesystem
Wraps any filesystem to add metrics and logging:

```rust
use nanograph_vfs::{FileSystem, MemoryFileSystem, MonitoredFilesystem};

let inner = MemoryFileSystem::new();
let fs = MonitoredFilesystem::new(inner, "memory");

// All operations are now tracked with metrics
let mut file = fs.create_file("/test.txt")?;
file.write_all(b"data")?;

// Access statistics
println!("Bytes written: {}", fs.bytes_written());
println!("Open files: {:?}", fs.open_files());
```

### OverlayFilesystem
Layered filesystem with copy-on-write semantics:

```rust
use nanograph_vfs::{FileSystem, MemoryFileSystem, OverlayFilesystem};
use std::sync::Arc;

let upper = Arc::new(MemoryFileSystem::new());
let lower = Arc::new(MemoryFileSystem::new());

// Create read-only data in lower layer
lower.create_file("/config.toml")?;

let fs = OverlayFilesystem::new(
    vec![upper as Arc<_>, lower as Arc<_>].into_iter()
);

// Reads from lower, writes to upper
assert!(fs.exists("/config.toml")?);
fs.create_file("/new.txt")?; // Goes to upper layer
```

### MountableFilesystem
Mount different filesystems at different paths:

```rust
use nanograph_vfs::{FileSystem, MemoryFileSystem, MountableFilesystem};
use std::sync::Arc;

let mut fs = MountableFilesystem::new();
let mem_fs = Arc::new(MemoryFileSystem::new());

fs.mount("/tmp", mem_fs);
fs.create_file("/tmp/test.txt")?;
```

### FileSystemManager
Scheme-based filesystem routing:

```rust
use nanograph_vfs::{FileSystemManager, MemoryFileSystem};
use std::sync::Arc;

let manager = FileSystemManager::new();
let mem_fs = Arc::new(MemoryFileSystem::new());

manager.register("mem", mem_fs);

// Use scheme-based paths
manager.create_file("mem:///data/test.txt")?;
```

## Metrics Integration

The VFS crate integrates with the `metrics` crate to provide comprehensive observability.

### Available Metrics

#### Counters
- `nanograph_vfs_operations_total` - Total operations by type, filesystem, and status
- `nanograph_vfs_bytes_read_total` - Total bytes read
- `nanograph_vfs_bytes_written_total` - Total bytes written
- `nanograph_vfs_cache_accesses_total` - Cache hits/misses
- `nanograph_vfs_mount_operations_total` - Mount/unmount operations
- `nanograph_vfs_overlay_layer_accesses_total` - Layer access in overlay filesystems

#### Histograms
- `nanograph_vfs_operation_duration_microseconds` - Operation latency
- `nanograph_vfs_file_size_bytes` - File size distribution

#### Gauges
- `nanograph_vfs_open_files` - Currently open files

### Using Metrics

```rust
use nanograph_vfs::{FileSystem, MemoryFileSystem, MonitoredFilesystem};

// Wrap any filesystem with MonitoredFilesystem
let fs = MonitoredFilesystem::new(
    MemoryFileSystem::new(),
    "memory" // Filesystem type label
);

// All operations are automatically tracked
fs.create_file("/test.txt")?;

// Metrics are recorded via the metrics crate
// Configure your metrics backend (e.g., Prometheus, StatsD)
```

### Manual Metrics Recording

You can also use the metrics module directly:

```rust
use nanograph_vfs::metrics;

metrics::record_operation("custom_op", "my_fs", true);
metrics::record_bytes_written("my_fs", 1024);
metrics::record_operation_duration("read", "my_fs", 150);
```

## Pluggability

### Creating a Custom Filesystem

Implement the `FileSystem` and `File` traits:

```rust
use nanograph_vfs::{FileSystem, File, FileSystemResult};

#[derive(Debug)]
struct MyFilesystem {
    // Your implementation
}

#[derive(Debug)]
struct MyFile {
    // Your implementation
}

impl FileSystem for MyFilesystem {
    type File = MyFile;
    
    fn exists(&self, path: &str) -> FileSystemResult<bool> {
        // Your implementation
    }
    
    // Implement other methods...
}

impl File for MyFile {
    // Implement File trait methods...
}
```

### Using with Dynamic Dispatch

```rust
use nanograph_vfs::DynamicFileSystem;
use std::sync::Arc;

let filesystems: Vec<Arc<dyn DynamicFileSystem>> = vec![
    Arc::new(MemoryFileSystem::new()),
    Arc::new(LocalFilesystem::new("/tmp")),
];

for fs in filesystems {
    fs.create_file("/test.txt")?;
}
```

## Path Handling

The `Path` struct provides normalized path handling:

```rust
use nanograph_vfs::Path;

let path = Path::parse("/a/b/../c");
assert_eq!(path.to_string(), "/a/c");

// Scheme support
let path = Path::parse("mem:///data/file.txt");
assert_eq!(path.scheme, Some("mem".to_string()));

// Path manipulation
let mut path = Path::parse("/a/b");
path.push("c");
assert_eq!(path.to_string(), "/a/b/c");
```

## Error Handling

All operations return `FileSystemResult<T>` which is an alias for `Result<T, FileSystemError>`:

```rust
use nanograph_vfs::{FileSystemError, FileSystemResult};

fn my_operation() -> FileSystemResult<()> {
    // Operations that may fail
    Ok(())
}
```

## Testing

The crate includes a comprehensive test suite:

```rust
use nanograph_vfs::test_suite::run_generic_test_suite;

#[test]
fn test_my_filesystem() {
    let fs = MyFilesystem::new();
    run_generic_test_suite(fs);
}
```

## Performance Considerations

- **MemoryFileSystem**: Fastest, but volatile
- **LocalFilesystem**: OS-dependent performance
- **MonitoredFilesystem**: Small overhead for metrics collection
- **OverlayFilesystem**: Overhead for layer traversal
- **MountableFilesystem**: Overhead for mount point resolution

## Thread Safety

All filesystem implementations are `Send + Sync` and can be safely shared across threads:

```rust
use std::sync::Arc;
use std::thread;

let fs = Arc::new(MemoryFileSystem::new());
let fs_clone = fs.clone();

thread::spawn(move || {
    fs_clone.create_file("/thread_file.txt").unwrap();
});
```

## License

Licensed under the Apache License, Version 2.0.