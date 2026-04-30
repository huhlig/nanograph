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

//! Integration tests for runtime and network integration

use nanograph_core::object::NodeId;
use nanograph_raft::{ConsensusManager, ReplicationConfig};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_manager_creation() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    assert_eq!(manager.node_id(), node_id);
    assert!(!manager.is_server_running().await);
}

#[tokio::test]
async fn test_server_lifecycle() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    // Server should not be running initially
    assert!(!manager.is_server_running().await);
    assert!(manager.server_address().await.is_none());

    // Start the server
    let bind_addr = "127.0.0.1:50051".parse().unwrap();
    let start_result = manager.clone().start_server(bind_addr).await;
    assert!(
        start_result.is_ok(),
        "Failed to start server: {:?}",
        start_result
    );

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Server should now be running
    assert!(manager.is_server_running().await);
    assert_eq!(manager.server_address().await, Some(bind_addr));

    // Trying to start again should fail
    let restart_result = manager.clone().start_server(bind_addr).await;
    assert!(restart_result.is_err());

    // Stop the server
    let stop_result = manager.stop_server().await;
    assert!(
        stop_result.is_ok(),
        "Failed to stop server: {:?}",
        stop_result
    );

    // Server should no longer be running
    assert!(!manager.is_server_running().await);
    assert!(manager.server_address().await.is_none());
}

#[tokio::test]
async fn test_peer_management() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    // Initially no peers
    assert_eq!(manager.peer_nodes().await.len(), 0);

    // Add a peer
    let peer_id = NodeId::new(2);
    let peer_info = nanograph_raft::NodeInfo {
        node: peer_id,
        raft_addr: "127.0.0.1:50052".parse().unwrap(),
        api_addr: "127.0.0.1:8082".parse().unwrap(),
        status: Default::default(),
        capacity: Default::default(),
        zone: None,
        rack: None,
    };
    manager.add_peer(peer_id, peer_info.clone()).await;

    // Should have one peer
    assert_eq!(manager.peer_nodes().await.len(), 1);
    assert!(manager.peer_nodes().await.contains(&peer_id));

    // Get peer info
    let retrieved = manager.get_peer(peer_id).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().node, peer_id);

    // Remove peer
    let removed = manager.remove_peer(peer_id).await;
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().node, peer_id);

    // Should have no peers again
    assert_eq!(manager.peer_nodes().await.len(), 0);
}

#[tokio::test]
async fn test_multiple_managers_different_ports() {
    let config = ReplicationConfig::default();

    let manager1 = Arc::new(ConsensusManager::new(NodeId::new(1), config.clone()));
    let manager2 = Arc::new(ConsensusManager::new(NodeId::new(2), config));

    // Start both servers on different ports
    let addr1 = "127.0.0.1:50061".parse().unwrap();
    let addr2 = "127.0.0.1:50062".parse().unwrap();

    let start1 = manager1.clone().start_server(addr1).await;
    assert!(start1.is_ok(), "Failed to start manager1: {:?}", start1);

    let start2 = manager2.clone().start_server(addr2).await;
    assert!(start2.is_ok(), "Failed to start manager2: {:?}", start2);

    // Give servers time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Both should be running
    assert!(manager1.is_server_running().await);
    assert!(manager2.is_server_running().await);

    // Stop both
    let stop1 = manager1.stop_server().await;
    let stop2 = manager2.stop_server().await;

    assert!(stop1.is_ok(), "Failed to stop manager1: {:?}", stop1);
    assert!(stop2.is_ok(), "Failed to stop manager2: {:?}", stop2);
}

#[tokio::test]
async fn test_config_access() {
    let node_id = NodeId::new(1);
    let mut config = ReplicationConfig::default();
    config.replication_factor = 5;
    config.election_timeout_ms = 500;

    let manager = ConsensusManager::new(node_id, config.clone());

    assert_eq!(manager.config().replication_factor, 5);
    assert_eq!(manager.config().election_timeout_ms, 500);
}

#[test]
fn test_runtime_integration() {
    // Test that the manager works with a custom tokio runtime
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async {
        let node_id = NodeId::new(1);
        let config = ReplicationConfig::default();
        let manager = Arc::new(ConsensusManager::new(node_id, config));

        // Start server
        let bind_addr = "127.0.0.1:50071".parse().unwrap();
        let result = manager.clone().start_server(bind_addr).await;
        assert!(result.is_ok());

        // Give it time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(manager.is_server_running().await);

        // Stop server
        let stop_result = manager.stop_server().await;
        assert!(stop_result.is_ok());
    });
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    // Start server
    let bind_addr = "127.0.0.1:50081".parse().unwrap();
    manager.clone().start_server(bind_addr).await.unwrap();

    // Give it time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shutdown should be clean
    let shutdown_result = manager.stop_server().await;
    assert!(shutdown_result.is_ok());

    // Multiple shutdowns should be safe
    let second_shutdown = manager.stop_server().await;
    assert!(second_shutdown.is_ok());
}
