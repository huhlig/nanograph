//! Persistence usage example for Nanograph ART
//!
//! This example demonstrates saving and loading ART trees to/from disk
//! using the VFS abstraction layer.

use nanograph_art::{AdaptiveRadixTree, ArtPersistence};
use nanograph_vfs::MemoryFileSystem;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Nanograph ART Persistence Example ===\n");

    // Create filesystem and persistence manager
    // Using MemoryFileSystem for this example (can also use LocalFilesystem for real disk I/O)
    let fs = Arc::new(MemoryFileSystem::new());
    let persistence = ArtPersistence::new(fs.clone(), "/art_data".to_string())?;
    println!("✓ Created persistence manager with in-memory filesystem");

    // Example 1: Save and load a tree
    println!("\n--- Example 1: Save and Load ---");
    {
        // Create and populate a tree
        let mut tree = AdaptiveRadixTree::new();
        tree.insert(b"apple".to_vec(), "A red fruit".to_string())?;
        tree.insert(b"banana".to_vec(), "A yellow fruit".to_string())?;
        tree.insert(b"cherry".to_vec(), "A small red fruit".to_string())?;
        tree.insert(b"date".to_vec(), "A sweet brown fruit".to_string())?;
        println!("✓ Created tree with 4 entries");

        // Save to disk
        persistence.save_tree(&tree)?;
        println!("✓ Saved tree to disk");

        println!("✓ Tree saved successfully");
    }

    // Load the tree back
    {
        let loaded_tree: AdaptiveRadixTree<String> = persistence.load_tree()?;
        println!("✓ Loaded tree from disk");

        // Verify data
        println!("\nVerifying loaded data:");
        assert_eq!(loaded_tree.get(b"apple"), Some("A red fruit".to_string()));
        println!("  ✓ apple = A red fruit");
        assert_eq!(
            loaded_tree.get(b"banana"),
            Some("A yellow fruit".to_string())
        );
        println!("  ✓ banana = A yellow fruit");
        assert_eq!(
            loaded_tree.get(b"cherry"),
            Some("A small red fruit".to_string())
        );
        println!("  ✓ cherry = A small red fruit");
        assert_eq!(
            loaded_tree.get(b"date"),
            Some("A sweet brown fruit".to_string())
        );
        println!("  ✓ date = A sweet brown fruit");
    }

    // Example 2: Update and save again
    println!("\n--- Example 2: Update and Save ---");
    {
        let mut tree: AdaptiveRadixTree<String> = persistence.load_tree()?;
        println!("✓ Loaded existing tree");

        // Add more entries
        tree.insert(b"elderberry".to_vec(), "A dark purple berry".to_string())?;
        tree.insert(b"fig".to_vec(), "A sweet soft fruit".to_string())?;
        println!("✓ Added 2 more entries");

        // Remove one entry
        tree.remove(b"banana")?;
        println!("✓ Removed banana");

        // Save updated tree
        persistence.save_tree(&tree)?;
        println!("✓ Saved updated tree");
    }

    // Verify updates
    {
        let tree: AdaptiveRadixTree<String> = persistence.load_tree()?;
        println!("✓ Loaded updated tree");

        println!("\nVerifying updates:");
        assert_eq!(
            tree.get(b"elderberry"),
            Some("A dark purple berry".to_string())
        );
        println!("  ✓ elderberry exists");
        assert_eq!(tree.get(b"fig"), Some("A sweet soft fruit".to_string()));
        println!("  ✓ fig exists");
        assert_eq!(tree.get(b"banana"), None);
        println!("  ✓ banana was deleted");
    }

    // Example 3: Working with different value types
    println!("\n--- Example 3: Different Value Types ---");
    {
        // Create a tree with integer values
        let mut int_tree = AdaptiveRadixTree::new();
        int_tree.insert(b"one".to_vec(), 1)?;
        int_tree.insert(b"two".to_vec(), 2)?;
        int_tree.insert(b"three".to_vec(), 3)?;
        println!("✓ Created tree with integer values");

        // Save to a different path
        let int_persistence = ArtPersistence::new(fs.clone(), "/int_tree".to_string())?;
        int_persistence.save_tree(&int_tree)?;
        println!("✓ Saved integer tree");

        // Load and verify
        let loaded_int_tree: AdaptiveRadixTree<i32> = int_persistence.load_tree()?;
        assert_eq!(loaded_int_tree.get(b"one"), Some(1));
        assert_eq!(loaded_int_tree.get(b"two"), Some(2));
        assert_eq!(loaded_int_tree.get(b"three"), Some(3));
        println!("✓ Loaded and verified integer tree");
    }

    // Example 4: Empty tree
    println!("\n--- Example 4: Empty Tree ---");
    {
        let empty_tree: AdaptiveRadixTree<String> = AdaptiveRadixTree::new();
        persistence.save_tree(&empty_tree)?;
        println!("✓ Saved empty tree");

        let loaded_empty: AdaptiveRadixTree<String> = persistence.load_tree()?;
        assert_eq!(loaded_empty.len(), 0);
        println!("✓ Loaded empty tree (length = 0)");
    }

    println!("\n=== Example Complete ===");
    println!("Note: This example used an in-memory filesystem.");
    println!("For real disk persistence, use LocalFilesystem instead of MemoryFileSystem.");
    Ok(())
}

// Made with Bob
