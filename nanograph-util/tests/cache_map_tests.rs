use nanograph_util::CacheMap;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_cache_map_ttl() {
    let ttl = Duration::from_millis(100);
    let mut cache = CacheMap::new(ttl);

    cache.insert("key1".to_string(), "value1".to_string());
    assert!(cache.get(&"key1".to_string()).is_some());

    sleep(Duration::from_millis(150));
    cache.prune();
    assert!(cache.get(&"key1".to_string()).is_none());
}

#[test]
fn test_cache_map_touch() {
    let ttl = Duration::from_millis(200);
    let mut cache = CacheMap::new(ttl);

    cache.insert("key1".to_string(), "value1".to_string());

    sleep(Duration::from_millis(150));
    // Touch the entry - now only needs &cache
    let cache_ref = &cache;
    assert!(cache_ref.get(&"key1".to_string()).is_some());

    sleep(Duration::from_millis(100));
    cache.prune(); // prune still needs mut
    // Should still be there because it was touched
    assert!(cache.get(&"key1".to_string()).is_some());

    sleep(Duration::from_millis(250));
    cache.prune();
    // Now it should be gone
    assert!(cache.get(&"key1".to_string()).is_none());
}

#[test]
fn test_cache_map_insert_overwrite() {
    let ttl = Duration::from_millis(100);
    let mut cache = CacheMap::new(ttl);

    cache.insert("key1".to_string(), "value1".to_string());
    let old = cache.insert("key1".to_string(), "value2".to_string());

    assert_eq!(old, Some("value1".to_string()));
    assert_eq!(cache.get(&"key1".to_string()), Some(&"value2".to_string()));
}

#[test]
fn test_cache_map_values() {
    let mut cache = CacheMap::new(Duration::from_secs(10));
    cache.insert(1, "one".to_string());
    cache.insert(2, "two".to_string());

    let values: Vec<_> = cache.values().collect();
    assert_eq!(values.len(), 2);
    assert!(values.contains(&&"one".to_string()));
    assert!(values.contains(&&"two".to_string()));
}

#[test]
fn test_cache_map_entry() {
    let mut cache = CacheMap::new(Duration::from_millis(200));

    // Test or_insert
    cache.entry(1).or_insert("one".to_string());
    assert_eq!(cache.get(&1), Some(&"one".to_string()));

    // Test or_insert on existing
    cache.entry(1).or_insert("updated".to_string());
    assert_eq!(cache.get(&1), Some(&"one".to_string()));

    // Test touch via entry
    sleep(Duration::from_millis(150));
    cache
        .entry(1)
        .and_modify(|v| *v = "one_modified".to_string());

    sleep(Duration::from_millis(100));
    cache.prune();
    // Entry 1 should still be there because and_modify's f calls get_mut which touches it
    assert_eq!(cache.get(&1), Some(&"one_modified".to_string()));

    sleep(Duration::from_millis(250));
    cache.prune();
    assert!(cache.get(&1).is_none());
}
