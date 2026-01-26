# Nanograph Raft Consensus Implementation Plan

## Executive Summary

This document outlines the comprehensive plan to implement distributed consensus in nanograph-raft using:
- **Cap'n Proto** for efficient serialization
- **Cap'n Proto RPC** for network communication
- **OpenRaft 0.10** for Raft consensus protocol
- **openraft-multi 0.10** for managing multiple Raft groups

## Current Status

The nanograph-raft crate has a solid foundation but requires full Raft protocol implementation:

**✅ Complete:**
- Type system and error handling
- Hash-based routing and partitioning
- Storage adapter framework
- Metadata management framework
- Configuration and testing infrastructure

**🟡 Partial:**
- Shard Raft groups (framework only)
- Metadata Raft group (framework only)
- Network layer (placeholder)
- Snapshot support (framework only)

**🔴 Not Implemented:**
- Actual Raft log replication
- Leader election protocol
- ReadIndex for linearizable reads
- Cap'n Proto RPC integration
- openraft-multi integration

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
└────────────────────────────┬────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│              ConsensusManager (Router)                       │
│  - Routes operations to correct shard                        │
│  - Manages multiple Raft groups via openraft-multi          │
└────────────────────────────┬────────────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
┌────────▼────────┐  ┌───────▼────────┐  ┌──────▼──────────┐
│ SystemRaftGroup │  │ContainerRaft   │  │ TableShardRaft  │
│  (Metadata)     │  │    Groups      │  │    Groups       │
└────────┬────────┘  └───────┬────────┘  └──────┬──────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│              OpenRaft-Multi Router                           │
│  - GroupRouter: Routes to correct Raft group                │
│  - GroupNetworkFactory: Creates per-group networks          │
└────────────────────────────┬────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│           Cap'n Proto RPC Network Layer                      │
│  - RaftService: append_entries, vote, install_snapshot      │
│  - Connection pooling and management                         │
│  - Retry logic and circuit breakers                          │
└────────────────────────────┬────────────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
┌────────▼────────┐  ┌───────▼────────┐  ┌──────▼──────────┐
│  RaftLogStore   │  │ RaftStateMachine│  │  Snapshot Mgr   │
│  (WAL-backed)   │  │  (KV Store)     │  │                 │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

## Phase 1: Cap'n Proto Schema Design

### 1.1 Core Raft RPC Schemas

Create comprehensive schemas in `nanograph-core/schema/consensus.capnp`:

```capnp
@0xe2b7b6418ff8375f;

# Node and Group Identifiers
struct NodeId {
    upper @0 :UInt64;
    lower @0 :UInt64;
}

struct ShardId {
    value @0 :UInt64;
}

# Vote Request/Response
struct VoteRequest {
    term @0 :UInt64;
    candidateId @1 :NodeId;
    lastLogIndex @2 :UInt64;
    lastLogTerm @3 :UInt64;
}

struct VoteResponse {
    term @0 :UInt64;
    voteGranted @1 :Bool;
}

# AppendEntries Request/Response
struct LogEntry {
    term @0 :UInt64;
    index @1 :UInt64;
    data @2 :Data;  # Serialized Operation
}

struct AppendEntriesRequest {
    term @0 :UInt64;
    leaderId @1 :NodeId;
    prevLogIndex @2 :UInt64;
    prevLogTerm @3 :UInt64;
    entries @4 :List(LogEntry);
    leaderCommit @5 :UInt64;
}

struct AppendEntriesResponse {
    term @0 :UInt64;
    success @1 :Bool;
    conflictIndex @2 :UInt64;
}

# Snapshot Transfer
struct SnapshotMetadata {
    lastIncludedIndex @0 :UInt64;
    lastIncludedTerm @1 :UInt64;
    membership @2 :Data;
}

struct InstallSnapshotRequest {
    term @0 :UInt64;
    leaderId @1 :NodeId;
    metadata @2 :SnapshotMetadata;
    offset @3 :UInt64;
    data @4 :Data;
    done @5 :Bool;
}

struct InstallSnapshotResponse {
    term @0 :UInt64;
}

# RPC Service Interface
interface RaftService {
    vote @0 (groupId :ShardId, request :VoteRequest) 
        -> (response :VoteResponse);
    
    appendEntries @1 (groupId :ShardId, request :AppendEntriesRequest) 
        -> (response :AppendEntriesResponse);
    
    installSnapshot @2 (groupId :ShardId, request :InstallSnapshotRequest) 
        -> (response :InstallSnapshotResponse);
}
```

