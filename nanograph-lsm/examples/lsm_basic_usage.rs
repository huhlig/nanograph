//! Basic usage example for Nanograph LSM Tree
//!
//! This example demonstrates the core functionality of the LSM Tree engine,
//! including insertion, retrieval, deletion, and the write-optimized architecture.

use nanograph_lsm::{LSMTreeEngine, LSMTreeOptions};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph LSM Tree Basic Usage Example ===\n");

    // Create filesystem and options
    let fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
    let options = LSMTreeOptions::default();

    // Create WAL
    let wal_config = WriteAheadLogConfig::default();
    let wal = WriteAheadLogManager::new(fs.clone(), Path::from("/wal"), wal_config)?;

    // Create a new LSM Tree engine
    let engine = LSMTreeEngine::new(fs, "/lsm_data".to_string(), options, wal)?;
    println!("✓ Created new LSM Tree engine");

    // Insert some key-value pairs (writes go to MemTable first)
    println!("\n--- Inserting Data (Write-Optimized) ---");
    engine.put(b"apple".to_vec(), b"A red fruit".to_vec())?;
    engine.put(b"apricot".to_vec(), b"An orange fruit".to_vec())?;
    engine.put(b"banana".to_vec(), b"A yellow fruit".to_vec())?;
    engine.put(b"cherry".to_vec(), b"A small red fruit".to_vec())?;
    engine.put(b"date".to_vec(), b"A sweet brown fruit".to_vec())?;
    println!("✓ Inserted 5 fruits (all in MemTable)");

    // Retrieve values (checks MemTable first, then SSTables)
    println!("\n--- Retrieving Data ---");
    if let Some(value) = engine.get(b"apple")? {
        println!("apple: {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = engine.get(b"banana")? {
        println!("banana: {}", String::from_utf8_lossy(&value));
    }

    // Check for non-existent key
    match engine.get(b"grape")? {
        Some(_) => println!("grape: found"),
        None => println!("grape: not found"),
    }

    // Get engine statistics
    println!("\n--- Engine Statistics ---");
    let stats = engine.stats();
    println!("MemTable size: {} bytes", stats.memtable_size);
    println!("Total levels: {}", stats.levels.len());
    println!(
        "Total SSTables: {}",
        stats.levels.iter().map(|l| l.num_sstables).sum::<usize>()
    );

    // Flush MemTable to SSTable (simulating write buffer full)
    println!("\n--- Flushing MemTable to Disk ---");
    engine.flush()?;
    println!("✓ Flushed MemTable to SSTable");

    // Check stats after flush
    let stats = engine.stats();
    println!("MemTable size after flush: {} bytes", stats.memtable_size);
    println!(
        "Level 0 SSTables: {}",
        stats.levels.get(0).map(|l| l.num_sstables).unwrap_or(0)
    );

    // Insert more data
    println!("\n--- Inserting More Data ---");
    engine.put(b"elderberry".to_vec(), b"A dark purple berry".to_vec())?;
    engine.put(b"fig".to_vec(), b"A sweet soft fruit".to_vec())?;
    engine.put(b"grape".to_vec(), b"A purple fruit".to_vec())?;
    println!("✓ Inserted 3 more fruits");

    // Delete an entry (tombstone marker)
    println!("\n--- Deleting Data ---");
    engine.delete(b"banana".to_vec())?;
    println!("✓ Deleted 'banana' (tombstone added)");

    // Verify deletion
    match engine.get(b"banana")? {
        Some(_) => println!("✗ banana still exists (unexpected)"),
        None => println!("✓ banana successfully deleted"),
    }

    // Note: Range scan and full scan are not yet implemented in the engine
    println!("\n--- Note ---");
    println!("Range scan and full scan features are planned for future implementation.");

    // Final statistics
    println!("\n--- Final Statistics ---");
    let stats = engine.stats();
    println!("MemTable size: {} bytes", stats.memtable_size);
    println!(
        "Immutable MemTable size: {} bytes",
        stats.immutable_memtable_size
    );
    println!(
        "Total SSTables: {}",
        stats.levels.iter().map(|l| l.num_sstables).sum::<usize>()
    );
    println!("Total writes: {}", stats.total_writes);
    println!("Total reads: {}", stats.total_reads);
    println!("Total flushes: {}", stats.total_flushes);
    println!("Total compactions: {}", stats.total_compactions);

    // Show level information
    for level_stats in &stats.levels {
        if level_stats.num_sstables > 0 {
            println!(
                "Level {}: {} SSTables, {} bytes",
                level_stats.level, level_stats.num_sstables, level_stats.total_size
            );
        }
    }

    println!("\n=== Example Complete ===");
    println!("Note: LSM Trees are optimized for write-heavy workloads.");
    println!("Writes go to MemTable (fast), then flush to immutable SSTables.");
    println!("Compaction merges SSTables to maintain read performance.");
    Ok(())
}

// Made with Bob
