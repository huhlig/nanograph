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

use crate::network::adapter::ConsensusNetworkAdapter as ConsensusNetworkFactory;
use crate::types::{ConsensusTypeConfig, NodeInfo, ConsensusRPCError as RPCError, ConsensusStreamingError as StreamingError};
use async_trait::async_trait;
use nanograph_core::object::{NodeId, ShardId};
use openraft::error::{InstallSnapshotError, RaftError, ReplicationClosed, Unreachable};
use openraft::network::{Backoff, RPCOption};
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, SnapshotResponse, TransferLeaderRequest, VoteRequest, VoteResponse};
use std::collections::BTreeMap;
use std::sync::Arc;
use openraft::{OptionalSend, Snapshot, Vote};
use tokio::sync::Mutex;

type NodeTx = tokio::sync::mpsc::Sender<NodeMessage>;

pub struct NodeMessage {
    pub group_id: ShardId,
    pub path: String,
    pub payload: Vec<u8>,
    pub response_tx: tokio::sync::oneshot::Sender<String>,
}

#[derive(Clone)]
pub struct ConsensusGroupRouter {
    factory: Arc<ConsensusNetworkFactory>,
    nodes: Arc<Mutex<BTreeMap<NodeId, NodeTx>>>,
}

fn encode<T: serde::Serialize>(val: &T) -> Vec<u8> {
    serde_json::to_vec(val).unwrap_or_default()
}

fn decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouterError(pub String);

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RouterError: {}", self.0)
    }
}

impl std::error::Error for RouterError {}

impl ConsensusGroupRouter {
    pub fn new(factory: Arc<ConsensusNetworkFactory>) -> Self {
        Self {
            factory,
            nodes: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Register a node connection. All groups on this node will use this connection.
    pub async fn register_node(&self, node_id: NodeId, tx: NodeTx) {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(node_id, tx);
    }

    /// Unregister a node connection.
    pub async fn unregister_node(&self, node_id: NodeId) -> Option<NodeTx> {
        let mut nodes = self.nodes.lock().await;
        nodes.remove(&node_id)
    }

    /// Send a request to a specific (node, group).
    pub async fn send<Req, Resp>(
        &self,
        to_node: NodeId,
        to_group: &ShardId,
        path: &str,
        req: Req,
    ) -> Result<Resp, Unreachable<ConsensusTypeConfig>>
    where
        Req: serde::Serialize,
        Result<Resp, RaftError<ConsensusTypeConfig>>: serde::de::DeserializeOwned,
    {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

        let encoded_req = encode(&req);
        tracing::debug!(
            "send to: node={}, group={}, path={}, req={:?}",
            to_node,
            to_group,
            path,
            encoded_req
        );

        // Clone the sender and release the lock before async send
        let tx = {
            let nodes = self.nodes.lock().await;
            nodes
                .get(&to_node)
                .ok_or_else(|| {
                    Unreachable::new(&RouterError(format!("node {} not connected", to_node)))
                })?
                .clone()
        };

        let msg = NodeMessage {
            group_id: to_group.clone(),
            path: path.to_string(),
            payload: encoded_req,
            response_tx: resp_tx,
        };

        tx.send(msg)
            .await
            .map_err(|e| Unreachable::new(&RouterError(e.to_string())))?;

        let resp_str = resp_rx
            .await
            .map_err(|e| Unreachable::new(&RouterError(e.to_string())))?;
        
        tracing::debug!(
            "resp from: node={}, group={}, path={}, resp={}",
            to_node,
            to_group,
            path,
            resp_str
        );

        let res = decode::<Result<Resp, RaftError<ConsensusTypeConfig>>>(resp_str.as_bytes())
            .map_err(|e| Unreachable::new(&RouterError(e)))?;
        res.map_err(|e| Unreachable::new(&e))
    }

    pub async fn has_node(&self, node_id: NodeId) -> bool {
        let nodes = self.nodes.lock().await;
        nodes.contains_key(&node_id)
    }
}

pub trait GroupRouter<C: openraft::RaftTypeConfig, G> {
    fn append_entries(&self, target: NodeId, group_id: G, rpc: AppendEntriesRequest<C>, option: RPCOption) -> impl std::future::Future<Output = Result<AppendEntriesResponse<C>, RPCError>> + Send;
    fn vote(&self, target: NodeId, group_id: G, rpc: VoteRequest<C>, option: RPCOption) -> impl std::future::Future<Output = Result<VoteResponse<C>, RPCError>> + Send;
    fn full_snapshot(&self, target: NodeId, group_id: G, vote: Vote<C>, snapshot: Snapshot<C>, cancel: impl std::future::Future<Output = ReplicationClosed> + OptionalSend + 'static, option: RPCOption) -> impl std::future::Future<Output = Result<SnapshotResponse<C>, StreamingError>> + Send;
    fn transfer_leader(&self, target: NodeId, group_id: G, req: TransferLeaderRequest<C>, option: RPCOption) -> impl std::future::Future<Output = Result<(), RPCError>> + Send;
    fn backoff(&self) -> Backoff;
}

impl GroupRouter<ConsensusTypeConfig, ShardId> for ConsensusGroupRouter {
    async fn append_entries(
        &self,
        target: NodeId,
        group_id: ShardId,
        rpc: AppendEntriesRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<ConsensusTypeConfig>, RPCError> {
        self.send(target, &group_id, "/raft/append", rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn vote(
        &self,
        target: NodeId,
        group_id: ShardId,
        rpc: VoteRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<ConsensusTypeConfig>, RPCError> {
        self.send(target, &group_id, "/raft/vote", rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn full_snapshot(
        &self,
        target: NodeId,
        group_id: ShardId,
        vote: Vote<ConsensusTypeConfig>,
        snapshot: Snapshot<ConsensusTypeConfig>,
        _cancel: impl std::future::Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<ConsensusTypeConfig>, StreamingError> {
        // Read snapshot data from the File
        use std::io::Read;
        let mut data = Vec::new();
        let mut file = snapshot.snapshot;
        file.read_to_end(&mut data)
            .map_err(|e| StreamingError::Unreachable(Unreachable::new(&e)))?;
        
        self.send::<(Vote<ConsensusTypeConfig>, openraft::SnapshotMeta<ConsensusTypeConfig>, Vec<u8>), SnapshotResponse<ConsensusTypeConfig>>(
            target,
            &group_id,
            "/raft/snapshot",
            (vote, snapshot.meta, data),
        )
        .await
        .map_err(StreamingError::Unreachable)
    }

    async fn transfer_leader(
        &self,
        target: NodeId,
        group_id: ShardId,
        req: TransferLeaderRequest<ConsensusTypeConfig>,
        _option: RPCOption,
    ) -> Result<(), RPCError> {
        self.send(target, &group_id, "/raft/transfer_leader", req)
            .await
            .map_err(RPCError::Unreachable)
    }

    fn backoff(&self) -> Backoff {
        Backoff::new(std::iter::repeat(std::time::Duration::from_millis(500)))
    }
}
