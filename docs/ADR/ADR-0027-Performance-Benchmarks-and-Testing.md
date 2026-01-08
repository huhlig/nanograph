---
parent: ADR
nav_order: 0027
title: Performance Benchmarks and Testing
status: proposed
date: 2026-01-07
deciders: Hans W. Uhlig
---

# ADR-0027: Performance Benchmarks and Testing

## Status

Proposed

## Context

Nanograph must meet specific performance targets to be competitive with existing databases and fulfill the PRD requirements. Without comprehensive benchmarking and performance testing:

1. **Performance regressions** go undetected
2. **Optimization efforts** lack baseline measurements
3. **Capacity planning** becomes guesswork
4. **User expectations** cannot be validated
5. **Bottlenecks** remain hidden

We need a systematic approach to performance testing that covers all critical paths, scales with the system, and provides actionable insights.

## Decision

Establish a comprehensive performance benchmarking and testing framework with:

1. **Micro-benchmarks** - Individual component performance
2. **Macro-benchmarks** - End-to-end system performance
3. **Scalability tests** - Multi-node performance characteristics
4. **Stress tests** - Behavior under extreme load
5. **Regression detection** - Automated performance monitoring
6. **Profiling integration** - CPU, memory, and I/O analysis

All benchmarks will be:
- **Reproducible** - Deterministic and consistent
- **Automated** - Run in CI/CD pipeline
- **Documented** - Clear methodology and interpretation
- **Versioned** - Track performance over time

## Decision Drivers

* PRD performance requirements (sub-10ms reads, linear scalability)
* Need for regression detection
* Optimization guidance
* Capacity planning requirements
* Competitive analysis
* User confidence

## Design

### 1. Performance Targets (from PRD)

#### Latency Targets

| Operation | Target (p50) | Target (p99) | Target (p999) |
|-----------|--------------|--------------|---------------|
| Local KV read | < 1ms | < 5ms | < 10ms |
| Local KV write | < 2ms | < 10ms | < 20ms |
| Document read | < 5ms | < 15ms | < 30ms |
| Document write | < 10ms | < 30ms | < 50ms |
| Graph traversal (1-hop) | < 5ms | < 20ms | < 40ms |
| Graph traversal (3-hop) | < 50ms | < 200ms | < 500ms |
| Vector search (k=10) | < 20ms | < 100ms | < 200ms |
| Hybrid search | < 50ms | < 150ms | < 300ms |

#### Throughput Targets

| Workload | Target (ops/sec) | Notes |
|----------|------------------|-------|
| KV reads (single node) | > 100,000 | Cached |
| KV writes (single node) | > 50,000 | Durable |
| Document inserts | > 10,000 | With indexing |
| Vector search | > 1,000 | k=10, 1M vectors |
| Graph traversals | > 5,000 | 2-hop average |

#### Scalability Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Read scalability | Linear to 10 nodes | 90% efficiency |
| Write scalability | Linear to 5 nodes | With replication |
| Storage efficiency | > 70% | After compaction |
| Index build time | < 1 hour | 10M vectors |

### 2. Micro-Benchmark Suite

