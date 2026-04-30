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

//! Common benchmark utilities for KeyValueShardStore implementations
//!
//! This module provides reusable benchmark functions that can be used across
//! different KeyValueShardStore implementations to ensure consistent and
//! comparable performance measurements.

use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use futures::StreamExt;
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardId};
use std::ops::Bound;
use std::sync::atomic::{AtomicU64, Ordering};

/// Benchmark single put operations with various value sizes
pub fn bench_single_put<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_single_put", name));

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let value = vec![0u8; size];
            let counter = AtomicU64::new(0);

            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let count = counter.fetch_add(1, Ordering::Relaxed);
                    let key = format!("key{:016}", count);
                    store
                        .put(shard, key.as_bytes(), black_box(&value))
                        .await
                        .unwrap();
                });
        });
    }

    group.finish();
}

/// Benchmark single get operations with various value sizes
pub fn bench_single_get<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_single_get", name));

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let value = vec![0u8; size];

            // Pre-populate with data
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                for i in 0..1000 {
                    let key = format!("key{:016}", i);
                    store.put(shard, key.as_bytes(), &value).await.unwrap();
                }
            });

            let counter = AtomicU64::new(0);
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let count = counter.fetch_add(1, Ordering::Relaxed);
                    let key = format!("key{:016}", count % 1000);
                    store.get(shard, black_box(key.as_bytes())).await.unwrap();
                });
        });
    }

    group.finish();
}

/// Benchmark batch put operations
pub fn bench_batch_put<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_batch_put", name));

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let value = vec![0u8; 256];

                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let mut pairs = Vec::new();
                        for i in 0..batch_size {
                            let key = format!("key{:016}", i);
                            pairs.push((key.as_bytes().to_vec(), value.clone()));
                        }
                        let pairs_ref: Vec<(&[u8], &[u8])> = pairs
                            .iter()
                            .map(|(k, v)| (k.as_slice(), v.as_slice()))
                            .collect();
                        store.batch_put(shard, black_box(&pairs_ref)).await.unwrap();
                    });
            },
        );
    }

    group.finish();
}

/// Benchmark batch get operations
pub fn bench_batch_get<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_batch_get", name));

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let value = vec![0u8; 256];

                // Pre-populate with data
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    for i in 0..batch_size {
                        let key = format!("key{:016}", i);
                        store.put(shard, key.as_bytes(), &value).await.unwrap();
                    }
                });

                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let keys: Vec<Vec<u8>> = (0..batch_size)
                            .map(|i| format!("key{:016}", i).into_bytes())
                            .collect();
                        let keys_ref: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
                        store.batch_get(shard, black_box(&keys_ref)).await.unwrap();
                    });
            },
        );
    }

    group.finish();
}

/// Benchmark full scan operations
pub fn bench_scan_full<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_scan_full", name));
    group.sample_size(10); // Reduce sample size for large scans

    for num_keys in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_keys as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_keys),
            num_keys,
            |b, &num_keys| {
                let value = vec![0u8; 256];

                // Pre-populate with data
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    for i in 0..num_keys {
                        let key = format!("key{:016}", i);
                        store.put(shard, key.as_bytes(), &value).await.unwrap();
                    }
                });

                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let range = KeyRange::all();
                        let mut iter = store.scan(shard, black_box(range)).await.unwrap();
                        let mut count = 0;
                        while let Some(_) = iter.next().await {
                            count += 1;
                        }
                        count
                    });
            },
        );
    }

    group.finish();
}

/// Benchmark prefix scan operations
pub fn bench_scan_prefix<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_scan_prefix", name));

    let num_keys = 10000;
    let value = vec![0u8; 256];

    // Pre-populate with data using different prefixes
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        for i in 0..num_keys {
            let prefix = i % 10; // 10 different prefixes
            let key = format!("prefix{:02}_key{:016}", prefix, i);
            store.put(shard, key.as_bytes(), &value).await.unwrap();
        }
    });

    group.bench_function("prefix_scan", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let prefix = b"prefix05";
                let mut iter = store
                    .scan_prefix(shard, black_box(prefix), None)
                    .await
                    .unwrap();
                let mut count = 0;
                while let Some(_) = iter.next().await {
                    count += 1;
                }
                count
            });
    });

    group.finish();
}

