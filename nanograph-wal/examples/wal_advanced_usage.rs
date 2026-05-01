//! Advanced usage example for Nanograph Write-Ahead Log
//!
//! This example demonstrates advanced WAL features including:
//! - Compression and encryption
//! - Integrity checking
//! - Segment management and truncation
//! - Metrics monitoring

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{
    CompressionAlgorithm, Durability, EncryptionAlgorithm, IntegrityAlgorithm, WriteAheadLogConfig,
    WriteAheadLogManager, WriteAheadLogRecord,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph WAL Advanced Usage Example ===\n");

    // Example 1: Compression
    println!("--- Example 1: Compression ---");
    demonstrate_compression()?;

    // Example 2: Integrity Checking
    println!("\n--- Example 2: Integrity Checking ---");
    demonstrate_integrity()?;

    // Example 3: Encryption
    println!("\n--- Example 3: Encryption ---");
    demonstrate_encryption()?;

    // Example 4: Segment Management
    println!("\n--- Example 4: Segment Management ---");
    demonstrate_segment_management()?;

    // Example 5: Combined Features
    println!("\n--- Example 5: Combined Features ---");
    demonstrate_combined_features()?;

    println!("\n=== Example Complete ===");
    Ok(())
}

fn demonstrate_compression() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MemoryFileSystem::new();

    // Create WAL with LZ4 compression
    let config = WriteAheadLogConfig::new(0).with_compression(CompressionAlgorithm::Lz4);

    println!("✓ Created WAL with LZ4 compression");
    println!("  Compression: {:?}", config.compression);

    let wal = WriteAheadLogManager::new(fs, "/wal_compressed", config)?;
    let mut writer = wal.writer()?;

    // Write highly compressible data
    let repetitive_data = "AAAAAAAAAA".repeat(100); // 1000 bytes of 'A'
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: repetitive_data.as_bytes(),
    };

    let lsn = writer.append(record, Durability::Sync)?;
    println!("✓ Wrote 1000 bytes of repetitive data at LSN {:?}", lsn);
    println!("  (Compression should significantly reduce storage size)");

    // Read it back
    let mut reader = wal.reader_from(lsn)?;
    if let Some(entry) = reader.next()? {
        println!("✓ Read back {} bytes (decompressed)", entry.payload.len());
        assert_eq!(entry.payload.len(), 1000);
    }

    Ok(())
}

fn demonstrate_integrity() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MemoryFileSystem::new();

    // Create WAL with different integrity algorithms
    println!("\nAvailable integrity algorithms:");
    println!("  - None: No checksum (fastest, no protection)");
    println!("  - CRC32c: Fast, good error detection");
    println!("  - XXHash32: Very fast, good distribution");
    println!("  - XXHash64: Fast, excellent distribution");

    let config = WriteAheadLogConfig::new(0).with_integrity(IntegrityAlgorithm::Crc32c);

    println!("\n✓ Created WAL with CRC32c integrity checking");

    let wal = WriteAheadLogManager::new(fs, "/wal_integrity", config)?;
    let mut writer = wal.writer()?;

    // Write data with integrity protection
    let payload = "Critical data that must not be corrupted";
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: payload.as_bytes(),
    };

    let lsn = writer.append(record, Durability::Sync)?;
    println!("✓ Wrote record with CRC32c checksum at LSN {:?}", lsn);

    // Read and verify
    let mut reader = wal.reader_from(lsn)?;
    if let Some(entry) = reader.next()? {
        println!("✓ Read and verified record (checksum valid)");
        let payload_str = String::from_utf8_lossy(&entry.payload);
        println!("  Payload: {}", payload_str);
    }

    Ok(())
}

fn demonstrate_encryption() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MemoryFileSystem::new();

    // Generate encryption key
    let encryption_key = EncryptionAlgorithm::Aes256Gcm.generate_key();

    println!("✓ Generated AES-256-GCM encryption key");
    println!(
        "  Key Algorithm: {:?} Key Lenth: {}",
        encryption_key.algorithm,
        encryption_key.key.len()
    );

    // Create WAL with encryption
    let config = WriteAheadLogConfig::new(0)
        .with_encryption(EncryptionAlgorithm::Aes256Gcm, Some(encryption_key));

    println!("✓ Created WAL with AES-256-GCM encryption");

    let wal = WriteAheadLogManager::new(fs, "/wal_encrypted", config)?;
    let mut writer = wal.writer()?;

    // Write sensitive data
    let sensitive_data = "SSN: 123-45-6789, Credit Card: 4111-1111-1111-1111";
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: sensitive_data.as_bytes(),
    };

    let lsn = writer.append(record, Durability::Sync)?;
    println!("✓ Wrote encrypted sensitive data at LSN {:?}", lsn);
    println!("  (Data is encrypted at rest)");

    // Read and decrypt
    let mut reader = wal.reader_from(lsn)?;
    if let Some(entry) = reader.next()? {
        let decrypted = String::from_utf8_lossy(&entry.payload);
        println!("✓ Read and decrypted data");
        println!("  Decrypted: {}", decrypted);
    }

    Ok(())
}

