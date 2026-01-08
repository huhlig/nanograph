//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! Large-scale persistence benchmarks for B+Tree using on-disk storage

use nanograph_btree::{BPlusTree, BPlusTreeConfig, BTreePersistence};
use nanograph_vfs::{DynamicFileSystem, LocalFilesystem};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

/// Helper to create a tree with on-disk persistence
fn create_persistent_tree() -> (BPlusTree, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_str().unwrap().to_string();

    // Use LocalFilesystem for actual disk I/O
    let fs: Arc<dyn DynamicFileSystem> = Arc::new(LocalFilesystem::new(temp_dir.path()));

    let config = BPlusTreeConfig {
        max_keys: 128,
        min_keys: 64,
    };

    let persistence = Arc::new(BTreePersistence::new(fs, base_path).unwrap());
    let tree = BPlusTree::with_persistence(config, persistence).unwrap();

    (tree, temp_dir)
}

/// Benchmark: Write 100K sequential keys with persistence
fn bench_large_sequential_write() {
    println!("\n=== Large Sequential Write Benchmark (100K keys) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    let num_keys = 100_000;
    let start = Instant::now();

    for i in 0..num_keys {
        let key = format!("key_{:010}", i);
        let value = format!(
            "value_{:010}_with_some_additional_data_to_make_it_realistic",
            i
        );
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();

        if i % 10_000 == 0 && i > 0 {
            println!("  Written {} keys...", i);
        }
    }

    // Flush to disk
    tree.flush().unwrap();

    let duration = start.elapsed();
    println!("  Total time: {:?}", duration);
    println!(
        "  Throughput: {:.2} ops/sec",
        num_keys as f64 / duration.as_secs_f64()
    );
    println!(
        "  Avg latency: {:.2} µs/op",
        duration.as_micros() as f64 / num_keys as f64
    );
}

/// Benchmark: Write 100K random keys with persistence
fn bench_large_random_write() {
    println!("\n=== Large Random Write Benchmark (100K keys) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    let num_keys = 100_000;
    let start = Instant::now();

    // Use a simple LCG for deterministic random keys
    let mut state: u64 = 42;
    let lcg_next = |s: &mut u64| {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s
    };

    for i in 0..num_keys {
        let random_id = lcg_next(&mut state);
        let key = format!("key_{:016x}", random_id);
        let value = format!("value_{:016x}_with_additional_payload_data", random_id);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();

        if i % 10_000 == 0 && i > 0 {
            println!("  Written {} keys...", i);
        }
    }

    // Flush to disk
    tree.flush().unwrap();

    let duration = start.elapsed();
    println!("  Total time: {:?}", duration);
    println!(
        "  Throughput: {:.2} ops/sec",
        num_keys as f64 / duration.as_secs_f64()
    );
    println!(
        "  Avg latency: {:.2} µs/op",
        duration.as_micros() as f64 / num_keys as f64
    );
}

/// Benchmark: Write-Read-Update cycle with persistence
fn bench_write_read_update_cycle() {
    println!("\n=== Write-Read-Update Cycle Benchmark (50K keys) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    let num_keys = 50_000;

    // Phase 1: Initial write
    println!("  Phase 1: Initial write...");
    let write_start = Instant::now();
    for i in 0..num_keys {
        let key = format!("key_{:010}", i);
        let value = format!("initial_value_{:010}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }
    tree.flush().unwrap();
    let write_duration = write_start.elapsed();
    println!(
        "    Write time: {:?} ({:.2} ops/sec)",
        write_duration,
        num_keys as f64 / write_duration.as_secs_f64()
    );

    // Phase 2: Read all keys
    println!("  Phase 2: Reading all keys...");
    let read_start = Instant::now();
    for i in 0..num_keys {
        let key = format!("key_{:010}", i);
        let value = tree.get(&key.into_bytes()).unwrap();
        assert!(value.is_some());
    }
    let read_duration = read_start.elapsed();
    println!(
        "    Read time: {:?} ({:.2} ops/sec)",
        read_duration,
        num_keys as f64 / read_duration.as_secs_f64()
    );

    // Phase 3: Update all keys
    println!("  Phase 3: Updating all keys...");
    let update_start = Instant::now();
    for i in 0..num_keys {
        let key = format!("key_{:010}", i);
        let value = format!("updated_value_{:010}_with_more_data", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }
    tree.flush().unwrap();
    let update_duration = update_start.elapsed();
    println!(
        "    Update time: {:?} ({:.2} ops/sec)",
        update_duration,
        num_keys as f64 / update_duration.as_secs_f64()
    );

    let total_duration = write_duration + read_duration + update_duration;
    println!("  Total cycle time: {:?}", total_duration);
}

/// Benchmark: Persistence and recovery
fn bench_persistence_recovery() {
    println!("\n=== Persistence and Recovery Benchmark ===");

    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_str().unwrap().to_string();
    let num_keys = 20_000;

    // Phase 1: Write data and persist
    println!("  Phase 1: Writing {} keys and persisting...", num_keys);
    {
        let fs: Arc<dyn DynamicFileSystem> = Arc::new(LocalFilesystem::new(temp_dir.path()));
        let config = BPlusTreeConfig::default();
        let persistence = Arc::new(BTreePersistence::new(fs, base_path.clone()).unwrap());
        let tree = BPlusTree::with_persistence(config, persistence).unwrap();

        let write_start = Instant::now();
        for i in 0..num_keys {
            let key = format!("persist_key_{:010}", i);
            let value = format!("persist_value_{:010}_data", i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
        }

        // Force flush to disk
        tree.flush().unwrap();

        let write_duration = write_start.elapsed();
        println!("    Write + flush time: {:?}", write_duration);
    }
    // Tree dropped here, simulating shutdown

    // Phase 2: Recover and verify
    println!("  Phase 2: Recovering from disk...");
    {
        let fs: Arc<dyn DynamicFileSystem> = Arc::new(LocalFilesystem::new(temp_dir.path()));
        let config = BPlusTreeConfig::default();
        let persistence = Arc::new(BTreePersistence::new(fs, base_path.clone()).unwrap());

        let recovery_start = Instant::now();
        let tree = BPlusTree::with_persistence(config, persistence).unwrap();
        let recovery_duration = recovery_start.elapsed();
        println!("    Recovery time: {:?}", recovery_duration);

        // Verify all keys are present
        println!("  Phase 3: Verifying {} keys...", num_keys);
        let verify_start = Instant::now();
        let mut found = 0;
        for i in 0..num_keys {
            let key = format!("persist_key_{:010}", i);
            if tree.get(&key.into_bytes()).unwrap().is_some() {
                found += 1;
            }
        }
        let verify_duration = verify_start.elapsed();
        println!("    Verification time: {:?}", verify_duration);
        println!("    Keys found: {}/{}", found, num_keys);

        assert_eq!(found, num_keys, "Not all keys recovered!");
    }
}

/// Benchmark: Large value writes (1KB values)
fn bench_large_value_writes() {
    println!("\n=== Large Value Write Benchmark (10K keys, 1KB values) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    let num_keys = 10_000;
    let value_size = 1024; // 1KB
    let large_value = "x".repeat(value_size);

    let start = Instant::now();
    for i in 0..num_keys {
        let key = format!("large_key_{:010}", i);
        let value = format!("{}{:010}", large_value, i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();

        if i % 1_000 == 0 && i > 0 {
            println!("  Written {} keys...", i);
        }
    }

    tree.flush().unwrap();

    let duration = start.elapsed();
    let total_bytes = num_keys * value_size;
    println!("  Total time: {:?}", duration);
    println!(
        "  Throughput: {:.2} ops/sec",
        num_keys as f64 / duration.as_secs_f64()
    );
    println!(
        "  Data written: {:.2} MB",
        total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Write bandwidth: {:.2} MB/s",
        (total_bytes as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64()
    );
}

/// Benchmark: Mixed workload (70% reads, 30% writes)
fn bench_mixed_workload() {
    println!("\n=== Mixed Workload Benchmark (70% reads, 30% writes, 50K ops) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    // Pre-populate with some data
    println!("  Pre-populating with 10K keys...");
    for i in 0..10_000 {
        let key = format!("mixed_key_{:010}", i);
        let value = format!("mixed_value_{:010}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }
    tree.flush().unwrap();

    println!("  Running mixed workload...");
    let num_ops = 50_000;
    let mut state: u64 = 12345;
    let lcg_next = |s: &mut u64| {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s
    };

    let start = Instant::now();
    let mut reads = 0;
    let mut writes = 0;

    for i in 0..num_ops {
        let rand = lcg_next(&mut state);
        let key_id = (rand % 15_000) as usize; // Access range larger than initial data
        let key = format!("mixed_key_{:010}", key_id);

        if rand % 100 < 70 {
            // 70% reads
            let _ = tree.get(&key.into_bytes()).unwrap();
            reads += 1;
        } else {
            // 30% writes
            let value = format!("updated_value_{:010}_{}", key_id, i);
            tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
            writes += 1;
        }

        if i % 10_000 == 0 && i > 0 {
            println!("  Completed {} ops...", i);
        }
    }

    tree.flush().unwrap();

    let duration = start.elapsed();
    println!("  Total time: {:?}", duration);
    println!(
        "  Total ops: {} ({} reads, {} writes)",
        num_ops, reads, writes
    );
    println!(
        "  Throughput: {:.2} ops/sec",
        num_ops as f64 / duration.as_secs_f64()
    );
    println!(
        "  Avg latency: {:.2} µs/op",
        duration.as_micros() as f64 / num_ops as f64
    );
}

/// Benchmark: Tree statistics and structure
fn bench_tree_statistics() {
    println!("\n=== Tree Statistics Benchmark (50K keys) ===");
    let (tree, _temp_dir) = create_persistent_tree();

    let num_keys = 50_000;

    println!("  Inserting {} keys...", num_keys);
    for i in 0..num_keys {
        let key = format!("stats_key_{:010}", i);
        let value = format!("stats_value_{:010}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }

    tree.flush().unwrap();

    let stats = tree.stats();
    println!("  Tree Statistics:");
    println!("    Height: {}", stats.height);
    println!(
        "    Total nodes: {}",
        stats.num_internal_nodes + stats.num_leaf_nodes
    );
    println!("    Internal nodes: {}", stats.num_internal_nodes);
    println!("    Leaf nodes: {}", stats.num_leaf_nodes);
    println!("    Total keys: {}", stats.num_keys);
    println!(
        "    Avg keys per node: {:.2}",
        stats.num_keys as f64 / (stats.num_internal_nodes + stats.num_leaf_nodes) as f64
    );
}

fn main() {
    println!("B+Tree Large-Scale Persistence Benchmarks");
    println!("==========================================");

    bench_large_sequential_write();
    bench_large_random_write();
    bench_write_read_update_cycle();
    bench_persistence_recovery();
    bench_large_value_writes();
    bench_mixed_workload();
    bench_tree_statistics();

    println!("\n=== All benchmarks completed ===");
}
