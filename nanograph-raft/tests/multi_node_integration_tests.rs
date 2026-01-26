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

//! Multi-node integration tests for consensus

use nanograph_core::object::NodeId;
use nanograph_raft::{ConsensusManager, NodeInfo, ReplicationConfig};
use std::sync::Arc;
use std::time::Duration;

/// Helper to create a test cluster
async fn create_test_cluster(size: usize) -> Vec<Arc<ConsensusManager>> {
    let config = ReplicationConfig::default();
    let mut managers = Vec::new();

    for i in 1..=size {
        let node_id = NodeId::new(i as u128);
        let manager = Arc::new(ConsensusManager::new(node_id, config.clone()));
        managers.push(manager);
    }

    managers
}

/// Helper to start all servers in a cluster
async fn start_cluster_servers(managers: &[Arc<ConsensusManager>], base_port: u16) {
    for (i, manager) in managers.iter().enumerate() {
        let port = base_port + i as u16;
        let addr = format!("127.0.0.1:{}", port).parse().unwrap();
        manager.clone().start_server(addr).await.ok();
    }
    
    // Give servers time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
}

/// Helper to stop all servers in a cluster
async fn stop_cluster_servers(managers: &[Arc<ConsensusManager>]) {
    for manager in managers {
        manager.stop_server().await.ok();
    }
}

/// Helper to connect peers in a cluster
async fn connect_cluster_peers(managers: &[Arc<ConsensusManager>], base_port: u16) {
    for (i, manager) in managers.iter().enumerate() {
        for (j, peer_manager) in managers.iter().enumerate() {
            if i != j {
                let peer_id = peer_manager.node_id();
                let peer_port = base_port + j as u16;
                let peer_info = NodeInfo {
                    node: peer_id,
                    raft_addr: format!("127.0.0.1:{}", peer_port).parse().unwrap(),
                    api_addr: format!("127.0.0.1:{}", 8000 + peer_port).parse().unwrap(),
                    status: Default::default(),
                    capacity: Default::default(),
                    zone: None,
                    rack: None,
                };
                manager.add_peer(peer_id, peer_info).await;
            }
        }
    }
}

