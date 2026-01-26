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

//! Unit tests for ConsensusManager

use nanograph_core::object::NodeId;
use nanograph_raft::{ConsensusManager, NodeInfo, ReplicationConfig};
use std::sync::Arc;

#[tokio::test]
async fn test_manager_initialization() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config.clone());

    assert_eq!(manager.node_id(), node_id);
    assert_eq!(manager.config().replication_factor, config.replication_factor);
    assert!(!manager.is_server_running().await);
    assert_eq!(manager.peer_nodes().await.len(), 0);
}

#[tokio::test]
async fn test_manager_with_custom_config() {
    let node_id = NodeId::new(1);
    let mut config = ReplicationConfig::default();
    config.replication_factor = 5;
    config.election_timeout_ms = 1000;
    config.heartbeat_interval_ms = 200;

    let manager = ConsensusManager::new(node_id, config);

    assert_eq!(manager.config().replication_factor, 5);
    assert_eq!(manager.config().election_timeout_ms, 1000);
    assert_eq!(manager.config().heartbeat_interval_ms, 200);
}

#[tokio::test]
async fn test_shard_count_management() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    // Set shard count
    manager.set_shard_count(8).await;

    // Verify routing uses the shard count
    let key1 = b"test_key_1";
    let key2 = b"test_key_2";
    
    let shard1 = manager.get_table_shard_for_key(key1).await;
    let shard2 = manager.get_table_shard_for_key(key2).await;

    // Same key should always route to same shard
    assert_eq!(shard1, manager.get_table_shard_for_key(key1).await);
    assert_eq!(shard2, manager.get_table_shard_for_key(key2).await);
}

#[tokio::test]
async fn test_consistent_key_routing() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    manager.set_shard_count(4).await;

    let key = b"consistent_key";
    let shard = manager.get_table_shard_for_key(key).await;

    // Run multiple times to ensure consistency
    for _ in 0..100 {
        assert_eq!(shard, manager.get_table_shard_for_key(key).await);
    }
}

#[tokio::test]
async fn test_peer_addition_and_removal() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    // Add multiple peers
    for i in 2..=5 {
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

    assert_eq!(manager.peer_nodes().await.len(), 4);

    // Remove a peer
    let removed = manager.remove_peer(NodeId::new(3)).await;
    assert!(removed.is_some());
    assert_eq!(manager.peer_nodes().await.len(), 3);

    // Try to remove non-existent peer
    let not_found = manager.remove_peer(NodeId::new(99)).await;
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_peer_info() {
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
        zone: Some("us-west-1a".to_string()),
        rack: Some("rack-1".to_string()),
    };

    manager.add_peer(peer_id, peer_info.clone()).await;

    let retrieved = manager.get_peer(peer_id).await;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.node, peer_id);
    assert_eq!(retrieved.zone, Some("us-west-1a".to_string()));
    assert_eq!(retrieved.rack, Some("rack-1".to_string()));
}

#[tokio::test]
async fn test_all_peers() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    // Add several peers
    for i in 2..=4 {
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

    let all_peers = manager.all_peers().await;
    assert_eq!(all_peers.len(), 3);
    assert!(all_peers.contains_key(&NodeId::new(2)));
    assert!(all_peers.contains_key(&NodeId::new(3)));
    assert!(all_peers.contains_key(&NodeId::new(4)));
}

#[tokio::test]
async fn test_local_shards_empty() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    let shards = manager.local_shards().await;
    assert_eq!(shards.len(), 0);
}

#[tokio::test]
async fn test_system_metadata_access() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    let metadata = manager.system_metadata();
    assert!(Arc::strong_count(&metadata) >= 1);
}

#[tokio::test]
async fn test_multiple_managers_independent() {
    let config = ReplicationConfig::default();
    
    let manager1 = ConsensusManager::new(NodeId::new(1), config.clone());
    let manager2 = ConsensusManager::new(NodeId::new(2), config);

    // Add peer to manager1
    let peer_info = NodeInfo {
        node: NodeId::new(3),
        raft_addr: "127.0.0.1:50053".parse().unwrap(),
        api_addr: "127.0.0.1:8083".parse().unwrap(),
        status: Default::default(),
        capacity: Default::default(),
        zone: None,
        rack: None,
    };
    manager1.add_peer(NodeId::new(3), peer_info).await;

    // Managers should be independent
    assert_eq!(manager1.peer_nodes().await.len(), 1);
    assert_eq!(manager2.peer_nodes().await.len(), 0);
}

#[tokio::test]
async fn test_shard_routing_distribution() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let manager = ConsensusManager::new(node_id, config);

    manager.set_shard_count(4).await;

    // Generate many keys and check distribution
    let mut shard_counts = std::collections::HashMap::new();
    for i in 0..1000 {
        let key = format!("key_{}", i);
        let shard = manager.get_table_shard_for_key(key.as_bytes()).await;
        *shard_counts.entry(shard).or_insert(0) += 1;
    }

    // Should have distributed across shards (not perfectly, but reasonably)
    assert!(shard_counts.len() > 1, "Keys should distribute across multiple shards");
}


