//! MonitoredFilesystem usage example for Nanograph VFS
//!
//! This example demonstrates the monitored filesystem implementation,
//! which wraps any filesystem to add metrics collection and monitoring.

use nanograph_vfs::{File, FileSystem, MemoryFileSystem, MonitoredFilesystem};
use std::io::{Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph VFS MonitoredFilesystem Example ===\n");

    // Create a base filesystem
    let base_fs = MemoryFileSystem::new();
    println!("✓ Created base MemoryFileSystem");

    // Wrap it with monitoring
    let fs = MonitoredFilesystem::new(base_fs, "memory");
    println!("✓ Wrapped with MonitoredFilesystem");
    println!("  - Tracks all filesystem operations");
    println!("  - Collects performance metrics");
    println!("  - Records bytes read/written");
    println!("  - Monitors open files\n");

    // Perform some operations and track metrics
    println!("--- Basic Operations with Monitoring ---");

    // Create directories
    fs.create_directory_all("/app/data")?;
    fs.create_directory_all("/app/logs")?;
    println!("✓ Created directories");

    // Create and write files
    let mut file1 = fs.create_file("/app/data/file1.txt")?;
    file1.write_all(b"Hello, World!")?;
    file1.sync_all()?;
    println!("✓ Created and wrote to file1.txt");

    let mut file2 = fs.create_file("/app/data/file2.txt")?;
    file2.write_all(b"This is a longer piece of text for testing.")?;
    file2.sync_all()?;
    println!("✓ Created and wrote to file2.txt");

    let mut log = fs.create_file("/app/logs/app.log")?;
    log.write_all(b"[INFO] Application started\n")?;
    log.write_all(b"[INFO] Processing data\n")?;
    log.sync_all()?;
    println!("✓ Created and wrote to app.log");

    // Read files
    println!("\n--- Reading Files ---");
    let mut file = fs.open_file("/app/data/file1.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read file1.txt: {}", contents);

    let mut file = fs.open_file("/app/data/file2.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Read file2.txt: {}", contents);

    // Check metrics
    println!("\n--- Metrics After Basic Operations ---");
    println!("Total operations: {}", fs.total_operations());
    println!("Successful operations: {}", fs.successful_operations());
    println!("Failed operations: {}", fs.failed_operations());
    println!("Bytes written: {}", fs.bytes_written());
    println!("Bytes read: {}", fs.bytes_read());
    println!("Currently open files: {}", fs.open_files().len());

    // Demonstrate operation tracking
    println!("\n--- Operation Type Breakdown ---");
    println!(
        "Create file operations: {}",
        fs.operation_count("create_file")
    );
    println!("Open file operations: {}", fs.operation_count("open_file"));
    println!(
        "Create directory operations: {}",
        fs.operation_count("create_directory_all")
    );
    println!("Write operations: {}", fs.operation_count("write"));
    println!("Read operations: {}", fs.operation_count("read"));

    // Demonstrate performance monitoring
    println!("\n--- Performance Monitoring ---");
    let start = std::time::Instant::now();

    // Perform a batch of operations
    for i in 0..100 {
        let path = format!("/app/data/batch_{}.txt", i);
        let mut file = fs.create_file(&path)?;
        file.write_all(format!("Batch file {}", i).as_bytes())?;
    }

    let elapsed = start.elapsed();
    println!("Created 100 files in {:?}", elapsed);
    println!("Average time per file: {:?}", elapsed / 100);

    // Check updated metrics
    println!("\nMetrics after batch operations:");
    println!("Total operations: {}", fs.total_operations());
    println!("Bytes written: {}", fs.bytes_written());

    // Demonstrate error tracking
    println!("\n--- Error Tracking ---");
    println!("Failed operations before error: {}", fs.failed_operations());

    // Try to open a non-existent file
    match fs.open_file("/nonexistent.txt") {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("Expected error: {}", e),
    }

    println!("Failed operations after error: {}", fs.failed_operations());

    // Demonstrate open file tracking
    println!("\n--- Open File Tracking ---");
    let file1 = fs.open_file("/app/data/file1.txt")?;
    let file2 = fs.open_file("/app/data/file2.txt")?;
    let file3 = fs.open_file("/app/logs/app.log")?;

    println!("Opened 3 files");
    println!("Currently open files: {}", fs.open_files().len());
    println!("Open file paths:");
    for path in fs.open_files() {
        println!("  - {}", path);
    }

    // Drop files to close them
    drop(file1);
    drop(file2);
    drop(file3);

    println!("\nAfter closing files:");
    println!("Currently open files: {}", fs.open_files().len());

    // Demonstrate directory operation tracking
    println!("\n--- Directory Operations ---");
    let entries = fs.list_directory("/app/data")?;
    println!("Listed /app/data: {} entries", entries.len());

    fs.remove_file("/app/data/file1.txt")?;
    println!("Deleted file1.txt");

    println!("\nDirectory operation metrics:");
    println!(
        "List directory operations: {}",
        fs.operation_count("list_directory")
    );
    println!(
        "Remove file operations: {}",
        fs.operation_count("remove_file")
    );

    // Demonstrate metrics reset
    println!("\n--- Metrics Reset ---");
    println!("Metrics before reset:");
    println!("  Total operations: {}", fs.total_operations());
    println!("  Bytes written: {}", fs.bytes_written());
    println!("  Bytes read: {}", fs.bytes_read());

    fs.reset_metrics();
    println!("\n✓ Reset metrics");

    println!("\nMetrics after reset:");
    println!("  Total operations: {}", fs.total_operations());
    println!("  Bytes written: {}", fs.bytes_written());
    println!("  Bytes read: {}", fs.bytes_read());

    // Perform new operations after reset
    println!("\n--- Operations After Reset ---");
    let mut file = fs.create_file("/app/test_after_reset.txt")?;
    file.write_all(b"Testing metrics after reset")?;

    let mut file = fs.open_file("/app/test_after_reset.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    println!("New metrics:");
    println!("  Total operations: {}", fs.total_operations());
    println!("  Bytes written: {}", fs.bytes_written());
    println!("  Bytes read: {}", fs.bytes_read());

    // Use case: Performance profiling
    println!("\n--- Use Case: Performance Profiling ---");
    fs.reset_metrics();

    let operations = vec![
        ("Small file", 100),
        ("Medium file", 1000),
        ("Large file", 10000),
    ];

    for (name, size) in operations {
        let start = std::time::Instant::now();

        let path = format!("/app/perf_{}.dat", name.replace(" ", "_"));
        let mut file = fs.create_file(&path)?;
        let data = vec![b'X'; size];
        file.write_all(&data)?;
        file.sync_all()?;

        let elapsed = start.elapsed();
        println!("{} ({} bytes): {:?}", name, size, elapsed);
    }

    println!("\nTotal bytes written: {}", fs.bytes_written());
    println!("Total operations: {}", fs.total_operations());

    // Use case: Resource monitoring
    println!("\n--- Use Case: Resource Monitoring ---");
    fs.reset_metrics();

    // Simulate application workload
    println!("Simulating application workload...");

    // Create some files
    for i in 0..10 {
        let path = format!("/app/data/user_{}.json", i);
        let mut file = fs.create_file(&path)?;
        file.write_all(
            format!(r#"{{"id": {}, "name": "User {}", "active": true}}"#, i, i).as_bytes(),
        )?;
    }

    // Read some files
    for i in 0..5 {
        let path = format!("/app/data/user_{}.json", i);
        let mut file = fs.open_file(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
    }

    // Update some files
    for i in 0..3 {
        let path = format!("/app/data/user_{}.json", i);
        let mut file = fs.open_file(&path)?;
        file.write_all(b", \"updated\": true}")?;
    }

    println!("\nWorkload complete. Resource usage:");
    println!("  Total operations: {}", fs.total_operations());
    println!("  Successful: {}", fs.successful_operations());
    println!("  Failed: {}", fs.failed_operations());
    println!(
        "  Success rate: {:.2}%",
        (fs.successful_operations() as f64 / fs.total_operations() as f64) * 100.0
    );
    println!("  Bytes written: {}", fs.bytes_written());
    println!("  Bytes read: {}", fs.bytes_read());
    println!(
        "  I/O ratio (read/write): {:.2}",
        fs.bytes_read() as f64 / fs.bytes_written() as f64
    );

    // Use case: Debugging and troubleshooting
    println!("\n--- Use Case: Debugging ---");
    fs.reset_metrics();

    println!("Performing operations with potential issues...");

    // Mix of successful and failed operations
    let _ = fs.create_file("/app/debug1.txt");
    let _ = fs.open_file("/nonexistent1.txt"); // Will fail
    let _ = fs.create_file("/app/debug2.txt");
    let _ = fs.open_file("/nonexistent2.txt"); // Will fail
    let _ = fs.create_file("/app/debug3.txt");

    println!("\nDebug metrics:");
    println!("  Total operations: {}", fs.total_operations());
    println!("  Successful: {}", fs.successful_operations());
    println!("  Failed: {}", fs.failed_operations());
    println!(
        "  Failure rate: {:.2}%",
        (fs.failed_operations() as f64 / fs.total_operations() as f64) * 100.0
    );

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • MonitoredFilesystem wraps any filesystem with metrics");
    println!("  • Tracks all operations, successes, and failures");
    println!("  • Records bytes read and written");
    println!("  • Monitors currently open files");
    println!("  • Perfect for performance profiling and debugging");
    println!("  • Metrics can be reset for different test scenarios");
    println!("  • Zero overhead when metrics aren't being collected");

    Ok(())
}
