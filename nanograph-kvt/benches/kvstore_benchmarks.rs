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

//! Reference Benchmark Implementation for MemoryKeyValueShardStore
//!
//! This demonstrates how to use the common benchmark suite with a specific
//! KeyValueShardStore implementation.

mod common;

use criterion::{criterion_group, criterion_main, Criterion};
use nanograph_kvt::{MemoryKeyValueShardStore, ShardId};
use std::sync::Arc;

/// Setup a Memory store for benchmarking
fn setup_store() -> (Arc<MemoryKeyValueShardStore>, ShardId) {
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let shard_id = ShardId::new(1);
    (store, shard_id)
}

fn memory_single_operations(c: &mut Criterion) {
    let (store, shard_id) = setup_store();
    common::bench_single_operations(c, "Memory", store.as_ref(), shard_id);
}

fn memory_batch_operations(c: &mut Criterion) {
    let (store, shard_id) = setup_store();
    common::bench_batch_operations(c, "Memory", store.as_ref(), shard_id);
}

fn memory_scan_operations(c: &mut Criterion) {
    let (store, shard_id) = setup_store();
    common::bench_scan_operations(c, "Memory", store.as_ref(), shard_id);
}

fn memory_mixed_workloads(c: &mut Criterion) {
    let (store, shard_id) = setup_store();
    common::bench_mixed_workloads(c, "Memory", store.as_ref(), shard_id);
}

criterion_group!(
    benches,
    memory_single_operations,
    memory_batch_operations,
    memory_scan_operations,
    memory_mixed_workloads
);
criterion_main!(benches);

// Made with Bob
