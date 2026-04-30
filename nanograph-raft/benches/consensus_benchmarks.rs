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

//! Performance benchmarks for consensus operations

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use nanograph_core::object::NodeId;
use nanograph_raft::{ConsensusManager, NodeInfo, ReplicationConfig};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn bench_manager_creation(c: &mut Criterion) {
    c.bench_function("manager_creation", |b| {
        b.iter(|| {
            let node_id = NodeId::new(1);
            let config = ReplicationConfig::default();
            black_box(ConsensusManager::new(node_id, config))
        });
    });
}

fn bench_peer_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("add_peer", |b| {
        b.to_async(&rt).iter(|| async {
            let node_id = NodeId::new(1);
            let config = ReplicationConfig::default();
            let manager = ConsensusManager::new(node_id, config);

            let peer_id = NodeId::new(2);
            let peer_info = NodeInfo {
                node: peer_id,
                raft_addr: "127.0.0.1:50052".parse().unwrap(),
                api_addr: "127.0.0.1:8082".parse().unwrap(),
                status: Default::default(),
                capacity: Default::default(),
                zone: None,
                rack: None,
            };

            manager.add_peer(peer_id, peer_info).await;
            black_box(manager)
        });
    });

    c.bench_function("get_peer", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let node_id = NodeId::new(1);
                let config = ReplicationConfig::default();
                let manager = ConsensusManager::new(node_id, config);

                let peer_id = NodeId::new(2);
                let peer_info = NodeInfo {
                    node: peer_id,
                    raft_addr: "127.0.0.1:50052".parse().unwrap(),
                    api_addr: "127.0.0.1:8082".parse().unwrap(),
                    status: Default::default(),
                    capacity: Default::default(),
                    zone: None,
                    rack: None,
                };

                rt.block_on(async {
                    manager.add_peer(peer_id, peer_info).await;
                });

                (manager, peer_id)
            },
            |(manager, peer_id)| async move { black_box(manager.get_peer(peer_id).await) },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("peer_nodes_list", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let node_id = NodeId::new(1);
                let config = ReplicationConfig::default();
                let manager = ConsensusManager::new(node_id, config);

                rt.block_on(async {
                    for i in 2..=10 {
                        let peer_id = NodeId::new(i);
                        let peer_info = NodeInfo {
                            node: peer_id,
                            raft_addr: format!("127.0.0.1:5005{}", i).parse().unwrap(),
                            api_addr: format!("127.0.0.1:808{}", i).parse().unwrap(),
                            status: Default::default(),
                            capacity: Default::default(),
                            zone: None,
                            rack: None,
                        };
                        manager.add_peer(peer_id, peer_info).await;
                    }
                });

                manager
            },
            |manager| async move { black_box(manager.peer_nodes().await) },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_shard_routing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("shard_routing");

    for shard_count in [1, 4, 8, 16, 32].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(shard_count),
            shard_count,
            |b, &shard_count| {
                b.to_async(&rt).iter_batched(
                    || {
                        let node_id = NodeId::new(1);
                        let config = ReplicationConfig::default();
                        let manager = ConsensusManager::new(node_id, config);
                        rt.block_on(async {
                            manager.set_shard_count(shard_count).await;
                        });
                        manager
                    },
                    |manager| async move {
                        let key = b"benchmark_key";
                        black_box(manager.get_table_shard_for_key(key).await)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");

    for concurrency in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter_batched(
                    || {
                        let node_id = NodeId::new(1);
                        let config = ReplicationConfig::default();
                        Arc::new(ConsensusManager::new(node_id, config))
                    },
                    |manager| async move {
                        let mut handles = vec![];
                        for i in 0..concurrency {
                            let mgr = manager.clone();
                            handles.push(tokio::spawn(async move {
                                let key = format!("key_{}", i);
                                mgr.get_table_shard_for_key(key.as_bytes()).await
                            }));
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }

                        black_box(manager)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_server_lifecycle(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("server_start_stop", |b| {
        let mut port = 60000;
        b.to_async(&rt).iter_batched(
            || {
                let node_id = NodeId::new(1);
                let config = ReplicationConfig::default();
                let manager = Arc::new(ConsensusManager::new(node_id, config));
                port += 1;
                let bind_addr = format!("127.0.0.1:{}", port).parse().unwrap();
                (manager, bind_addr)
            },
            |(manager, bind_addr)| async move {
                manager.clone().start_server(bind_addr).await.ok();
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                manager.stop_server().await.ok();
                black_box(manager)
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_key_distribution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("key_distribution");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("hash_1000_keys", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let node_id = NodeId::new(1);
                let config = ReplicationConfig::default();
                let manager = ConsensusManager::new(node_id, config);
                rt.block_on(async {
                    manager.set_shard_count(16).await;
                });
                manager
            },
            |manager| async move {
                for i in 0..1000 {
                    let key = format!("key_{}", i);
                    black_box(manager.get_table_shard_for_key(key.as_bytes()).await);
                }
                black_box(manager)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_peer_lookup_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("peer_lookup_scaling");

    for peer_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(peer_count),
            peer_count,
            |b, &peer_count| {
                b.to_async(&rt).iter_batched(
                    || {
                        let node_id = NodeId::new(1);
                        let config = ReplicationConfig::default();
                        let manager = ConsensusManager::new(node_id, config);

                        rt.block_on(async {
                            for i in 2..=(peer_count + 1) {
                                let peer_id = NodeId::new(i);
                                let peer_info = NodeInfo {
                                    node: peer_id,
                                    raft_addr: format!("127.0.0.1:{}", 50000 + i).parse().unwrap(),
                                    api_addr: format!("127.0.0.1:{}", 8000 + i).parse().unwrap(),
                                    status: Default::default(),
                                    capacity: Default::default(),
                                    zone: None,
                                    rack: None,
                                };
                                manager.add_peer(peer_id, peer_info).await;
                            }
                        });

                        manager
                    },
                    |manager| async move {
                        // Lookup a peer in the middle
                        let target = NodeId::new(peer_count / 2);
                        black_box(manager.get_peer(target).await)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_manager_creation,
    bench_peer_operations,
    bench_shard_routing,
    bench_concurrent_operations,
    bench_server_lifecycle,
    bench_key_distribution,
    bench_peer_lookup_scaling,
);

criterion_main!(benches);
