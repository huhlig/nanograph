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

use criterion::{Criterion, criterion_group, criterion_main};
use nanograph_art::ArtKeyValueStore;
use nanograph_core::object::{
    ClusterCreate, ContainerId, DatabaseCreate, NodeId, Permission, PermissionGrant, ResourceScope,
    SecurityPrincipal, StorageEngineType, SystemUserRecord, TableCreate, TableRecord,
    TablespaceCreate, TenantCreate, UserId,
};
use nanograph_core::types::Timestamp;
use nanograph_kvm::{KeyValueDatabaseConfig, KeyValueDatabaseManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn create_test_principal() -> SecurityPrincipal {
    let user_record = SystemUserRecord {
        user_id: UserId::new(1),
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

fn setup_manager() -> (Arc<KeyValueDatabaseManager>, SecurityPrincipal, Runtime) {
    let rt = Runtime::new().unwrap();
    let config = KeyValueDatabaseConfig {
        node_id: NodeId::new(1),
        cache_ttl: Duration::from_secs(60),
    };
    let manager = rt
        .block_on(KeyValueDatabaseManager::new_standalone(config))
        .unwrap();
    let manager = Arc::new(manager);
    let principal = create_test_principal();

    (manager, principal, rt)
}

async fn setup_hierarchy(
    manager: &KeyValueDatabaseManager,
    principal: &SecurityPrincipal,
) -> (ContainerId, TableRecord) {
    manager
        .initialize_cluster(principal, ClusterCreate::new("bench-cluster"))
        .await
        .unwrap();
    manager
        .create_tablespace(principal, TablespaceCreate::new("bench-ts", "Hot"))
        .await
        .unwrap();
    let tenant = manager
        .create_tenant(principal, TenantCreate::new("bench-tenant"))
        .await
        .unwrap();
    let db = manager
        .create_database(principal, &tenant.id, DatabaseCreate::new("bench-db"))
        .await
        .unwrap();
    let container_id = ContainerId::from_parts(tenant.id, db.id);

    let table = manager
        .create_table(
            principal,
            &container_id,
            TableCreate::new("bench-table", "path", StorageEngineType::new("ART")),
        )
        .await
        .unwrap();

    (container_id, table)
}

fn bench_metadata_ops(c: &mut Criterion) {
    let (manager, principal, rt) = setup_manager();

    c.bench_function("metadata_create_tenant", |b| {
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            async move {
                manager
                    .create_tenant(&principal, TenantCreate::new("tenant"))
                    .await
                    .unwrap();
            }
        })
    });

    c.bench_function("metadata_create_database", |b| {
        let tenant = rt
            .block_on(manager.create_tenant(&principal, TenantCreate::new("t")))
            .unwrap();
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            let tenant_id = tenant.id;
            async move {
                manager
                    .create_database(&principal, &tenant_id, DatabaseCreate::new("db"))
                    .await
                    .unwrap();
            }
        })
    });
}

fn bench_kv_ops(c: &mut Criterion) {
    let (manager, principal, rt) = setup_manager();
    let (container_id, table) = rt.block_on(setup_hierarchy(&manager, &principal));
    let table_id = table.id;

    c.bench_function("kv_put", |b| {
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            let container_id = container_id;
            let table_id = table_id;
            async move {
                manager
                    .put(&principal, &container_id, &table_id, b"key", b"value")
                    .await
                    .unwrap();
            }
        })
    });

    rt.block_on(manager.put(&principal, &container_id, &table_id, b"key", b"value"))
        .unwrap();

    c.bench_function("kv_get", |b| {
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            let container_id = container_id;
            let table_id = table_id;
            async move {
                manager
                    .get(&principal, &container_id, &table_id, b"key")
                    .await
                    .unwrap();
            }
        })
    });

    c.bench_function("kv_delete", |b| {
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            let container_id = container_id;
            let table_id = table_id;
            async move {
                manager
                    .delete(&principal, &container_id, &table_id, b"key")
                    .await
                    .unwrap();
            }
        })
    });
}

fn bench_batch_ops(c: &mut Criterion) {
    let (manager, principal, rt) = setup_manager();
    let (container_id, table) = rt.block_on(setup_hierarchy(&manager, &principal));
    let table_id = table.id;

    let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..100)
        .map(|i| {
            (
                format!("key-{}", i).into_bytes(),
                format!("value-{}", i).into_bytes(),
            )
        })
        .collect();

    c.bench_function("kv_batch_put_100", |b| {
        b.to_async(&rt).iter(|| {
            let manager = Arc::clone(&manager);
            let principal = principal.clone();
            let container_id = container_id;
            let table_id = table_id;
            let pairs = pairs.clone();
            async move {
                let borrowed_pairs: Vec<(&[u8], &[u8])> = pairs
                    .iter()
                    .map(|(k, v)| (k.as_slice(), v.as_slice()))
                    .collect();
                manager
                    .batch_put(&principal, &container_id, &table_id, &borrowed_pairs)
                    .await
                    .unwrap();
            }
        })
    });
}

criterion_group!(benches, bench_metadata_ops, bench_kv_ops, bench_batch_ops);
criterion_main!(benches);