### 1.2 Build Integration

Update `nanograph-raft/build.rs`:

```rust
fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("./schema")
        .file("./schema/protocol.capnp")
        .run()
        .expect("schema compiler command");
}
```

### 1.3 Type Conversions

Create `nanograph-raft/src/capnp_conv.rs` for bidirectional conversions:

```rust
// Convert OpenRaft types to Cap'n Proto
impl From<openraft::Vote<ConsensusTypeConfig>> for consensus_capnp::vote_request::Builder {
    fn from(vote: openraft::Vote<ConsensusTypeConfig>) -> Self {
        // Implementation
    }
}

// Convert Cap'n Proto to OpenRaft types
impl TryFrom<consensus_capnp::vote_request::Reader> for openraft::Vote<ConsensusTypeConfig> {
    type Error = ConsensusError;
    fn try_from(reader: consensus_capnp::vote_request::Reader) -> Result<Self, Self::Error> {
        // Implementation
    }
}
```

## Phase 2: OpenRaft Storage Implementation

### 2.1 Complete RaftLogStore

File: `nanograph-raft/src/storage/logstore.rs`

```rust
use openraft::storage::RaftLogStorage;
use nanograph_wal::WriteAheadLogManager;

pub struct ConsensusLogStore {
    wal: Arc<WriteAheadLogManager>,
    vote_cache: Arc<RwLock<Option<Vote<ConsensusTypeConfig>>>>,
}

#[async_trait]
impl RaftLogStorage<ConsensusTypeConfig> for ConsensusLogStore {
    async fn save_vote(&mut self, vote: &Vote<ConsensusTypeConfig>) -> Result<()> {
        // Serialize vote using postcard
        let data = postcard::to_allocvec(vote)?;
        
        // Write to special vote record in WAL
        self.wal.write_vote_record(data).await?;
        
        // Update cache
        *self.vote_cache.write().await = Some(vote.clone());
        
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<ConsensusTypeConfig>>> {
        // Check cache first
        if let Some(vote) = self.vote_cache.read().await.as_ref() {
            return Ok(Some(vote.clone()));
        }
        
        // Read from WAL
        if let Some(data) = self.wal.read_vote_record().await? {
            let vote = postcard::from_bytes(&data)?;
            *self.vote_cache.write().await = Some(vote.clone());
            Ok(Some(vote))
        } else {
            Ok(None)
        }
    }

    async fn append<I>(&mut self, entries: I) -> Result<()>
    where
        I: IntoIterator<Item = Entry<ConsensusTypeConfig>> + Send,
    {
        for entry in entries {
            // Serialize entry
            let data = postcard::to_allocvec(&entry)?;
            
            // Write to WAL
            self.wal.append(data).await?;
        }
        
        Ok(())
    }

    async fn delete_conflict_logs_since(&mut self, log_id: LogId<NodeId>) -> Result<()> {
        // Truncate WAL from this point
        self.wal.truncate_from(log_id.index).await?;
        Ok(())
    }

    async fn purge(&mut self, upto: LogId<NodeId>) -> Result<()> {
        // Compact WAL up to this point
        self.wal.compact_to(upto.index).await?;
        Ok(())
    }

    // ... other required methods
}
```

### 2.2 Complete RaftStateMachine

File: `nanograph-raft/src/storage/machine.rs`

