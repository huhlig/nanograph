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

use crate::metrics::{EngineStats, ShardStats};
use crate::{
    KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, Transaction, TransactionId,
};
use async_trait::async_trait;
use futures_core::Stream;
use nanograph_core::object::{KeyRange, ShardId};
use nanograph_core::types::Timestamp;
use nanograph_vfs::{DynamicFileSystem, Path};
use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

type ShardData = BTreeMap<Vec<u8>, Vec<u8>>;

#[derive(Debug, Default)]
pub struct MemoryKeyValueShardStore {
    shards: Arc<RwLock<HashMap<ShardId, Arc<RwLock<ShardData>>>>>,
}

impl MemoryKeyValueShardStore {
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_shard(&self, shard_id: ShardId) -> KeyValueResult<Arc<RwLock<ShardData>>> {
        self.shards
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?
            .get(&shard_id)
            .cloned()
            .ok_or(KeyValueError::ShardNotFound(shard_id))
    }
}

#[async_trait]
impl KeyValueShardStore for MemoryKeyValueShardStore {
    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(data.get(key).cloned())
    }

    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;
        let mut data = shard_data
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let shard_data = self.get_shard(shard)?;
        let mut data = shard_data
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(data.remove(key).is_some())
    }

    async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(data.contains_key(key))
    }

    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(data.get(*key).cloned());
        }
        Ok(results)
    }

    async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;
        let mut data = shard_data
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        for (key, value) in pairs {
            data.insert(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let shard_data = self.get_shard(shard)?;
        let mut data = shard_data
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let mut count = 0;
        for key in keys {
            if data.remove(*key).is_some() {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;

        let items: Vec<(Vec<u8>, Vec<u8>)> = if range.reverse {
            data.range((range.start.clone(), range.end.clone()))
                .rev()
                .take(range.limit.unwrap_or(usize::MAX))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            data.range((range.start.clone(), range.end.clone()))
                .take(range.limit.unwrap_or(usize::MAX))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        Ok(Box::new(MemoryKeyValueIterator::new(items)))
    }

    async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(data.len() as u64)
    }

    async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats> {
        let shard_data = self.get_shard(shard)?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;

        let size: u64 = data.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() as u64;

        Ok(ShardStats {
            key_count: data.len() as u64,
            total_bytes: size,
            data_bytes: size,
            index_bytes: 0,
            last_modified: None,
            engine_stats: EngineStats::default(),
        })
    }

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        Ok(Arc::new(MemoryTransaction::new(self.shards.clone())))
    }

    fn create_shard(
        &self,
        shard: ShardId,
        _vfs: Arc<dyn DynamicFileSystem>,
        _data_path: Path,
        _wal_path: Path,
    ) -> KeyValueResult<()> {
        // Memory store doesn't use filesystem paths
        let mut shards = self
            .shards
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shards
            .entry(shard)
            .or_insert_with(|| Arc::new(RwLock::new(BTreeMap::new())));
        Ok(())
    }

    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let mut shards = self
            .shards
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shards.remove(&shard);
        Ok(())
    }

    async fn clear(&self, shard: ShardId) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;
        let mut data = shard_data
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        data.clear();
        Ok(())
    }

    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>> {
        let shards = self
            .shards
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(shards.keys().cloned().collect())
    }

    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool> {
        let shards = self
            .shards
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(shards.contains_key(&shard))
    }

    async fn flush(&self) -> KeyValueResult<()> {
        Ok(())
    }

    async fn compact(&self, _shard: Option<ShardId>) -> KeyValueResult<()> {
        Ok(())
    }
}

pub struct MemoryKeyValueIterator {
    items: Vec<(Vec<u8>, Vec<u8>)>,
    index: usize,
}

impl MemoryKeyValueIterator {
    fn new(items: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
        Self { items, index: 0 }
    }
}

impl Stream for MemoryKeyValueIterator {
    type Item = KeyValueResult<(Vec<u8>, Vec<u8>)>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Poll::Ready(Some(Ok(item)))
        } else {
            Poll::Ready(None)
        }
    }
}

impl KeyValueIterator for MemoryKeyValueIterator {
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()> {
        if let Some(pos) = self.items.iter().position(|(k, _)| k.as_slice() >= key) {
            self.index = pos;
        } else {
            self.index = self.items.len();
        }
        Ok(())
    }

    fn position(&self) -> Option<Vec<u8>> {
        self.items.get(self.index).map(|(k, _)| k.clone())
    }

    fn valid(&self) -> bool {
        self.index < self.items.len()
    }
}

pub struct MemoryTransaction {
    shards_ref: Arc<RwLock<HashMap<ShardId, Arc<RwLock<ShardData>>>>>,
    // For simplicity, we'll just track writes and apply them on commit.
    // This isn't full snapshot isolation but it's a start for a memory store.
    pending_writes: RwLock<HashMap<ShardId, HashMap<Vec<u8>, Option<Vec<u8>>>>>,
    id: TransactionId,
    ts: Timestamp,
}

