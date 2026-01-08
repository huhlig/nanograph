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

//! Comprehensive benchmarks for B+Tree implementation

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use futures::StreamExt;
use nanograph_btree::{BPlusTree, BTreeKeyValueStore, tree::BPlusTreeConfig};
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardIndex, TableId};
use std::ops::Bound;
use std::sync::Arc;
use tokio::runtime::Runtime;

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_sequential_keys(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| format!("key{:08}", i).into_bytes())
        .collect()
}

fn generate_sequential_kvs(count: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..count)
        .map(|i| {
            let key = format!("key{:08}", i).into_bytes();
            let value = format!("value_{}", i).into_bytes();
            (key, value)
        })
        .collect()
}

fn create_populated_tree(count: usize) -> Arc<BPlusTree> {
    let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));
    let kvs = generate_sequential_kvs(count);

    for (key, value) in kvs {
        tree.insert(key, value).unwrap();
    }

    tree
}

async fn create_populated_store(count: usize) -> (BTreeKeyValueStore, nanograph_kvt::ShardId) {
    let store = BTreeKeyValueStore::default();
    let table_id = TableId::new(1);
    let shard_index = ShardIndex::new(0);
    let shard = store.create_shard(table_id, shard_index).await.unwrap();
    let kvs = generate_sequential_kvs(count);

    for (key, value) in kvs {
        store.put(shard, &key, &value).await.unwrap();
    }

    (store, shard)
}

// ============================================================================
// Insert Benchmarks
// ============================================================================