#### 2.1 Storage Engine Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_kv_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_operations");
    
    // Sequential writes
    group.bench_function("sequential_write", |b| {
        let store = setup_store();
        let mut key = 0u64;
        b.iter(|| {
            let k = key.to_be_bytes();
            let v = vec![0u8; 1024];
            store.put(black_box(&k), black_box(&v)).unwrap();
            key += 1;
        });
    });
    
    // Random reads
    group.bench_function("random_read", |b| {
        let store = setup_store_with_data(100_000);
        b.iter(|| {
            let key = rand::random::<u64>() % 100_000;
            let k = key.to_be_bytes();
            store.get(black_box(&k)).unwrap();
        });
    });
    
    // Range scans
    for size in [10, 100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("range_scan", size),
            &size,
            |b, &size| {
                let store = setup_store_with_data(100_000);
                b.iter(|| {
                    let start = rand::random::<u64>() % (100_000 - size);
                    store.scan(start.to_be_bytes(), size).unwrap();
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_kv_operations);
criterion_main!(benches);
```

#### 2.2 WAL Benchmarks

```rust
fn bench_wal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_operations");
    
    // Write throughput
    group.bench_function("wal_append", |b| {
        let wal = setup_wal();
        b.iter(|| {
            let entry = create_test_entry();
            wal.append(black_box(entry)).unwrap();
        });
    });
    
    // Fsync latency
    group.bench_function("wal_sync", |b| {
        let wal = setup_wal();
        b.iter(|| {
            wal.sync().unwrap();
        });
    });
    
    // Recovery time
    for entry_count in [1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::new("wal_recovery", entry_count),
            &entry_count,
            |b, &count| {
                let wal_file = create_wal_with_entries(count);
                b.iter(|| {
                    recover_from_wal(black_box(&wal_file)).unwrap();
                });
            },
        );
    }
    
    group.finish();
}
```

#### 2.3 Index Benchmarks

```rust
fn bench_vector_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_index");
    
    // Index build time
    for vector_count in [10_000, 100_000, 1_000_000] {
        group.bench_with_input(
            BenchmarkId::new("hnsw_build", vector_count),
            &vector_count,
            |b, &count| {
                let vectors = generate_random_vectors(count, 768);
                b.iter(|| {
                    build_hnsw_index(black_box(&vectors)).unwrap();
                });
            },
        );
    }
    
    // Search latency
    for k in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("hnsw_search", k),
            &k,
            |b, &k| {
                let index = setup_index_with_data(100_000);
                let query = generate_random_vector(768);
                b.iter(|| {
                    index.search(black_box(&query), black_box(k)).unwrap();
                });
            },
        );
    }
    
    group.finish();
}
```

### 3. Macro-Benchmark Suite

#### 3.1 YCSB-Style Workloads

```rust
// Workload A: Update heavy (50% read, 50% update)
fn workload_a(store: &Store, duration: Duration) -> WorkloadStats {
    let start = Instant::now();
    let mut stats = WorkloadStats::new();
    
    while start.elapsed() < duration {
        let key = zipfian_key();
        
        if rand::random::<f64>() < 0.5 {
            // Read
            let start = Instant::now();
            store.get(&key).unwrap();
            stats.record_read(start.elapsed());
        } else {
            // Update
            let start = Instant::now();
            store.put(&key, &random_value()).unwrap();
            stats.record_write(start.elapsed());
        }
    }
    
    stats
}

// Workload B: Read heavy (95% read, 5% update)
fn workload_b(store: &Store, duration: Duration) -> WorkloadStats {
    // Similar to workload_a with different ratio
}

// Workload C: Read only
fn workload_c(store: &Store, duration: Duration) -> WorkloadStats {
    // 100% reads
}

// Workload D: Read latest (95% read, 5% insert)
fn workload_d(store: &Store, duration: Duration) -> WorkloadStats {
    // Reads favor recently inserted keys
}

// Workload E: Scan heavy (95% scan, 5% insert)
fn workload_e(store: &Store, duration: Duration) -> WorkloadStats {
    // Range scans with inserts
}
```

#### 3.2 Graph Workloads

```rust
fn bench_graph_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_traversal");
    
    // Social network graph (power-law distribution)
    let graph = generate_social_graph(100_000, 50); // 100k nodes, avg 50 edges
    
    // 1-hop neighborhood
    group.bench_function("1_hop_neighbors", |b| {
        b.iter(|| {
            let node = random_node(&graph);
            graph.neighbors(black_box(node), Direction::Both).unwrap();
        });
    });
    
    // 2-hop traversal
    group.bench_function("2_hop_traversal", |b| {
        b.iter(|| {
            let node = random_node(&graph);
            graph.traverse(black_box(node), 2).unwrap();
        });
    });
    
    // Shortest path
    group.bench_function("shortest_path", |b| {
        b.iter(|| {
            let (from, to) = random_node_pair(&graph);
            graph.shortest_path(black_box(from), black_box(to), 6).unwrap();
        });
    });
    
    group.finish();
}
```

#### 3.3 Hybrid Query Workloads

```rust
fn bench_hybrid_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_queries");
    
    // Setup: 1M documents with vectors
    let store = setup_hybrid_store(1_000_000);
    
    // Vector search only
    group.bench_function("vector_only", |b| {
        let query = random_vector(768);
        b.iter(|| {
            store.vector_search(black_box(&query), 10).unwrap();
        });
    });
    
    // Vector + metadata filter
    group.bench_function("vector_with_filter", |b| {
        let query = random_vector(768);
        let filter = Filter::eq("category", "electronics");
        b.iter(|| {
            store.hybrid_search(
                black_box(&query),
                black_box(&filter),
                10
            ).unwrap();
        });
    });
    
    // Vector + graph traversal
    group.bench_function("vector_graph_hybrid", |b| {
        let query = random_vector(768);
        b.iter(|| {
            // Find similar vectors, then traverse their graph connections
            let results = store.vector_search(black_box(&query), 10).unwrap();
            for result in results {
                store.graph_neighbors(result.id, Direction::Both).unwrap();
            }
        });
    });
    
    group.finish();
}
```

### 4. Scalability Tests

#### 4.1 Multi-Node Benchmarks

```rust
struct ClusterBenchmark {
    nodes: Vec<Node>,
    workload: Workload,
}

