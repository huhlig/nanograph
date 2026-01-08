//! Persistence usage example for Nanograph LSM Tree
//!
//! This example demonstrates the LSM Tree's persistence architecture,
//! including MemTable flushing, SSTable management, WAL, and compaction.

use nanograph_lsm::{LSMTreeEngine, LSMTreeOptions};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph LSM Tree Persistence Example ===\n");

    // Create filesystem and options
    // Using MemoryFileSystem for this example (can also use LocalFilesystem for real disk I/O)
    let fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
    let mut options = LSMTreeOptions::default();
    options.memtable_size = 1024;
    println!("✓ Created in-memory filesystem");
    println!("  MemTable size limit: {} bytes", options.memtable_size);

    // Example 1: Basic persistence with WAL
    println!("\n--- Example 1: Write-Ahead Log (WAL) ---");
    {
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(fs.clone(), Path::from("/wal1"), wal_config)?;
        let engine = LSMTreeEngine::new(
            fs.clone(),
            "/lsm_data1".to_string(),
            options.clone(),
            wal,
        )?;
        println!("✓ Created LSM engine with WAL enabled");

        // Writes are logged to WAL before being applied
        engine.put(b"key1".to_vec(), b"value1".to_vec())?;
        engine.put(b"key2".to_vec(), b"value2".to_vec())?;
        engine.put(b"key3".to_vec(), b"value3".to_vec())?;
        println!("✓ Inserted 3 entries (logged to WAL)");

        // Data is in MemTable and WAL
        let stats = engine.stats();
        println!("MemTable size: {} bytes", stats.memtable_size);
    }

    // Example 2: MemTable flush to SSTable
    println!("\n--- Example 2: MemTable Flush ---");
    {
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(fs.clone(), Path::from("/wal2"), wal_config)?;
        let engine =
            LSMTreeEngine::new(fs.clone(), "/lsm_data2".to_string(), options.clone(), wal)?;

        // Insert data
        for i in 0..10 {
            let key = format!("user:{}", i);
            let value = format!("User {}", i);
            engine.put(key.into_bytes(), value.into_bytes())?;
        }
        println!("✓ Inserted 10 entries");

        let stats = engine.stats();
        println!("Before flush:");
        println!("  MemTable size: {} bytes", stats.memtable_size);
        println!(
            "  Level 0 SSTables: {}",
            stats.levels.get(0).map(|l| l.num_sstables).unwrap_or(0)
        );

        // Flush MemTable to SSTable
        engine.flush()?;
        println!("✓ Flushed MemTable to SSTable");

        let stats = engine.stats();
        println!("After flush:");
        println!("  MemTable size: {} bytes", stats.memtable_size);
        println!(
            "  Level 0 SSTables: {}",
            stats.levels.get(0).map(|l| l.num_sstables).unwrap_or(0)
        );

        // Data is still accessible (now from SSTable)
        if let Some(value) = engine.get(b"user:5")? {
            println!("✓ Retrieved user:5 = {}", String::from_utf8_lossy(&value));
        }
    }

    // Example 3: Multiple flushes and levels
    println!("\n--- Example 3: Multi-Level Structure ---");
    {
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(fs.clone(), Path::from("/wal3"), wal_config)?;
        let engine =
            LSMTreeEngine::new(fs.clone(), "/lsm_data3".to_string(), options.clone(), wal)?;

        // Insert and flush multiple times
        for batch in 0..3 {
            for i in 0..5 {
                let key = format!("batch:{}:item:{}", batch, i);
                let value = format!("Batch {} Item {}", batch, i);
                engine.put(key.into_bytes(), value.into_bytes())?;
            }
            engine.flush()?;
            println!("✓ Flushed batch {}", batch);
        }

        let stats = engine.stats();
        println!("\nLSM Tree structure:");
        for level_stats in &stats.levels {
            println!(
                "  Level {}: {} SSTables, {} bytes",
                level_stats.level, level_stats.num_sstables, level_stats.total_size
            );
        }
    }

    // Example 4: Compaction
    println!("\n--- Example 4: Compaction ---");
    {
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(fs.clone(), Path::from("/wal4"), wal_config)?;
        let engine =
            LSMTreeEngine::new(fs.clone(), "/lsm_data4".to_string(), options.clone(), wal)?;

        // Insert lots of data to trigger compaction
        for i in 0..50 {
            let key = format!("data:{:03}", i);
            let value = format!("Value {}", i);
            engine.put(key.into_bytes(), value.into_bytes())?;
        }
        println!("✓ Inserted 50 entries");

        // Flush to create SSTables
        engine.flush()?;
        println!("✓ Flushed to SSTable");

        // Insert more data
        for i in 50..100 {
            let key = format!("data:{:03}", i);
            let value = format!("Value {}", i);
            engine.put(key.into_bytes(), value.into_bytes())?;
        }
        engine.flush()?;
        println!("✓ Flushed second batch");

        let stats = engine.stats();
        println!("\nBefore compaction:");
        println!(
            "  Total SSTables: {}",
            stats.levels.iter().map(|l| l.num_sstables).sum::<usize>()
        );

        // Note: Compaction would normally run in background
        // This is a simplified example showing the concept
        println!("\nCompaction merges overlapping SSTables to:");
        println!("  • Reduce number of files");
        println!("  • Remove deleted entries (tombstones)");
        println!("  • Improve read performance");
        println!("  • Reclaim disk space");
    }

    // Example 5: WAL Recovery
    println!("\n--- Example 5: WAL Recovery ---");
    {
        let data_path = "/lsm_data5".to_string();
        let wal_path = Path::from("/wal5");

        // Create engine and write data
        {
            let wal_config = WriteAheadLogConfig::default();
            let wal = WriteAheadLogManager::new(fs.clone(), wal_path.clone(), wal_config)?;
            let engine =
                LSMTreeEngine::new(fs.clone(), data_path.clone(), options.clone(), wal)?;

            engine.put(b"persistent:1".to_vec(), b"Data 1".to_vec())?;
            engine.put(b"persistent:2".to_vec(), b"Data 2".to_vec())?;
            println!("✓ Wrote data with WAL");
            // Engine dropped here (simulating crash)
        }

        // Reopen engine - should recover from WAL
        {
            let wal_config = WriteAheadLogConfig::default();
            let wal = WriteAheadLogManager::new(fs.clone(), wal_path, wal_config)?;
            let engine = LSMTreeEngine::new(fs.clone(), data_path, options.clone(), wal)?;
            println!("✓ Reopened engine (recovered from WAL)");

            // Data should be available
            if let Some(value) = engine.get(b"persistent:1")? {
                println!(
                    "✓ Recovered persistent:1 = {}",
                    String::from_utf8_lossy(&value)
                );
            }
            if let Some(value) = engine.get(b"persistent:2")? {
                println!(
                    "✓ Recovered persistent:2 = {}",
                    String::from_utf8_lossy(&value)
                );
            }
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nLSM Tree Persistence Summary:");
    println!("  • Writes go to WAL (durability) then MemTable (speed)");
    println!("  • MemTable flushes to immutable SSTables when full");
    println!("  • SSTables organized in levels (Level 0, 1, 2, ...)");
    println!("  • Compaction merges SSTables to maintain performance");
    println!("  • Bloom filters speed up negative lookups");
    Ok(())
}

// Made with Bob
