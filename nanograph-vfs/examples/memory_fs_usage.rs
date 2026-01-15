//! MemoryFileSystem usage example for Nanograph VFS
//!
//! This example demonstrates the in-memory filesystem implementation,
//! which is ideal for testing, caching, and temporary storage.

use nanograph_vfs::{File, FileSystem, MemoryFileSystem};
use std::io::{Read, Seek, SeekFrom, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS MemoryFileSystem Example ===\n");

    // Create a new in-memory filesystem
    let fs = MemoryFileSystem::new();
    println!("✓ Created MemoryFileSystem");
    println!("  - Fast: All operations in RAM");
    println!("  - Volatile: Data lost when dropped");
    println!("  - Thread-safe: Can be shared across threads\n");

    // Demonstrate fast file creation
    println!("--- Fast File Operations ---");
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let path = format!("/file_{}.txt", i);
        let mut file = fs.create_file(&path)?;
        file.write_all(format!("Content {}", i).as_bytes())?;
    }
    let elapsed = start.elapsed();
    println!("✓ Created 1000 files in {:?}", elapsed);

    // Demonstrate directory structure
    println!("\n--- Complex Directory Structure ---");
    fs.create_directory_all("/app/config")?;
    fs.create_directory_all("/app/data/cache")?;
    fs.create_directory_all("/app/logs/2024/01")?;
    fs.create_directory_all("/app/temp")?;
    println!("✓ Created nested directory structure");

    // Create configuration files
    let mut config = fs.create_file("/app/config/settings.json")?;
    config.write_all(
        br#"{
  "app_name": "MyApp",
  "version": "1.0.0",
  "debug": true,
  "max_connections": 100
}"#,
    )?;
    println!("✓ Created /app/config/settings.json");

    // Create data files
    let mut data = fs.create_file("/app/data/cache/user_123.json")?;
    data.write_all(br#"{"id": 123, "name": "Alice", "active": true}"#)?;
    println!("✓ Created /app/data/cache/user_123.json");

    // Create log files
    let mut log = fs.create_file("/app/logs/2024/01/app.log")?;
    log.write_all(b"[2024-01-01 10:00:00] INFO: Application started\n")?;
    log.write_all(b"[2024-01-01 10:00:01] INFO: Loading configuration\n")?;
    log.write_all(b"[2024-01-01 10:00:02] INFO: Connecting to database\n")?;
    println!("✓ Created /app/logs/2024/01/app.log");

    // Demonstrate file seeking and random access
    println!("\n--- Random Access Operations ---");
    let mut file = fs.open_file("/app/config/settings.json")?;

    // Seek to different positions
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0u8; 10];
    file.read_exact(&mut buffer)?;
    println!("First 10 bytes: {}", String::from_utf8_lossy(&buffer));

    file.seek(SeekFrom::Start(20))?;
    let mut buffer = vec![0u8; 10];
    file.read_exact(&mut buffer)?;
    println!("Bytes 20-30: {}", String::from_utf8_lossy(&buffer));

    // Get file size
    let size = file.get_size()?;
    println!("File size: {} bytes", size);

    // Seek from end
    file.seek(SeekFrom::End(-10))?;
    let mut buffer = vec![0u8; 10];
    file.read_exact(&mut buffer)?;
    println!("Last 10 bytes: {}", String::from_utf8_lossy(&buffer));

    // Demonstrate file truncation and resizing
    println!("\n--- File Truncation and Resizing ---");
    let mut file = fs.create_file("/app/temp/test.dat")?;
    file.write_all(b"0123456789ABCDEFGHIJ")?;
    println!("Original size: {} bytes", file.get_size()?);

    file.set_size(10)?;
    println!("After truncate to 10: {} bytes", file.get_size()?);

    file.set_size(20)?;
    println!("After extend to 20: {} bytes", file.get_size()?);

    // Demonstrate offset-based I/O
    println!("\n--- Offset-Based I/O ---");
    let mut file = fs.create_file("/app/temp/random_access.dat")?;
    file.write_all(b"AAAAAAAAAA")?; // 10 A's

    // Write at specific offsets without changing cursor
    file.write_to_offset(2, b"BB")?;
    file.write_to_offset(5, b"CCC")?;
    file.write_to_offset(9, b"D")?;

    // Read entire file
    file.seek(SeekFrom::Start(0))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("After offset writes: {}", contents);

    // Read at specific offset
    let mut buffer = vec![0u8; 3];
    file.read_at_offset(5, &mut buffer)?;
    println!(
        "Read 3 bytes at offset 5: {}",
        String::from_utf8_lossy(&buffer)
    );

    // List all files in a directory recursively
    println!("\n--- Directory Listing ---");
    fn list_recursive(
        fs: &MemoryFileSystem,
        path: &str,
        indent: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entries = fs.list_directory(path)?;
        for entry in entries {
            let full_path = format!("{}/{}", path, entry);
            let prefix = "  ".repeat(indent);
            if fs.is_directory(&full_path)? {
                println!("{}📁 {}/", prefix, entry);
                list_recursive(fs, &full_path, indent + 1)?;
            } else {
                let size = fs.filesize(&full_path)?;
                println!("{}📄 {} ({} bytes)", prefix, entry, size);
            }
        }
        Ok(())
    }

    println!("/app structure:");
    list_recursive(&fs, "/app", 1)?;

    // Demonstrate file deletion
    println!("\n--- File Deletion ---");
    let temp_files = fs.list_directory("/app/temp")?;
    println!("Files in /app/temp before deletion: {}", temp_files.len());

    for file in temp_files {
        fs.remove_file(&format!("/app/temp/{}", file))?;
    }

    let temp_files = fs.list_directory("/app/temp")?;
    println!("Files in /app/temp after deletion: {}", temp_files.len());

    // Demonstrate directory deletion
    println!("\n--- Directory Deletion ---");
    fs.remove_directory_all("/app/logs")?;
    println!("✓ Deleted /app/logs recursively");
    println!("/app/logs exists: {}", fs.exists("/app/logs")?);

    // Performance comparison
    println!("\n--- Performance Characteristics ---");
    let start = std::time::Instant::now();
    for i in 0..10000 {
        let path = format!("/perf_test_{}.dat", i);
        let mut file = fs.create_file(&path)?;
        file.write_all(b"test data")?;
    }
    let create_time = start.elapsed();

    let start = std::time::Instant::now();
    for i in 0..10000 {
        let path = format!("/perf_test_{}.dat", i);
        let mut file = fs.open_file(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
    }
    let read_time = start.elapsed();

    let start = std::time::Instant::now();
    for i in 0..10000 {
        let path = format!("/perf_test_{}.dat", i);
        fs.remove_file(&path)?;
    }
    let delete_time = start.elapsed();

    println!("Created 10,000 files in: {:?}", create_time);
    println!("Read 10,000 files in: {:?}", read_time);
    println!("Deleted 10,000 files in: {:?}", delete_time);

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • MemoryFileSystem is extremely fast");
    println!("  • Perfect for testing and temporary storage");
    println!("  • All data is lost when the filesystem is dropped");
    println!("  • Thread-safe and can be shared via Arc");

    Ok(())
}