impl ClusterBenchmark {
    fn run_scalability_test(&self) -> ScalabilityReport {
        let mut report = ScalabilityReport::new();
        
        // Test with increasing node counts
        for node_count in [1, 2, 3, 5, 10] {
            let cluster = self.setup_cluster(node_count);
            
            // Run workload
            let stats = self.run_workload(&cluster, Duration::from_secs(60));
            
            report.add_result(node_count, stats);
        }
        
        report.calculate_efficiency();
        report
    }
    
    fn measure_replication_lag(&self) -> ReplicationStats {
        // Measure time for writes to propagate to replicas
        let mut stats = ReplicationStats::new();
        
        for _ in 0..1000 {
            let start = Instant::now();
            let key = random_key();
            
            // Write to leader
            self.leader().put(&key, &random_value()).unwrap();
            
            // Wait for replication
            self.wait_for_replication(&key);
            
            stats.record_lag(start.elapsed());
        }
        
        stats
    }
}
```

#### 4.2 Shard Rebalancing Performance

```rust
fn bench_rebalancing(c: &mut Criterion) {
    let mut group = c.benchmark_group("rebalancing");
    
    // Measure time to rebalance shards
    for data_size_gb in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("rebalance_time", data_size_gb),
            &data_size_gb,
            |b, &size| {
                let cluster = setup_cluster_with_data(size);
                b.iter(|| {
                    cluster.add_node().unwrap();
                    cluster.wait_for_rebalance().unwrap();
                });
            },
        );
    }
    
    group.finish();
}
```

### 5. Stress Tests

#### 5.1 Load Testing

```rust
struct LoadTest {
    target_qps: u64,
    duration: Duration,
    workload: Workload,
}

impl LoadTest {
    fn run(&self, store: &Store) -> LoadTestReport {
        let mut report = LoadTestReport::new();
        let rate_limiter = RateLimiter::new(self.target_qps);
        
        let start = Instant::now();
        while start.elapsed() < self.duration {
            rate_limiter.wait();
            
            let op_start = Instant::now();
            let result = self.workload.execute_operation(store);
            let latency = op_start.elapsed();
            
            report.record(result, latency);
        }
        
        report
    }
    
    fn find_max_throughput(&self, store: &Store) -> u64 {
        // Binary search for maximum sustainable QPS
        let mut low = 1000;
        let mut high = 1_000_000;
        
        while low < high {
            let mid = (low + high + 1) / 2;
            let test = LoadTest {
                target_qps: mid,
                duration: Duration::from_secs(30),
                workload: self.workload.clone(),
            };
            
            let report = test.run(store);
            
            if report.p99_latency() < Duration::from_millis(100) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        
        low
    }
}
```

#### 5.2 Endurance Testing

```rust
fn endurance_test(store: &Store, duration: Duration) -> EnduranceReport {
    let mut report = EnduranceReport::new();
    let start = Instant::now();
    
    while start.elapsed() < duration {
        // Mixed workload
        let workload = random_workload();
        let stats = workload.run(store, Duration::from_secs(60));
        
        report.add_interval(stats);
        
        // Check for degradation
        if report.is_degrading() {
            report.mark_degradation(start.elapsed());
        }
        
        // Periodic compaction
        if start.elapsed().as_secs() % 3600 == 0 {
            store.compact().unwrap();
        }
    }
    
    report
}
```

### 6. Regression Detection

#### 6.1 Automated Performance CI

```yaml
# .github/workflows/performance.yml
name: Performance Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Run benchmarks
        run: cargo bench --bench '*' -- --save-baseline current
      
      - name: Compare with baseline
        run: |
          cargo bench --bench '*' -- --baseline main --load-baseline current
      
      - name: Check for regressions
        run: |
          python scripts/check_regression.py \
            --threshold 10 \
            --baseline main \
            --current current
      
      - name: Upload results
        uses: actions/upload-artifact@v2
        with:
          name: benchmark-results
          path: target/criterion/
```

#### 6.2 Regression Detection Script

```python
import json
import sys

def check_regression(baseline, current, threshold_percent):
    regressions = []
    
    for bench_name in current:
        if bench_name not in baseline:
            continue
        
        baseline_time = baseline[bench_name]['mean']
        current_time = current[bench_name]['mean']
        
        change_percent = ((current_time - baseline_time) / baseline_time) * 100
        
        if change_percent > threshold_percent:
            regressions.append({
                'benchmark': bench_name,
                'baseline': baseline_time,
                'current': current_time,
                'change': change_percent
            })
    