```rust
use openraft::storage::RaftStateMachine;
use nanograph_kvt::KeyValueShardStore;

pub struct ConsensusStorageAdapter {
    storage: Box<dyn KeyValueShardStore>,
    shard_id: ShardId,
    last_applied: Arc<RwLock<Option<LogId<NodeId>>>>,
    snapshot: Arc<RwLock<Option<ShardSnapshot>>>,
}

#[async_trait]
impl RaftStateMachine<ConsensusTypeConfig> for ConsensusStorageAdapter {
    async fn apply<I>(&mut self, entries: I) -> Result<Vec<OperationResponse>>
    where
        I: IntoIterator<Item = Entry<ConsensusTypeConfig>> + Send,
    {
        let mut responses = Vec::new();
        
        for entry in entries {
            let response = match entry.payload {
                EntryPayload::Normal(operation) => {
                    self.apply_operation(operation).await?
                }
                EntryPayload::Membership(_) => {
                    // Handle membership changes
                    OperationResponse::default()
                }
                _ => OperationResponse::default(),
            };
            
            responses.push(response);
            *self.last_applied.write().await = Some(entry.log_id);
        }
        
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        ShardSnapshotBuilder::new(
            self.storage.clone(),
            self.shard_id,
            self.last_applied.read().await.clone(),
        )
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<Self::SnapshotData>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId>,
        snapshot: Box<Self::SnapshotData>,
    ) -> Result<()> {
        // Deserialize snapshot data
        let data = snapshot.into_inner();
        let snapshot: ShardSnapshot = postcard::from_bytes(&data)?;
        
        // Clear existing data
        self.storage.clear().await?;
        
        // Restore from snapshot
        for (key, value) in snapshot.data {
            self.storage.put(&key, &value).await?;
        }
        
        // Update last applied
        *self.last_applied.write().await = Some(meta.last_log_id);
        
        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<ConsensusTypeConfig>>> {
        if let Some(snapshot) = self.snapshot.read().await.as_ref() {
            Ok(Some(snapshot.to_openraft_snapshot()?))
        } else {
            Ok(None)
        }
    }
}
```

## Phase 3: Cap'n Proto RPC Network Layer

### 3.1 RPC Server Implementation

File: `nanograph-raft/src/network/server.rs`

```rust
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::net::TcpListener;

pub struct RaftRpcServer {
    addr: SocketAddr,
    router: Arc<ConsensusManager>,
}

impl RaftRpcServer {
    pub async fn start(self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        
        loop {
            let (stream, _) = listener.accept().await?;
            let router = self.router.clone();
            
            tokio::spawn(async move {
                let (reader, writer) = tokio_util::compat::TokioAsyncReadCompatExt::compat(stream).split();
                
                let network = twoparty::VatNetwork::new(
                    reader,
                    writer,
                    rpc_twoparty_capnp::Side::Server,
                    Default::default(),
                );
                
                let rpc_system = RpcSystem::new(Box::new(network), None);
                
                // Set up RaftService implementation
                let service = RaftServiceImpl::new(router);
                let client: consensus_capnp::raft_service::Client = 
                    capnp_rpc::new_client(service);
                
                rpc_system.await.unwrap();
            });
        }
    }
}

struct RaftServiceImpl {
    router: Arc<ConsensusManager>,
}

impl consensus_capnp::raft_service::Server for RaftServiceImpl {
    fn vote(
        &mut self,
        params: consensus_capnp::raft_service::VoteParams,
        mut results: consensus_capnp::raft_service::VoteResults,
    ) -> Promise<(), capnp::Error> {
        let router = self.router.clone();
        
        Promise::from_future(async move {
            let params = params.get()?;
            let group_id = params.get_group_id()?.into();
            let request = params.get_request()?.try_into()?;
            
            // Get the Raft group
            let group = router.shard_group(group_id).await?;
            
            // Forward to OpenRaft
            let response = group.raft.vote(request).await?;
            
            // Convert response
            let mut result = results.get().init_response();
            result.set_from(response);
            
            Ok(())
        })
    }

    fn append_entries(
        &mut self,
        params: consensus_capnp::raft_service::AppendEntriesParams,
        mut results: consensus_capnp::raft_service::AppendEntriesResults,
    ) -> Promise<(), capnp::Error> {
        // Similar implementation
    }

    fn install_snapshot(
        &mut self,
        params: consensus_capnp::raft_service::InstallSnapshotParams,
        mut results: consensus_capnp::raft_service::InstallSnapshotResults,
    ) -> Promise<(), capnp::Error> {
        // Similar implementation
    }
}
```

