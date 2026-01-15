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

//! Persistence and I/O benchmarks for Adaptive Radix Tree

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use futures::StreamExt;
use nanograph_art::{AdaptiveRadixTree, ArtKeyValueStore};
use nanograph_kvt::{KeyValueShardStore, ShardIndex, TableId};
use std::hint::black_box;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::runtime::Runtime;

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_sequential_kvs(count: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..count)
        .map(|i| {
            let key = format!("key{:08}", i).into_bytes();
            let value = format!("value_{}", i).into_bytes();
            (key, value)
        })
        .collect()
}

fn create_populated_tree(count: usize) -> AdaptiveRadixTree<Vec<u8>> {
    let mut tree = AdaptiveRadixTree::new();
    let kvs = generate_sequential_kvs(count);

    for (key, value) in kvs {
        tree.insert(key, value).unwrap();
    }

    tree
}

async fn create_populated_store(
    count: usize,
) -> (ArtKeyValueStore, nanograph_kvt::ShardId, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = ArtKeyValueStore::default();
    let table_id = TableId::new(1);
    let shard_index = ShardIndex::new(0);
    let shard = store.create_shard(table_id, shard_index).await.unwrap();
    let kvs = generate_sequential_kvs(count);

    for (key, value) in kvs {
        store.put(shard, &key, &value).await.unwrap();
    }

    (store, shard, temp_dir)
}

// ============================================================================
// KVStore Operation Benchmarks
// ============================================================================

fn bench_kvstore_put(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("kvstore_put");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let store = ArtKeyValueStore::default();
                let table_id = TableId::new(1);
                let shard_index = ShardIndex::new(0);
                let shard = store.create_shard(table_id, shard_index).await.unwrap();
                let kvs = generate_sequential_kvs(size);

                for (key, value) in kvs {
                    store
                        .put(shard, black_box(&key), black_box(&value))
                        .await
                        .unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_kvstore_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("kvstore_get");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size));
            let keys: Vec<_> = (0..size)
                .map(|i| format!("key{:08}", i).into_bytes())
                .collect();

            b.to_async(&rt).iter(|| async {
                for key in &keys {
                    black_box(store.get(shard, black_box(key)).await.unwrap());
                }
            });
        });
    }

    group.finish();
}

fn bench_kvstore_delete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("kvstore_delete");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || rt.block_on(create_populated_store(size)),
                |(store, shard, _temp_dir)| async move {
                    let keys: Vec<_> = (0..size)
                        .map(|i| format!("key{:08}", i).into_bytes())
                        .collect();

                    for key in keys {
                        store.delete(shard, black_box(&key)).await.unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ============================================================================
// Range Query Benchmarks
// ============================================================================

fn bench_kvstore_range_scan(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("kvstore_range_scan");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size));

            b.to_async(&rt).iter(|| async {
                let range = nanograph_kvt::KeyRange::new(
                    std::ops::Bound::Unbounded,
                    std::ops::Bound::Unbounded,
                );
                let mut stream = store.scan(shard, range).await.unwrap();
                let mut count = 0;

                while let Some(result) = stream.next().await {
                    black_box(result.unwrap());
                    count += 1;
                }

                black_box(count);
            });
        });
    }

    group.finish();
}

