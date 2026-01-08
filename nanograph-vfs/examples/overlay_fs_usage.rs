//! OverlayFilesystem usage example for Nanograph VFS
//!
//! This example demonstrates the overlay filesystem implementation,
//! which provides layered filesystem with copy-on-write semantics.

use nanograph_vfs::{DynamicFileSystem, File, MemoryFileSystem, OverlayFilesystem};
use std::io::{Read, Write};
use std::ops::Deref;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS OverlayFilesystem Example ===\n");

    // Create multiple filesystem layers
    let upper = Arc::new(MemoryFileSystem::new());
    let middle = Arc::new(MemoryFileSystem::new());
    let lower = Arc::new(MemoryFileSystem::new());

    println!("✓ Created three filesystem layers:");
    println!("  - Upper: Writable layer (top)");
    println!("  - Middle: Read-only layer");
    println!("  - Lower: Read-only layer (base)\n");

    // Populate the lower layer with base configuration
    println!("--- Populating Lower Layer (Base Configuration) ---");
    DynamicFileSystem::create_directory_all(lower.deref(), "/config")?;
    DynamicFileSystem::create_directory_all(lower.deref(), "/data")?;

    let mut config = DynamicFileSystem::create_file(lower.deref(), "/config/default.toml")?;
    config.write_all(
        br#"[app]
name = "MyApp"
version = "1.0.0"
debug = false

[database]
host = "localhost"
port = 5432
"#,
    )?;
    println!("✓ Created /config/default.toml in lower layer");

    let mut readme = DynamicFileSystem::create_file(lower.deref(), "/README.md")?;
    readme.write_all(b"# MyApp\n\nDefault configuration and documentation.\n")?;
    println!("✓ Created /README.md in lower layer");

    // Populate the middle layer with environment-specific overrides
    println!("\n--- Populating Middle Layer (Environment Overrides) ---");
    DynamicFileSystem::create_directory_all(middle.deref(), "/config")?;

    let mut env_config = DynamicFileSystem::create_file(middle.deref(), "/config/production.toml")?;
    env_config.write_all(
        br#"[app]
debug = false

[database]
host = "prod-db.example.com"
port = 5432
"#,
    )?;
    println!("✓ Created /config/production.toml in middle layer");

    let mut secrets = DynamicFileSystem::create_file(middle.deref(), "/config/secrets.toml")?;
    secrets.write_all(b"[database]\npassword = \"prod-secret\"\n")?;
    println!("✓ Created /config/secrets.toml in middle layer");

    // Create the overlay filesystem
    println!("\n--- Creating Overlay Filesystem ---");
    let overlay = OverlayFilesystem::new(
        vec![
            upper.clone() as Arc<dyn DynamicFileSystem>,
            middle.clone() as Arc<dyn DynamicFileSystem>,
            lower.clone() as Arc<dyn DynamicFileSystem>,
        ]
        .into_iter(),
    );
    println!("✓ Created overlay with 3 layers");
    println!("  Read order: upper → middle → lower");
    println!("  Write target: upper layer only\n");

    // Read files from different layers
    println!("--- Reading Files Through Overlay ---");

    // This file exists only in lower layer
    let mut file = DynamicFileSystem::open_file(&overlay, "/README.md")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read /README.md (from lower layer):");
    println!("{}", contents);

    // This file exists only in middle layer
    let mut file = DynamicFileSystem::open_file(&overlay, "/config/secrets.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("\nRead /config/secrets.toml (from middle layer):");
    println!("{}", contents);

    // List all visible files
    println!("\n--- Listing Files Through Overlay ---");
    println!("/config directory contains:");
    let entries = DynamicFileSystem::list_directory(&overlay, "/config")?;
    for entry in entries {
        println!("  - {}", entry);
    }

    // Demonstrate copy-on-write: modifying a file
    println!("\n--- Copy-on-Write Behavior ---");
    println!("Modifying /README.md (exists in lower layer)...");

    // Check where the file exists before modification
    println!("Before modification:");
    println!(
        "  Exists in upper: {}",
        DynamicFileSystem::exists(upper.deref(), "/README.md")?
    );
    println!(
        "  Exists in middle: {}",
        DynamicFileSystem::exists(middle.deref(), "/README.md")?
    );
    println!(
        "  Exists in lower: {}",
        DynamicFileSystem::exists(lower.deref(), "/README.md")?
    );

    // Modify through overlay - this should create a copy in upper layer
    let mut file = DynamicFileSystem::open_file(&overlay, "/README.md")?;
    file.write_all(b"\n## Modified\n\nThis file was modified through the overlay.\n")?;
    file.sync_all()?;
    println!("\n✓ Modified /README.md");

    println!("\nAfter modification:");
    println!(
        "  Exists in upper: {}",
        DynamicFileSystem::exists(upper.deref(), "/README.md")?
    );
    println!(
        "  Exists in middle: {}",
        DynamicFileSystem::exists(middle.deref(), "/README.md")?
    );
    println!(
        "  Exists in lower: {}",
        DynamicFileSystem::exists(lower.deref(), "/README.md")?
    );

    // Read the modified file
    let mut file = DynamicFileSystem::open_file(&overlay, "/README.md")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("\nModified content (from upper layer):");
    println!("{}", contents);

    // Original file in lower layer is unchanged
    let mut file = DynamicFileSystem::open_file(lower.deref(), "/README.md")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("\nOriginal content (still in lower layer):");
    println!("{}", contents);

    // Create a new file through overlay
    println!("\n--- Creating New Files ---");
    let mut new_file = DynamicFileSystem::create_file(&overlay, "/config/local.toml")?;
    new_file.write_all(
        br#"[app]
debug = true

[logging]
level = "debug"
"#,
    )?;
    new_file.sync_all()?;
    println!("✓ Created /config/local.toml");

    println!("\nFile location:");
    println!(
        "  Exists in upper: {}",
        DynamicFileSystem::exists(upper.deref(), "/config/local.toml")?
    );
    println!(
        "  Exists in middle: {}",
        DynamicFileSystem::exists(middle.deref(), "/config/local.toml")?
    );
    println!(
        "  Exists in lower: {}",
        DynamicFileSystem::exists(lower.deref(), "/config/local.toml")?
    );

    // Demonstrate layer priority
    println!("\n--- Layer Priority Demonstration ---");

    // Create a file with the same name in multiple layers
    let mut lower_file = DynamicFileSystem::create_file(lower.deref(), "/priority_test.txt")?;
    lower_file.write_all(b"Content from LOWER layer")?;

    let mut middle_file = DynamicFileSystem::create_file(middle.deref(), "/priority_test.txt")?;
    middle_file.write_all(b"Content from MIDDLE layer")?;

    let mut upper_file = DynamicFileSystem::create_file(upper.deref(), "/priority_test.txt")?;
    upper_file.write_all(b"Content from UPPER layer")?;

    println!("Created /priority_test.txt in all three layers");

    // Read through overlay - should get upper layer content
    let mut file = DynamicFileSystem::open_file(&overlay, "/priority_test.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("\nReading through overlay:");
    println!("  Result: {}", contents);
    println!("  (Upper layer takes priority)");

    // Demonstrate directory merging
    println!("\n--- Directory Merging ---");

    // Create different files in each layer's /data directory
    DynamicFileSystem::create_directory_all(lower.deref(), "/data")?;
    let mut f1 = DynamicFileSystem::create_file(lower.deref(), "/data/base.txt")?;
    f1.write_all(b"Base data")?;

    DynamicFileSystem::create_directory_all(middle.deref(), "/data")?;
    let mut f2 = DynamicFileSystem::create_file(middle.deref(), "/data/env.txt")?;
    f2.write_all(b"Environment data")?;

    DynamicFileSystem::create_directory_all(upper.deref(), "/data")?;
    let mut f3 = DynamicFileSystem::create_file(upper.deref(), "/data/local.txt")?;
    f3.write_all(b"Local data")?;

    println!("Created different files in /data across all layers");

    // List through overlay - should see merged view
    let entries = DynamicFileSystem::list_directory(&overlay, "/data")?;
    println!("\n/data directory (merged view):");
    for entry in entries {
        let path = format!("/data/{}", entry);
        let mut file = DynamicFileSystem::open_file(&overlay, &path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        println!("  - {}: {}", entry, contents);
    }

    // Demonstrate deletion behavior
    println!("\n--- Deletion Behavior ---");
    println!("Deleting /priority_test.txt through overlay...");
    DynamicFileSystem::remove_file(&overlay, "/priority_test.txt")?;

    println!("\nAfter deletion:");
    println!(
        "  Exists in upper: {}",
        DynamicFileSystem::exists(upper.deref(), "/priority_test.txt")?
    );
    println!(
        "  Exists in middle: {}",
        DynamicFileSystem::exists(middle.deref(), "/priority_test.txt")?
    );
    println!(
        "  Exists in lower: {}",
        DynamicFileSystem::exists(lower.deref(), "/priority_test.txt")?
    );
    println!(
        "  Visible through overlay: {}",
        DynamicFileSystem::exists(&overlay, "/priority_test.txt")?
    );

    // Use case: Configuration hierarchy
    println!("\n--- Use Case: Configuration Hierarchy ---");
    println!("Typical use case: Base config → Environment config → Local overrides");
    println!("\nConfiguration files visible through overlay:");
    let config_files = DynamicFileSystem::list_directory(&overlay, "/config")?;
    for file in config_files {
        println!("  - {}", file);
    }

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • OverlayFilesystem provides layered file access");
    println!("  • Reads check layers from top to bottom");
    println!("  • Writes always go to the top (upper) layer");
    println!("  • Copy-on-write: modifying lower files creates copies in upper");
    println!("  • Perfect for configuration hierarchies and containerization");
    println!("  • Directories are merged across all layers");

    Ok(())
}

// Made with Bob
