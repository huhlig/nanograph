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

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardId};
use nanograph_vfs::{Path, MemoryFileSystem};
use std::sync::Arc;
use tempfile::TempDir;

fn setup_store() -> (LMDBKeyValueStore, ShardId, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

    (store, shard_id, temp_dir)
}

fn bench_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_put");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard_id, _temp_dir) = setup_store();
            let value = vec![0u8; size];
            let mut counter = 0u64;

            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let key = format!("key{:016}", counter);
                    counter += 1;
                    store
                        .put(shard_id, key.as_bytes(), black_box(&value))
                        .await
                        .unwrap();
                });
        });
    }

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_get");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (store, shard_id, _temp_dir) = setup_store();
            let value = vec![0u8; size];

            // Pre-populate with data
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                for i in 0..1000 {
                    let key = format!("key{:016}", i);
                    store.put(shard_id, key.as_bytes(), &value).await.unwrap();
                }
            });

            let mut counter = 0u64;
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let key = format!("key{:016}", counter % 1000);
                    counter += 1;
                    store
                        .get(shard_id, black_box(key.as_bytes()))
                        .await
                        .unwrap();
                });
        });
    }

    group.finish();
}

fn bench_batch_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_batch_put");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let (store, shard_id, _temp_dir) = setup_store();
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
                        store
                            .batch_put(shard_id, black_box(&pairs_ref))
                            .await
                            .unwrap();
                    });
            },
        );
    }

    group.finish();
}

fn bench_batch_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_batch_get");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let (store, shard_id, _temp_dir) = setup_store();
                let value = vec![0u8; 256];

                // Pre-populate with data
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    for i in 0..batch_size {
                        let key = format!("key{:016}", i);
                        store.put(shard_id, key.as_bytes(), &value).await.unwrap();
                    }
                });

                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let keys: Vec<Vec<u8>> = (0..batch_size)
                            .map(|i| format!("key{:016}", i).into_bytes())
                            .collect();
                        let keys_ref: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
                        store
                            .batch_get(shard_id, black_box(&keys_ref))
                            .await
                            .unwrap();
                    });
            },
        );
    }

    group.finish();
}

fn bench_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_scan");

    for num_keys in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_keys as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_keys),
            num_keys,
            |b, &num_keys| {
                let (store, shard_id, _temp_dir) = setup_store();
                let value = vec![0u8; 256];

                // Pre-populate with data
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    for i in 0..num_keys {
                        let key = format!("key{:016}", i);
                        store.put(shard_id, key.as_bytes(), &value).await.unwrap();
                    }
                });

                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let range = KeyRange::all();
                        let mut iter = store.scan(shard_id, black_box(range)).await.unwrap();
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

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmdb_mixed_workload");

    group.bench_function("90_read_10_write", |b| {
        let (store, shard_id, _temp_dir) = setup_store();
        let value = vec![0u8; 256];

        // Pre-populate with data
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("key{:016}", i);
                store.put(shard_id, key.as_bytes(), &value).await.unwrap();
            }
        });

        let mut counter = 0u64;
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let key = format!("key{:016}", counter % 1000);
                counter += 1;

                // 90% reads, 10% writes
                if counter % 10 == 0 {
                    store
                        .put(shard_id, key.as_bytes(), black_box(&value))
                        .await
                        .unwrap();
                } else {
                    store
                        .get(shard_id, black_box(key.as_bytes()))
                        .await
                        .unwrap();
                }
            });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_put,
    bench_get,
    bench_batch_put,
    bench_batch_get,
    bench_scan,
    bench_mixed_workload
);
criterion_main!(benches);

// Made with Bob
