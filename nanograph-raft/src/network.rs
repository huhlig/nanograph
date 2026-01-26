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

//! Network layer for Raft consensus communication
//!
//! This module provides the networking infrastructure for Raft consensus,
//! including gRPC client/server implementations, network adapters for
//! OpenRaft, and routing logic for multi-group Raft.

/// Network adapter for OpenRaft integration
pub mod adapter;

/// gRPC client for Raft RPC calls
pub mod client;

/// Network factory implementation
mod factory;

/// Router for multi-group Raft networks
pub mod router;

/// gRPC server for handling Raft RPC requests
pub mod server;

/// Type alias for the consensus network factory
///
/// This factory creates network instances for different Raft groups,
/// enabling multi-group Raft consensus across shards.
pub type ConsensusNetworkFactory = openraft_multi::GroupNetworkFactory<
    router::ConsensusGroupRouter,
    nanograph_core::object::ShardId,
>;
