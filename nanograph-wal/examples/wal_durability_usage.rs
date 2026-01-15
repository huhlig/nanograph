//! Durability levels example for Nanograph Write-Ahead Log
//!
//! This example demonstrates the three durability levels (Memory, Flush, Sync)
//! and their performance/safety tradeoffs.

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{Durability, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph WAL Durability Levels Example ===\n");

    // Create WAL
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(0);
    let wal = WriteAheadLogManager::new(fs, "/wal", config)?;
    let mut writer = wal.writer()?;

    println!("This example demonstrates three durability levels:\n");
    println!("1. Memory: Fastest, data in memory buffer only");
    println!("   - Lost on crash or power failure");
    println!("   - Use for: Non-critical data, temporary operations\n");

    println!("2. Flush: Balanced, data written to OS buffers");
    println!("   - Survives application crash (if OS survives)");
    println!("   - Lost on power failure or OS crash");
    println!("   - Use for: Most production workloads\n");

    println!("3. Sync: Safest, data persisted to stable storage");
    println!("   - Survives crash and power failure");
    println!("   - Slowest due to fsync() call");
    println!("   - Use for: Critical transactions, financial data\n");

    // Benchmark each durability level
    let record_count = 100;

    // Memory durability benchmark
    println!("--- Memory Durability Benchmark ---");
    let start = Instant::now();
    for i in 0..record_count {
        let payload = format!("Memory record {}", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Memory)?;
    }
    let memory_duration = start.elapsed();
    println!("✓ Wrote {} records with Memory durability", record_count);
    println!("  Time: {:?}", memory_duration);
    println!(
        "  Avg: {:?} per record",
        memory_duration / record_count as u32
    );

    // Flush durability benchmark
    println!("\n--- Flush Durability Benchmark ---");
    let start = Instant::now();
    for i in 0..record_count {
        let payload = format!("Flush record {}", i);
        let record = WriteAheadLogRecord {
            kind: 2,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Flush)?;
    }
    let flush_duration = start.elapsed();
    println!("✓ Wrote {} records with Flush durability", record_count);
    println!("  Time: {:?}", flush_duration);
    println!(
        "  Avg: {:?} per record",
        flush_duration / record_count as u32
    );

    // Sync durability benchmark
    println!("\n--- Sync Durability Benchmark ---");
    let start = Instant::now();
    for i in 0..record_count {
        let payload = format!("Sync record {}", i);
        let record = WriteAheadLogRecord {
            kind: 3,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Sync)?;
    }
    let sync_duration = start.elapsed();
    println!("✓ Wrote {} records with Sync durability", record_count);
    println!("  Time: {:?}", sync_duration);
    println!(
        "  Avg: {:?} per record",
        sync_duration / record_count as u32
    );

    // Performance comparison
    println!("\n--- Performance Comparison ---");
    println!("Memory: {:?} (baseline)", memory_duration);
    println!(
        "Flush:  {:?} ({:.2}x slower)",
        flush_duration,
        flush_duration.as_secs_f64() / memory_duration.as_secs_f64()
    );
    println!(
        "Sync:   {:?} ({:.2}x slower)",
        sync_duration,
        sync_duration.as_secs_f64() / memory_duration.as_secs_f64()
    );

    // Demonstrate mixed durability strategy
    println!("\n--- Mixed Durability Strategy ---");
    println!("Real applications often use different levels for different operations:\n");

    // Regular operations with Flush
    let payload = "Regular transaction";
    let record = WriteAheadLogRecord {
        kind: 10,
        payload: payload.as_bytes(),
    };
    let lsn1 = writer.append(record, Durability::Flush)?;
    println!("✓ Regular transaction at LSN {:?} (Flush)", lsn1);

    // Critical operation with Sync
    let payload = "Critical financial transaction";
    let record = WriteAheadLogRecord {
        kind: 20,
        payload: payload.as_bytes(),
    };
    let lsn2 = writer.append(record, Durability::Sync)?;
    println!("✓ Critical transaction at LSN {:?} (Sync)", lsn2);

    // Temporary data with Memory
    let payload = "Temporary cache entry";
    let record = WriteAheadLogRecord {
        kind: 30,
        payload: payload.as_bytes(),
    };
    let lsn3 = writer.append(record, Durability::Memory)?;
    println!("✓ Temporary data at LSN {:?} (Memory)", lsn3);

    // Group commit pattern
    println!("\n--- Group Commit Pattern ---");
    println!("Write multiple records with Memory, then Sync once:\n");

    let start = Instant::now();

    // Write batch with Memory durability
    for i in 0..10 {
        let payload = format!("Batch item {}", i);
        let record = WriteAheadLogRecord {
            kind: 40,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Memory)?;
    }

    // Final sync to persist all
    let payload = "Batch commit marker";
    let record = WriteAheadLogRecord {
        kind: 41,
        payload: payload.as_bytes(),
    };
    writer.append(record, Durability::Sync)?;

    let group_commit_duration = start.elapsed();
    println!("✓ Wrote 10 records + 1 sync in {:?}", group_commit_duration);
    println!("  This is much faster than 11 individual Sync operations!");

    println!("\n=== Example Complete ===");
    println!("\nBest Practices:");
    println!("• Use Flush for most operations (good balance)");
    println!("• Use Sync for critical data that must survive power loss");
    println!("• Use Memory for temporary/cache data or with group commits");
    println!("• Consider group commits to amortize sync cost across multiple records");
    println!("• Monitor metrics to understand actual durability overhead");

    Ok(())
}
