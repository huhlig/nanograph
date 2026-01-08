//! LocalFilesystem usage example for Nanograph VFS
//!
//! This example demonstrates the local filesystem implementation,
//! which provides access to the OS filesystem with a root directory.

use nanograph_vfs::{File, FileSystem, LocalFilesystem};
use std::io::{Read, Seek, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS LocalFilesystem Example ===\n");

    // Create a temporary directory for this example
    let temp_dir = std::env::temp_dir().join("nanograph_vfs_example");
    let root_path = temp_dir.to_str().unwrap();

    // Clean up any previous runs
    if std::path::Path::new(root_path).exists() {
        std::fs::remove_dir_all(root_path)?;
    }
    std::fs::create_dir_all(root_path)?;

    println!("Using root directory: {}", root_path);

    // Create a LocalFilesystem with the root directory
    let fs = LocalFilesystem::new(root_path);
    println!("✓ Created LocalFilesystem");
    println!("  - Persistent: Data survives program restart");
    println!("  - OS-backed: Uses native filesystem");
    println!("  - Sandboxed: All paths relative to root\n");

    // Create directory structure
    println!("--- Creating Directory Structure ---");
    fs.create_directory_all("/app/config")?;
    fs.create_directory_all("/app/data/users")?;
    fs.create_directory_all("/app/logs")?;
    fs.create_directory_all("/app/cache")?;
    println!("✓ Created directory structure");

    // Create configuration file
    println!("\n--- Creating Configuration Files ---");
    let mut config = fs.create_file("/app/config/app.toml")?;
    config.write_all(
        br#"[app]
name = "MyApplication"
version = "1.0.0"

[database]
host = "localhost"
port = 5432
name = "mydb"

[logging]
level = "info"
file = "/app/logs/app.log"
"#,
    )?;
    config.sync_all()?;
    println!("✓ Created /app/config/app.toml");

    // Create user data files
    println!("\n--- Creating User Data ---");
    let users = vec![
        (
            "alice",
            r#"{"id": 1, "name": "Alice", "email": "alice@example.com"}"#,
        ),
        (
            "bob",
            r#"{"id": 2, "name": "Bob", "email": "bob@example.com"}"#,
        ),
        (
            "charlie",
            r#"{"id": 3, "name": "Charlie", "email": "charlie@example.com"}"#,
        ),
    ];

    for (username, data) in users {
        let path = format!("/app/data/users/{}.json", username);
        let mut file = fs.create_file(&path)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
        println!("✓ Created {}", path);
    }

    // Create log file
    println!("\n--- Creating Log File ---");
    let mut log = fs.create_file("/app/logs/app.log")?;
    log.write_all(b"[2024-01-01 10:00:00] INFO: Application started\n")?;
    log.write_all(b"[2024-01-01 10:00:01] INFO: Configuration loaded\n")?;
    log.write_all(b"[2024-01-01 10:00:02] INFO: Database connected\n")?;
    log.sync_all()?;
    println!("✓ Created /app/logs/app.log");

    // Read configuration file
    println!("\n--- Reading Configuration ---");
    let mut config = fs.open_file("/app/config/app.toml")?;
    let mut contents = String::new();
    config.read_to_string(&mut contents)?;
    println!("Configuration file contents:");
    println!("{}", contents);

    // List directory contents
    println!("\n--- Listing Directory Contents ---");
    let users = fs.list_directory("/app/data/users")?;
    println!("/app/data/users contains {} files:", users.len());
    for user in users {
        let path = format!("/app/data/users/{}", user);
        let size = fs.filesize(&path)?;
        println!("  - {} ({} bytes)", user, size);
    }

    // Demonstrate file metadata
    println!("\n--- File Metadata ---");
    let paths = vec![
        "/app/config/app.toml",
        "/app/data/users/alice.json",
        "/app/logs/app.log",
    ];

    for path in paths {
        println!("{}:", path);
        println!("  Exists: {}", fs.exists(path)?);
        println!("  Is file: {}", fs.is_file(path)?);
        println!("  Is directory: {}", fs.is_directory(path)?);
        println!("  Size: {} bytes", fs.filesize(path)?);
    }

    // Append to log file
    println!("\n--- Appending to Log File ---");
    let mut log = fs.open_file("/app/logs/app.log")?;
    log.seek(std::io::SeekFrom::End(0))?;
    log.write_all(b"[2024-01-01 10:00:03] INFO: Processing user requests\n")?;
    log.write_all(b"[2024-01-01 10:00:04] INFO: Cache updated\n")?;
    log.sync_all()?;
    println!("✓ Appended to log file");

    // Read updated log file
    let mut log = fs.open_file("/app/logs/app.log")?;
    let mut contents = String::new();
    log.read_to_string(&mut contents)?;
    println!("\nUpdated log file:");
    println!("{}", contents);

    // Demonstrate file operations
    println!("\n--- File Operations ---");

    // Copy a file (manual implementation)
    let mut source = fs.open_file("/app/data/users/alice.json")?;
    let mut buffer = Vec::new();
    source.read_to_end(&mut buffer)?;

    let mut dest = fs.create_file("/app/cache/alice_backup.json")?;
    dest.write_all(&buffer)?;
    dest.sync_all()?;
    println!("✓ Copied alice.json to cache/alice_backup.json");

    // Verify the copy
    let original_size = fs.filesize("/app/data/users/alice.json")?;
    let copy_size = fs.filesize("/app/cache/alice_backup.json")?;
    println!("  Original size: {} bytes", original_size);
    println!("  Copy size: {} bytes", copy_size);
    println!("  Sizes match: {}", original_size == copy_size);

    // Delete a file
    println!("\n--- Deleting Files ---");
    fs.remove_file("/app/cache/alice_backup.json")?;
    println!("✓ Deleted /app/cache/alice_backup.json");
    println!(
        "  File exists: {}",
        fs.exists("/app/cache/alice_backup.json")?
    );

    // Recursive directory listing
    println!("\n--- Recursive Directory Listing ---");
    fn list_recursive(
        fs: &LocalFilesystem,
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

    // Demonstrate persistence
    println!("\n--- Demonstrating Persistence ---");
    println!("Creating a test file...");
    let mut test = fs.create_file("/app/persistence_test.txt")?;
    test.write_all(b"This data persists across program runs")?;
    test.sync_all()?;
    println!("✓ Created /app/persistence_test.txt");

    // Drop the file handle
    drop(test);

    // Create a new filesystem instance pointing to the same root
    let fs2 = LocalFilesystem::new(root_path);
    println!("\n✓ Created new LocalFilesystem instance");

    // Verify the file still exists
    println!("Checking if file exists in new instance...");
    if fs2.exists("/app/persistence_test.txt")? {
        let mut file = fs2.open_file("/app/persistence_test.txt")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        println!("✓ File exists and contains: {}", contents);
    }

    // Clean up
    println!("\n--- Cleanup ---");
    fs.remove_directory_all("/app")?;
    println!("✓ Removed /app directory");

    // Remove the temporary directory
    std::fs::remove_dir_all(root_path)?;
    println!("✓ Removed temporary directory");

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • LocalFilesystem provides persistent storage");
    println!("  • All paths are relative to the root directory");
    println!("  • Data survives program restarts");
    println!("  • Uses native OS filesystem for performance");
    println!("  • Sandboxed access prevents escaping root directory");

    Ok(())
}

// Made with Bob
