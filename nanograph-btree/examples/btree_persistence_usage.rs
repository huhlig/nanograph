//! Persistence usage example for Nanograph B+Tree
//!
//! This example demonstrates saving and loading B+Tree data to/from disk
//! using the VFS abstraction layer and Write-Ahead Log (WAL) for durability.

use nanograph_btree::{BPlusTree, BPlusTreeConfig, BTreePersistence};
use nanograph_vfs::MemoryFileSystem;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph B+Tree Persistence Example ===\n");

    // Create filesystem and persistence manager
    // Using MemoryFileSystem for this example (can also use LocalFilesystem for real disk I/O)
    let fs = Arc::new(MemoryFileSystem::new());
    let persistence = Arc::new(BTreePersistence::new(
        fs.clone(),
        "/btree_data".to_string(),
    )?);
    println!("✓ Created persistence manager with in-memory filesystem");

    // Example 1: Save and load a tree
    println!("\n--- Example 1: Save and Load ---");
    {
        // Create and populate a tree with persistence
        let config = BPlusTreeConfig::default();
        let mut tree = BPlusTree::with_persistence(config, persistence.clone())?;

        tree.insert(b"apple".to_vec(), b"A red fruit".to_vec())?;
        tree.insert(b"banana".to_vec(), b"A yellow fruit".to_vec())?;
        tree.insert(b"cherry".to_vec(), b"A small red fruit".to_vec())?;
        tree.insert(b"date".to_vec(), b"A sweet brown fruit".to_vec())?;
        println!("✓ Created tree with 4 entries");

        // Save to disk (flush)
        tree.flush()?;
        println!("✓ Saved tree to disk");
    }

    // Load the tree back
    {
        let config = BPlusTreeConfig::default();
        let loaded_tree = BPlusTree::with_persistence(config, persistence.clone())?;
        println!("✓ Loaded tree from disk");

        // Verify data
        println!("\nVerifying loaded data:");
        if let Some(value) = loaded_tree.get(b"apple")? {
            println!("  ✓ apple = {}", String::from_utf8_lossy(&value));
        }
        if let Some(value) = loaded_tree.get(b"banana")? {
            println!("  ✓ banana = {}", String::from_utf8_lossy(&value));
        }
        if let Some(value) = loaded_tree.get(b"cherry")? {
            println!("  ✓ cherry = {}", String::from_utf8_lossy(&value));
        }
        if let Some(value) = loaded_tree.get(b"date")? {
            println!("  ✓ date = {}", String::from_utf8_lossy(&value));
        }
    }

    // Example 2: Update and save again
    println!("\n--- Example 2: Update and Save ---");
    {
        let config = BPlusTreeConfig::default();
        let mut tree = BPlusTree::with_persistence(config, persistence.clone())?;
        println!("✓ Loaded existing tree");

        // Add more entries
        tree.insert(b"elderberry".to_vec(), b"A dark purple berry".to_vec())?;
        tree.insert(b"fig".to_vec(), b"A sweet soft fruit".to_vec())?;
        println!("✓ Added 2 more entries");

        // Remove one entry
        tree.delete(b"banana")?;
        println!("✓ Removed banana");

        // Save updated tree
        tree.flush()?;
        println!("✓ Saved updated tree");
    }

    // Verify updates
    {
        let config = BPlusTreeConfig::default();
        let tree = BPlusTree::with_persistence(config, persistence.clone())?;
        println!("✓ Loaded updated tree");

        println!("\nVerifying updates:");
        if let Some(value) = tree.get(b"elderberry")? {
            println!("  ✓ elderberry = {}", String::from_utf8_lossy(&value));
        }
        if let Some(value) = tree.get(b"fig")? {
            println!("  ✓ fig = {}", String::from_utf8_lossy(&value));
        }
        match tree.get(b"banana")? {
            Some(_) => println!("  ✗ banana still exists (unexpected)"),
            None => println!("  ✓ banana was deleted"),
        }
    }

    // Example 3: Working with persistence for durability
    println!("\n--- Example 3: Persistence for Durability ---");
    {
        let config = BPlusTreeConfig::default();
        let mut tree = BPlusTree::with_persistence(config, persistence.clone())?;
        println!("✓ Created tree with persistence support");

        // Insert operations are persisted
        tree.insert(b"grape".to_vec(), b"A purple fruit".to_vec())?;
        tree.insert(b"kiwi".to_vec(), b"A fuzzy green fruit".to_vec())?;
        println!("✓ Inserted 2 entries");

        // Flush saves tree state
        tree.flush()?;
        println!("✓ Flush completed (tree state saved)");
    }

    // Example 4: Tree statistics
    println!("\n--- Example 4: Tree Statistics ---");
    {
        let config = BPlusTreeConfig::default();
        let tree = BPlusTree::with_persistence(config, persistence.clone())?;

        let stats = tree.stats();
        println!("Tree statistics:");
        println!("  Total keys: {}", stats.num_keys);
        println!("  Tree height: {}", stats.height);
        println!("  Leaf nodes: {}", stats.num_leaf_nodes);
        println!("  Internal nodes: {}", stats.num_internal_nodes);
        println!(
            "  Average keys per leaf: {:.2}",
            stats.num_keys as f64 / stats.num_leaf_nodes as f64
        );
    }

    println!("\n=== Example Complete ===");
    println!("Note: This example used an in-memory filesystem.");
    println!("For real disk persistence, use LocalFilesystem instead of MemoryFileSystem.");
    Ok(())
}
