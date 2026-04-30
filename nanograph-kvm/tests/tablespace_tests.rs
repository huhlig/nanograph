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

use nanograph_core::object::{
    ClusterId, NodeId, Permission, PermissionGrant, ResourceScope, SecurityPrincipal, StorageTier,
    SubjectId, SystemUserRecord, TablespaceCreate, TablespaceId, TablespaceUpdate, UserId,
};
use nanograph_core::types::Timestamp;
use nanograph_kvm::{KeyValueDatabaseConfig, KeyValueDatabaseManager};
use nanograph_raft::ConsensusError::Storage;
use std::collections::HashMap;
use std::time::Duration;

fn create_test_principal() -> SecurityPrincipal {
    let user_record = SystemUserRecord {
        user_id: UserId::new(SubjectId::new(1)),
        username: "admin".to_string(),
        version: 1,
        created_at: Timestamp::now(),
        last_modified: Timestamp::now(),
        group_ids: vec![],
        role_ids: vec![],
        grants: vec![PermissionGrant::new(
            Permission::GlobalSuperuser,
            ResourceScope::System,
        )],
        enabled: true,
        password_hash: None,
        options: HashMap::new(),
        metadata: HashMap::new(),
    };
    SecurityPrincipal::from_system_user(&user_record, &[], &[])
}

#[tokio::test]
async fn test_create_tablespace() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create tablespace
    let create_config = TablespaceCreate::new("hot_storage", "Hot");

    let result = db_manager
        .create_tablespace(&principal, create_config)
        .await;
    assert!(result.is_ok(), "Failed to create tablespace: {:?}", result);
    let metadata = result.unwrap();

    // Verify tablespace was created
    let tablespace = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap();
    assert!(tablespace.is_some(), "Tablespace not found after creation");

    let tablespace = tablespace.unwrap();
    assert_eq!(tablespace.id, metadata.id);
    assert_eq!(tablespace.name, "hot_storage");
    assert_eq!(tablespace.tier, StorageTier::Hot);
    assert_eq!(tablespace.version, 1);
}

#[tokio::test]
async fn test_list_tablespaces() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create multiple tablespaces
    let tablespaces = vec![
        TablespaceCreate::new("hot_storage", "Hot"),
        TablespaceCreate::new("warm_storage", "Warm"),
        TablespaceCreate::new("cold_storage", "Cold"),
    ];

    for config in &tablespaces {
        db_manager
            .create_tablespace(&principal, config.to_owned())
            .await
            .unwrap();
    }

    // List all tablespaces
    let result = db_manager.get_tablespaces(&principal).await.unwrap();
    let tablespace_list: Vec<_> = result.into_iter().collect();

    assert!(
        tablespace_list.len() >= 3,
        "Expected at least 3 tablespaces"
    );

    // Verify all tablespaces are present
    for config in &tablespaces {
        let found = tablespace_list.iter().any(|(_, name)| name == &config.name);
        assert!(found, "Tablespace {} not found in list", config.name);
    }
}

#[tokio::test]
async fn test_update_tablespace() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create tablespace
    let create_config = TablespaceCreate::new("test_storage", "Hot");
    let metadata = db_manager
        .create_tablespace(&principal, create_config)
        .await
        .unwrap();

    // Get initial version
    let initial = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap()
        .unwrap();
    let initial_version = initial.version;
    let initial_modified = initial.updated_at;

    // Small delay to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update tablespace
    let update_config = TablespaceUpdate::default().set_tier(StorageTier::Warm);

    let result = db_manager
        .update_tablespace(&principal, &metadata.id, update_config)
        .await;
    assert!(result.is_ok(), "Failed to update tablespace: {:?}", result);

    // Verify updates
    let updated = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.tier, StorageTier::Warm);
    assert_eq!(
        updated.version,
        initial_version + 1,
        "Version should increment"
    );
    assert!(
        updated.updated_at > initial_modified,
        "Timestamp should update"
    );
}

#[tokio::test]
async fn test_update_nonexistent_tablespace() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Try to update non-existent tablespace
    let tablespace_id = TablespaceId::new(999);
    let update_config = TablespaceUpdate::default();

    let result = db_manager
        .update_tablespace(&principal, &tablespace_id, update_config)
        .await;
    assert!(
        result.is_err(),
        "Should fail to update non-existent tablespace"
    );
}

#[tokio::test]
async fn test_delete_tablespace() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create tablespace
    let create_config = TablespaceCreate::new("temp_storage", StorageTier::Cold);
    let metadata = db_manager
        .create_tablespace(&principal, create_config)
        .await
        .unwrap();

    // Verify it exists
    let exists = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap();
    assert!(exists.is_some(), "Tablespace should exist before deletion");

    // Delete tablespace
    let result = db_manager.delete_tablespace(&principal, &metadata.id).await;
    assert!(result.is_ok(), "Failed to delete tablespace: {:?}", result);

    // Verify it's gone
    let deleted = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap();
    assert!(
        deleted.is_none(),
        "Tablespace should not exist after deletion"
    );
}

#[tokio::test]
async fn test_tablespace_with_options_and_metadata() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create tablespace with options and metadata
    let mut create_config = TablespaceCreate::new("configured_storage", StorageTier::Hot);

    create_config = create_config
        .add_option("compression", "zstd")
        .add_option("encryption", "aes256")
        .add_metadata("owner", "admin")
        .add_metadata("purpose", "production");

    let metadata = db_manager
        .create_tablespace(&principal, create_config)
        .await
        .unwrap();

    // Verify options and metadata were stored
    let tablespace = db_manager
        .get_tablespace(&principal, &metadata.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        tablespace.options.get("compression"),
        Some(&"zstd".to_string())
    );
    assert_eq!(
        tablespace.options.get("encryption"),
        Some(&"aes256".to_string())
    );
    assert_eq!(tablespace.metadata.get("owner"), Some(&"admin".to_string()));
    assert_eq!(
        tablespace.metadata.get("purpose"),
        Some(&"production".to_string())
    );
}

#[tokio::test]
async fn test_tablespace_storage_tiers() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Create tablespaces for each tier
    let tiers = vec!["Hot", "Warm", "Cold", "Archive"];

    for tier in &tiers {
        let create_config =
            TablespaceCreate::new(format!("{}_storage", tier.to_lowercase()), *tier);
        db_manager
            .create_tablespace(&principal, create_config)
            .await
            .unwrap();
    }

    // Verify all tiers were created correctly
    let all_tablespaces = db_manager.get_tablespaces(&principal).await.unwrap();
    let tablespace_list: Vec<_> = all_tablespaces.into_iter().collect();

    assert!(
        tablespace_list.len() >= 4,
        "Should have at least 4 tablespaces"
    );

    for tier in &tiers {
        let found = tablespace_list
            .iter()
            .any(|(_, name)| name == &format!("{}_storage", tier.to_lowercase()));
        assert!(found, "Tier {} not found", tier);
    }
}

#[tokio::test]
async fn test_get_nonexistent_tablespace() {
    // Setup
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let db_manager = KeyValueDatabaseManager::new_standalone(config)
        .await
        .unwrap();
    let principal = create_test_principal();

    // Try to get non-existent tablespace
    let result = db_manager
        .get_tablespace(&principal, &TablespaceId::new(999))
        .await;
    assert!(
        result.is_ok(),
        "Should not error on non-existent tablespace"
    );
    assert!(
        result.unwrap().is_none(),
        "Should return None for non-existent tablespace"
    );
}
