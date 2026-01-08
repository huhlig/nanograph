//! MountableFilesystem usage example for Nanograph VFS
//!
//! This example demonstrates the mountable filesystem implementation,
//! which allows mounting different filesystems at different paths.

use nanograph_vfs::{FileSystem, MemoryFileSystem, MountableFilesystem};
use std::io::{Read, Write};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS MountableFilesystem Example ===\n");

    // Create the main mountable filesystem
    let mut fs = MountableFilesystem::new();
    println!("✓ Created MountableFilesystem");
    println!("  - Allows mounting different filesystems at different paths");
    println!("  - Similar to Unix mount points\n");

    // Create separate filesystems for different purposes
    let system_fs = Arc::new(MemoryFileSystem::new());
    let user_fs = Arc::new(MemoryFileSystem::new());
    let temp_fs = Arc::new(MemoryFileSystem::new());
    let cache_fs = Arc::new(MemoryFileSystem::new());

    println!("✓ Created 4 separate filesystems:");
    println!("  - system_fs: For system files");
    println!("  - user_fs: For user data");
    println!("  - temp_fs: For temporary files");
    println!("  - cache_fs: For cache data\n");

    // Mount filesystems at different paths
    println!("--- Mounting Filesystems ---");
    fs.mount("/system", system_fs.clone());
    println!("✓ Mounted system_fs at /system");

    fs.mount("/home", user_fs.clone());
    println!("✓ Mounted user_fs at /home");

    fs.mount("/tmp", temp_fs.clone());
    println!("✓ Mounted temp_fs at /tmp");

    fs.mount("/var/cache", cache_fs.clone());
    println!("✓ Mounted cache_fs at /var/cache");

    // Populate system filesystem
    println!("\n--- Populating System Filesystem ---");
    system_fs.create_directory_all("/config")?;
    let mut config = system_fs.create_file("/config/system.conf")?;
    config.write_all(b"# System Configuration\nversion=1.0\n")?;
    println!("✓ Created /system/config/system.conf");

    // Populate user filesystem
    println!("\n--- Populating User Filesystem ---");
    user_fs.create_directory_all("/alice/documents")?;
    user_fs.create_directory_all("/bob/documents")?;

    let mut alice_doc = user_fs.create_file("/alice/documents/notes.txt")?;
    alice_doc.write_all(b"Alice's personal notes\n")?;
    println!("✓ Created /home/alice/documents/notes.txt");

    let mut bob_doc = user_fs.create_file("/bob/documents/todo.txt")?;
    bob_doc.write_all(b"Bob's todo list\n")?;
    println!("✓ Created /home/bob/documents/todo.txt");

    // Populate temp filesystem
    println!("\n--- Populating Temp Filesystem ---");
    temp_fs.create_directory_all("/session_123")?;
    let mut temp_file = temp_fs.create_file("/session_123/data.tmp")?;
    temp_file.write_all(b"Temporary session data\n")?;
    println!("✓ Created /tmp/session_123/data.tmp");

    // Populate cache filesystem
    println!("\n--- Populating Cache Filesystem ---");
    cache_fs.create_directory_all("/web")?;
    let mut cache_file = cache_fs.create_file("/web/page_123.html")?;
    cache_file.write_all(b"<html>Cached page</html>\n")?;
    println!("✓ Created /var/cache/web/page_123.html");

    // Access files through mount points
    println!("\n--- Accessing Files Through Mount Points ---");

    let mut file = fs.open_file("/system/config/system.conf")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read /system/config/system.conf:");
    println!("{}", contents);

    let mut file = fs.open_file("/home/alice/documents/notes.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read /home/alice/documents/notes.txt:");
    println!("{}", contents);

    // List directory across mount points
    println!("\n--- Listing Directories ---");

    println!("/home contents:");
    let entries = fs.list_directory("/home")?;
    for entry in entries {
        println!("  - {}", entry);
    }

    println!("\n/home/alice contents:");
    let entries = fs.list_directory("/home/alice")?;
    for entry in entries {
        println!("  - {}", entry);
    }

    // Demonstrate mount point isolation
    println!("\n--- Mount Point Isolation ---");
    println!("Each mounted filesystem is independent:");

    // Create file in system_fs directly
    let mut direct_file = system_fs.create_file("/direct.txt")?;
    direct_file.write_all(b"Created directly in system_fs")?;

    // Access through mount point
    let mut mounted_file = fs.open_file("/system/direct.txt")?;
    let mut contents = String::new();
    mounted_file.read_to_string(&mut contents)?;
    println!("✓ File created in system_fs is accessible at /system/direct.txt");
    println!("  Content: {}", contents);

    // Demonstrate nested mount points
    println!("\n--- Nested Mount Points ---");
    let nested_fs = Arc::new(MemoryFileSystem::new());
    nested_fs.create_directory_all("/data")?;
    let mut nested_file = nested_fs.create_file("/data/nested.txt")?;
    nested_file.write_all(b"Data in nested mount")?;

    fs.mount("/home/alice/special", nested_fs.clone());
    println!("✓ Mounted nested_fs at /home/alice/special");

    let mut file = fs.open_file("/home/alice/special/data/nested.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read /home/alice/special/data/nested.txt:");
    println!("{}", contents);

    // Demonstrate unmounting
    println!("\n--- Unmounting Filesystems ---");
    println!("Before unmount:");
    println!("  /tmp exists: {}", fs.exists("/tmp")?);
    println!(
        "  /tmp/session_123/data.tmp exists: {}",
        fs.exists("/tmp/session_123/data.tmp")?
    );

    fs.unmount("/tmp");
    println!("\n✓ Unmounted /tmp");

    println!("\nAfter unmount:");
    println!("  /tmp exists: {}", fs.exists("/tmp")?);

    // The underlying filesystem still has the data
    println!(
        "  Data still exists in temp_fs: {}",
        temp_fs.exists("/session_123/data.tmp")?
    );

    // Re-mount at a different location
    println!("\n--- Re-mounting at Different Location ---");
    fs.mount("/temporary", temp_fs.clone());
    println!("✓ Re-mounted temp_fs at /temporary");
    println!(
        "  /temporary/session_123/data.tmp exists: {}",
        fs.exists("/temporary/session_123/data.tmp")?
    );

    // Demonstrate mount point precedence
    println!("\n--- Mount Point Precedence ---");
    let override_fs = Arc::new(MemoryFileSystem::new());
    override_fs.create_directory_all("/data")?;
    let mut override_file = override_fs.create_file("/data/file.txt")?;
    override_file.write_all(b"Override content")?;

    // Mount at a more specific path
    fs.mount("/home/alice/documents", override_fs.clone());
    println!("✓ Mounted override_fs at /home/alice/documents");

    // More specific mount takes precedence
    let mut file = fs.open_file("/home/alice/documents/data/file.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read /home/alice/documents/data/file.txt:");
    println!("{}", contents);

    // Use case: Multi-tenant application
    println!("\n--- Use Case: Multi-Tenant Application ---");
    let tenant1_fs = Arc::new(MemoryFileSystem::new());
    let tenant2_fs = Arc::new(MemoryFileSystem::new());

    // Set up tenant 1
    tenant1_fs.create_directory_all("/data")?;
    let mut t1_file = tenant1_fs.create_file("/data/config.json")?;
    t1_file.write_all(br#"{"tenant": "Company A", "plan": "premium"}"#)?;

    // Set up tenant 2
    tenant2_fs.create_directory_all("/data")?;
    let mut t2_file = tenant2_fs.create_file("/data/config.json")?;
    t2_file.write_all(br#"{"tenant": "Company B", "plan": "basic"}"#)?;

    // Mount each tenant's filesystem
    fs.mount("/tenants/company-a", tenant1_fs);
    fs.mount("/tenants/company-b", tenant2_fs);

    println!("✓ Mounted tenant filesystems");
    println!("\nTenant A config:");
    let mut file = fs.open_file("/tenants/company-a/data/config.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("{}", contents);

    println!("\nTenant B config:");
    let mut file = fs.open_file("/tenants/company-b/data/config.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("{}", contents);

    // Demonstrate mount point listing
    println!("\n--- Listing Mount Points ---");
    println!("Current mount points:");
    let mounts = fs.list_mounts();
    for mount in mounts {
        println!("  - {}", mount);
    }

    // Use case: Plugin system
    println!("\n--- Use Case: Plugin System ---");
    let plugin1_fs = Arc::new(MemoryFileSystem::new());
    let plugin2_fs = Arc::new(MemoryFileSystem::new());

    plugin1_fs.create_directory_all("/resources")?;
    let mut p1_manifest = plugin1_fs.create_file("/manifest.json")?;
    p1_manifest.write_all(br#"{"name": "Plugin1", "version": "1.0"}"#)?;

    plugin2_fs.create_directory_all("/resources")?;
    let mut p2_manifest = plugin2_fs.create_file("/manifest.json")?;
    p2_manifest.write_all(br#"{"name": "Plugin2", "version": "2.0"}"#)?;

    fs.mount("/plugins/plugin1", plugin1_fs);
    fs.mount("/plugins/plugin2", plugin2_fs);

    println!("✓ Mounted plugin filesystems");
    println!("\nAvailable plugins:");
    let plugins = fs.list_directory("/plugins")?;
    for plugin in plugins {
        let manifest_path = format!("/plugins/{}/manifest.json", plugin);
        if fs.exists(&manifest_path)? {
            let mut file = fs.open_file(&manifest_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            println!("  - {}: {}", plugin, contents.trim());
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • MountableFilesystem allows mounting different filesystems at paths");
    println!("  • Each mount point is independent and isolated");
    println!("  • More specific mount points take precedence");
    println!("  • Perfect for multi-tenant applications and plugin systems");
    println!("  • Filesystems can be mounted, unmounted, and re-mounted");
    println!("  • Similar to Unix/Linux mount points");

    Ok(())
}

// Made with Bob