#[tokio::test]
async fn test_three_node_cluster_setup() {
    let managers = create_test_cluster(3).await;
    let base_port = 51000;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Verify all nodes are running
    for manager in &managers {
        assert!(manager.is_server_running().await);
    }

    // Verify peer connections
    for manager in &managers {
        assert_eq!(manager.peer_nodes().await.len(), 2);
    }

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_five_node_cluster_setup() {
    let managers = create_test_cluster(5).await;
    let base_port = 51010;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Verify all nodes are running
    for manager in &managers {
        assert!(manager.is_server_running().await);
    }

    // Verify peer connections (each node should see 4 peers)
    for manager in &managers {
        assert_eq!(manager.peer_nodes().await.len(), 4);
    }

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_node_addition() {
    let mut managers = create_test_cluster(3).await;
    let base_port = 51020;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Add a new node
    let new_node_id = NodeId::new(4);
    let new_manager = Arc::new(ConsensusManager::new(new_node_id, ReplicationConfig::default()));
    let new_port = base_port + 3;
    let new_addr = format!("127.0.0.1:{}", new_port).parse().unwrap();
    
    new_manager.clone().start_server(new_addr).await.ok();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect new node to existing cluster
    for (i, manager) in managers.iter().enumerate() {
        let peer_id = manager.node_id();
        let peer_port = base_port + i as u16;
        let peer_info = NodeInfo {
            node: peer_id,
            raft_addr: format!("127.0.0.1:{}", peer_port).parse().unwrap(),
            api_addr: format!("127.0.0.1:{}", 8000 + peer_port).parse().unwrap(),
            status: Default::default(),
            capacity: Default::default(),
            zone: None,
            rack: None,
        };
        new_manager.add_peer(peer_id, peer_info).await;

        // Add new node to existing nodes
        let new_peer_info = NodeInfo {
            node: new_node_id,
            raft_addr: new_addr,
            api_addr: format!("127.0.0.1:{}", 8000 + new_port).parse().unwrap(),
            status: Default::default(),
            capacity: Default::default(),
            zone: None,
            rack: None,
        };
        manager.add_peer(new_node_id, new_peer_info).await;
    }

    // Verify new node sees all peers
    assert_eq!(new_manager.peer_nodes().await.len(), 3);

    // Verify existing nodes see new peer
    for manager in &managers {
        assert_eq!(manager.peer_nodes().await.len(), 3);
    }

    managers.push(new_manager);
    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_node_removal() {
    let managers = create_test_cluster(4).await;
    let base_port = 51030;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Remove node 3 from all other nodes
    let removed_id = NodeId::new(3);
    for manager in &managers {
        if manager.node_id() != removed_id {
            manager.remove_peer(removed_id).await;
        }
    }

    // Verify node 3 is removed from peer lists
    for manager in &managers {
        if manager.node_id() != removed_id {
            assert_eq!(manager.peer_nodes().await.len(), 2);
            assert!(!manager.peer_nodes().await.contains(&removed_id));
        }
    }

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_partial_failure() {
    let managers = create_test_cluster(5).await;
    let base_port = 51040;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Simulate failure of 2 nodes by stopping their servers
    managers[1].stop_server().await.ok();
    managers[3].stop_server().await.ok();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify failed nodes are not running
    assert!(!managers[1].is_server_running().await);
    assert!(!managers[3].is_server_running().await);

    // Verify other nodes are still running
    assert!(managers[0].is_server_running().await);
    assert!(managers[2].is_server_running().await);
    assert!(managers[4].is_server_running().await);

    // Cleanup
    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_recovery() {
    let managers = create_test_cluster(3).await;
    let base_port = 51050;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Stop one node
    managers[1].stop_server().await.ok();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!managers[1].is_server_running().await);

    // Restart the node
    let addr = format!("127.0.0.1:{}", base_port + 1).parse().unwrap();
    managers[1].clone().start_server(addr).await.ok();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify node is back up
    assert!(managers[1].is_server_running().await);

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_concurrent_operations() {
    let managers = create_test_cluster(3).await;
    let base_port = 51060;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Perform concurrent operations on all nodes
    let mut handles = vec![];
    for manager in &managers {
        let mgr = manager.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..10 {
                let key = format!("key_{}", i);
                mgr.get_table_shard_for_key(key.as_bytes()).await;
            }
        }));
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_cluster_with_zones() {
    let managers = create_test_cluster(3).await;
    let base_port = 51070;

    start_cluster_servers(&managers, base_port).await;

    // Connect peers with zone information
    let zones = vec!["us-west-1a", "us-west-1b", "us-west-1c"];
    for (i, manager) in managers.iter().enumerate() {
        for (j, peer_manager) in managers.iter().enumerate() {
            if i != j {
                let peer_id = peer_manager.node_id();
                let peer_port = base_port + j as u16;
                let peer_info = NodeInfo {
                    node: peer_id,
                    raft_addr: format!("127.0.0.1:{}", peer_port).parse().unwrap(),
                    api_addr: format!("127.0.0.1:{}", 8000 + peer_port).parse().unwrap(),
                    status: Default::default(),
                    capacity: Default::default(),
                    zone: Some(zones[j].to_string()),
                    rack: Some(format!("rack-{}", j)),
                };
                manager.add_peer(peer_id, peer_info).await;
            }
        }
    }

    // Verify zone information is preserved
    for (i, manager) in managers.iter().enumerate() {
        for (j, _) in managers.iter().enumerate() {
            if i != j {
                let peer_id = NodeId::new((j + 1) as u128);
                let peer_info = manager.get_peer(peer_id).await;
                assert!(peer_info.is_some());
                let peer_info = peer_info.unwrap();
                assert_eq!(peer_info.zone, Some(zones[j].to_string()));
                assert_eq!(peer_info.rack, Some(format!("rack-{}", j)));
            }
        }
    }

    stop_cluster_servers(&managers).await;
}

#[tokio::test]
async fn test_large_cluster() {
    let managers = create_test_cluster(10).await;
    let base_port = 51080;

    start_cluster_servers(&managers, base_port).await;
    connect_cluster_peers(&managers, base_port).await;

    // Verify all nodes are running
    for manager in &managers {
        assert!(manager.is_server_running().await);
        assert_eq!(manager.peer_nodes().await.len(), 9);
    }

    stop_cluster_servers(&managers).await;
}


