//! Basic usage example for Nanograph ART
//!
//! This example demonstrates the core functionality of the Adaptive Radix Tree,
//! including insertion, retrieval, deletion, and iteration.

use nanograph_art::{AdaptiveRadixTree, ArtError};

fn main() -> Result<(), ArtError> {
    println!("=== Nanograph ART Basic Usage Example ===\n");

    // Create a new tree
    let mut tree = AdaptiveRadixTree::new();
    println!("✓ Created new ART");

    // Insert some key-value pairs
    println!("\n--- Inserting Data ---");
    tree.insert(b"apple".to_vec(), "A red fruit".to_string())?;
    tree.insert(b"apricot".to_vec(), "An orange fruit".to_string())?;
    tree.insert(b"banana".to_vec(), "A yellow fruit".to_string())?;
    tree.insert(b"cherry".to_vec(), "A small red fruit".to_string())?;
    tree.insert(b"date".to_vec(), "A sweet brown fruit".to_string())?;
    println!("✓ Inserted 5 fruits");

    // Retrieve values
    println!("\n--- Retrieving Data ---");
    if let Some(value) = tree.get(b"apple") {
        println!("apple: {}", value);
    }
    if let Some(value) = tree.get(b"banana") {
        println!("banana: {}", value);
    }

    // Check for non-existent key
    match tree.get(b"grape") {
        Some(_) => println!("grape: found"),
        None => println!("grape: not found"),
    }

    // Iterate over all entries
    println!("\n--- Iterating Over All Entries ---");
    for (key, value) in tree.iter() {
        let key_str = String::from_utf8_lossy(&key);
        println!("{}: {}", key_str, value);
    }

    // Delete an entry
    println!("\n--- Deleting Data ---");
    tree.remove(b"banana")?;
    println!("✓ Deleted 'banana'");

    // Verify deletion
    match tree.get(b"banana") {
        Some(_) => println!("✗ banana still exists (unexpected)"),
        None => println!("✓ banana successfully deleted"),
    }

    // Show remaining entries
    println!("\n--- Remaining Entries ---");
    for (key, value) in tree.iter() {
        let key_str = String::from_utf8_lossy(&key);
        println!("{}: {}", key_str, value);
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
