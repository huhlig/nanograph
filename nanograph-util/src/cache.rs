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

use atomic_time::AtomicInstant;
use std::collections::hash_map::{Entry, HashMap};
use std::fmt;
use std::hash::Hash;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// A Map that tracks when an entry was last touched and ejects entries that haven't been touched recently.
pub struct CacheMap<K: Eq + Hash, V> {
    map: HashMap<K, (AtomicInstant, V)>,
    ttl: Duration,
}

impl<K: Eq + Hash + fmt::Debug, V: fmt::Debug> fmt::Debug for CacheMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("CacheMap");
        debug_struct.field("ttl", &self.ttl);
        let mut debug_map = HashMap::new();
        for (k, (timestamp, v)) in self.map.iter() {
            debug_map.insert(k, (timestamp.load(Ordering::Relaxed), v));
        }
        debug_struct.field("map", &debug_map);
        debug_struct.finish()
    }
}

/// An entry in the CacheMap.
pub enum CacheEntry<'a, K: 'a, V: 'a> {
    /// An occupied entry.
    Occupied(OccupiedCacheEntry<'a, K, V>),
    /// A vacant entry.
    Vacant(VacantCacheEntry<'a, K, V>),
}

impl<'a, K: Eq + Hash, V> CacheEntry<'a, K, V> {
    /// Ensures a value is in the entry by inserting the default if empty, and returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            CacheEntry::Occupied(entry) => entry.into_mut(),
            CacheEntry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty, and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            CacheEntry::Occupied(entry) => entry.into_mut(),
            CacheEntry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Returns a reference to this entry's key.
    pub fn key(&self) -> &K {
        match self {
            CacheEntry::Occupied(entry) => entry.key(),
            CacheEntry::Vacant(entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any potential inserts into the map.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            CacheEntry::Occupied(mut entry) => {
                f(entry.get_mut());
                CacheEntry::Occupied(entry)
            }
            CacheEntry::Vacant(entry) => CacheEntry::Vacant(entry),
        }
    }

    /// Gets a reference to the value in the entry without updating the timestamp.
    pub fn peek(&self) -> Option<&V> {
        match self {
            CacheEntry::Occupied(entry) => Some(entry.peek()),
            CacheEntry::Vacant(_) => None,
        }
    }
}

/// An occupied entry in the CacheMap.
pub struct OccupiedCacheEntry<'a, K: 'a, V: 'a> {
    entry: std::collections::hash_map::OccupiedEntry<'a, K, (AtomicInstant, V)>,
}

impl<'a, K: Eq + Hash, V> OccupiedCacheEntry<'a, K, V> {
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &K {
        self.entry.key()
    }

    /// Take the ownership of the key and value from the map.
    pub fn remove_entry(self) -> (K, V) {
        let (k, (_, v)) = self.entry.remove_entry();
        (k, v)
    }

    /// Gets a reference to the value in the entry without updating the timestamp.
    pub fn peek(&self) -> &V {
        &self.entry.get().1
    }

    /// Gets a reference to the value in the entry and updates the timestamp.
    pub fn get(&self) -> &V {
        let (timestamp, value) = self.entry.get();
        timestamp.store(Instant::now(), Ordering::Relaxed);
        value
    }

    /// Gets a mutable reference to the value in the entry and updates the timestamp.
    pub fn get_mut(&mut self) -> &mut V {
        let (timestamp, value) = self.entry.get_mut();
        timestamp.store(Instant::now(), Ordering::Relaxed);
        value
    }

    /// Converts the OccupiedEntry into a mutable reference to the value in the entry with a lifetime bound to the map itself.
    pub fn into_mut(self) -> &'a mut V {
        let (timestamp, value) = self.entry.into_mut();
        timestamp.store(Instant::now(), Ordering::Relaxed);
        value
    }

    /// Sets the value of the entry, and returns the entry's old value.
    pub fn insert(&mut self, value: V) -> V {
        self.entry.insert((AtomicInstant::now(), value)).1
    }

    /// Takes the value out of the entry, and returns it.
    pub fn remove(self) -> V {
        self.entry.remove().1
    }
}

/// A vacant entry in the CacheMap.
pub struct VacantCacheEntry<'a, K: 'a, V: 'a> {
    entry: std::collections::hash_map::VacantEntry<'a, K, (AtomicInstant, V)>,
}

impl<'a, K: Eq + Hash, V> VacantCacheEntry<'a, K, V> {
    /// Gets a reference to the key that would be used when inserting a value through the VacantEntry.
    pub fn key(&self) -> &K {
        self.entry.key()
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> K {
        self.entry.into_key()
    }

    /// Sets the value of the entry with the VacantEntry's key, and returns a mutable reference to it.
    pub fn insert(self, value: V) -> &'a mut V {
        &mut self.entry.insert((AtomicInstant::now(), value)).1
    }
}

impl<K: Eq + Hash, V> CacheMap<K, V> {
    /// Create a new CacheMap with a configurable TTL.
    pub fn new(ttl: Duration) -> Self {
        CacheMap {
            map: HashMap::new(),
            ttl,
        }
    }

    /// Get a reference to a value and update its last touched timestamp.
    pub fn get(&self, key: &K) -> Option<&V> {
        if let Some((timestamp, value)) = self.map.get(key) {
            timestamp.store(Instant::now(), Ordering::Relaxed);
            Some(value)
        } else {
            None
        }
    }

    /// Get a reference to a value without updating its last touched timestamp.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|(_, v)| v)
    }

    /// Get a mutable reference to a value and update its last touched timestamp.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Some((timestamp, value)) = self.map.get_mut(key) {
            timestamp.store(Instant::now(), Ordering::Relaxed);
            Some(value)
        } else {
            None
        }
    }

    /// Insert a value into the cache.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.map
            .insert(key, (AtomicInstant::now(), value))
            .map(|(_, v)| v)
    }

    /// Remove a value from the cache.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.map.remove(key).map(|(_, v)| v)
    }

    /// Check if the cache contains a key without updating its timestamp.
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Prune entries that haven't been touched within the TTL.
    pub fn prune(&mut self) {
        let now = Instant::now();
        let ttl = self.ttl;
        self.map.retain(|_, (timestamp, _)| {
            now.duration_since(timestamp.load(Ordering::Relaxed)) < ttl
        });
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over values in the cache.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.map.values().map(|(_, v)| v)
    }

    /// Returns an iterator over the keys and values of the cache.
    /// This does NOT update the last touched timestamp of the entries.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map.iter().map(|(k, (_, v))| (k, v))
    }

    /// Returns the entry for the given key for in-place manipulation.
    pub fn entry(&mut self, key: K) -> CacheEntry<'_, K, V> {
        match self.map.entry(key) {
            Entry::Occupied(entry) => CacheEntry::Occupied(OccupiedCacheEntry { entry }),
            Entry::Vacant(entry) => CacheEntry::Vacant(VacantCacheEntry { entry }),
        }
    }
    /// Clear all entries from the cache.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

impl<K: Eq + Hash, V> Default for CacheMap<K, V> {
    fn default() -> Self {
        CacheMap::new(Duration::from_secs(60 * 60))
    }
}