impl MemoryTransaction {
    fn new(shards_ref: Arc<RwLock<HashMap<ShardId, Arc<RwLock<ShardData>>>>>) -> Self {
        use rand::Rng;
        let mut rng = rand::rng();
        Self {
            shards_ref,
            pending_writes: RwLock::new(HashMap::new()),
            id: TransactionId(rng.random()),
            ts: Timestamp::now(),
        }
    }
}

#[async_trait]
impl Transaction for MemoryTransaction {
    fn id(&self) -> TransactionId {
        self.id
    }

    fn snapshot_ts(&self) -> Timestamp {
        self.ts
    }

    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        // Check pending writes first
        let pending = self
            .pending_writes
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        if let Some(shard_pending) = pending.get(&shard) {
            if let Some(value_opt) = shard_pending.get(key) {
                return Ok(value_opt.clone());
            }
        }

        // Otherwise check the store
        let shards = self
            .shards_ref
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        if let Some(shard_data) = shards.get(&shard) {
            let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;
            Ok(data.get(key).cloned())
        } else {
            Err(KeyValueError::ShardNotFound(shard))
        }
    }

    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let mut pending = self
            .pending_writes
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        pending
            .entry(shard)
            .or_default()
            .insert(key.to_vec(), Some(value.to_vec()));
        Ok(())
    }

    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let mut pending = self
            .pending_writes
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        pending.entry(shard).or_default().insert(key.to_vec(), None);
        Ok(true) // We don't know if it existed yet without checking, but it's scheduled for deletion
    }

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        // This is complex because we need to merge pending writes with the store.
        // For a simple memory store used for testing, we might just scan the store and apply pending writes if they fall in range.

        let shards = self
            .shards_ref
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let shard_data = shards
            .get(&shard)
            .ok_or(KeyValueError::ShardNotFound(shard))?;
        let data = shard_data.read().map_err(|_| KeyValueError::LockPoisoned)?;

        let mut merged_data = data.clone();

        let pending = self
            .pending_writes
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        if let Some(shard_pending) = pending.get(&shard) {
            for (k, v_opt) in shard_pending {
                if let Some(v) = v_opt {
                    merged_data.insert(k.clone(), v.clone());
                } else {
                    merged_data.remove(k);
                }
            }
        }

        let items: Vec<(Vec<u8>, Vec<u8>)> = if range.reverse {
            merged_data
                .range((range.start.clone(), range.end.clone()))
                .rev()
                .take(range.limit.unwrap_or(usize::MAX))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            merged_data
                .range((range.start.clone(), range.end.clone()))
                .take(range.limit.unwrap_or(usize::MAX))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        Ok(Box::new(MemoryKeyValueIterator::new(items)))
    }

    async fn commit(self: Arc<Self>, _durability: nanograph_wal::Durability) -> KeyValueResult<()> {
        // Memory store doesn't use durability - it's always in-memory only
        let pending = self
            .pending_writes
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let mut shards = self
            .shards_ref
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        for (shard_id, writes) in pending.iter() {
            let shard_data = shards
                .get_mut(shard_id)
                .ok_or(KeyValueError::ShardNotFound(*shard_id))?;
            let mut data = shard_data
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            for (k, v_opt) in writes {
                if let Some(v) = v_opt {
                    data.insert(k.clone(), v.clone());
                } else {
                    data.remove(k);
                }
            }
        }
        Ok(())
    }

    async fn rollback(self: Arc<Self>) -> KeyValueResult<()> {
        let mut pending = self
            .pending_writes
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use nanograph_core::object::{DatabaseId, ShardNumber, TableId, TenantId};

    async fn setup_store() -> (MemoryKeyValueShardStore, ShardId) {
        let store = MemoryKeyValueShardStore::new();
        let tenant_id = TenantId::from(1);
        let database_id = DatabaseId::from(1);
        let table_id = TableId::from(1);
        let shard_number = ShardNumber::from(1);
        let shard_id = ShardId::from_parts(tenant_id, database_id, table_id.0, shard_number);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();
        (store, shard_id)
    }

    #[tokio::test]
    async fn test_basic_ops() {
        let (store, shard) = setup_store().await;

        // Put and Get
        store.put(shard, b"key1", b"value1").await.unwrap();
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Exists
        assert!(store.exists(shard, b"key1").await.unwrap());
        assert!(!store.exists(shard, b"key2").await.unwrap());

        // Delete
        assert!(store.delete(shard, b"key1").await.unwrap());
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
        assert!(!store.delete(shard, b"key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_batch_ops() {
        let (store, shard) = setup_store().await;

        let pairs: &[(&[u8], &[u8])] = &[(b"k1", b"v1"), (b"k2", b"v2"), (b"k3", b"v3")];

        store.batch_put(shard, pairs).await.unwrap();

        let keys: &[&[u8]] = &[b"k1", b"k2", b"k4"];
        let results = store.batch_get(shard, keys).await.unwrap();
        assert_eq!(results[0], Some(b"v1".to_vec()));
        assert_eq!(results[1], Some(b"v2".to_vec()));
        assert_eq!(results[2], None);

        let deleted = store
            .batch_delete(shard, &[b"k1", b"k3", b"k5"])
            .await
            .unwrap();
        assert_eq!(deleted, 2);
        assert!(!store.exists(shard, b"k1").await.unwrap());
        assert!(store.exists(shard, b"k2").await.unwrap());
    }

    #[tokio::test]
    async fn test_shard_management() {
        let store = MemoryKeyValueShardStore::new();
        let tenant1 = TenantId::from(1);
        let database1 = DatabaseId::from(1);
        let table1 = TableId::from(1);
        let shard1 = ShardId::from_parts(tenant1, database1, table1.0, ShardNumber::from(1));
        let shard2 = ShardId::from_parts(tenant1, database1, table1.0, ShardNumber::from(2));
        let table2 = TableId::from(2);
        let shard3 = ShardId::from_parts(tenant1, database1, table2.0, ShardNumber::from(1));
        let shard4 = ShardId::from_parts(tenant1, database1, table2.0, ShardNumber::from(2));

        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard1, vfs.clone(), data_path.clone(), wal_path.clone())
            .unwrap();
        store
            .create_shard(shard2, vfs.clone(), data_path.clone(), wal_path.clone())
            .unwrap();
        store
            .create_shard(shard3, vfs, data_path, wal_path)
            .unwrap();

        let shards = store.list_shards().await.unwrap();
        assert_eq!(shards.len(), 3);
        assert!(shards.contains(&shard1));
        assert!(shards.contains(&shard2));
        assert!(shards.contains(&shard3));

        assert!(store.shard_exists(shard1).await.unwrap());
        store.drop_shard(shard1).await.unwrap();
        assert!(!store.shard_exists(shard1).await.unwrap());

        let shards = store.list_shards().await.unwrap();
        assert_eq!(shards.len(), 2);
    }

    #[tokio::test]
    async fn test_scan() {
        let (store, shard) = setup_store().await;

        store.put(shard, b"a1", b"v1").await.unwrap();
        store.put(shard, b"a2", b"v2").await.unwrap();
        store.put(shard, b"b1", b"v3").await.unwrap();
        store.put(shard, b"c1", b"v4").await.unwrap();

        // Prefix scan
        let mut iter = store.scan_prefix(shard, b"a", None).await.unwrap();
        let mut items = Vec::new();
        while let Some(res) = iter.next().await {
            items.push(res.unwrap());
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, b"a1");
        assert_eq!(items[1].0, b"a2");

        // Range scan (from_to uses Excluded end bound, so c1 is not included)
        let range = KeyRange::from_to(b"a2".to_vec(), b"c1".to_vec());
        let mut iter = store.scan(shard, range).await.unwrap();
        let mut items = Vec::new();
        while let Some(res) = iter.next().await {
            items.push(res.unwrap());
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, b"a2");
        assert_eq!(items[1].0, b"b1");

        // Reverse scan
        let mut range = KeyRange::all();
        range.reverse = true;
        let mut iter = store.scan(shard, range).await.unwrap();
        let mut items = Vec::new();
        while let Some(res) = iter.next().await {
            items.push(res.unwrap());
        }
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].0, b"c1");
        assert_eq!(items[3].0, b"a1");
    }

    #[tokio::test]
    async fn test_transaction() {
        let (store, shard) = setup_store().await;
        store.put(shard, b"key1", b"initial").await.unwrap();

        let txn = store.begin_transaction().await.unwrap();
        txn.put(shard, b"key1", b"updated").await.unwrap();
        txn.put(shard, b"key2", b"new").await.unwrap();

        // Transaction sees its own writes
        assert_eq!(
            txn.get(shard, b"key1").await.unwrap(),
            Some(b"updated".to_vec())
        );
        assert_eq!(
            txn.get(shard, b"key2").await.unwrap(),
            Some(b"new".to_vec())
        );

        // Store doesn't see them yet
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"initial".to_vec())
        );
        assert_eq!(store.get(shard, b"key2").await.unwrap(), None);

        Arc::clone(&txn).commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Now store sees them
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"updated".to_vec())
        );
        assert_eq!(
            store.get(shard, b"key2").await.unwrap(),
            Some(b"new".to_vec())
        );
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let (store, shard) = setup_store().await;

        let txn = store.begin_transaction().await.unwrap();
        txn.put(shard, b"key1", b"value1").await.unwrap();
        Arc::clone(&txn).rollback().await.unwrap();

        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_common_test_suite() {
        let store = MemoryKeyValueShardStore::new();
        crate::test_suite::run_kvstore_test_suite(&store).await;
    }
}