fn bench_kvstore_bounded_range(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("kvstore_bounded_range");

    for size in [1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(100));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size));

            b.to_async(&rt).iter(|| async {
                let start_key = format!("key{:08}", size / 4).into_bytes();
                let end_key = format!("key{:08}", size / 4 + 100).into_bytes();
                let range = nanograph_kvt::KeyRange::new(
                    std::ops::Bound::Included(start_key),
                    std::ops::Bound::Excluded(end_key),
                );

                let mut stream = store.scan(shard, range).await.unwrap();
                let mut count = 0;

                while let Some(result) = stream.next().await {
                    black_box(result.unwrap());
                    count += 1;
                }

                black_box(count);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Concurrent Operation Benchmarks
// ============================================================================

fn bench_concurrent_reads(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_reads");

    for size in [1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size));
            let store = Arc::new(store);

            b.to_async(&rt).iter(|| {
                let store = Arc::clone(&store);
                async move {
                    let mut handles = vec![];

                    for i in 0..10 {
                        let store = Arc::clone(&store);
                        let handle = tokio::spawn(async move {
                            for j in 0..size / 10 {
                                let key = format!("key{:08}", i * (size / 10) + j).into_bytes();
                                black_box(store.get(shard, &key).await.unwrap());
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                }
            });
        });
    }

    group.finish();
}

fn bench_concurrent_writes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_writes");

    for size in [1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let store = Arc::new(ArtKeyValueStore::default());
                let table_id = TableId::new(1);
                let shard_index = ShardIndex::new(0);
                let shard = store.create_shard(table_id, shard_index).await.unwrap();

                let mut handles = vec![];

                for i in 0..10 {
                    let store = Arc::clone(&store);
                    let handle = tokio::spawn(async move {
                        for j in 0..size / 10 {
                            let key = format!("key_{}_{}", i, j).into_bytes();
                            let value = format!("value_{}_{}", i, j).into_bytes();
                            store.put(shard, &key, &value).await.unwrap();
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_concurrent_mixed(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_mixed");

    for size in [1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size / 2));
            let store = Arc::new(store);

            b.to_async(&rt).iter(|| {
                let store = Arc::clone(&store);
                async move {
                    let mut handles = vec![];

                    // Readers
                    for i in 0..5 {
                        let store = Arc::clone(&store);
                        let handle = tokio::spawn(async move {
                            for j in 0..size / 10 {
                                let key = format!("key{:08}", i * (size / 10) + j).into_bytes();
                                black_box(store.get(shard, &key).await.unwrap());
                            }
                        });
                        handles.push(handle);
                    }

                    // Writers
                    for i in 0..5 {
                        let store = Arc::clone(&store);
                        let handle = tokio::spawn(async move {
                            for j in 0..size / 10 {
                                let key = format!("key_new_{}_{}", i, j).into_bytes();
                                let value = format!("value_{}_{}", i, j).into_bytes();
                                store.put(shard, &key, &value).await.unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Batch Operation Benchmarks
// ============================================================================

fn bench_batch_insert(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_insert");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let kvs = generate_sequential_kvs(size);

            b.to_async(&rt).iter(|| async {
                let store = ArtKeyValueStore::default();
                let table_id = TableId::new(1);
                let shard_index = ShardIndex::new(0);
                let shard = store.create_shard(table_id, shard_index).await.unwrap();

                for (key, value) in &kvs {
                    store
                        .put(shard, black_box(key), black_box(value))
                        .await
                        .unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_batch_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_get");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard, _temp_dir) = rt.block_on(create_populated_store(size));
            let keys: Vec<_> = (0..size)
                .map(|i| format!("key{:08}", i).into_bytes())
                .collect();

            b.to_async(&rt).iter(|| async {
                for key in &keys {
                    black_box(store.get(shard, black_box(key)).await.unwrap());
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Memory and Size Benchmarks
// ============================================================================

fn bench_tree_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_memory_usage");

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let tree = create_populated_tree(size);
                black_box(tree.memory_usage());
            });
        });
    }

    group.finish();
}

fn bench_tree_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_size");

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let tree = create_populated_tree(size);

            b.iter(|| {
                black_box(tree.len());
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    kvstore_benches,
    bench_kvstore_put,
    bench_kvstore_get,
    bench_kvstore_delete
);

criterion_group!(
    range_benches,
    bench_kvstore_range_scan,
    bench_kvstore_bounded_range
);

criterion_group!(
    concurrent_benches,
    bench_concurrent_reads,
    bench_concurrent_writes,
    bench_concurrent_mixed
);

criterion_group!(batch_benches, bench_batch_insert, bench_batch_get);

criterion_group!(memory_benches, bench_tree_memory_usage, bench_tree_size);

criterion_main!(
    kvstore_benches,
    range_benches,
    concurrent_benches,
    batch_benches,
    memory_benches
);
