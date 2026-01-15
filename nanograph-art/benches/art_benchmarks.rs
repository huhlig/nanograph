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

//! Comprehensive benchmarks for Adaptive Radix Tree implementation

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use nanograph_art::AdaptiveRadixTree;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::Arc;

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_sequential_keys(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| format!("key{:08}", i).into_bytes())
        .collect()
}

fn generate_random_keys(count: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let len = rng.random_range(8..32);
            (0..len).map(|_| rng.random()).collect()
        })
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

fn create_populated_tree(count: usize) -> AdaptiveRadixTree<usize> {
    let mut tree = AdaptiveRadixTree::new();
    let kvs = generate_sequential_kvs(count);

    for (key, _) in kvs {
        tree.insert(key.clone(), 0).unwrap();
    }

    tree
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
                let mut tree = AdaptiveRadixTree::new();
                let kvs = generate_sequential_kvs(size);

                for (key, _) in kvs {
                    tree.insert(black_box(key), black_box(0)).unwrap();
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
            let keys = generate_random_keys(size, 42);
            b.iter(|| {
                let mut tree = AdaptiveRadixTree::new();

                for key in &keys {
                    tree.insert(black_box(key.clone()), black_box(0)).unwrap();
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
                let mut tree = AdaptiveRadixTree::new();
                let mut kvs = generate_sequential_kvs(size);
                kvs.reverse();

                for (key, _) in kvs {
                    tree.insert(black_box(key), black_box(0)).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_insert_with_common_prefix(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_common_prefix");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut tree = AdaptiveRadixTree::new();

                for i in 0..size {
                    let key = format!("common_prefix_{:08}", i).into_bytes();
                    tree.insert(black_box(key), black_box(i)).unwrap();
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Lookup Benchmarks
// ============================================================================

fn bench_lookup_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_sequential");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);
        let keys = generate_sequential_keys(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for key in &keys {
                    black_box(tree.get(black_box(key)));
                }
            });
        });
    }

    group.finish();
}

fn bench_lookup_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_random");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);
        let keys = generate_random_keys(*size, 42);

        // Insert random keys first
        let mut tree_mut = tree.clone();
        for key in &keys {
            tree_mut.insert(key.clone(), 0).unwrap();
        }

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for key in &keys {
                    black_box(tree_mut.get(black_box(key)));
                }
            });
        });
    }

    group.finish();
}

fn bench_lookup_missing(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_missing");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);
        let missing_keys = generate_random_keys(*size, 999);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for key in &missing_keys {
                    black_box(tree.get(black_box(key)));
                }
            });
        });
    }

    group.finish();
}

fn bench_contains_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("contains_key");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);
        let keys = generate_sequential_keys(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for key in &keys {
                    black_box(tree.contains_key(black_box(key)));
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Delete Benchmarks
// ============================================================================

fn bench_delete_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_sequential");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || create_populated_tree(size),
                |mut tree| {
                    let keys = generate_sequential_keys(size);
                    for key in keys {
                        tree.remove(black_box(&key)).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_delete_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_random");

    for size in [100, 1_000, 10_000].iter() {
        let keys = generate_random_keys(*size, 42);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut tree = AdaptiveRadixTree::new();
                    for key in &keys {
                        tree.insert(key.clone(), 0).unwrap();
                    }
                    tree
                },
                |mut tree| {
                    for key in &keys {
                        tree.remove(black_box(key)).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_delete_alternating(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_alternating");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements((*size / 2) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || create_populated_tree(size),
                |mut tree| {
                    let keys = generate_sequential_keys(size);
                    for (i, key) in keys.iter().enumerate() {
                        if i % 2 == 0 {
                            tree.remove(black_box(key)).unwrap();
                        }
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ============================================================================
// Iterator Benchmarks
// ============================================================================

fn bench_iterator_full_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterator_full_scan");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let count = tree.iter().count();
                black_box(count);
            });
        });
    }

    group.finish();
}

fn bench_iterator_collect(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterator_collect");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let items: Vec<_> = tree.iter().collect();
                black_box(items);
            });
        });
    }

    group.finish();
}

fn bench_keys_iterator(c: &mut Criterion) {
    let mut group = c.benchmark_group("keys_iterator");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let keys: Vec<_> = tree.keys().collect();
                black_box(keys);
            });
        });
    }

    group.finish();
}

fn bench_values_iterator(c: &mut Criterion) {
    let mut group = c.benchmark_group("values_iterator");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let values: Vec<_> = tree.values().collect();
                black_box(values);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Memory Benchmarks
// ============================================================================

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

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

fn bench_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone");

    for size in [100, 1_000, 10_000].iter() {
        let tree = create_populated_tree(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let cloned = tree.clone();
                black_box(cloned);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Mixed Workload Benchmarks
// ============================================================================

fn bench_mixed_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_read_write");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut tree = create_populated_tree(size / 2);
                let keys = generate_sequential_keys(size);

                for (i, key) in keys.iter().enumerate() {
                    if i % 2 == 0 {
                        tree.insert(black_box(key.clone()), black_box(i)).unwrap();
                    } else {
                        black_box(tree.get(black_box(key)));
                    }
                }
            });
        });
    }

    group.finish();
}

fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut tree = create_populated_tree(size);
                let keys = generate_sequential_keys(size);

                for (i, key) in keys.iter().enumerate() {
                    match i % 3 {
                        0 => {
                            tree.insert(black_box(key.clone()), black_box(i)).unwrap();
                        }
                        1 => {
                            black_box(tree.get(black_box(key)));
                        }
                        _ => {
                            tree.remove(black_box(key)).unwrap();
                        }
                    }
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Node Type Transition Benchmarks
// ============================================================================

fn bench_node_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_growth");

    group.bench_function("node4_to_node16", |b| {
        b.iter(|| {
            let mut tree = AdaptiveRadixTree::new();
            for i in 0..20 {
                let key = format!("key{}", i).into_bytes();
                tree.insert(black_box(key), black_box(i)).unwrap();
            }
        });
    });

    group.bench_function("node16_to_node48", |b| {
        b.iter(|| {
            let mut tree = AdaptiveRadixTree::new();
            for i in 0..60 {
                let key = format!("key{}", i).into_bytes();
                tree.insert(black_box(key), black_box(i)).unwrap();
            }
        });
    });

    group.bench_function("node48_to_node256", |b| {
        b.iter(|| {
            let mut tree = AdaptiveRadixTree::new();
            for i in 0..300 {
                let key = format!("key{}", i).into_bytes();
                tree.insert(black_box(key), black_box(i)).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    insert_benches,
    bench_insert_sequential,
    bench_insert_random,
    bench_insert_reverse,
    bench_insert_with_common_prefix
);

criterion_group!(
    lookup_benches,
    bench_lookup_sequential,
    bench_lookup_random,
    bench_lookup_missing,
    bench_contains_key
);

criterion_group!(
    delete_benches,
    bench_delete_sequential,
    bench_delete_random,
    bench_delete_alternating
);

criterion_group!(
    iterator_benches,
    bench_iterator_full_scan,
    bench_iterator_collect,
    bench_keys_iterator,
    bench_values_iterator
);

criterion_group!(memory_benches, bench_memory_usage, bench_clone);

criterion_group!(
    mixed_benches,
    bench_mixed_read_write,
    bench_mixed_operations
);

criterion_group!(node_benches, bench_node_growth);

criterion_main!(
    insert_benches,
    lookup_benches,
    delete_benches,
    iterator_benches,
    memory_benches,
    mixed_benches,
    node_benches
);