fn bench_insert_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_sequential");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let tree = BPlusTree::new(BPlusTreeConfig::default());
                let kvs = generate_sequential_kvs(size);

                for (key, value) in kvs {
                    tree.insert(black_box(key), black_box(value)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_insert_reverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_reverse");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let tree = BPlusTree::new(BPlusTreeConfig::default());
                let mut kvs = generate_sequential_kvs(size);
                kvs.reverse();

                for (key, value) in kvs {
                    tree.insert(black_box(key), black_box(value)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_insert_with_splits(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_with_splits");

    // Use small node size to force frequent splits
    let config = BPlusTreeConfig {
        max_keys: 8,
        min_keys: 4,
    };

    for size in [100, 1_000, 5_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let tree = BPlusTree::new(config.clone());
                let kvs = generate_sequential_kvs(size);

                for (key, value) in kvs {
                    tree.insert(black_box(key), black_box(value)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_insert_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_random");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let tree = BPlusTree::new(BPlusTreeConfig::default());
                let mut kvs = generate_sequential_kvs(size);

                // Shuffle to create random insertion order
                use rand::seq::SliceRandom;
                let mut rng = rand::rng();
                kvs.shuffle(&mut rng);

                for (key, value) in kvs {
                    tree.insert(black_box(key), black_box(value)).unwrap();
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Get Benchmarks
// ============================================================================

fn bench_get_existing(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_existing");

    for size in [1_000, 10_000, 100_000].iter() {
        let tree = create_populated_tree(*size);
        let keys = generate_sequential_keys(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for key in &keys {
                    let _ = tree.get(black_box(key)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_get_missing(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_missing");

    for size in [1_000, 10_000, 100_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for i in 0..1000 {
                    let key = format!("missing{:08}", i).into_bytes();
                    let _ = tree.get(black_box(&key)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_get_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_random_access");

    for size in [1_000, 10_000, 100_000].iter() {
        let tree = create_populated_tree(*size);
        let keys = generate_sequential_keys(*size);

        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                // Access keys in a pseudo-random pattern
                for i in 0..1000 {
                    let idx = (i * 7919) % keys.len(); // Prime number for distribution
                    let _ = tree.get(black_box(&keys[idx])).unwrap();
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Delete Benchmarks
// ============================================================================

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || create_populated_tree(size),
                |tree| {
                    let keys = generate_sequential_keys(size);
                    for key in keys {
                        tree.delete(black_box(&key)).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ============================================================================
// Range Scan Benchmarks
// ============================================================================

fn bench_scan_full(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("scan_full");

    for size in [1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, table) = rt.block_on(create_populated_store(size));

            b.to_async(&rt).iter(|| async {
                let range = KeyRange {
                    start: Bound::Unbounded,
                    end: Bound::Unbounded,
                    limit: None,
                    reverse: false,
                };

                let mut iter = store.scan(table, range).await.unwrap();
                let mut count = 0;

                while let Some(Ok(_)) = iter.next().await {
                    count += 1;
                }

                black_box(count);
            });
        });
    }

    group.finish();
}

fn bench_scan_range(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("scan_range");

    let size = 100_000;
    let (store, table) = rt.block_on(create_populated_store(size));

    for range_size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*range_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            range_size,
            |b, &range_size| {
                b.to_async(&rt).iter(|| async {
                    let start_key = format!("key{:08}", 10_000).into_bytes();
                    let end_key = format!("key{:08}", 10_000 + range_size).into_bytes();

                    let range = KeyRange {
                        start: Bound::Included(start_key),
                        end: Bound::Excluded(end_key),
                        limit: None,
                        reverse: false,
                    };

                    let mut iter = store.scan(table, range).await.unwrap();
                    let mut count = 0;

                    while let Some(Ok(_)) = iter.next().await {
                        count += 1;
                    }

                    black_box(count);
                });
            },
        );
    }

    group.finish();
}

fn bench_scan_with_limit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("scan_with_limit");

    let size = 100_000;
    let (store, table) = rt.block_on(create_populated_store(size));

    for limit in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*limit as u64));
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.to_async(&rt).iter(|| async {
                let range = KeyRange {
                    start: Bound::Unbounded,
                    end: Bound::Unbounded,
                    limit: Some(limit),
                    reverse: false,
                };

                let mut iter = store.scan(table, range).await.unwrap();
                let mut count = 0;

                while let Some(Ok(_)) = iter.next().await {
                    count += 1;
                }

                black_box(count);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Batch Operations Benchmarks
// ============================================================================

fn bench_batch_put(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_put");

    for batch_size in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let kvs = generate_sequential_kvs(batch_size);
                let pairs: Vec<(&[u8], &[u8])> = kvs
                    .iter()
                    .map(|(k, v)| (k.as_slice(), v.as_slice()))
                    .collect();

                b.to_async(&rt).iter(|| async {
                    let store = BTreeKeyValueStore::default();
                    let table_id = TableId::new(1);
                    let shard_index = ShardIndex::new(0);
                    let shard = store.create_shard(table_id, shard_index).await.unwrap();
                    store.batch_put(shard, black_box(&pairs)).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_batch_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_get");

    for batch_size in [10, 100, 1_000].iter() {
        let (store, table) = rt.block_on(create_populated_store(10_000));
        let keys = generate_sequential_keys(*batch_size);
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let _ = store.batch_get(table, black_box(&key_refs)).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Mixed Workload Benchmarks
// ============================================================================

fn bench_mixed_read_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("mixed_read_write");

    group.bench_function("90_read_10_write", |b| {
        let (store, table) = rt.block_on(create_populated_store(10_000));

        b.to_async(&rt).iter(|| async {
            // 90% reads
            for i in 0..90 {
                let key = format!("key{:08}", i * 100).into_bytes();
                let _ = store.get(table, black_box(&key)).await.unwrap();
            }

            // 10% writes
            for i in 0..10 {
                let key = format!("newkey{:08}", i).into_bytes();
                let value = format!("newvalue{}", i).into_bytes();
                store
                    .put(table, black_box(&key), black_box(&value))
                    .await
                    .unwrap();
            }
        });
    });

    group.bench_function("50_read_50_write", |b| {
        let (store, table) = rt.block_on(create_populated_store(10_000));

        b.to_async(&rt).iter(|| async {
            // 50% reads
            for i in 0..50 {
                let key = format!("key{:08}", i * 100).into_bytes();
                let _ = store.get(table, black_box(&key)).await.unwrap();
            }

            // 50% writes
            for i in 0..50 {
                let key = format!("newkey{:08}", i).into_bytes();
                let value = format!("newvalue{}", i).into_bytes();
                store
                    .put(table, black_box(&key), black_box(&value))
                    .await
                    .unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Tree Structure Benchmarks
// ============================================================================

fn bench_tree_height_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_height_impact");

    // Test with different node sizes (affects tree height)
    for max_keys in [8, 32, 128, 512].iter() {
        let config = BPlusTreeConfig {
            max_keys: *max_keys,
            min_keys: max_keys / 2,
        };

        group.bench_with_input(BenchmarkId::new("max_keys", max_keys), max_keys, |b, _| {
            b.iter(|| {
                let tree = BPlusTree::new(config.clone());
                let kvs = generate_sequential_kvs(10_000);

                for (key, value) in kvs {
                    tree.insert(black_box(key), black_box(value)).unwrap();
                }

                // Measure tree stats
                let stats = tree.stats();
                black_box(stats);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_insert_sequential,
    bench_insert_reverse,
    bench_insert_random,
    bench_insert_with_splits,
    bench_get_existing,
    bench_get_missing,
    bench_get_random_access,
    bench_delete,
    bench_scan_full,
    bench_scan_range,
    bench_scan_with_limit,
    bench_batch_put,
    bench_batch_get,
    bench_mixed_read_write,
    bench_tree_height_impact,
);

criterion_main!(benches);
