//! KeyValueStore usage example for Nanograph ART
//!
//! This example demonstrates using ART as a KeyValueShardStore,
//! including shard management, batch operations, and basic queries.

use nanograph_art::ArtKeyValueStore;
use nanograph_kvt::{IndexNumber, KeyValueShardStore, ShardId, TableId};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph ART KeyValueStore Example ===\n");

    // Create the store
    let store = Arc::new(ArtKeyValueStore::new());
    println!("✓ Created ART KeyValueStore");

    // Create a shard
    println!("\n--- Creating Shard ---");
    let shard_id = ShardId::new(1);
    store.create_shard(shard_id).await?;
    println!("✓ Created shard: {:?}", shard_id);

    // Basic operations
    println!("\n--- Basic Operations ---");
    store.put(shard_id, b"user:1", b"Alice").await?;
    store.put(shard_id, b"user:2", b"Bob").await?;
    store.put(shard_id, b"user:3", b"Charlie").await?;
    println!("✓ Inserted 3 users");

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

    // Batch operations
    println!("\n--- Batch Operations ---");
    let batch = vec![
        (&b"product:1"[..], &b"Laptop"[..]),
        (&b"product:2"[..], &b"Mouse"[..]),
        (&b"product:3"[..], &b"Keyboard"[..]),
    ];
    store.batch_put(shard_id, &batch).await?;
    println!("✓ Batch inserted 3 products");

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

    // Delete operation
    println!("\n--- Delete Operation ---");
    let deleted = store.delete(shard_id, b"user:2").await?;
    println!("✓ Deleted user:2: {}", deleted);

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
    let remaining_keys = vec![&b"user:1"[..], &b"user:3"[..], &b"product:2"[..]];
    let values = store.batch_get(shard_id, &remaining_keys).await?;
    println!("Remaining entries:");
    for (key, value) in remaining_keys.iter().zip(values.iter()) {
        let key_str = String::from_utf8_lossy(key);
        match value {
            Some(v) => println!("  {} = {}", key_str, String::from_utf8_lossy(v)),
            None => println!("  {} = <not found>", key_str),
        }
    }

    // Drop shard
    println!("\n--- Cleanup ---");
    store.drop_shard(shard_id).await?;
    println!("✓ Dropped shard");

    println!("\n=== Example Complete ===");
    Ok(())
}
