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

use crate::sstable::DataBlock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Key for identifying a block in the cache
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockKey {
    pub file_number: u64,
    pub block_offset: u64,
}

impl BlockKey {
    pub fn new(file_number: u64, block_offset: u64) -> Self {
        Self {
            file_number,
            block_offset,
        }
    }
}

/// Entry in the LRU list with access tracking
#[derive(Debug, Clone)]
struct LruEntry {
    key: BlockKey,
    size: usize,
    access_count: u64,
    last_access: u64,
}

/// LRU Block Cache
///
/// Implements a Least Recently Used (LRU) cache for data blocks.
/// Thread-safe with interior mutability.
/// Uses access frequency and recency for eviction decisions.
pub struct BlockCache {
    inner: Arc<Mutex<BlockCacheInner>>,
    capacity: usize,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

struct BlockCacheInner {
    map: HashMap<BlockKey, Arc<DataBlock>>,
    lru_list: Vec<LruEntry>,
    current_size: usize,
    access_counter: u64,
}

impl BlockCache {
    /// Create a new block cache with the given capacity in bytes
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BlockCacheInner {
                map: HashMap::new(),
                lru_list: Vec::new(),
                current_size: 0,
                access_counter: 0,
            })),
            capacity,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Get a block from the cache
    pub fn get(&self, key: &BlockKey) -> Option<Arc<DataBlock>> {
        let mut inner = self.inner.lock().unwrap();

        if let Some(block) = inner.map.get(key) {
            let result = Arc::clone(block);

            // Update access tracking for LRU
            inner.access_counter += 1;
            let current_counter = inner.access_counter;
            if let Some(entry) = inner.lru_list.iter_mut().find(|e| e.key == *key) {
                entry.access_count += 1;
                entry.last_access = current_counter;
            }

            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(result)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Insert a block into the cache
    pub fn insert(&self, key: BlockKey, block: Arc<DataBlock>) {
        let block_size = self.estimate_block_size(&block);
        let mut inner = self.inner.lock().unwrap();

        // If block already exists, remove it first
        if inner.map.contains_key(&key) {
            self.remove_internal(&mut inner, &key);
        }

        // Evict blocks if necessary to make room
        while inner.current_size + block_size > self.capacity && !inner.lru_list.is_empty() {
            // Find victim using LRU with frequency consideration
            let victim_idx = self.select_eviction_victim(&inner.lru_list);
            if let Some(idx) = victim_idx {
                let victim_key = inner.lru_list[idx].key;
                self.remove_internal(&mut inner, &victim_key);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        // Insert new block
        if block_size <= self.capacity {
            inner.access_counter += 1;
            let current_counter = inner.access_counter;
            inner.map.insert(key, block);
            inner.lru_list.push(LruEntry {
                key,
                size: block_size,
                access_count: 1,
                last_access: current_counter,
            });
            inner.current_size += block_size;
        }
    }

    /// Select a victim for eviction using LRU with frequency consideration
    /// Uses a score based on recency and frequency
    fn select_eviction_victim(&self, lru_list: &[LruEntry]) -> Option<usize> {
        if lru_list.is_empty() {
            return None;
        }

        let current_time = lru_list.iter().map(|e| e.last_access).max().unwrap_or(0);

        // Find entry with lowest score (oldest and least frequently accessed)
        let mut min_score = f64::MAX;
        let mut victim_idx = 0;

        for (idx, entry) in lru_list.iter().enumerate() {
            // Score combines recency and frequency
            // Lower score = better candidate for eviction
            let recency = (current_time - entry.last_access) as f64;
            let frequency = entry.access_count as f64;

            // Weight recency more heavily than frequency
            let score = frequency / (1.0 + recency);

            if score < min_score {
                min_score = score;
                victim_idx = idx;
            }
        }

        Some(victim_idx)
    }

    /// Remove a block from the cache
    pub fn remove(&self, key: &BlockKey) {
        let mut inner = self.inner.lock().unwrap();
        self.remove_internal(&mut inner, key);
    }

    /// Internal remove without locking
    fn remove_internal(&self, inner: &mut BlockCacheInner, key: &BlockKey) {
        if inner.map.remove(key).is_some() {
            if let Some(pos) = inner.lru_list.iter().position(|e| e.key == *key) {
                let entry = inner.lru_list.remove(pos);
                inner.current_size = inner.current_size.saturating_sub(entry.size);
            }
        }
    }

    /// Clear all entries from the cache
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.map.clear();
        inner.lru_list.clear();
        inner.current_size = 0;
    }

    /// Get current cache size in bytes
    pub fn size(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.current_size
    }

    /// Get number of entries in cache
    pub fn entry_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.map.len()
    }

    /// Get cache capacity in bytes
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get cache hit count
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get cache miss count
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get cache eviction count
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits();
        let misses = self.misses();
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
    }

    /// Estimate the size of a block in bytes
    fn estimate_block_size(&self, block: &DataBlock) -> usize {
        // Estimate: sum of entry sizes + overhead
        let mut size = 0;
        for entry in &block.entries {
            size += entry.size();
        }
        size += block.restart_points.len() * 4; // 4 bytes per restart point
        size += 64; // Overhead for the DataBlock struct itself
        size
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            capacity: self.capacity,
            size: self.size(),
            entry_count: self.entry_count(),
            hits: self.hits(),
            misses: self.misses(),
            evictions: self.evictions(),
            hit_rate: self.hit_rate(),
        }
    }
}