    if regressions:
        print("Performance regressions detected:")
        for reg in regressions:
            print(f"  {reg['benchmark']}: {reg['change']:.2f}% slower")
        sys.exit(1)
    else:
        print("No performance regressions detected")
        sys.exit(0)
```

### 7. Profiling Integration

#### 7.1 CPU Profiling

```rust
#[cfg(feature = "profiling")]
fn profile_operation<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let guard = pprof::ProfilerGuard::new(100).unwrap();
    let result = f();
    
    if let Ok(report) = guard.report().build() {
        let file = std::fs::File::create(format!("profile_{}.svg", name)).unwrap();
        report.flamegraph(file).unwrap();
    }
    
    result
}
```

#### 7.2 Memory Profiling

```rust
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn profile_memory<F, R>(f: F) -> (R, MemoryStats)
where
    F: FnOnce() -> R,
{
    let _profiler = dhat::Profiler::new_heap();
    
    let start_stats = get_memory_stats();
    let result = f();
    let end_stats = get_memory_stats();
    
    let stats = MemoryStats {
        allocated: end_stats.allocated - start_stats.allocated,
        peak: end_stats.peak,
        allocations: end_stats.allocations - start_stats.allocations,
    };
    
    (result, stats)
}
```

### 8. Reporting and Visualization

#### 8.1 Benchmark Report Format

```rust
struct BenchmarkReport {
    timestamp: DateTime<Utc>,
    git_commit: String,
    results: Vec<BenchmarkResult>,
}

struct BenchmarkResult {
    name: String,
    mean: Duration,
    std_dev: Duration,
    median: Duration,
    p95: Duration,
    p99: Duration,
    p999: Duration,
    throughput: Option<f64>,
}

impl BenchmarkReport {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
    
    fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Benchmark Report\n\n");
        md.push_str(&format!("**Date:** {}\n", self.timestamp));
        md.push_str(&format!("**Commit:** {}\n\n", self.git_commit));
        
        md.push_str("| Benchmark | Mean | p99 | Throughput |\n");
        md.push_str("|-----------|------|-----|------------|\n");
        
        for result in &self.results {
            md.push_str(&format!(
                "| {} | {:?} | {:?} | {} |\n",
                result.name,
                result.mean,
                result.p99,
                result.throughput.map_or("N/A".to_string(), |t| format!("{:.0} ops/s", t))
            ));
        }
        
        md
    }
}
```

## Consequences

### Positive

* **Performance visibility** - Clear understanding of system behavior
* **Regression prevention** - Automated detection of performance issues
* **Optimization guidance** - Data-driven performance improvements
* **Capacity planning** - Accurate resource requirement estimates
* **User confidence** - Validated performance claims
* **Competitive analysis** - Benchmark against alternatives

### Negative

* **CI/CD overhead** - Benchmarks add time to pipeline
* **Maintenance burden** - Benchmarks need updates with code changes
* **Infrastructure cost** - Performance testing requires resources
* **Complexity** - Large benchmark suite to maintain

### Risks

* **Benchmark drift** - Tests may not reflect real workloads
* **Environment sensitivity** - Results vary across hardware
* **False positives** - Noise in measurements
* **Optimization for benchmarks** - Gaming the metrics

## Alternatives Considered

### 1. Manual Performance Testing

**Rejected** - Not scalable, inconsistent, error-prone.

### 2. Production Monitoring Only

**Rejected** - Catches issues too late, no regression prevention.

### 3. Minimal Benchmark Suite

**Rejected** - Insufficient coverage, misses edge cases.

## Implementation Notes

### Phase 1: Micro-Benchmarks (Week 8)
- Set up Criterion framework
- Implement storage engine benchmarks
- Add to CI pipeline

### Phase 2: Macro-Benchmarks (Week 14)
- Implement YCSB workloads
- Add multi-node tests
- Create reporting tools

### Phase 3: Continuous Monitoring (Week 38)
- Set up performance dashboard
- Implement regression detection
- Add alerting

## Related ADRs

* [ADR-0023: Testing, Fault Injection, and Simulation Strategy](ADR-0023-Testing-Fault-Injection-and-Simulation-Strategy.md)
* [ADR-0025: Core API Specifications](ADR-0025-Core-API-Specifications.md)
* [ADR-0026: Data Format Specifications](ADR-0026-Data-Format-Specifications.md)

## References

* YCSB benchmark suite
* Criterion.rs documentation
* Performance testing best practices
* Database benchmarking methodologies

---

**Next Steps:**
1. Set up Criterion framework
2. Implement initial micro-benchmarks
3. Create benchmark CI pipeline
4. Establish baseline measurements
5. Document benchmark methodology