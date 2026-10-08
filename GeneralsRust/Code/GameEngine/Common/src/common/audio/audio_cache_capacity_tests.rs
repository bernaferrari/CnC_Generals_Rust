//! C++ cache capacity changes affect later admission without pruning at setter time.

use std::path::Path;
use std::sync::Arc;

use super::AudioFileCache;

fn insert(cache: &AudioFileCache, key: &str, bytes: Vec<u8>) -> Option<Arc<Vec<u8>>> {
    cache.get_or_insert_named(key, || Some(bytes))
}

#[test]
fn shrinking_capacity_reports_new_limit_and_defers_eviction_until_pressure() {
    let cache = AudioFileCache::new(8);
    let live = insert(&cache, "live.wav", vec![1, 2, 3, 4]).expect("live buffer");
    insert(&cache, "idle.wav", vec![5, 6]).expect("idle buffer");
    cache.close_named("idle.wav");

    // C++ setMaxSize only updates m_maxSize. Existing cache entries, including
    // zero-reference entries, remain until a later allocation needs capacity.
    cache.set_max_size(4);
    assert_eq!(cache.cache_info(), (6, 4, 2));
    assert_eq!(cache.memory_info().0, 6);
    assert_eq!(cache.memory_info().1, 4);
    assert_eq!(cache.memory_info().2, 150.0);
    assert_eq!(cache.get_statistics().max_size, 4);
    assert!(cache.is_cached(Path::new("live.wav")));
    assert!(cache.is_cached(Path::new("idle.wav")));
    assert!(Arc::ptr_eq(
        &live,
        &cache
            .get_or_insert_named("live.wav", || panic!("existing hit remains valid"))
            .expect("existing entry remains available")
    ));

    // A new request first evicts eligible idle bytes, then rejects because the
    // still-live four-byte buffer leaves no room under the new four-byte limit.
    assert!(insert(&cache, "too-large-together.wav", vec![7]).is_none());
    assert!(!cache.is_cached(Path::new("idle.wav")));
    assert!(cache.is_cached(Path::new("live.wav")));
    assert_eq!(cache.cache_info(), (4, 4, 1));
    assert_eq!(cache.get_statistics().eviction_count, 1);

    // The earlier explicit hit acquired one more reference, so close both.
    cache.close_named("live.wav");
    cache.close_named("live.wav");
    let replacement = insert(&cache, "replacement.wav", vec![8, 9, 10, 11])
        .expect("eligible old bytes are evicted when a later request needs space");
    assert_eq!(replacement.as_slice(), &[8, 9, 10, 11]);
    assert!(!cache.is_cached(Path::new("live.wav")));
    assert_eq!(cache.cache_info(), (4, 4, 1));
}

#[test]
fn growing_capacity_admits_bytes_that_exceeded_the_old_limit_and_is_instance_local() {
    let cache = AudioFileCache::new(4);
    let sibling = AudioFileCache::new(4);

    cache.set_max_size(8);
    let admitted = insert(&cache, "six-bytes.wav", vec![1, 2, 3, 4, 5, 6])
        .expect("grown capacity admits the formerly oversized buffer");
    assert_eq!(admitted.as_slice(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(cache.cache_info(), (6, 8, 1));
    assert_eq!(cache.memory_info().1, 8);
    assert_eq!(cache.get_statistics().max_size, 8);

    // Capacity belongs to this cache instance; changing it cannot update a
    // sibling that happens to contain the same key or use the same type.
    assert_eq!(sibling.cache_info(), (0, 4, 0));
    assert!(insert(&sibling, "six-bytes.wav", vec![6; 6]).is_none());
    assert_eq!(sibling.cache_info(), (0, 4, 0));
}

#[test]
fn zero_capacity_rejects_new_nonempty_entries_but_existing_hits_still_work() {
    let cache = AudioFileCache::new(4);
    let sibling = AudioFileCache::new(4);
    let original = insert(&cache, "kept.wav", vec![1, 2, 3, 4]).expect("initial buffer");

    cache.set_max_size(0);
    assert_eq!(cache.cache_info(), (4, 0, 1));
    assert_eq!(cache.memory_info().0, 4);
    assert_eq!(cache.memory_info().1, 0);
    assert_eq!(cache.get_statistics().max_size, 0);
    assert!(cache.is_cached(Path::new("kept.wav")));

    let hit = cache
        .get_or_insert_named("kept.wav", || {
            panic!("hit path must precede capacity admission")
        })
        .expect("existing entry remains available at zero capacity");
    assert!(Arc::ptr_eq(&original, &hit));
    assert!(insert(&cache, "new.wav", vec![9]).is_none());
    assert_eq!(cache.cache_info(), (4, 0, 1));
    // One initial load miss plus the rejected new key.
    assert_eq!(cache.get_statistics().miss_count, 2);
    cache.clear_cache();
    assert!(insert(&cache, "after-clear.wav", vec![9]).is_none());
    assert_eq!(cache.cache_info(), (0, 0, 0));

    // The sibling retains its original capacity and continues admitting data.
    assert_eq!(sibling.cache_info(), (0, 4, 0));
    assert!(insert(&sibling, "new.wav", vec![9]).is_some());
    assert_eq!(sibling.cache_info(), (1, 4, 1));
}