impl Default for BlockCache {
    fn default() -> Self {
        Self::new(8 * 1024 * 1024) // 8MB default
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub capacity: usize,
    pub size: usize,
    pub entry_count: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub hit_rate: f64,
}

impl CacheStats {
    pub fn print_summary(&self) {
        println!("=== Block Cache Statistics ===");
        println!(
            "Capacity: {} bytes ({:.2} MB)",
            self.capacity,
            self.capacity as f64 / 1024.0 / 1024.0
        );
        println!(
            "Current Size: {} bytes ({:.2} MB)",
            self.size,
            self.size as f64 / 1024.0 / 1024.0
        );
        println!("Entries: {}", self.entry_count);
        println!("Hits: {}", self.hits);
        println!("Misses: {}", self.misses);
        println!("Evictions: {}", self.evictions);
        println!("Hit Rate: {:.2}%", self.hit_rate * 100.0);
        println!(
            "Utilization: {:.2}%",
            (self.size as f64 / self.capacity as f64) * 100.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memtable::Entry;

    fn create_test_block(size: usize) -> Arc<DataBlock> {
        let mut block = DataBlock::new();
        for i in 0..size {
            block.add_entry(Entry::new(
                format!("key{}", i).into_bytes(),
                Some(format!("value{}", i).into_bytes()),
                i as u64,
            ));
        }
        Arc::new(block)
    }

    #[test]
    fn test_cache_basic_operations() {
        let cache = BlockCache::new(1024 * 1024); // 1MB
        let key = BlockKey::new(1, 0);
        let block = create_test_block(10);

        // Insert and get
        cache.insert(key, Arc::clone(&block));
        assert!(cache.get(&key).is_some());
        assert_eq!(cache.entry_count(), 1);

        // Remove
        cache.remove(&key);
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn test_cache_hit_miss() {
        let cache = BlockCache::new(1024 * 1024);
        let key = BlockKey::new(1, 0);
        let block = create_test_block(10);

        // Miss
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.misses(), 1);

        // Insert and hit
        cache.insert(key, block);
        assert!(cache.get(&key).is_some());
        assert_eq!(cache.hits(), 1);

        // Hit rate
        assert_eq!(cache.hit_rate(), 0.5); // 1 hit, 1 miss
    }

    #[test]
    fn test_cache_eviction() {
        let cache = BlockCache::new(1000); // Small cache

        // Insert multiple blocks
        for i in 0..10 {
            let key = BlockKey::new(i, 0);
            let block = create_test_block(5);
            cache.insert(key, block);
        }

        // Cache should have evicted some blocks
        assert!(cache.entry_count() < 10);
        assert!(cache.size() <= cache.capacity());
    }

    #[test]
    fn test_cache_lru() {
        // Create cache that can hold exactly 3 blocks
        let block = create_test_block(5);
        let block_size = {
            let temp_cache = BlockCache::new(10000);
            temp_cache.estimate_block_size(&block)
        };
        let cache = BlockCache::new(block_size * 3);

        let key1 = BlockKey::new(1, 0);
        let key2 = BlockKey::new(2, 0);
        let key3 = BlockKey::new(3, 0);

        // Insert three blocks (cache is now full)
        cache.insert(key1, Arc::clone(&block));
        cache.insert(key2, Arc::clone(&block));
        cache.insert(key3, Arc::clone(&block));

        // Access key1 to make it recently used
        cache.get(&key1);

        // Insert another block, should evict key2 (least recently used)
        let key4 = BlockKey::new(4, 0);
        cache.insert(key4, Arc::clone(&block));

        // key1 should still be in cache (was accessed), key2 should be evicted
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = BlockCache::new(1024 * 1024);
        let key = BlockKey::new(1, 0);
        let block = create_test_block(10);

        cache.insert(key, block);
        cache.get(&key); // Hit
        cache.get(&BlockKey::new(2, 0)); // Miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entry_count, 1);
        assert!(stats.size > 0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = BlockCache::new(1024 * 1024);

        for i in 0..5 {
            let key = BlockKey::new(i, 0);
            let block = create_test_block(10);
            cache.insert(key, block);
        }

        assert_eq!(cache.entry_count(), 5);

        cache.clear();

        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.size(), 0);
    }
}
