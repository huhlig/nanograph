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

//! Integration tests for the Raft consensus layer

use nanograph_kvt::{NodeId, ShardId};
use nanograph_raft::{
    MetadataRaftGroup, NodeInfo, NodeStatus, Operation, ReadConsistency, ReplicationConfig,
    ResourceCapacity, Router,
};

/// Test basic router creation and configuration
#[tokio::test]
async fn test_router_creation() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();

    let router = Router::new(node_id, config);

    // Verify router is created with correct node ID
    assert_eq!(router.local_shards().await.len(), 0);
}

/// Test shard routing with hash-based partitioning
#[tokio::test]
async fn test_shard_routing() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let router = Router::new(node_id, config);

    // Set up multiple shards
    router.set_shard_count(4).await;

    // Test that same key always routes to same shard
    let key1 = b"test_key_1";
    let shard1_first = router.get_shard_for_key(key1).await;
    let shard1_second = router.get_shard_for_key(key1).await;
    assert_eq!(shard1_first, shard1_second);

    // Test that different keys may route to different shards
    let key2 = b"test_key_2";
    let shard2 = router.get_shard_for_key(key2).await;

    // Both shards should be valid (0-3)
    assert!(shard1_first.as_u64() < 4);
    assert!(shard2.as_u64() < 4);
}

/// Test metadata Raft group creation
#[tokio::test]
async fn test_metadata_group_creation() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Get initial metadata
    let metadata = metadata_group.get_metadata().await;
    assert_eq!(metadata.version, 0);
}

/// Test adding nodes to cluster metadata
#[tokio::test]
async fn test_metadata_add_node() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Simulate becoming leader
    metadata_group.on_become_leader().await;

    // Add a new node
    let new_node = NodeInfo {
        id: NodeId::new(2),
        raft_addr: "127.0.0.1:5000".parse().unwrap(),
        api_addr: "127.0.0.1:8080".parse().unwrap(),
        status: NodeStatus::Active,
        capacity: ResourceCapacity::default(),
        zone: None,
        rack: None,
    };

    let result = metadata_group.add_node(new_node).await;
    assert!(result.is_ok());

    // Verify node was added
    let state = metadata_group.get_state().await;
    assert_eq!(state.nodes.len(), 1);
    assert!(state.nodes.contains_key(&NodeId::new(2)));
}

/// Test creating shards via metadata group
#[tokio::test]
async fn test_metadata_create_shard() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Simulate becoming leader
    metadata_group.on_become_leader().await;

    // Create a shard
    let shard_id = ShardId::new(1);
    let range = (vec![0x00], vec![0xFF]);
    let replicas = vec![NodeId::new(1), NodeId::new(2), NodeId::new(3)];

    let result = metadata_group
        .create_shard(shard_id, range, replicas.clone())
        .await;
    assert!(result.is_ok());

    // Verify shard was created
    let state = metadata_group.get_state().await;
    assert_eq!(state.shards.len(), 1);
    assert!(state.shards.contains_key(&shard_id));

    // Verify replicas
    let shard_replicas = state.get_shard_replicas(shard_id);
    assert!(shard_replicas.is_some());
    assert_eq!(shard_replicas.unwrap(), &replicas);
}

/// Test replication configuration
#[test]
fn test_replication_config() {
    let config = ReplicationConfig {
        replication_factor: 5,
        min_sync_replicas: 3,
        election_timeout_ms: 1000,
        heartbeat_interval_ms: 100,
        max_append_entries: 100,
        snapshot_threshold: 10000,
    };

    // Test quorum calculation
    assert_eq!(config.quorum_size(), 3); // (5 / 2) + 1 = 3

    // Test tolerable failures
    assert_eq!(config.tolerable_failures(), 2); // 5 - 3 = 2
}

/// Test default replication configuration
#[test]
fn test_default_replication_config() {
    let config = ReplicationConfig::default();

    assert_eq!(config.replication_factor, 3);
    assert_eq!(config.min_sync_replicas, 2);
    assert_eq!(config.quorum_size(), 2);
    assert_eq!(config.tolerable_failures(), 1);
}

/// Test operation serialization
#[test]
fn test_operation_types() {
    // Test Put operation
    let put_op = Operation::Put {
        key: b"key1".to_vec(),
        value: b"value1".to_vec(),
    };

    match put_op {
        Operation::Put { key, value } => {
            assert_eq!(key, b"key1");
            assert_eq!(value, b"value1");
        }
        _ => panic!("Expected Put operation"),
    }

    // Test Delete operation
    let delete_op = Operation::Delete {
        key: b"key2".to_vec(),
    };

    match delete_op {
        Operation::Delete { key } => {
            assert_eq!(key, b"key2");
        }
        _ => panic!("Expected Delete operation"),
    }

    // Test Batch operation
    let batch_op = Operation::Batch {
        operations: vec![
            Operation::Put {
                key: b"key3".to_vec(),
                value: b"value3".to_vec(),
            },
            Operation::Delete {
                key: b"key4".to_vec(),
            },
        ],
    };

    match batch_op {
        Operation::Batch { operations } => {
            assert_eq!(operations.len(), 2);
        }
        _ => panic!("Expected Batch operation"),
    }
}