### 3.2 RPC Client Implementation

File: `nanograph-raft/src/network/client.rs`

```rust
pub struct RaftRpcClient {
    connections: Arc<RwLock<HashMap<NodeId, consensus_capnp::raft_service::Client>>>,
    node_addresses: Arc<RwLock<HashMap<NodeId, SocketAddr>>>,
}

impl RaftRpcClient {
    pub async fn connect(&self, node_id: NodeId, addr: SocketAddr) -> Result<()> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = tokio_util::compat::TokioAsyncReadCompatExt::compat(stream).split();
        
        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        );
        
        let mut rpc_system = RpcSystem::new(Box::new(network), None);
        let client: consensus_capnp::raft_service::Client = 
            rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
        
        // Store connection
        self.connections.write().await.insert(node_id, client);
        self.node_addresses.write().await.insert(node_id, addr);
        
        // Run RPC system in background
        tokio::spawn(async move {
            rpc_system.await.unwrap();
        });
        
        Ok(())
    }

    pub async fn vote(
        &self,
        target: NodeId,
        group_id: ShardId,
        request: VoteRequest<ConsensusTypeConfig>,
    ) -> Result<VoteResponse<ConsensusTypeConfig>> {
        let client = self.get_client(target).await?;
        
        let mut req = client.vote_request();
        req.get().set_group_id(group_id.into());
        req.get().init_request().set_from(request);
        
        let response = req.send().promise.await?;
        let result = response.get()?.get_response()?.try_into()?;
        
        Ok(result)
    }

    async fn get_client(&self, node_id: NodeId) -> Result<consensus_capnp::raft_service::Client> {
        let connections = self.connections.read().await;
        connections.get(&node_id)
            .cloned()
            .ok_or_else(|| ConsensusError::NodeNotConnected { node_id })
    }
}
```

### 3.3 OpenRaft Network Integration

File: `nanograph-raft/src/network/adapter.rs`

```rust
pub struct ConsensusNetworkAdapter {
    client: Arc<RaftRpcClient>,
}

#[async_trait]
impl RaftNetworkFactory<ConsensusTypeConfig> for ConsensusNetworkAdapter {
    type Network = ConsensusNodeNetwork;

    async fn new_client(&mut self, target: NodeId, node: &NodeInfo) -> Self::Network {
        // Ensure connection exists
        if !self.client.is_connected(target).await {
            self.client.connect(target, node.raft_addr).await.ok();
        }
        
        ConsensusNodeNetwork {
            target,
            client: self.client.clone(),
        }
    }
}

pub struct ConsensusNodeNetwork {
    target: NodeId,
    client: Arc<RaftRpcClient>,
}

#[async_trait]
impl RaftNetwork<ConsensusTypeConfig> for ConsensusNodeNetwork {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<ConsensusTypeConfig>, RPCError<ConsensusTypeConfig>> {
        self.client
            .append_entries(self.target, req)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(e)))
    }

    async fn vote(
        &mut self,
        req: VoteRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<ConsensusTypeConfig>, RPCError<ConsensusTypeConfig>> {
        self.client
            .vote(self.target, req)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(e)))
    }

    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<ConsensusTypeConfig>, RPCError<ConsensusTypeConfig>> {
        self.client
            .install_snapshot(self.target, req)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(e)))
    }
}
```

## Phase 4: OpenRaft-Multi Integration

### 4.1 GroupRouter Implementation

File: `nanograph-raft/src/network/group_router.rs`

