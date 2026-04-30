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

mod logstore;
mod snapshot;
mod statestore;

pub use self::logstore::ConsensusLogStore;
pub use self::snapshot::{SnapshotConfig, SnapshotManager};
pub use self::statestore::ConsensusStateStore;

#[cfg(test)]
mod tests {
    use crate::storage::{ConsensusLogStore, ConsensusStateStore, SnapshotManager};
    use crate::types::ConsensusTypeConfig;
    use nanograph_core::config::StorageConfig;
    use nanograph_core::object::{
        DatabaseId, NodeId, ObjectId, ShardId, ShardNumber, StorageTier, TablespaceId,
        TablespaceRecord, TenantId,
    };
    use nanograph_kvt::{
        KeyValueShardStore, MemoryKeyValueShardStore, StoragePathResolver, Timestamp,
    };
    use nanograph_vfs::MemoryFileSystem;
    use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
    use openraft::StorageError;
    use openraft::testing::log::StoreBuilder;
    use std::sync::Arc;

    pub struct ConsensusStorageBuilder;

    impl StoreBuilder<ConsensusTypeConfig, ConsensusLogStore, ConsensusStateStore>
        for ConsensusStorageBuilder
    {
        async fn build(
            &self,
        ) -> Result<((), ConsensusLogStore, ConsensusStateStore), StorageError<ConsensusTypeConfig>>
        {
            use nanograph_vfs::FileSystem;

            // create a *fresh* instance every time
            let filesystem = Arc::new(MemoryFileSystem::new());

            // Create required directories
            filesystem.create_directory_all("/data").unwrap();
            filesystem.create_directory_all("/data/system").unwrap();
            filesystem.create_directory_all("/data/log").unwrap();

            let shard_id = ShardId::from_parts(
                TenantId::new(0),
                DatabaseId::new(0),
                ObjectId::new(0),
                ShardNumber::new(0),
            );

            let config = WriteAheadLogConfig {
                shard_id: shard_id.as_u128(),
                max_segment_size: 100,
                sync_on_rotate: false,
                checksum: Default::default(),
                compression: Default::default(),
                encryption: Default::default(),
                encryption_key: None,
            };
            let wal = Arc::new(
                WriteAheadLogManager::new(filesystem.clone(), "/data/log", config).unwrap(),
            );
            let memstore = Arc::new(MemoryKeyValueShardStore::new());
            // Pre-create the shard so it exists in the memstore
            let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
            let data_path = nanograph_vfs::Path::from("/data");
            let wal_path = nanograph_vfs::Path::from("/wal");
            memstore
                .create_shard(shard_id, vfs, data_path, wal_path)
                .unwrap();
            // Create basic directories for the resolver to be happy if it validates paths
            filesystem
                .create_directory_all("/data/system/system/metadata/snapshots")
                .unwrap();
            filesystem
                .create_directory_all("/data/system/system/metadata/logs")
                .unwrap();
            filesystem
                .create_directory_all("/data/system/system/data")
                .unwrap();
            filesystem
                .create_directory_all("/data/system/system/wal")
                .unwrap();
            filesystem.create_directory_all("/data/log").unwrap();

            let resolver = StoragePathResolver::new(
                filesystem.clone(),
                StorageConfig {
                    system_path: "/data/system".to_string(),
                    log_path: "/data/log".to_string(),
                    tablespaces: std::collections::HashMap::from([(
                        "default".to_string(),
                        nanograph_core::config::TablespaceConfig {
                            storage_path: "/data/system".to_string(),
                        },
                    )]),
                },
            );
            resolver
                .register_tablespace(TablespaceRecord {
                    id: TablespaceId::DEFAULT,
                    version: 0,
                    created_at: Timestamp::epoch(),
                    updated_at: Timestamp::epoch(),
                    name: "default".to_string(),
                    tier: StorageTier::Hot,
                    tenants: vec![],
                    options: Default::default(),
                    metadata: Default::default(),
                })
                .unwrap();
            let snapshot_path = resolver
                .system_raft_snapshots_path(TablespaceId::DEFAULT)
                .unwrap();

            // Create snapshot directory
            filesystem
                .create_directory_all(&snapshot_path.to_string())
                .unwrap();

            let snapshot_manager =
                Arc::new(SnapshotManager::new(filesystem.clone(), snapshot_path));
            Ok((
                (),
                ConsensusLogStore::new(wal),
                ConsensusStateStore::new(shard_id, memstore, snapshot_manager),
            ))
        }
    }

