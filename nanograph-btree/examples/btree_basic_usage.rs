//! Basic usage example for Nanograph B+Tree
//!
//! This example demonstrates the core functionality of the B+Tree,
//! including insertion, retrieval, deletion, and iteration.

use nanograph_btree::{BPlusTree, BPlusTreeConfig, BTreeError};

fn main() -> Result<(), BTreeError> {
    println!("=== Nanograph B+Tree Basic Usage Example ===\n");

    // Create a new tree with default configuration
    let config = BPlusTreeConfig::default();
    let mut tree = BPlusTree::new(config);
    println!("✓ Created new B+Tree (max_keys: 128, min_keys: 64)");

    // Insert some key-value pairs
    println!("\n--- Inserting Data ---");
    tree.insert(b"apple".to_vec(), b"A red fruit".to_vec())?;
    tree.insert(b"apricot".to_vec(), b"An orange fruit".to_vec())?;
    tree.insert(b"banana".to_vec(), b"A yellow fruit".to_vec())?;
    tree.insert(b"cherry".to_vec(), b"A small red fruit".to_vec())?;
    tree.insert(b"date".to_vec(), b"A sweet brown fruit".to_vec())?;
    println!("✓ Inserted 5 fruits");

    // Retrieve values
    println!("\n--- Retrieving Data ---");
    if let Some(value) = tree.get(b"apple")? {
        println!("apple: {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = tree.get(b"banana")? {
        println!("banana: {}", String::from_utf8_lossy(&value));
    }

    // Check for non-existent key
    match tree.get(b"grape")? {
        Some(_) => println!("grape: found"),
        None => println!("grape: not found"),
    }

    // Iterate over all entries (B+Tree maintains sorted order)
    println!("\n--- Iterating Over All Entries (Sorted) ---");
    println!("Note: Direct iteration requires using the iterator module");
    println!("For this example, we'll demonstrate with individual gets");

    // Get all keys we know about
    let keys: Vec<&[u8]> = vec![b"apple", b"apricot", b"banana", b"cherry", b"date"];
    for key in &keys {
        if let Some(value) = tree.get(*key)? {
            let key_str = String::from_utf8_lossy(key);
            let value_str = String::from_utf8_lossy(&value);
            println!("{}: {}", key_str, value_str);
        }
    }

    // Delete an entry
    println!("\n--- Deleting Data ---");
    tree.delete(b"banana")?;
    println!("✓ Deleted 'banana'");

    // Verify deletion
    match tree.get(b"banana")? {
        Some(_) => println!("✗ banana still exists (unexpected)"),
        None => println!("✓ banana successfully deleted"),
    }

    // Show remaining entries
    println!("\n--- Remaining Entries ---");
    let remaining_keys: Vec<&[u8]> = vec![b"apple", b"apricot", b"cherry", b"date"];
    for key in &remaining_keys {
        if let Some(value) = tree.get(*key)? {
            let key_str = String::from_utf8_lossy(key);
            let value_str = String::from_utf8_lossy(&value);
            println!("{}: {}", key_str, value_str);
        }
    }

    // Get tree statistics
    println!("\n--- Tree Statistics ---");
    let stats = tree.stats();
    println!("Total keys: {}", stats.num_keys);
    println!("Tree height: {}", stats.height);
    println!("Leaf nodes: {}", stats.num_leaf_nodes);
    println!("Internal nodes: {}", stats.num_internal_nodes);

    println!("\n=== Example Complete ===");
    Ok(())
}

// Made with Bob