```rust
use openraft_multi::{GroupRouter, GroupNetworkFactory};

pub struct ConsensusGroupRouter {
    client: Arc<RaftRpcClient>,
}

#[async_trait]
impl GroupRouter<ConsensusTypeConfig, ShardId> for ConsensusGroupRouter {
    async fn append_entries(
        &self,
        target: NodeId,
        group_id: ShardId,
        rpc: AppendEntriesRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<ConsensusTypeConfig>, RPCError<ConsensusTypeConfig>> {
        self.client
            .append_entries(target, group_id, rpc)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
    }

    async fn vote(
        &self,
        target: NodeId,
        group_id: ShardId,
        rpc: VoteRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<ConsensusTypeConfig>, RPCError<ConsensusTypeConfig>> {
        self.client
            .vote(target, group_id, rpc)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
    }

    async fn full_snapshot(
        &self,
        target: NodeId,
        group_id: ShardId,
        vote: Vote<ConsensusTypeConfig>,
        snapshot: Snapshot<ConsensusTypeConfig>,
        _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<ConsensusTypeConfig>, StreamingError<ConsensusTypeConfig>> {
        let data = snapshot.snapshot.into_inner();
        self.client
            .install_snapshot(target, group_id, vote, snapshot.meta, data)
            .await
            .map_err(|e| StreamingError::Unreachable(Unreachable::new(&e)))
    }

    fn backoff(&self) -> Backoff {
        Backoff::new(std::iter::repeat(Duration::from_millis(500)))
    }
}
```

### 4.2 Update ConsensusManager

File: `nanograph-raft/src/manager.rs`

```rust
use openraft_multi::Router as MultiRaftRouter;

pub struct ConsensusManager {
    local_node_id: NodeId,
    config: ReplicationConfig,
    
    // OpenRaft-Multi router
    multi_router: Arc<MultiRaftRouter<ConsensusTypeConfig, ShardId, ConsensusGroupRouter>>,
    
    // Raft groups
    system_metadata: Arc<SystemShardRaftGroup>,
    container_metadata: Arc<RwLock<HashMap<ContainerId, Arc<ContainerShardRaftGroup>>>>,
    table_shards: Arc<RwLock<HashMap<ShardId, Arc<TableShardRaftGroup>>>>,
}

impl ConsensusManager {
    pub async fn new(
        local_node_id: NodeId,
        config: ReplicationConfig,
        network: Arc<ConsensusGroupRouter>,
    ) -> Result<Self> {
        let multi_router = Arc::new(MultiRaftRouter::new(network));
        
        Ok(Self {
            local_node_id,
            config,
            multi_router,
            system_metadata: Arc::new(SystemShardRaftGroup::new(local_node_id)),
            container_metadata: Arc::new(RwLock::new(HashMap::new())),
            table_shards: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn add_table_shard(
        &self,
        shard_id: ShardId,
        storage: Box<dyn KeyValueShardStore>,
        peers: Vec<NodeId>,
    ) -> Result<()> {
        // Create storage adapters
        let log_store = ConsensusLogStore::new(wal);
        let state_machine = ConsensusStorageAdapter::new(storage, shard_id);
        
        // Create Raft node
        let raft = openraft::Raft::new(
            self.local_node_id,
            Arc::new(self.config.to_openraft_config()),
            self.multi_router.clone(),
            log_store,
            state_machine,
        ).await?;
        
        // Register with multi-router
        self.multi_router.add_group(shard_id, raft.clone()).await?;
        
        // Create shard group wrapper
        let shard_group = Arc::new(TableShardRaftGroup {
            shard_id,
            node_id: self.local_node_id,
            raft,
            peers: Arc::new(RwLock::new(peers)),
        });
        
        self.table_shards.write().await.insert(shard_id, shard_group);
        
        Ok(())
    }
}
```

## Phase 5: Raft Group Implementation

### 5.1 Complete ShardRaftGroup

File: `nanograph-raft/src/group/shard.rs`

