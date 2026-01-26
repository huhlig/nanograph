//! KeyValueStore usage example for Nanograph LSM Tree
//!
//! This example demonstrates using LSM Tree as a KeyValueShardStore,
//! including shard management, batch operations, and write-optimized performance.

use nanograph_kvt::metrics::StatValue;
use nanograph_kvt::{KeyValueShardStore, ShardId};
use nanograph_lsm::LSMKeyValueStore;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph LSM Tree KeyValueStore Example ===\n");

    // Create the store
    let store = Arc::new(LSMKeyValueStore::default());
    println!("✓ Created LSM Tree KeyValueStore");
    println!("  Optimized for write-heavy workloads");

    // Create a shard
    println!("\n--- Creating Shard ---");
    let shard_id = ShardId::new(1);
    store.create_shard(shard_id).await?;
    println!("✓ Created shard: {:?}", shard_id);

    // Basic operations (writes are fast - go to MemTable)
    println!("\n--- Basic Operations ---");
    store.put(shard_id, b"user:1", b"Alice").await?;
    store.put(shard_id, b"user:2", b"Bob").await?;
    store.put(shard_id, b"user:3", b"Charlie").await?;
    println!("✓ Inserted 3 users (fast writes to MemTable)");

    if let Some(value) = store.get(shard_id, b"user:1").await? {
        println!("user:1 = {}", String::from_utf8_lossy(&value));
    }

    if let Some(value) = store.get(shard_id, b"user:2").await? {
        println!("user:2 = {}", String::from_utf8_lossy(&value));
    }

    // Check existence
    let exists = store.exists(shard_id, b"user:1").await?;
    println!("user:1 exists: {}", exists);

    let exists = store.exists(shard_id, b"user:999").await?;
    println!("user:999 exists: {}", exists);

    // Batch operations (very efficient for LSM trees)
    println!("\n--- Batch Operations ---");
    let batch = vec![
        (&b"product:1"[..], &b"Laptop"[..]),
        (&b"product:2"[..], &b"Mouse"[..]),
        (&b"product:3"[..], &b"Keyboard"[..]),
        (&b"product:4"[..], &b"Monitor"[..]),
        (&b"product:5"[..], &b"Webcam"[..]),
    ];
    store.batch_put(shard_id, &batch).await?;
    println!("✓ Batch inserted 5 products (optimized for bulk writes)");

    // Batch get
    let keys = vec![&b"product:1"[..], &b"product:2"[..], &b"product:999"[..]];
    let values = store.batch_get(shard_id, &keys).await?;
    println!("\nBatch get results:");
    for (key, value) in keys.iter().zip(values.iter()) {
        let key_str = String::from_utf8_lossy(key);
        match value {
            Some(v) => println!("  {} = {}", key_str, String::from_utf8_lossy(v)),
            None => println!("  {} = <not found>", key_str),
        }
    }

    // Delete operation (adds tombstone marker)
    println!("\n--- Delete Operation ---");
    let deleted = store.delete(shard_id, b"user:2").await?;
    println!("✓ Deleted user:2: {} (tombstone added)", deleted);

    // Verify deletion
    match store.get(shard_id, b"user:2").await? {
        Some(_) => println!("✗ user:2 still exists (unexpected)"),
        None => println!("✓ user:2 successfully deleted"),
    }

    // Batch delete
    println!("\n--- Batch Delete ---");
    let keys_to_delete = vec![&b"product:1"[..], &b"product:3"[..]];
    let deleted_count = store.batch_delete(shard_id, &keys_to_delete).await?;
    println!("✓ Batch deleted {} products", deleted_count);

    // Verify remaining data
    println!("\n--- Verify Remaining Data ---");
    let remaining_keys = vec![
        &b"user:1"[..],
        &b"user:3"[..],
        &b"product:2"[..],
        &b"product:4"[..],
        &b"product:5"[..],
    ];
    let values = store.batch_get(shard_id, &remaining_keys).await?;
    println!("Remaining entries:");
    for (key, value) in remaining_keys.iter().zip(values.iter()) {
        let key_str = String::from_utf8_lossy(key);
        match value {
            Some(v) => println!("  {} = {}", key_str, String::from_utf8_lossy(v)),
            None => println!("  {} = <not found>", key_str),
        }
    }

    // Get shard statistics
    println!("\n--- Shard Statistics ---");
    let stats = store.shard_stats(shard_id).await?;
    println!("Shard stats:");
    for (key, value) in stats.iter() {
        match value {
            StatValue::None => println!("  {}: None", key),
            StatValue::U64(v) => println!("  {}: {}", key, v),
            StatValue::F64(v) => println!("  {}: {:.2}", key, v),
            StatValue::I64(v) => println!("  {}: {}", key, v),
            StatValue::Bool(v) => println!("  {}: {}", key, v),
            StatValue::String(v) => println!("  {}: {}", key, v),
            StatValue::List(v) => println!("  {}: {:?}", key, v),
            StatValue::Map(v) => println!("  {}: {:?}", key, v),
            StatValue::Timestamp(v) => println!("  {}: {}", key, v),
        }
    }

    // Demonstrate write amplification benefit
    println!("\n--- Write Performance Characteristics ---");
    println!("LSM Tree advantages:");
    println!("  • Sequential writes to MemTable (very fast)");
    println!("  • Batch writes are highly efficient");
    println!("  • Background compaction maintains read performance");
    println!("  • Ideal for write-heavy workloads");
    println!("  • Deletes are fast (tombstone markers)");

    // Drop shard
    println!("\n--- Cleanup ---");
    store.drop_shard(shard_id).await?;
    println!("✓ Dropped shard");

    println!("\n=== Example Complete ===");
    Ok(())
}
