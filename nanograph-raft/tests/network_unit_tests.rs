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

//! Unit tests for network components

use nanograph_core::object::NodeId;
use nanograph_raft::{ConsensusManager, NodeInfo, ReplicationConfig};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_server_start_stop() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50101".parse().unwrap();
    
    // Start server
    let result = manager.clone().start_server(bind_addr).await;
    assert!(result.is_ok());
    
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    assert!(manager.is_server_running().await);
    assert_eq!(manager.server_address().await, Some(bind_addr));
    
    // Stop server
    let result = manager.stop_server().await;
    assert!(result.is_ok());
    
    assert!(!manager.is_server_running().await);
    assert!(manager.server_address().await.is_none());
}

#[tokio::test]
async fn test_server_double_start_fails() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50102".parse().unwrap();
    
    // First start should succeed
    let result1 = manager.clone().start_server(bind_addr).await;
    assert!(result1.is_ok());
    
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Second start should fail
    let result2 = manager.clone().start_server(bind_addr).await;
    assert!(result2.is_err());
    
    // Cleanup
    manager.stop_server().await.ok();
}

#[tokio::test]
async fn test_server_stop_when_not_running() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    // Stopping when not running should be safe
    let result = manager.stop_server().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_server_multiple_stop_calls() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50103".parse().unwrap();
    
    manager.clone().start_server(bind_addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Multiple stops should be safe
    assert!(manager.stop_server().await.is_ok());
    assert!(manager.stop_server().await.is_ok());
    assert!(manager.stop_server().await.is_ok());
}

#[tokio::test]
async fn test_concurrent_server_operations() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50104".parse().unwrap();
    
    manager.clone().start_server(bind_addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Spawn multiple tasks checking server status
    let mut handles = vec![];
    for _ in 0..10 {
        let mgr = manager.clone();
        handles.push(tokio::spawn(async move {
            mgr.is_server_running().await
        }));
    }
    
    // All should see server as running
    for handle in handles {
        assert!(handle.await.unwrap());
    }
    
    manager.stop_server().await.ok();
}

#[tokio::test]
async fn test_server_on_different_ports() {
    let config = ReplicationConfig::default();
    
    let manager1 = Arc::new(ConsensusManager::new(NodeId::new(1), config.clone()));
    let manager2 = Arc::new(ConsensusManager::new(NodeId::new(2), config));

    let addr1 = "127.0.0.1:50105".parse().unwrap();
    let addr2 = "127.0.0.1:50106".parse().unwrap();

    // Start both servers
    assert!(manager1.clone().start_server(addr1).await.is_ok());
    assert!(manager2.clone().start_server(addr2).await.is_ok());
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Both should be running
    assert!(manager1.is_server_running().await);
    assert!(manager2.is_server_running().await);
    
    // Cleanup
    manager1.stop_server().await.ok();
    manager2.stop_server().await.ok();
}

#[tokio::test]
async fn test_server_address_tracking() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    // Initially no address
    assert!(manager.server_address().await.is_none());

    let bind_addr = "127.0.0.1:50107".parse().unwrap();
    manager.clone().start_server(bind_addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Should have address
    assert_eq!(manager.server_address().await, Some(bind_addr));

    manager.stop_server().await.unwrap();

    // Address should be cleared
    assert!(manager.server_address().await.is_none());
}

#[tokio::test]
async fn test_rapid_start_stop_cycles() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50108".parse().unwrap();

    // Perform multiple start/stop cycles
    for _ in 0..3 {
        manager.clone().start_server(bind_addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(manager.is_server_running().await);
        
        manager.stop_server().await.unwrap();
        assert!(!manager.is_server_running().await);
        
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn test_peer_management_with_server() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = Arc::new(ConsensusManager::new(node_id, config));

    let bind_addr = "127.0.0.1:50109".parse().unwrap();
    manager.clone().start_server(bind_addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Add peers while server is running
    for i in 2..=4 {
        let peer_id = NodeId::new(i);
        let peer_info = NodeInfo {
            node: peer_id,
            raft_addr: format!("127.0.0.1:5010{}", i).parse().unwrap(),
            api_addr: format!("127.0.0.1:808{}", i).parse().unwrap(),
            status: Default::default(),
            capacity: Default::default(),
            zone: None,
            rack: None,
        };
        manager.add_peer(peer_id, peer_info).await;
    }

    assert_eq!(manager.peer_nodes().await.len(), 3);
    assert!(manager.is_server_running().await);

    manager.stop_server().await.ok();
}

#[tokio::test]
async fn test_server_state_isolation() {
    let config = ReplicationConfig::default();
    
    let manager1 = Arc::new(ConsensusManager::new(NodeId::new(1), config.clone()));
    let manager2 = Arc::new(ConsensusManager::new(NodeId::new(2), config));

    let addr1 = "127.0.0.1:50110".parse().unwrap();
    let addr2 = "127.0.0.1:50111".parse().unwrap();

    // Start only manager1
    manager1.clone().start_server(addr1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // States should be independent
    assert!(manager1.is_server_running().await);
    assert!(!manager2.is_server_running().await);
    assert_eq!(manager1.server_address().await, Some(addr1));
    assert_eq!(manager2.server_address().await, None);

    // Start manager2
    manager2.clone().start_server(addr2).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Both should be running independently
    assert!(manager1.is_server_running().await);
    assert!(manager2.is_server_running().await);

    // Stop manager1, manager2 should still be running
    manager1.stop_server().await.unwrap();
    assert!(!manager1.is_server_running().await);
    assert!(manager2.is_server_running().await);

    manager2.stop_server().await.ok();
}

#[test]
fn test_server_with_custom_runtime() {
    // Test that server works with a custom runtime
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    runtime.block_on(async {
        let node_id = NodeId::new(1);
        let config = ReplicationConfig::default();
        let manager = Arc::new(ConsensusManager::new(node_id, config));

        let bind_addr = "127.0.0.1:50112".parse().unwrap();
        
        let result = manager.clone().start_server(bind_addr).await;
        assert!(result.is_ok());
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(manager.is_server_running().await);
        
        manager.stop_server().await.ok();
    });
}


