//! Recovery usage example for Nanograph Write-Ahead Log
//!
//! This example demonstrates how to use the WAL for crash recovery,
//! including replaying logs from a checkpoint and handling recovery scenarios.

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{Durability, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph WAL Recovery Usage Example ===\n");

    // Simulate a database system that uses WAL for durability
    println!("--- Scenario: Database with WAL-based Recovery ---\n");

    // Phase 1: Normal operation - write data and take a checkpoint
    println!("PHASE 1: Normal Operation");
    println!("─────────────────────────");

    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(0);

    let wal = WriteAheadLogManager::new(fs, "/wal", config.clone())?;
    let mut writer = wal.writer()?;

    // Simulate database operations
    println!("\n1. Writing initial data...");
    let records: Vec<(&str, &[u8])> = vec![
        ("INSERT", b"INSERT INTO users VALUES (1, 'Alice')"),
        ("INSERT", b"INSERT INTO users VALUES (2, 'Bob')"),
        ("INSERT", b"INSERT INTO users VALUES (3, 'Charlie')"),
    ];

    let mut lsns = Vec::new();
    for (op, data) in &records {
        let record = WriteAheadLogRecord {
            kind: 1, // INSERT operation
            payload: data,
        };
        let lsn = writer.append(record, Durability::Sync)?;
        println!("   ✓ {} at LSN {:?}", op, lsn);
        lsns.push(lsn);
    }

    // Take a checkpoint (simulate flushing in-memory state to disk)
    println!("\n2. Taking checkpoint...");
    let checkpoint_lsn = wal.tail_lsn()?;
    println!("   ✓ Checkpoint at LSN {:?}", checkpoint_lsn);
    println!("   (In a real system, this would flush all dirty pages to disk)");

    // Continue with more operations after checkpoint
    println!("\n3. Writing more data after checkpoint...");
    let post_checkpoint_records: Vec<(&str, &[u8])> = vec![
        ("UPDATE", b"UPDATE users SET name='Alice Smith' WHERE id=1"),
        ("DELETE", b"DELETE FROM users WHERE id=2"),
        ("INSERT", b"INSERT INTO users VALUES (4, 'David')"),
    ];

    for (op, data) in &post_checkpoint_records {
        let record = WriteAheadLogRecord {
            kind: match *op {
                "UPDATE" => 2,
                "DELETE" => 3,
                "INSERT" => 1,
                _ => 0,
            },
            payload: *data,
        };
        let lsn = writer.append(record, Durability::Sync)?;
        println!("   ✓ {} at LSN {:?}", op, lsn);
        lsns.push(lsn);
    }

    let final_lsn = wal.tail_lsn()?;
    println!("\n✓ Phase 1 complete. Final LSN: {:?}", final_lsn);

    // Phase 2: Simulate crash and recovery
    println!("\n\nPHASE 2: Crash Recovery");
    println!("─────────────────────────");
    println!("\n⚠️  SIMULATING SYSTEM CRASH...");
    println!("   (In-memory state lost, but WAL persisted)");

    // Drop the writer to simulate crash
    drop(writer);

    // Phase 3: Recovery process
    println!("\n\nPHASE 3: Recovery Process");
    println!("─────────────────────────");

    // Reopen the WAL (simulating system restart)
    println!("\n1. Reopening WAL after crash...");
    let fs2 = MemoryFileSystem::new();
    let wal_recovered = WriteAheadLogManager::new(fs2, "/wal", config)?;
    println!("   ✓ WAL reopened successfully");

    // Load checkpoint state
    println!("\n2. Loading checkpoint state...");
    println!("   ✓ Loaded checkpoint at LSN {:?}", checkpoint_lsn);
    println!("   (In a real system, this would load the last checkpoint from disk)");

    // Replay WAL from checkpoint
    println!("\n3. Replaying WAL from checkpoint...");
    let mut reader = wal_recovered.reader_from(checkpoint_lsn)?;
    let mut replayed_count = 0;

    while let Some(entry) = reader.next()? {
        replayed_count += 1;
        let operation = match entry.kind {
            1 => "INSERT",
            2 => "UPDATE",
            3 => "DELETE",
            _ => "UNKNOWN",
        };
        let payload_str = String::from_utf8_lossy(&entry.payload);
        println!(
            "   ↻ Replaying {} at LSN {:?}: {}",
            operation, entry.lsn, payload_str
        );
    }

    println!("\n   ✓ Replayed {} operations", replayed_count);

    // Verify recovery
    println!("\n4. Verifying recovery...");
    let recovered_tail = wal_recovered.tail_lsn()?;
    println!("   Original tail LSN: {:?}", final_lsn);
    println!("   Recovered tail LSN: {:?}", recovered_tail);

    if recovered_tail == final_lsn {
        println!("   ✓ Recovery successful - all data restored!");
    } else {
        println!("   ✗ Recovery incomplete - data loss detected!");
    }

    // Phase 4: Continue normal operation
    println!("\n\nPHASE 4: Resume Normal Operation");
    println!("─────────────────────────");

    let mut writer = wal_recovered.writer()?;
    println!("\n1. Writing new data after recovery...");

    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"INSERT INTO users VALUES (5, 'Eve')",
    };
    let new_lsn = writer.append(record, Durability::Sync)?;
    println!("   ✓ New INSERT at LSN {:?}", new_lsn);

    println!("\n✓ System fully recovered and operational!");

    // Demonstrate reading entire log
    println!("\n\nBONUS: Complete WAL History");
    println!("─────────────────────────");

    let head_lsn = wal_recovered.head_lsn()?;
    let mut reader = wal_recovered.reader_from(head_lsn)?;
    let mut total_count = 0;

    while let Some(entry) = reader.next()? {
        total_count += 1;
        let operation = match entry.kind {
            1 => "INSERT",
            2 => "UPDATE",
            3 => "DELETE",
            _ => "UNKNOWN",
        };
        let payload_str = String::from_utf8_lossy(&entry.payload);
        println!(
            "{}. {} at {:?}: {}",
            total_count, operation, entry.lsn, payload_str
        );
    }

    println!("\n✓ Total operations in WAL: {}", total_count);

    println!("\n=== Example Complete ===");
    println!("\nKey Recovery Concepts:");
    println!("• Checkpoint: Snapshot of database state at a specific LSN");
    println!("• Recovery: Replay WAL from checkpoint to restore state");
    println!("• Durability: WAL survives crashes when using Sync durability");
    println!("• Idempotency: Operations should be replayable safely");

    Ok(())
}

// Made with Bob
