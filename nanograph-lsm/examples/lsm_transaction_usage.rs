//! Transaction usage example for Nanograph LSM Tree
//!
//! This example demonstrates ACID transactions with snapshot isolation,
//! including commit, rollback, and isolation guarantees in an LSM Tree.

use nanograph_kvt::{KeyValueShardStore, ShardId};
use nanograph_lsm::LSMKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph LSM Tree Transaction Example ===\n");

    // Create the store (transaction manager is initialized by default)
    let store = Arc::new(LSMKeyValueStore::default());
    println!("✓ Created LSM Tree KeyValueStore with transaction support");

    // Create a shard
    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from("/data");
    let wal_path = Path::from("/wal");
    store.create_shard(shard_id, vfs, data_path, wal_path)?;
    println!("✓ Created shard: {:?}", shard_id);

    // Insert some initial data
    println!("\n--- Initial Data ---");
    store.put(shard_id, b"account:1", b"100").await?;
    store.put(shard_id, b"account:2", b"200").await?;
    println!("✓ account:1 = 100");
    println!("✓ account:2 = 200");

    // Example 1: Successful transaction
    println!("\n--- Example 1: Successful Transaction ---");
    {
        let tx = store.begin_transaction().await?;
        println!("✓ Started transaction");

        // Read current values
        let balance1 = tx.get(shard_id, b"account:1").await?;
        let balance2 = tx.get(shard_id, b"account:2").await?;
        println!(
            "  account:1 = {}",
            String::from_utf8_lossy(&balance1.unwrap())
        );
        println!(
            "  account:2 = {}",
            String::from_utf8_lossy(&balance2.unwrap())
        );

        // Transfer 50 from account:1 to account:2
        tx.put(shard_id, b"account:1", b"50").await?;
        tx.put(shard_id, b"account:2", b"250").await?;
        println!("  Transferred 50 from account:1 to account:2");

        // Commit transaction (writes to WAL, then MemTable)
        tx.commit().await?;
        println!("✓ Transaction committed (durable via WAL)");
    }

    // Verify changes
    let balance1 = store.get(shard_id, b"account:1").await?;
    let balance2 = store.get(shard_id, b"account:2").await?;
    println!("After commit:");
    println!(
        "  account:1 = {}",
        String::from_utf8_lossy(&balance1.unwrap())
    );
    println!(
        "  account:2 = {}",
        String::from_utf8_lossy(&balance2.unwrap())
    );

    // Example 2: Rolled back transaction
    println!("\n--- Example 2: Rolled Back Transaction ---");
    {
        let tx = store.begin_transaction().await?;
        println!("✓ Started transaction");

        // Make some changes
        tx.put(shard_id, b"account:1", b"0").await?;
        tx.put(shard_id, b"account:2", b"300").await?;
        println!("  Modified accounts (not yet committed)");

        // Read within transaction (sees uncommitted changes)
        let balance1 = tx.get(shard_id, b"account:1").await?;
        println!(
            "  account:1 within tx = {}",
            String::from_utf8_lossy(&balance1.unwrap())
        );

        // Rollback transaction
        tx.rollback().await?;
        println!("✓ Transaction rolled back (changes discarded)");
    }

    // Verify rollback - values should be unchanged
    let balance1 = store.get(shard_id, b"account:1").await?;
    let balance2 = store.get(shard_id, b"account:2").await?;
    println!("After rollback:");
    println!(
        "  account:1 = {} (unchanged)",
        String::from_utf8_lossy(&balance1.unwrap())
    );
    println!(
        "  account:2 = {} (unchanged)",
        String::from_utf8_lossy(&balance2.unwrap())
    );

    // Example 3: Transaction isolation
    println!("\n--- Example 3: Transaction Isolation ---");
    {
        let tx1 = store.begin_transaction().await?;
        println!("✓ Started transaction 1");

        // tx1 reads initial value
        let value = tx1.get(shard_id, b"account:1").await?;
        println!(
            "  tx1 reads account:1 = {}",
            String::from_utf8_lossy(&value.unwrap())
        );

        // Another transaction commits a change
        let tx2 = store.begin_transaction().await?;
        tx2.put(shard_id, b"account:1", b"999").await?;
        tx2.commit().await?;
        println!("  tx2 committed: account:1 = 999");

        // tx1 still sees the old value (snapshot isolation)
        let value = tx1.get(shard_id, b"account:1").await?;
        println!(
            "  tx1 still reads account:1 = {} (snapshot isolation)",
            String::from_utf8_lossy(&value.unwrap())
        );

        tx1.rollback().await?;
        println!("✓ Transaction 1 rolled back");
    }

    // Verify final state
    let balance1 = store.get(shard_id, b"account:1").await?;
    println!(
        "Final account:1 = {}",
        String::from_utf8_lossy(&balance1.unwrap())
    );

    // Example 4: Multiple operations in one transaction
    println!("\n--- Example 4: Multiple Operations ---");
    {
        let tx = store.begin_transaction().await?;
        println!("✓ Started transaction");

        // Multiple puts (batched efficiently in LSM)
        tx.put(shard_id, b"user:1", b"Alice").await?;
        tx.put(shard_id, b"user:2", b"Bob").await?;
        tx.put(shard_id, b"user:3", b"Charlie").await?;
        println!("  Added 3 users");

        // Delete one
        tx.delete(shard_id, b"user:2").await?;
        println!("  Deleted user:2");

        // Update one
        tx.put(shard_id, b"user:1", b"Alice Smith").await?;
        println!("  Updated user:1");

        // Commit all changes atomically
        tx.commit().await?;
        println!("✓ All operations committed atomically");
    }

    // Verify final state
    println!("\nFinal state:");
    if let Some(value) = store.get(shard_id, b"user:1").await? {
        println!("  user:1 = {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = store.get(shard_id, b"user:2").await? {
        println!("  user:2 = {}", String::from_utf8_lossy(&value));
    } else {
        println!("  user:2 = <deleted>");
    }
    if let Some(value) = store.get(shard_id, b"user:3").await? {
        println!("  user:3 = {}", String::from_utf8_lossy(&value));
    }

    // LSM-specific transaction benefits
    println!("\n--- LSM Transaction Characteristics ---");
    println!("LSM Tree transaction advantages:");
    println!("  • Fast commits (sequential writes to WAL + MemTable)");
    println!("  • MVCC via timestamps in MemTable entries");
    println!("  • Snapshot isolation without blocking writes");
    println!("  • Efficient batch operations within transactions");
    println!("  • Durability via Write-Ahead Log");

    // Cleanup
    println!("\n--- Cleanup ---");
    store.drop_shard(shard_id).await?;
    println!("✓ Dropped shard");

    println!("\n=== Example Complete ===");
    Ok(())
}