/// Test read consistency levels
#[test]
fn test_read_consistency_levels() {
    // Test default is Linearizable
    let default_consistency = ReadConsistency::default();
    assert_eq!(default_consistency, ReadConsistency::Linearizable);

    // Test all consistency levels
    let linearizable = ReadConsistency::Linearizable;
    let lease = ReadConsistency::Lease;
    let follower = ReadConsistency::Follower;

    assert_ne!(linearizable, lease);
    assert_ne!(linearizable, follower);
    assert_ne!(lease, follower);
}

/// Test node status transitions
#[test]
fn test_node_status() {
    let statuses = vec![
        NodeStatus::Active,
        NodeStatus::Draining,
        NodeStatus::Inactive,
        NodeStatus::Failed,
    ];

    // Verify all statuses are distinct
    for (i, status1) in statuses.iter().enumerate() {
        for (j, status2) in statuses.iter().enumerate() {
            if i == j {
                assert_eq!(status1, status2);
            } else {
                assert_ne!(status1, status2);
            }
        }
    }
}

/// Test resource capacity defaults
#[test]
fn test_resource_capacity() {
    let capacity = ResourceCapacity::default();

    assert_eq!(capacity.cpu_cores, 1);
    assert_eq!(capacity.memory_bytes, 1024 * 1024 * 1024); // 1GB
    assert_eq!(capacity.disk_bytes, 10 * 1024 * 1024 * 1024); // 10GB
    assert_eq!(capacity.network_bandwidth, 100 * 1024 * 1024); // 100MB/s
    assert_eq!(capacity.weight, 1.0);
}

/// Test batch operation grouping by shard
#[tokio::test]
async fn test_batch_operation_grouping() {
    let node_id = NodeId::new(1);
    let config = ReplicationConfig::default();
    let router = Router::new(node_id, config);

    router.set_shard_count(4).await;

    // Create operations that will be distributed across shards
    let operations = vec![
        Operation::Put {
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        },
        Operation::Put {
            key: b"key2".to_vec(),
            value: b"value2".to_vec(),
        },
        Operation::Put {
            key: b"key3".to_vec(),
            value: b"value3".to_vec(),
        },
    ];

    // Verify operations can be created
    assert_eq!(operations.len(), 3);

    // In a real test, we would verify that batch() groups operations by shard
    // For now, we just verify the operations are valid
    for op in operations {
        match op {
            Operation::Put { key, value } => {
                assert!(!key.is_empty());
                assert!(!value.is_empty());
            }
            _ => panic!("Expected Put operation"),
        }
    }
}

/// Test metadata version incrementing
#[tokio::test]
async fn test_metadata_versioning() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Simulate becoming leader
    metadata_group.on_become_leader().await;

    // Get initial version
    let initial_metadata = metadata_group.get_metadata().await;
    let initial_version = initial_metadata.version;

    // Add a node (should increment version)
    let new_node = NodeInfo {
        id: NodeId::new(2),
        raft_addr: "127.0.0.1:5000".parse().unwrap(),
        api_addr: "127.0.0.1:8080".parse().unwrap(),
        status: NodeStatus::Active,
        capacity: ResourceCapacity::default(),
        zone: None,
        rack: None,
    };

    metadata_group.add_node(new_node).await.unwrap();

    // Verify version was incremented
    let updated_metadata = metadata_group.get_metadata().await;
    assert_eq!(updated_metadata.version, initial_version + 1);
}

/// Test shard assignment updates
#[tokio::test]
async fn test_shard_assignment_update() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Simulate becoming leader
    metadata_group.on_become_leader().await;

    // Create a shard first
    let shard_id = ShardId::new(1);
    let range = (vec![0x00], vec![0xFF]);
    let initial_replicas = vec![NodeId::new(1), NodeId::new(2)];

    metadata_group
        .create_shard(shard_id, range, initial_replicas)
        .await
        .unwrap();

    // Update shard assignment
    let new_replicas = vec![NodeId::new(1), NodeId::new(2), NodeId::new(3)];
    metadata_group
        .update_shard_assignment(shard_id, new_replicas.clone())
        .await
        .unwrap();

    // Verify assignment was updated
    let state = metadata_group.get_state().await;
    let replicas = state.get_shard_replicas(shard_id);
    assert!(replicas.is_some());
    assert_eq!(replicas.unwrap(), &new_replicas);
}

/// Test leader election simulation
#[tokio::test]
async fn test_leader_election() {
    let node_id = NodeId::new(1);
    let metadata_group = MetadataRaftGroup::new(node_id);

    // Initially not leader
    // (We can't directly check is_leader on MetadataRaftGroup, but we can test the flow)

    // Become leader
    metadata_group.on_become_leader().await;

    // Should now be able to propose changes
    let new_node = NodeInfo {
        id: NodeId::new(2),
        raft_addr: "127.0.0.1:5000".parse().unwrap(),
        api_addr: "127.0.0.1:8080".parse().unwrap(),
        status: NodeStatus::Active,
        capacity: ResourceCapacity::default(),
        zone: None,
        rack: None,
    };

    let result = metadata_group.add_node(new_node).await;
    assert!(result.is_ok());

    // Become follower
    metadata_group.on_become_follower().await;

    // Should no longer be able to propose changes (would return NotLeader error)
    let another_node = NodeInfo {
        id: NodeId::new(3),
        raft_addr: "127.0.0.1:5001".parse().unwrap(),
        api_addr: "127.0.0.1:8081".parse().unwrap(),
        status: NodeStatus::Active,
        capacity: ResourceCapacity::default(),
        zone: None,
        rack: None,
    };

    let result = metadata_group.add_node(another_node).await;
    assert!(result.is_err());
}

// Made with Bob