/// Benchmark range scan operations
pub fn bench_scan_range<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_scan_range", name));

    let num_keys = 10000;
    let value = vec![0u8; 256];

    // Pre-populate with data
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        for i in 0..num_keys {
            let key = format!("key{:016}", i);
            store.put(shard, key.as_bytes(), &value).await.unwrap();
        }
    });

    for range_size in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*range_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            range_size,
            |b, &range_size| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let start = format!("key{:016}", 1000);
                        let end = format!("key{:016}", 1000 + range_size);
                        let range = KeyRange {
                            start: Bound::Included(start.into_bytes()),
                            end: Bound::Excluded(end.into_bytes()),
                            limit: None,
                            reverse: false,
                        };
                        let mut iter = store.scan(shard, black_box(range)).await.unwrap();
                        let mut count = 0;
                        while let Some(_) = iter.next().await {
                            count += 1;
                        }
                        count
                    });
            },
        );
    }

    group.finish();
}

/// Benchmark mixed workload: 90% reads, 10% writes
pub fn bench_mixed_90_10<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_mixed_90_10", name));

    let value = vec![0u8; 256];

    // Pre-populate with data
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("key{:016}", i);
            store.put(shard, key.as_bytes(), &value).await.unwrap();
        }
    });

    let counter = AtomicU64::new(0);
    group.bench_function("90_read_10_write", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let count = counter.fetch_add(1, Ordering::Relaxed);
                let key = format!("key{:016}", count % 1000);

                // 90% reads, 10% writes
                if count % 10 == 0 {
                    store
                        .put(shard, key.as_bytes(), black_box(&value))
                        .await
                        .unwrap();
                } else {
                    store.get(shard, black_box(key.as_bytes())).await.unwrap();
                }
            });
    });

    group.finish();
}

/// Benchmark mixed workload: 50% reads, 50% writes
pub fn bench_mixed_50_50<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    let mut group = c.benchmark_group(format!("{}_mixed_50_50", name));

    let value = vec![0u8; 256];

    // Pre-populate with data
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("key{:016}", i);
            store.put(shard, key.as_bytes(), &value).await.unwrap();
        }
    });

    let counter = AtomicU64::new(0);
    group.bench_function("50_read_50_write", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let count = counter.fetch_add(1, Ordering::Relaxed);
                let key = format!("key{:016}", count % 1000);

                // 50% reads, 50% writes
                if count % 2 == 0 {
                    store
                        .put(shard, key.as_bytes(), black_box(&value))
                        .await
                        .unwrap();
                } else {
                    store.get(shard, black_box(key.as_bytes())).await.unwrap();
                }
            });
    });

    group.finish();
}

/// Run all single operation benchmarks
pub fn bench_single_operations<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    bench_single_put(c, name, store, shard);
    bench_single_get(c, name, store, shard);
}

/// Run all batch operation benchmarks
pub fn bench_batch_operations<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    bench_batch_put(c, name, store, shard);
    bench_batch_get(c, name, store, shard);
}

/// Run all scan operation benchmarks
pub fn bench_scan_operations<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    bench_scan_full(c, name, store, shard);
    bench_scan_prefix(c, name, store, shard);
    bench_scan_range(c, name, store, shard);
}

/// Run all mixed workload benchmarks
pub fn bench_mixed_workloads<S: KeyValueShardStore>(
    c: &mut Criterion,
    name: &str,
    store: &S,
    shard: ShardId,
) {
    bench_mixed_90_10(c, name, store, shard);
    bench_mixed_50_50(c, name, store, shard);
}

/// Run all benchmarks for a KeyValueShardStore implementation
pub fn bench_all<S: KeyValueShardStore>(c: &mut Criterion, name: &str, store: &S, shard: ShardId) {
    bench_single_operations(c, name, store, shard);
    bench_batch_operations(c, name, store, shard);
    bench_scan_operations(c, name, store, shard);
    bench_mixed_workloads(c, name, store, shard);
}

// Made with Bob