fn demonstrate_segment_management() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MemoryFileSystem::new();

    // Create WAL with small segment size to trigger rotation
    let config = WriteAheadLogConfig::new(0).with_max_segment_size(1024); // 1 KB segments

    println!("✓ Created WAL with 1 KB max segment size");

    let wal = WriteAheadLogManager::new(fs, "/wal_segments", config)?;
    let mut writer = wal.writer()?;

    // Write enough data to potentially create multiple segments
    println!("\nWriting data...");
    let mut lsns = Vec::new();
    for i in 0..20 {
        let payload = format!("Record {} with some padding data to increase size", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Sync)?;
        lsns.push(lsn);
    }
    println!("✓ Wrote 20 records");

    // Check LSN range
    let head_lsn = wal.head_lsn()?;
    let tail_lsn = wal.tail_lsn()?;
    println!("\nLSN Range:");
    println!("  Head (oldest): {:?}", head_lsn);
    println!("  Tail (newest): {:?}", tail_lsn);

    // Demonstrate truncation
    if lsns.len() > 10 {
        let truncate_point = lsns[10];
        println!("\nTruncating WAL before LSN {:?}...", truncate_point);
        wal.truncate_before(truncate_point)?;
        println!("✓ Truncated old segments");

        let new_head = wal.head_lsn()?;
        println!("  New head LSN: {:?}", new_head);
    }

    Ok(())
}

fn demonstrate_combined_features() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MemoryFileSystem::new();

    // Create production-ready WAL with all features
    let encryption_key = EncryptionAlgorithm::Aes256Gcm.generate_key();

    let config = WriteAheadLogConfig::new(0)
        .with_max_segment_size(64 * 1024 * 1024) // 64 MB
        .with_compression(CompressionAlgorithm::Lz4)
        .with_integrity(IntegrityAlgorithm::Crc32c)
        .with_encryption(EncryptionAlgorithm::Aes256Gcm, Some(encryption_key))
        .with_sync_on_rotate(true);

    println!("✓ Created production-ready WAL with:");
    println!("  - 64 MB segments");
    println!("  - LZ4 compression");
    println!("  - CRC32c integrity checking");
    println!("  - AES-256-GCM encryption");
    println!("  - Sync on segment rotation");

    // Validate configuration
    config.validate()?;
    println!("✓ Configuration validated");

    let wal = WriteAheadLogManager::new(fs, "/wal_production", config)?;
    let mut writer = wal.writer()?;

    // Write various types of records
    println!("\nWriting different record types...");

    // Transaction begin
    let record = WriteAheadLogRecord {
        kind: 100,
        payload: b"BEGIN TRANSACTION",
    };
    let begin_lsn = writer.append(record, Durability::Buffered)?;
    println!("✓ Transaction begin at LSN {:?}", begin_lsn);

    // Data modifications
    for i in 0..5 {
        let payload = format!("UPDATE table SET value={} WHERE id={}", i * 10, i);
        let record = WriteAheadLogRecord {
            kind: 101,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Buffered)?;
    }
    println!("✓ Wrote 5 UPDATE operations");

    // Transaction commit
    let record = WriteAheadLogRecord {
        kind: 102,
        payload: b"COMMIT TRANSACTION",
    };
    let commit_lsn = writer.append(record, Durability::Sync)?;
    println!("✓ Transaction commit at LSN {:?}", commit_lsn);

    // Replay transaction
    println!("\nReplaying transaction from begin to commit...");
    let mut reader = wal.reader_from(begin_lsn)?;
    let mut count = 0;

    while let Some(entry) = reader.next()? {
        count += 1;
        let operation = match entry.kind {
            100 => "BEGIN",
            101 => "UPDATE",
            102 => "COMMIT",
            _ => "UNKNOWN",
        };
        println!("  {}. {} at {:?}", count, operation, entry.lsn);

        if entry.lsn == commit_lsn {
            break;
        }
    }

    println!("✓ Replayed {} operations", count);

    Ok(())
}
