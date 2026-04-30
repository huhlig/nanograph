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

//! Comparative Benchmarks for KeyValueShardStore Implementations
//!
//! This benchmark suite compares the performance of different KeyValueShardStore
//! implementations (Memory, LMDB, LSM) using identical test scenarios.
//!
//! Run with: `cargo bench --bench comparative_benchmarks`

mod common;

use criterion::{criterion_group, criterion_main, Criterion};
use nanograph_kvt::{KeyValueShardStore, MemoryKeyValueShardStore, ShardId};
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_lsm::LSMKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::sync::Arc;
use tempfile::TempDir;

// Setup functions for each implementation

fn setup_memory() -> (Arc<MemoryKeyValueShardStore>, ShardId) {
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let shard_id = ShardId::new(1);
    (store, shard_id)
}

fn setup_lmdb() -> (LMDBKeyValueStore, ShardId, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    KeyValueShardStore::create_shard(&store, shard_id, vfs, data_path, wal_path).unwrap();

    (store, shard_id, temp_dir)
}

fn setup_lsm() -> (Arc<LSMKeyValueStore>, ShardId, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LSMKeyValueStore::new());
    
    // Initialize transaction manager
    store.init_tx_manager();

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    KeyValueShardStore::create_shard(store.as_ref(), shard_id, vfs, data_path, wal_path).unwrap();

    (store, shard_id, temp_dir)
}

// Comparative benchmark functions

fn compare_single_operations(c: &mut Criterion) {
    // Memory
    let (memory_store, memory_shard) = setup_memory();
    common::bench_single_operations(c, "Memory", memory_store.as_ref(), memory_shard);

    // LMDB
    let (lmdb_store, lmdb_shard, _lmdb_temp) = setup_lmdb();
    common::bench_single_operations(c, "LMDB", &lmdb_store, lmdb_shard);

    // LSM
    let (lsm_store, lsm_shard, _lsm_temp) = setup_lsm();
    common::bench_single_operations(c, "LSM", lsm_store.as_ref(), lsm_shard);
}

fn compare_batch_operations(c: &mut Criterion) {
    // Memory
    let (memory_store, memory_shard) = setup_memory();
    common::bench_batch_operations(c, "Memory", memory_store.as_ref(), memory_shard);

    // LMDB
    let (lmdb_store, lmdb_shard, _lmdb_temp) = setup_lmdb();
    common::bench_batch_operations(c, "LMDB", &lmdb_store, lmdb_shard);

    // LSM
    let (lsm_store, lsm_shard, _lsm_temp) = setup_lsm();
    common::bench_batch_operations(c, "LSM", lsm_store.as_ref(), lsm_shard);
}

fn compare_scan_operations(c: &mut Criterion) {
    // Memory
    let (memory_store, memory_shard) = setup_memory();
    common::bench_scan_operations(c, "Memory", memory_store.as_ref(), memory_shard);

    // LMDB
    let (lmdb_store, lmdb_shard, _lmdb_temp) = setup_lmdb();
    common::bench_scan_operations(c, "LMDB", &lmdb_store, lmdb_shard);

    // LSM
    let (lsm_store, lsm_shard, _lsm_temp) = setup_lsm();
    common::bench_scan_operations(c, "LSM", lsm_store.as_ref(), lsm_shard);
}

fn compare_mixed_workloads(c: &mut Criterion) {
    // Memory
    let (memory_store, memory_shard) = setup_memory();
    common::bench_mixed_workloads(c, "Memory", memory_store.as_ref(), memory_shard);

    // LMDB
    let (lmdb_store, lmdb_shard, _lmdb_temp) = setup_lmdb();
    common::bench_mixed_workloads(c, "LMDB", &lmdb_store, lmdb_shard);

    // LSM
    let (lsm_store, lsm_shard, _lsm_temp) = setup_lsm();
    common::bench_mixed_workloads(c, "LSM", lsm_store.as_ref(), lsm_shard);
}

criterion_group!(
    benches,
    compare_single_operations,
    compare_batch_operations,
    compare_scan_operations,
    compare_mixed_workloads
);
criterion_main!(benches);

// Made with Bob
