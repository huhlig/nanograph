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

//! gRPC server for handling Raft RPC requests

use crate::error::{ConsensusError, ConsensusResult};
use crate::grpc::pb::raft_service_server::{RaftService as RaftServiceTrait, RaftServiceServer};
use crate::grpc::pb::*;
use crate::manager::ConsensusManager;
use nanograph_core::object::ShardId;
use std::sync::Arc;
use tonic::{Request, Response, Status};

/// gRPC service implementation for Raft RPC
///
/// Implements the Raft gRPC service that handles incoming RPC requests
/// from peer nodes. Routes requests to the appropriate Raft group based
/// on the group ID in the request.
pub struct RaftService {
    /// Reference to the consensus manager that routes requests to appropriate Raft groups
    manager: Arc<ConsensusManager>,
}

impl RaftService {
    /// Create a new RaftService
    ///
    /// # Arguments
    /// * `manager` - The consensus manager that will handle routing to Raft groups
    ///
    /// # Returns
    /// A new RaftService instance
    pub fn new(manager: Arc<ConsensusManager>) -> Self {
        Self { manager }
    }

    /// Create a gRPC server for this service
    ///
    /// Converts this service into a tonic gRPC server that can be added
    /// to a tonic transport server.
    ///
    /// # Returns
    /// A configured RaftServiceServer
    pub fn into_server(self) -> RaftServiceServer<Self> {
        RaftServiceServer::new(self)
    }

    /// Convert gRPC Status to ConsensusError
    ///
    /// Helper method to convert tonic Status errors into our error type.
    ///
    /// # Arguments
    /// * `status` - The gRPC status to convert
    ///
    /// # Returns
    /// A ConsensusError representing the gRPC error
    fn status_to_error(status: Status) -> ConsensusError {
        ConsensusError::Network {
            message: format!("gRPC error: {}", status.message()),
        }
    }

    /// Convert ConsensusError to gRPC Status
    ///
    /// Helper method to convert our error type into gRPC Status for responses.
    ///
    /// # Arguments
    /// * `error` - The consensus error to convert
    ///
    /// # Returns
    /// A gRPC Status representing the error
    fn error_to_status(error: ConsensusError) -> Status {
        Status::internal(format!("{:?}", error))
    }
}

#[tonic::async_trait]
impl RaftServiceTrait for RaftService {
    /// Handle vote request
    async fn vote(
        &self,
        request: Request<VoteRequestMessage>,
    ) -> Result<Response<VoteResponseMessage>, Status> {
        let msg = request.into_inner();

        // Extract group_id
        let group_id: ShardId = msg
            .group_id
            .ok_or_else(|| Status::invalid_argument("Missing group_id"))?
            .into();

        // Extract vote request
        let vote_request = msg
            .request
            .ok_or_else(|| Status::invalid_argument("Missing request"))?
            .try_into()
            .map_err(Self::error_to_status)?;

        // Route to appropriate Raft group
        let shard_group = self
            .manager
            .shard_group(group_id)
            .await
            .map_err(Self::error_to_status)?;
        let vote_response = shard_group
            .raft
            .vote(vote_request)
            .await
            .map_err(|e| Self::error_to_status(e.into()))?;

        // Convert response
        let response = VoteResponseMessage {
            response: Some(vote_response.into()),
        };

        Ok(Response::new(response))
    }

    /// Handle append entries request
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequestMessage>,
    ) -> Result<Response<AppendEntriesResponseMessage>, Status> {
        let msg = request.into_inner();

        // Extract group_id
        let group_id: ShardId = msg
            .group_id
            .ok_or_else(|| Status::invalid_argument("Missing group_id"))?
            .into();

        // Extract append entries request
        let append_request = msg
            .request
            .ok_or_else(|| Status::invalid_argument("Missing request"))?
            .try_into()
            .map_err(Self::error_to_status)?;

        // Route to appropriate Raft group
        let shard_group = self
            .manager
            .shard_group(group_id)
            .await
            .map_err(Self::error_to_status)?;
        let append_response = shard_group
            .raft
            .append_entries(append_request)
            .await
            .map_err(|e| Self::error_to_status(e.into()))?;

        // Convert response
        let response = AppendEntriesResponseMessage {
            response: Some(append_response.into()),
        };

        Ok(Response::new(response))
    }

    /// Handle install snapshot request (streaming)
    async fn install_snapshot(
        &self,
        request: Request<tonic::Streaming<InstallSnapshotRequestMessage>>,
    ) -> Result<Response<InstallSnapshotResponseMessage>, Status> {
        let mut stream = request.into_inner();

        // Get the first message which should contain metadata
        let first_msg = stream
            .message()
            .await
            .map_err(|e| Status::internal(format!("Stream error: {}", e)))?
            .ok_or_else(|| Status::invalid_argument("Empty snapshot stream"))?;

        // Extract group_id
        let group_id: ShardId = first_msg
            .group_id
            .ok_or_else(|| Status::invalid_argument("Missing group_id"))?
            .into();

        // Extract install snapshot request
        let install_request = first_msg
            .request
            .ok_or_else(|| Status::invalid_argument("Missing request"))?;

        // TODO: Handle streaming snapshot data
        // For now, we just handle single-message snapshots

        // Route to appropriate Raft group
        // Note: This is a simplified implementation
        // A full implementation would need to:
        // 1. Stream all chunks
        // 2. Assemble the complete snapshot
        // 3. Pass to Raft

        let response = InstallSnapshotResponseMessage {
            response: Some(InstallSnapshotResponse {
                term: install_request.term,
            }),
        };

        Ok(Response::new(response))
    }
}

/// Start the gRPC server
pub async fn start_server(
    manager: Arc<ConsensusManager>,
    addr: std::net::SocketAddr,
) -> ConsensusResult<()> {
    let service = RaftService::new(manager);
    let server = service.into_server();

    tonic::transport::Server::builder()
        .add_service(server)
        .serve(addr)
        .await
        .map_err(|e| ConsensusError::Network {
            message: format!("Failed to start gRPC server: {}", e),
        })?;

    Ok(())
}
