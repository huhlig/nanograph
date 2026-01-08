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

use nanograph_lsm::{BlockCache, BlockKey, DataBlock, Entry};
use std::sync::Arc;

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
fn test_lru_eviction_tracking() {
    let cache = BlockCache::new(1024 * 1024);
    
    // Initially no evictions
    assert_eq!(cache.evictions(), 0);
    
    let key = BlockKey::new(1, 0);
    let block = create_test_block(10);
    
    cache.insert(key, block);
    
    // Still no evictions
    assert_eq!(cache.evictions(), 0);
}

#[test]
fn test_lru_eviction_on_capacity() {
    // Create a small cache
    let block = create_test_block(5);
    let block_size = {
        let temp_cache = BlockCache::new(10000);
        temp_cache.insert(BlockKey::new(0, 0), Arc::clone(&block));
        temp_cache.size()
    };
    
    // Cache that can hold exactly 3 blocks
    let cache = BlockCache::new(block_size * 3);
    
    // Insert 3 blocks
    for i in 0..3 {
        cache.insert(BlockKey::new(i, 0), Arc::clone(&block));
    }
    
    assert_eq!(cache.entry_count(), 3);
    assert_eq!(cache.evictions(), 0);
    
    // Insert 4th block - should trigger eviction
    cache.insert(BlockKey::new(3, 0), Arc::clone(&block));
    
    assert_eq!(cache.entry_count(), 3);
    assert_eq!(cache.evictions(), 1);
}

#[test]
fn test_lru_frequency_based_eviction() {
    let block = create_test_block(5);
    let block_size = {
        let temp_cache = BlockCache::new(10000);
        temp_cache.insert(BlockKey::new(0, 0), Arc::clone(&block));
        temp_cache.size()
    };
    
    // Cache that can hold exactly 3 blocks
    let cache = BlockCache::new(block_size * 3);
    
    let key1 = BlockKey::new(1, 0);
    let key2 = BlockKey::new(2, 0);
    let key3 = BlockKey::new(3, 0);
    
    // Insert three blocks
    cache.insert(key1, Arc::clone(&block));
    cache.insert(key2, Arc::clone(&block));
    cache.insert(key3, Arc::clone(&block));
    
    // Access key1 multiple times to increase its frequency
    for _ in 0..5 {
        cache.get(&key1);
    }
    
    // Access key2 once
    cache.get(&key2);
    
    // Don't access key3 at all
    
    // Insert a new block - should evict key3 (least frequently accessed)
    let key4 = BlockKey::new(4, 0);
    cache.insert(key4, Arc::clone(&block));
    
    // key1 and key2 should still be in cache (frequently accessed)
    assert!(cache.get(&key1).is_some());
    assert!(cache.get(&key2).is_some());
    
    // key3 should have been evicted
    assert!(cache.get(&key3).is_none());
    
    // key4 should be in cache
    assert!(cache.get(&key4).is_some());
}

#[test]
fn test_lru_recency_based_eviction() {
    let block = create_test_block(5);
    let block_size = {
        let temp_cache = BlockCache::new(10000);
        temp_cache.insert(BlockKey::new(0, 0), Arc::clone(&block));
        temp_cache.size()
    };
    
    // Cache that can hold exactly 3 blocks
    let cache = BlockCache::new(block_size * 3);
    
    let key1 = BlockKey::new(1, 0);
    let key2 = BlockKey::new(2, 0);
    let key3 = BlockKey::new(3, 0);
    
    // Insert three blocks
    cache.insert(key1, Arc::clone(&block));
    cache.insert(key2, Arc::clone(&block));
    cache.insert(key3, Arc::clone(&block));
    
    // Access all with same frequency but different recency
    cache.get(&key1);
    cache.get(&key2);
    cache.get(&key3);
    
    // Access key3 again to make it most recent
    cache.get(&key3);
    
    // Insert a new block - should evict key1 (least recent)
    let key4 = BlockKey::new(4, 0);
    cache.insert(key4, Arc::clone(&block));
    
    // key1 should have been evicted (least recent)
    assert!(cache.get(&key1).is_none());
    
    // Others should still be in cache
    assert!(cache.get(&key2).is_some());
    assert!(cache.get(&key3).is_some());
    assert!(cache.get(&key4).is_some());
}

#[test]
fn test_cache_stats_with_evictions() {
    let block = create_test_block(5);
    let block_size = {
        let temp_cache = BlockCache::new(10000);
        temp_cache.insert(BlockKey::new(0, 0), Arc::clone(&block));
        temp_cache.size()
    };
    
    let cache = BlockCache::new(block_size * 2);
    
    // Insert blocks to trigger evictions
    for i in 0..5 {
        cache.insert(BlockKey::new(i, 0), Arc::clone(&block));
    }
    
    let stats = cache.stats();
    
    // Should have evicted 3 blocks (5 inserted - 2 capacity)
    assert_eq!(stats.evictions, 3);
    assert_eq!(stats.entry_count, 2);
    assert!(stats.size <= stats.capacity);
}

#[test]
fn test_cache_reset_stats() {
    let cache = BlockCache::new(1024 * 1024);
    let key = BlockKey::new(1, 0);
    let block = create_test_block(10);
    
    // Generate some stats
    cache.insert(key, Arc::clone(&block));
    cache.get(&key); // hit
    cache.get(&BlockKey::new(2, 0)); // miss
    
    assert!(cache.hits() > 0);
    assert!(cache.misses() > 0);
    
    // Reset stats
    cache.reset_stats();
    
    assert_eq!(cache.hits(), 0);
    assert_eq!(cache.misses(), 0);
    assert_eq!(cache.evictions(), 0);
}

#[test]
fn test_cache_utilization() {
    let block = create_test_block(10);
    let cache = BlockCache::new(1024 * 1024);
    
    // Initially empty
    assert_eq!(cache.size(), 0);
    
    // Insert a block
    cache.insert(BlockKey::new(1, 0), Arc::clone(&block));
    
    // Should have some size now
    assert!(cache.size() > 0);
    assert!(cache.size() <= cache.capacity());
    
    let stats = cache.stats();
    assert_eq!(stats.entry_count, 1);
}

#[test]
fn test_multiple_evictions() {
    let block = create_test_block(5);
    let block_size = {
        let temp_cache = BlockCache::new(10000);
        temp_cache.insert(BlockKey::new(0, 0), Arc::clone(&block));
        temp_cache.size()
    };
    
    // Very small cache - can hold only 1 block
    let cache = BlockCache::new(block_size);
    
    // Insert multiple blocks - each should evict the previous
    for i in 0..10 {
        cache.insert(BlockKey::new(i, 0), Arc::clone(&block));
    }
    
    // Should have 9 evictions (10 inserts - 1 capacity)
    assert_eq!(cache.evictions(), 9);
    assert_eq!(cache.entry_count(), 1);
}

// Made with Bob
