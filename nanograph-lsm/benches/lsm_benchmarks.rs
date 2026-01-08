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

//! Benchmarks for nanograph-lsm

use nanograph_lsm::{LSMTreeEngine, LSMTreeOptions};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

/// Helper to create a test engine
fn create_bench_engine() -> (LSMTreeEngine, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_str().unwrap().to_string();

    let wal_fs = MemoryFileSystem::new();
    let sstable_fs: Arc<dyn nanograph_vfs::DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
    let wal_path = Path::from("/wal");
    let wal_config = WriteAheadLogConfig::new(0);
    let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config).unwrap();

    let options = LSMTreeOptions::default();
    let engine = LSMTreeEngine::new(sstable_fs, base_path, options, wal).unwrap();

    (engine, temp_dir)
}

fn bench_sequential_writes(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    let start = Instant::now();
    for i in 0..n {
        let key = format!("{:010}", i);
        let value = format!("value{}", i);
        black_box(engine.put(key.into_bytes(), value.into_bytes()).unwrap());
    }
    start.elapsed()
}

fn bench_random_writes(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    // Generate random keys
    let mut keys: Vec<usize> = (0..n).collect();
    let mut rng = rand::rng();
    rand::seq::SliceRandom::shuffle(keys.as_mut_slice(), &mut rng);

    let start = Instant::now();
    for i in keys {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        black_box(engine.put(key.into_bytes(), value.into_bytes()).unwrap());
    }
    start.elapsed()
}

fn bench_sequential_reads(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    // Populate data
    for i in 0..n {
        let key = format!("{:010}", i);
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    let start = Instant::now();
    for i in 0..n {
        let key = format!("{:010}", i);
        black_box(engine.get(key.as_bytes()).unwrap());
    }
    start.elapsed()
}

fn bench_random_reads(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    // Populate data
    for i in 0..n {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Generate random keys
    let mut keys: Vec<usize> = (0..n).collect();
    let mut rng = rand::rng();
    rand::seq::SliceRandom::shuffle(keys.as_mut_slice(), &mut rng);

    let start = Instant::now();
    for i in keys {
        let key = format!("key{}", i);
        black_box(engine.get(key.as_bytes()).unwrap());
    }
    start.elapsed()
}

fn bench_mixed_workload(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    let start = Instant::now();
    for i in 0..n {
        if i % 3 == 0 {
            // Write
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            black_box(engine.put(key.into_bytes(), value.into_bytes()).unwrap());
        } else if i % 3 == 1 {
            // Read
            let key = format!("key{}", i / 3);
            black_box(engine.get(key.as_bytes()).unwrap());
        } else {
            // Update
            let key = format!("key{}", i / 3);
            let value = format!("updated_value{}", i);
            black_box(engine.put(key.into_bytes(), value.into_bytes()).unwrap());
        }
    }
    start.elapsed()
}

fn bench_large_values(n: usize, value_size: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    let large_value = vec![b'x'; value_size];

    let start = Instant::now();
    for i in 0..n {
        let key = format!("key{}", i);
        black_box(engine.put(key.into_bytes(), large_value.clone()).unwrap());
    }
    start.elapsed()
}

fn bench_deletes(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    // Populate data
    for i in 0..n {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    let start = Instant::now();
    for i in 0..n {
        let key = format!("key{}", i);
        black_box(engine.delete(key.into_bytes()).unwrap());
    }
    start.elapsed()
}

fn bench_memtable_flush(n: usize) -> std::time::Duration {
    let (engine, _temp_dir) = create_bench_engine();

    // Fill memtable
    let large_value = vec![b'x'; 1024]; // 1KB
    for i in 0..n {
        let key = format!("key{}", i);
        engine.put(key.into_bytes(), large_value.clone()).unwrap();
    }

    let start = Instant::now();
    black_box(engine.flush().unwrap());
    start.elapsed()
}

fn print_benchmark_result(name: &str, duration: std::time::Duration, ops: usize) {
    let ops_per_sec = ops as f64 / duration.as_secs_f64();
    let latency_us = duration.as_micros() as f64 / ops as f64;

    println!(
        "{:30} | {:>10.2} ops/sec | {:>8.2} µs/op | {:>8.2} ms total",
        name,
        ops_per_sec,
        latency_us,
        duration.as_millis()
    );
}

fn main() {
    println!("\n=== LSM Tree Benchmarks ===\n");
    println!(
        "{:30} | {:>15} | {:>12} | {:>14}",
        "Benchmark", "Throughput", "Latency", "Total Time"
    );
    println!("{:-<30}-+-{:-<15}-+-{:-<12}-+-{:-<14}", "", "", "", "");

    // Sequential writes
    let duration = bench_sequential_writes(10_000);
    print_benchmark_result("Sequential Writes (10K)", duration, 10_000);

    // Random writes
    let duration = bench_random_writes(10_000);
    print_benchmark_result("Random Writes (10K)", duration, 10_000);

    // Sequential reads
    let duration = bench_sequential_reads(10_000);
    print_benchmark_result("Sequential Reads (10K)", duration, 10_000);

    // Random reads
    let duration = bench_random_reads(10_000);
    print_benchmark_result("Random Reads (10K)", duration, 10_000);

    // Mixed workload
    let duration = bench_mixed_workload(10_000);
    print_benchmark_result("Mixed Workload (10K)", duration, 10_000);

    // Large values (1KB)
    let duration = bench_large_values(1_000, 1024);
    print_benchmark_result("Large Values 1KB (1K)", duration, 1_000);

    // Large values (10KB)
    let duration = bench_large_values(1_000, 10_240);
    print_benchmark_result("Large Values 10KB (1K)", duration, 1_000);

    // Large values (100KB)
    let duration = bench_large_values(100, 102_400);
    print_benchmark_result("Large Values 100KB (100)", duration, 100);

    // Deletes
    let duration = bench_deletes(10_000);
    print_benchmark_result("Deletes (10K)", duration, 10_000);

    // Memtable flush
    let duration = bench_memtable_flush(1_000);
    print_benchmark_result("Memtable Flush (1K)", duration, 1);

    println!("\n=== Scalability Tests ===\n");
    println!(
        "{:30} | {:>15} | {:>12} | {:>14}",
        "Benchmark", "Throughput", "Latency", "Total Time"
    );
    println!("{:-<30}-+-{:-<15}-+-{:-<12}-+-{:-<14}", "", "", "", "");

    // Test different scales
    for &n in &[100, 1_000, 10_000, 100_000] {
        let duration = bench_sequential_writes(n);
        let name = format!("Sequential Writes ({})", n);
        print_benchmark_result(&name, duration, n);
    }

    println!("\n");
}

// Made with Bob