    // Disabled: This test runs the full openraft test suite which can be very slow
    // It has been replaced with individual tests.
    #[ignore]
    #[tokio::test]
    async fn test_consensus_log_store_all() {
        openraft::testing::log::Suite::test_all(ConsensusStorageBuilder)
            .await
            .unwrap();
    }

    // Individual tests from openraft::testing::log::Suite
    // These tests are extracted from Suite::test_all for granular testing

    #[tokio::test]
    async fn test_last_membership_in_log_initial() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::last_membership_in_log_initial(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_last_membership_in_log() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::last_membership_in_log(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_last_membership_in_log_multi_step() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::last_membership_in_log_multi_step(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_initial() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_initial(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_from_log_and_empty_sm() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_from_log_and_empty_sm(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_from_empty_log_and_sm() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_from_empty_log_and_sm(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_from_log_le_sm_last_applied() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_from_log_le_sm_last_applied(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_from_log_gt_sm_last_applied_1() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_from_log_gt_sm_last_applied_1(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_membership_from_log_gt_sm_last_applied_2() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_membership_from_log_gt_sm_last_applied_2(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_without_init() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_without_init(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_membership_from_empty_log_and_sm() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_membership_from_empty_log_and_sm(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_membership_from_sm_inlog_is_smaller() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_membership_from_sm_inlog_is_smaller(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_membership_from_log_insm_is_smaller() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_membership_from_log_insm_is_smaller(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_with_state() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_with_state(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_last_log_gt_sm() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_last_log_gt_sm(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_last_log_lt_sm() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_last_log_lt_sm(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_log_ids() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_log_ids(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_initial_state_re_apply_committed() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_initial_state_re_apply_committed(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_save_vote() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::save_vote(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_log_entries() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_log_entries(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_limited_get_log_entries() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::limited_get_log_entries(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_leader_bounded_stream() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::leader_bounded_stream(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_entries_stream() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::entries_stream(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_try_get_log_entry() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::try_get_log_entry(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_log_reader_reads_new_entries() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::log_reader_reads_new_entries(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_initial_logs() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::initial_logs(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_log_state() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_log_state(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_get_log_id() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::get_log_id(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_last_id_in_log() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::last_id_in_log(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_last_applied_state() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::last_applied_state(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_purge_logs_upto_0() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::purge_logs_upto_0(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_purge_logs_upto_5() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::purge_logs_upto_5(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_purge_logs_upto_20() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::purge_logs_upto_20(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_delete_logs_after_11() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::delete_logs_after_11(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_delete_logs_after_5() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::delete_logs_after_5(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_delete_logs_after_0() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::delete_logs_after_0(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_delete_logs_after_none() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::delete_logs_after_none(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_append_to_log() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::append_to_log(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_snapshot_meta() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::snapshot_meta(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_snapshot_meta_optional() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::snapshot_meta_optional(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_apply_single() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::apply_single(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_apply_multiple() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        let (_, store, sm) = builder.build().await.unwrap();
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::apply_multiple(store, sm)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_transfer_snapshot() {
        use openraft::testing::log::Suite;
        let builder = ConsensusStorageBuilder;
        Suite::<
            ConsensusTypeConfig,
            ConsensusLogStore,
            ConsensusStateStore,
            ConsensusStorageBuilder,
            (),
        >::transfer_snapshot(&builder)
        .await
        .unwrap();
    }
}