```rust
pub struct TableShardRaftGroup {
    shard_id: ShardId,
    node_id: NodeId,
    raft: Arc<Raft<ConsensusTypeConfig>>,
    peers: Arc<RwLock<Vec<NodeId>>>,
}

impl TableShardRaftGroup {
    pub async fn propose_write(&self, operation: Operation) -> Result<OperationResponse> {
        // Use OpenRaft's client_write
        let response = self.raft
            .client_write(ClientWriteRequest::new(operation))
            .await?;
        
        Ok(response.data)
    }

    pub async fn read(
        &self,
        key: &[u8],
        consistency: ReadConsistency,
    ) -> Result<Option<Vec<u8>>> {
        match consistency {
            ReadConsistency::Linearizable => {
                self.linearizable_read(key).await
            }
            ReadConsistency::Lease => {
                self.lease_read(key).await
            }
            ReadConsistency::Follower => {
                self.follower_read(key).await
            }
        }
    }

    async fn linearizable_read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Use OpenRaft's ensure_linearizable
        self.raft.ensure_linearizable().await?;
        
        // Now safe to read from state machine
        self.storage.get(key).await
    }

    async fn lease_read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Check if we're the leader with valid lease
        if self.raft.is_leader().await {
            self.storage.get(key).await
        } else {
            // Forward to leader
            Err(ConsensusError::NotLeader)
        }
    }

    async fn follower_read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Direct read, potentially stale
        self.storage.get(key).await
    }

    pub async fn add_learner(&self, node_id: NodeId, node: NodeInfo) -> Result<()> {
        self.raft.add_learner(node_id, node, true).await?;
        Ok(())
    }

    pub async fn change_membership(
        &self,
        members: BTreeSet<NodeId>,
    ) -> Result<()> {
        self.raft.change_membership(members, false).await?;
        Ok(())
    }
}
```

## Phase 6-10: Additional Implementation Details

Due to length constraints, the remaining phases (Snapshot Management, Membership Changes, Testing, Observability, and Documentation) follow similar patterns:

1. **Snapshot Management**: Implement efficient serialization, streaming, and restoration
2. **Membership Changes**: Use OpenRaft's built-in membership change APIs
3. **Testing**: Comprehensive unit, integration, and chaos tests
4. **Observability**: Metrics and logging using `metrics` and `tracing` crates
5. **Documentation**: API docs, guides, and examples

## Implementation Timeline

- **Phase 1-2**: 2-3 weeks (Schema design and storage implementation)
- **Phase 3**: 2 weeks (Network layer)
- **Phase 4**: 1 week (openraft-multi integration)
- **Phase 5**: 2 weeks (Raft group implementation)
- **Phase 6-7**: 2 weeks (Snapshots and membership)
- **Phase 8**: 3 weeks (Testing)
- **Phase 9-10**: 1 week (Observability and docs)

**Total**: ~12-14 weeks for complete implementation

## Key Dependencies

```toml
[dependencies]
openraft = "0.10"
openraft-multi = "0.10"
capnp = "0.20"
capnp-rpc = "0.20"
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["compat"] }
async-trait = "0.1"
postcard = { version = "1.0", features = ["alloc"] }
serde = { version = "1.0", features = ["derive"] }
tracing = "0.1"
metrics = "0.24"

[build-dependencies]
capnpc = "0.20"
```

## Success Criteria

1. ✅ All OpenRaft storage traits fully implemented
2. ✅ Cap'n Proto RPC working for all Raft operations
3. ✅ Multiple Raft groups managed via openraft-multi
4. ✅ Leader election and log replication functional
5. ✅ Linearizable reads working via ReadIndex
6. ✅ Snapshot creation and transfer working
7. ✅ Membership changes working
8. ✅ All tests passing (unit, integration, chaos)
9. ✅ Performance benchmarks meeting targets
10. ✅ Complete documentation

## References

- [OpenRaft Documentation](https://docs.rs/openraft/0.10.0/openraft/)
- [openraft-multi Documentation](https://docs.rs/openraft-multi/0.10.0/openraft_multi/)
- [Cap'n Proto Rust](https://github.com/capnproto/capnproto-rust)
- [Raft Paper](https://raft.github.io/raft.pdf)
- [Nanograph Architecture Docs](../docs/)