//! FileSystemManager usage example for Nanograph VFS
//!
//! This example demonstrates the filesystem manager implementation,
//! which provides scheme-based routing to different filesystems.

use nanograph_vfs::{File, FileSystem, FileSystemManager, LocalFilesystem, MemoryFileSystem};
use std::io::{Read, Write};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS FileSystemManager Example ===\n");

    // Create the filesystem manager
    let manager = FileSystemManager::new();
    println!("✓ Created FileSystemManager");
    println!("  - Routes filesystem operations based on URI schemes");
    println!("  - Similar to protocol handlers in web browsers\n");

    // Register different filesystems with different schemes
    println!("--- Registering Filesystems ---");

    // Memory filesystem for temporary/cache data
    let mem_fs = Arc::new(MemoryFileSystem::new());
    manager.register("mem", mem_fs.clone());
    println!("✓ Registered MemoryFileSystem with scheme 'mem://'");

    // Another memory filesystem for testing
    let test_fs = Arc::new(MemoryFileSystem::new());
    manager.register("test", test_fs.clone());
    println!("✓ Registered test filesystem with scheme 'test://'");

    // Local filesystem for persistent storage
    let temp_dir = std::env::temp_dir().join("nanograph_manager_example");
    let temp_path = temp_dir.to_str().unwrap();
    if std::path::Path::new(temp_path).exists() {
        std::fs::remove_dir_all(temp_path)?;
    }
    std::fs::create_dir_all(temp_path)?;

    let local_fs = Arc::new(LocalFilesystem::new(temp_path));
    manager.register("file", local_fs.clone());
    println!("✓ Registered LocalFilesystem with scheme 'file://'");

    // Demonstrate scheme-based access
    println!("\n--- Scheme-Based File Access ---");

    // Create files in different filesystems using schemes
    let mut mem_file = manager.create_file("mem:///cache/data.txt")?;
    mem_file.write_all(b"Cached data in memory")?;
    println!("✓ Created mem:///cache/data.txt");

    let mut test_file = manager.create_file("test:///temp/test.txt")?;
    test_file.write_all(b"Test data")?;
    println!("✓ Created test:///temp/test.txt");

    let mut local_file = manager.create_file("file:///persistent/config.json")?;
    local_file.write_all(br#"{"version": "1.0", "persistent": true}"#)?;
    local_file.sync_all()?;
    println!("✓ Created file:///persistent/config.json");

    // Read files using schemes
    println!("\n--- Reading Files by Scheme ---");

    let mut file = manager.open_file("mem:///cache/data.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("mem:///cache/data.txt: {}", contents);

    let mut file = manager.open_file("test:///temp/test.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("test:///temp/test.txt: {}", contents);

    let mut file = manager.open_file("file:///persistent/config.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("file:///persistent/config.json: {}", contents);

    // Demonstrate directory operations with schemes
    println!("\n--- Directory Operations ---");

    manager.create_directory_all("mem:///app/data")?;
    manager.create_directory_all("mem:///app/logs")?;
    println!("✓ Created directories in mem://");

    let entries = manager.list_directory("mem:///app")?;
    println!("mem:///app contains:");
    for entry in entries {
        println!("  - {}", entry);
    }

    // Use case: Configuration hierarchy
    println!("\n--- Use Case: Configuration Hierarchy ---");

    // Default config in memory
    let mut default_config = manager.create_file("mem:///config/default.toml")?;
    default_config.write_all(
        br#"[app]
name = "MyApp"
debug = false
port = 8080
"#,
    )?;
    println!("✓ Created default config in mem://");

    // User config in local filesystem
    let mut user_config = manager.create_file("file:///config/user.toml")?;
    user_config.write_all(
        br#"[app]
debug = true
port = 3000
"#,
    )?;
    user_config.sync_all()?;
    println!("✓ Created user config in file://");

    // Application can load configs from different sources
    println!("\nLoading configuration from multiple sources:");

    let mut file = manager.open_file("mem:///config/default.toml")?;
    let mut default = String::new();
    file.read_to_string(&mut default)?;
    println!("Default config (mem://):\n{}", default);

    let mut file = manager.open_file("file:///config/user.toml")?;
    let mut user = String::new();
    file.read_to_string(&mut user)?;
    println!("User config (file://):\n{}", user);

    // Use case: Multi-environment deployment
    println!("\n--- Use Case: Multi-Environment Deployment ---");

    // Development environment
    let dev_fs = Arc::new(MemoryFileSystem::new());
    manager.register("dev", dev_fs.clone());

    let mut dev_config = manager.create_file("dev:///config/app.toml")?;
    dev_config.write_all(b"[env]\nname = \"development\"\nlog_level = \"debug\"\n")?;
    println!("✓ Registered dev:// environment");

    // Production environment
    let prod_fs = Arc::new(MemoryFileSystem::new());
    manager.register("prod", prod_fs.clone());

    let mut prod_config = manager.create_file("prod:///config/app.toml")?;
    prod_config.write_all(b"[env]\nname = \"production\"\nlog_level = \"error\"\n")?;
    println!("✓ Registered prod:// environment");

    // Application can switch between environments by changing scheme
    let environments = vec!["dev", "prod"];
    println!("\nEnvironment configurations:");
    for env in environments {
        let path = format!("{}:///config/app.toml", env);
        let mut file = manager.open_file(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        println!("\n{}:\n{}", env, contents);
    }

    // Use case: Plugin system with isolated storage
    println!("\n--- Use Case: Plugin System ---");

    // Each plugin gets its own filesystem
    let plugin1_fs = Arc::new(MemoryFileSystem::new());
    let plugin2_fs = Arc::new(MemoryFileSystem::new());

    manager.register("plugin1", plugin1_fs.clone());
    manager.register("plugin2", plugin2_fs.clone());

    // Plugin 1 stores its data
    let mut p1_data = manager.create_file("plugin1:///data/state.json")?;
    p1_data.write_all(br#"{"plugin": "Plugin1", "enabled": true}"#)?;
    println!("✓ Plugin1 stored data in plugin1://");

    // Plugin 2 stores its data
    let mut p2_data = manager.create_file("plugin2:///data/state.json")?;
    p2_data.write_all(br#"{"plugin": "Plugin2", "enabled": true}"#)?;
    println!("✓ Plugin2 stored data in plugin2://");

    // Plugins are isolated - they can't access each other's data
    println!("\nPlugin isolation:");
    println!(
        "  plugin1:///data/state.json exists: {}",
        manager.exists("plugin1:///data/state.json")?
    );
    println!(
        "  plugin2:///data/state.json exists: {}",
        manager.exists("plugin2:///data/state.json")?
    );

    // Use case: Resource management
    println!("\n--- Use Case: Resource Management ---");

    // Assets in memory for fast access
    let assets_fs = Arc::new(MemoryFileSystem::new());
    manager.register("assets", assets_fs.clone());

    let mut texture = manager.create_file("assets:///textures/player.png")?;
    texture.write_all(b"<binary texture data>")?;
    println!("✓ Loaded texture into assets://");

    let mut sound = manager.create_file("assets:///sounds/jump.wav")?;
    sound.write_all(b"<binary sound data>")?;
    println!("✓ Loaded sound into assets://");

    // User data in persistent storage
    let mut save = manager.create_file("file:///saves/game1.sav")?;
    save.write_all(b"<save game data>")?;
    save.sync_all()?;
    println!("✓ Saved game to file://");

    println!("\nResource locations:");
    println!("  Fast assets: assets:// (memory)");
    println!("  Persistent saves: file:// (disk)");

    // Demonstrate unregistering a scheme
    println!("\n--- Unregistering Schemes ---");
    println!("Before unregister:");
    println!(
        "  test:///temp/test.txt exists: {}",
        manager.exists("test:///temp/test.txt")?
    );

    manager.deregister("test");
    println!("\n✓ Unregistered 'test' scheme");

    println!("\nAfter unregister:");
    match manager.exists("test:///temp/test.txt") {
        Ok(_) => println!("  Unexpected success"),
        Err(e) => println!("  Expected error: {}", e),
    }

    // List registered schemes
    println!("\n--- Registered Schemes ---");
    let schemes = manager.list_schemes();
    println!("Currently registered schemes:");
    for scheme in schemes {
        println!("  - {}://", scheme);
    }

    // Demonstrate default scheme
    println!("\n--- Default Scheme ---");
    manager.set_default_scheme("mem");
    println!("✓ Set default scheme to 'mem'");

    // Can now use paths without scheme prefix
    let mut file = manager.create_file("/default/test.txt")?;
    file.write_all(b"Using default scheme")?;
    println!("✓ Created /default/test.txt (uses mem:// by default)");

    // Verify it's in the memory filesystem
    println!(
        "  File exists in mem://: {}",
        mem_fs.exists("/default/test.txt")?
    );

    // Use case: Testing with mock filesystems
    println!("\n--- Use Case: Testing ---");

    let mock_fs = Arc::new(MemoryFileSystem::new());
    manager.register("mock", mock_fs.clone());

    // Set up test data
    let mut test_data = manager.create_file("mock:///test/input.txt")?;
    test_data.write_all(b"Test input data")?;
    println!("✓ Set up test data in mock://");

    // Test code can use mock:// scheme
    let mut file = manager.open_file("mock:///test/input.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Test read: {}", contents);

    // Clean up test data
    manager.remove_file("mock:///test/input.txt")?;
    println!("✓ Cleaned up test data");

    // Clean up
    println!("\n--- Cleanup ---");
    std::fs::remove_dir_all(temp_path)?;
    println!("✓ Removed temporary directory");

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • FileSystemManager routes operations by URI scheme");
    println!("  • Multiple filesystems can coexist with different schemes");
    println!("  • Perfect for multi-environment and plugin architectures");
    println!("  • Provides isolation between different storage contexts");
    println!("  • Supports default scheme for convenience");
    println!("  • Schemes can be registered and unregistered dynamically");
    println!("  • Similar to protocol handlers in web browsers");

    Ok(())
}

// Made with Bob
