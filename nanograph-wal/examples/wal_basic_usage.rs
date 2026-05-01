//! Basic usage example for Nanograph Write-Ahead Log
//!
//! This example demonstrates the core functionality of the WAL,
//! including writing records, reading them back, and understanding LSNs.

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{Durability, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph WAL Basic Usage Example ===\n");

    // Create a filesystem and WAL configuration
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(0) // shard_id = 0
        .with_max_segment_size(1024 * 1024); // 1 MB segments

    println!("✓ Created WAL configuration");
    println!("  Shard ID: {}", config.shard_id);
    println!("  Max segment size: {} bytes", config.max_segment_size);
    println!("  Checksum: {:?}", config.checksum);

    // Initialize the WAL manager
    let wal = WriteAheadLogManager::new(fs, "/wal", config)?;
    println!("\n✓ Initialized WAL manager at /wal");

    // Get the initial LSN (Log Sequence Number)
    let initial_lsn = wal.tail_lsn()?;
    println!("\n--- Initial State ---");
    println!("Head LSN: {:?}", wal.head_lsn()?);
    println!("Tail LSN: {:?}", initial_lsn);

    // Create a writer
    let mut writer = wal.writer()?;
    println!("\n✓ Created WAL writer");

    // Write some records with different durability levels
    println!("\n--- Writing Records ---");

    // Record 1: Memory durability (fastest, least durable)
    let record1 = WriteAheadLogRecord {
        kind: 1, // Custom record type
        payload: b"First record - memory durability",
    };
    let lsn1 = writer.append(record1, Durability::None)?;
    println!("✓ Wrote record 1 (Memory): LSN {:?}", lsn1);

    // Record 2: Flush durability (balanced)
    let record2 = WriteAheadLogRecord {
        kind: 2,
        payload: b"Second record - flush durability",
    };
    let lsn2 = writer.append(record2, Durability::Buffered)?;
    println!("✓ Wrote record 2 (Flush): LSN {:?}", lsn2);

    // Record 3: Sync durability (slowest, most durable)
    let record3 = WriteAheadLogRecord {
        kind: 3,
        payload: b"Third record - sync durability",
    };
    let lsn3 = writer.append(record3, Durability::Sync)?;
    println!("✓ Wrote record 3 (Sync): LSN {:?}", lsn3);

    // Write a batch of records
    println!("\n--- Writing Batch ---");
    for i in 4..=8 {
        let payload = format!("Batch record {}", i);
        let record = WriteAheadLogRecord {
            kind: i,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Buffered)?;
        println!("✓ Wrote record {} at LSN {:?}", i, lsn);
    }

    // Check the new tail LSN
    let final_lsn = wal.tail_lsn()?;
    println!("\n--- Final State ---");
    println!("Head LSN: {:?}", wal.head_lsn()?);
    println!("Tail LSN: {:?}", final_lsn);

    // Read records back from the beginning
    println!("\n--- Reading Records from Start ---");
    let mut reader = wal.reader_from(initial_lsn)?;
    let mut count = 0;

    while let Some(entry) = reader.next()? {
        count += 1;
        let payload_str = String::from_utf8_lossy(&entry.payload);
        println!(
            "Record {}: kind={}, LSN={:?}, payload='{}'",
            count, entry.kind, entry.lsn, payload_str
        );
    }

    println!("\n✓ Read {} records total", count);

    // Read from a specific LSN
    println!("\n--- Reading from Specific LSN ---");
    println!("Reading from LSN {:?} onwards...", lsn2);
    let mut reader = wal.reader_from(lsn2)?;
    let mut count = 0;

    while let Some(entry) = reader.next()? {
        count += 1;
        let payload_str = String::from_utf8_lossy(&entry.payload);
        println!(
            "Record {}: kind={}, LSN={:?}, payload='{}'",
            count, entry.kind, entry.lsn, payload_str
        );
    }

    println!("\n✓ Read {} records from LSN {:?}", count, lsn2);

    println!("\n=== Example Complete ===");
    println!("\nKey Concepts:");
    println!("• LSN (Log Sequence Number): Unique identifier for each record");
    println!("• Durability levels: Memory < Flush < Sync");
    println!("• WAL provides sequential write performance");
    println!("• Records can be replayed from any LSN for recovery");

    Ok(())
}
