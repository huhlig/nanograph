# Nanograph Raft Test Suite

This directory contains comprehensive tests for the nanograph-raft consensus implementation.

## Test Organization

### Unit Tests

#### `manager_unit_tests.rs`
Tests for the `ConsensusManager` core functionality:
- Manager initialization and configuration
- Shard count management
- Consistent key routing
- Peer addition and removal
- Peer information retrieval
- Multiple manager independence
- Shard routing distribution

**Run with:**
```bash
cargo nextest run --test manager_unit_tests
```

#### `network_unit_tests.rs`
Tests for network components and server lifecycle:
- Server start/stop operations
- Double start prevention
- Multiple stop calls safety
- Concurrent server operations
- Multiple servers on different ports
- Server address tracking
- Rapid start/stop cycles
- Peer management with running server
- Server state isolation
- Custom runtime integration

**Run with:**
```bash
cargo nextest run --test network_unit_tests
```

#### `runtime_integration_tests.rs`
Tests for tokio runtime integration:
- Manager creation
- Server lifecycle management
- Peer management
- Multiple managers on different ports
- Configuration access
- Runtime integration
- Graceful shutdown

**Run with:**
```bash
cargo nextest run --test runtime_integration_tests
```

### Integration Tests

#### `multi_node_integration_tests.rs`
Multi-node cluster scenarios:
- 3-node cluster setup
- 5-node cluster setup
- Dynamic node addition
- Node removal
- Partial failure scenarios
- Node recovery
- Concurrent operations across nodes
- Zone-aware clustering
- Large cluster (10 nodes)

**Run with:**
```bash
cargo nextest run --test multi_node_integration_tests
```

### Storage Tests

The storage layer has extensive tests in `src/storage/`:
- Log store operations
- State store operations
- Snapshot management
- Persistence and recovery

**Run with:**
```bash
cargo nextest run --lib storage
```

### Protocol Tests

gRPC protocol conversion tests in `src/grpc.rs`:
- NodeId conversion
- ShardId conversion
- Vote request/response
- Append entries request/response
- Log entry serialization

**Run with:**
```bash
cargo nextest run --lib grpc
```

## Benchmarks

### `consensus_benchmarks.rs`
Performance benchmarks for:
- Manager creation
- Peer operations (add, get, list)
- Shard routing with different shard counts
- Concurrent operations (1, 10, 50, 100 concurrent)
- Server lifecycle
- Key distribution (1000 keys)
- Peer lookup scaling (10, 50, 100, 500 peers)

**Run with:**
```bash
cargo bench
```

**Run specific benchmark:**
```bash
cargo bench --bench consensus_benchmarks -- manager_creation
```

## Running All Tests

### Run all tests with nextest:
```bash
cd nanograph-raft
cargo nextest run --all-features
```

### Run tests with standard cargo:
```bash
cd nanograph-raft
cargo test --all-features
```

### Run tests with output:
```bash
cargo nextest run --all-features --nocapture
```

### Run specific test:
```bash
cargo nextest run --test manager_unit_tests test_manager_initialization
```

## Test Coverage

Current test coverage includes:

### Core Functionality
- ✅ Manager creation and initialization
- ✅ Configuration management
- ✅ Peer management (add, remove, get, list)
- ✅ Shard routing and distribution
- ✅ Server lifecycle (start, stop, status)

### Network Operations
- ✅ gRPC server binding
- ✅ Multiple servers on different ports
- ✅ Graceful shutdown
- ✅ Concurrent operations
- ✅ Server state isolation

### Multi-Node Scenarios
- ✅ Cluster formation (3, 5, 10 nodes)
- ✅ Dynamic membership changes
- ✅ Partial failures
- ✅ Node recovery
- ✅ Zone-aware clustering

### Storage Layer
- ✅ Log persistence
- ✅ State machine snapshots
- ✅ WAL operations
- ✅ Recovery from disk

### Protocol
- ✅ gRPC message conversion
- ✅ Raft RPC serialization
- ✅ Error handling

## Test Patterns

### Async Test Pattern
```rust
#[tokio::test]
async fn test_example() {
    let manager = ConsensusManager::new(node_id, config);
    // Test async operations
}
```

### Multi-Node Test Pattern
```rust
#[tokio::test]
async fn test_cluster() {
    let managers = create_test_cluster(3).await;
    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;
    
    // Test cluster operations
    
    stop_cluster_servers(&managers).await;
}
```

### Benchmark Pattern
```rust
fn bench_operation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    c.bench_function("operation_name", |b| {
        b.to_async(&rt).iter(|| async {
            // Benchmark code
        });
    });
}
```

## Continuous Integration

Tests are designed to run in CI environments:
- No external dependencies required
- Deterministic port allocation
- Proper cleanup in all scenarios
- Timeout protection

## Performance Targets

Based on benchmarks:
- Manager creation: < 1μs
- Peer operations: < 10μs
- Shard routing: < 100ns
- Server start/stop: < 100ms

## Adding New Tests

When adding new tests:

1. **Choose the right file:**
   - Unit tests → `*_unit_tests.rs`
   - Integration tests → `*_integration_tests.rs`
   - Benchmarks → `benches/`

2. **Follow naming conventions:**
   - Test functions: `test_<feature>_<scenario>`
   - Benchmark functions: `bench_<operation>`

3. **Include cleanup:**
   - Always stop servers
   - Clean up resources
   - Use proper async patterns

4. **Document complex tests:**
   - Add comments for non-obvious logic
   - Explain test scenarios
   - Note any timing dependencies

## Troubleshooting

### Port Conflicts
If tests fail with "address already in use":
- Tests use ports 50000-52000
- Ensure no other services use these ports
- Tests clean up properly, but may need time between runs

### Timing Issues
If tests are flaky:
- Increase sleep durations for slower systems
- Check for race conditions
- Verify proper async/await usage

### Resource Limits
For large cluster tests:
- May need to increase file descriptor limits
- Check system resource availability
- Consider running fewer concurrent tests

## Future Test Additions

Planned test coverage:
- [ ] Consensus protocol correctness
- [ ] Leader election scenarios
- [ ] Log replication verification
- [ ] Snapshot transfer
- [ ] Network partition handling
- [ ] Byzantine failure scenarios
- [ ] Performance under load
- [ ] Memory usage profiling