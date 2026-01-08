//! Basic usage example for Nanograph VFS
//!
//! This example demonstrates the core functionality of the Virtual File System,
//! including file and directory operations using MemoryFileSystem.

use nanograph_vfs::{File, FileSystem, MemoryFileSystem};
use std::io::{Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS Basic Usage Example ===\n");

    // Create a new in-memory filesystem
    let fs = MemoryFileSystem::new();
    println!("✓ Created new MemoryFileSystem");

    // Create directories
    println!("\n--- Creating Directories ---");
    fs.create_directory("/data")?;
    fs.create_directory_all("/logs/2024/01")?;
    println!("✓ Created /data");
    println!("✓ Created /logs/2024/01 (with parents)");

    // Check if paths exist
    println!("\n--- Checking Paths ---");
    println!("/data exists: {}", fs.exists("/data")?);
    println!("/data is directory: {}", fs.is_directory("/data")?);
    println!("/logs/2024/01 exists: {}", fs.exists("/logs/2024/01")?);

    // Create and write to files
    println!("\n--- Creating and Writing Files ---");
    let mut file1 = fs.create_file("/data/hello.txt")?;
    file1.write_all(b"Hello, VFS!")?;
    file1.sync_all()?;
    println!("✓ Created /data/hello.txt");

    let mut file2 = fs.create_file("/data/config.json")?;
    file2.write_all(br#"{"version": "1.0", "enabled": true}"#)?;
    file2.sync_all()?;
    println!("✓ Created /data/config.json");

    let mut file3 = fs.create_file("/logs/2024/01/app.log")?;
    file3.write_all(b"[INFO] Application started\n")?;
    file3.write_all(b"[INFO] Processing data\n")?;
    file3.sync_all()?;
    println!("✓ Created /logs/2024/01/app.log");

    // Read files
    println!("\n--- Reading Files ---");
    let mut file = fs.open_file("/data/hello.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("/data/hello.txt: {}", contents);

    let mut file = fs.open_file("/data/config.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("/data/config.json: {}", contents);

    // Get file size
    println!("\n--- File Sizes ---");
    println!("/data/hello.txt: {} bytes", fs.filesize("/data/hello.txt")?);
    println!(
        "/data/config.json: {} bytes",
        fs.filesize("/data/config.json")?
    );

    // List directory contents
    println!("\n--- Listing Directory Contents ---");
    let entries = fs.list_directory("/data")?;
    println!("/data contains:");
    for entry in entries {
        println!("  - {}", entry);
    }

    // Use read_at_offset and write_to_offset
    println!("\n--- Random Access Operations ---");
    let mut file = fs.open_file("/data/hello.txt")?;
    let mut buffer = vec![0u8; 5];
    file.read_at_offset(7, &mut buffer)?;
    println!(
        "Read 5 bytes at offset 7: {}",
        String::from_utf8_lossy(&buffer)
    );

    file.write_to_offset(0, b"Greetings")?;
    file.sync_all()?;
    println!("✓ Modified file at offset 0");

    // Read modified content
    let mut file = fs.open_file("/data/hello.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Modified content: {}", contents);

    // Delete a file
    println!("\n--- Deleting Files ---");
    fs.remove_file("/data/config.json")?;
    println!("✓ Deleted /data/config.json");
    println!(
        "/data/config.json exists: {}",
        fs.exists("/data/config.json")?
    );

    // Delete a directory
    println!("\n--- Deleting Directories ---");
    fs.remove_directory_all("/logs")?;
    println!("✓ Deleted /logs (recursive)");
    println!("/logs exists: {}", fs.exists("/logs")?);

    // Final directory listing
    println!("\n--- Final State ---");
    let entries = fs.list_directory("/data")?;
    println!("/data contains:");
    for entry in entries {
        println!("  - {}", entry);
    }

    println!("\n=== Example Complete ===");
    Ok(())
}

// Made with Bob
