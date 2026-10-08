//! Preserve cache concurrency, instance lifetime, and accounting through public calls.

use std::io::Read;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

use super::AudioFileCache;

const WAIT: Duration = Duration::from_secs(10);
const DIAGNOSTIC_WAIT: Duration = Duration::from_secs(2);
const CHILD_MODE: &str = "GENERALS_AUDIO_CACHE_OWNER_CHILD";

fn read_child_output(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}

#[test]
fn concurrent_same_key_open_is_single_load_and_preserves_refcounts() {
    if std::env::var_os(CHILD_MODE).is_none() {
        // The regression intentionally overlaps locks. Run it in a fresh,
        // bounded child so a broken implementation cannot hang the test suite.
        let module = module_path!()
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(module_path!());
        let test_name =
            format!("{module}::concurrent_same_key_open_is_single_load_and_preserves_refcounts");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(&test_name)
            .arg("--nocapture")
            .env(CHILD_MODE, "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let readers = [
            child
                .stdout
                .take()
                .map(|pipe| thread::spawn(move || read_child_output(pipe))),
            child
                .stderr
                .take()
                .map(|pipe| thread::spawn(move || read_child_output(pipe))),
        ];
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break child.wait().unwrap();
            }
            thread::sleep(Duration::from_millis(20));
        };
        let [stdout, stderr] = readers.map(|reader| reader.unwrap().join().unwrap().unwrap());
        let stdout = String::from_utf8_lossy(&stdout);
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            !timed_out,
            "audio cache concurrency child exceeded deadline: {stdout}{stderr}"
        );
        assert!(
            status.success(),
            "audio cache concurrency child failed: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("1 passed; 0 failed"),
            "exact child must run one test: {stdout}{stderr}"
        );
        return;
    }
    concurrent_same_key_open_preserves_transaction_and_diagnostics();
}

fn concurrent_same_key_open_preserves_transaction_and_diagnostics() {
    let cache = Arc::new(AudioFileCache::new(4));
    let sibling = Arc::new(AudioFileCache::new(4));
    let loader_calls = Arc::new(AtomicUsize::new(0));
    let (loader_entered_tx, loader_entered_rx) = mpsc::channel();
    let (release_loader_tx, release_loader_rx) = mpsc::channel();
    let (second_started_tx, second_started_rx) = mpsc::channel();
    let (second_finished_tx, second_finished_rx) = mpsc::channel();

    let first_cache = Arc::clone(&cache);
    let first_calls = Arc::clone(&loader_calls);
    let first_sibling = Arc::clone(&sibling);
    let first = thread::spawn(move || {
        let primary = first_cache.get_or_insert_named("same.wav", || {
            first_calls.fetch_add(1, Ordering::SeqCst);
            loader_entered_tx.send(()).unwrap();

            // A different cache with the same key must make progress while this
            // cache's transaction is in its loader phase.
            let sibling_value = first_sibling
                .get_or_insert_named("same.wav", || Some(vec![9, 9, 9, 9]))
                .expect("sibling cache load");
            assert_eq!(sibling_value.as_slice(), &[9, 9, 9, 9]);

            release_loader_rx
                .recv_timeout(WAIT)
                .expect("test releases the first loader");
            Some(vec![1, 2, 3, 4])
        });
        primary.expect("primary cache load")
    });

    loader_entered_rx
        .recv_timeout(WAIT)
        .expect("first loader entered");

    let second_cache = Arc::clone(&cache);
    let second_calls = Arc::clone(&loader_calls);
    let second = thread::spawn(move || {
        second_started_tx.send(()).unwrap();
        let value = second_cache
            .get_or_insert_named("same.wav", || {
                second_calls.fetch_add(1, Ordering::SeqCst);
                Some(vec![4, 3, 2, 1])
            })
            .expect("second open reuses first result");
        second_finished_tx.send(()).unwrap();
        value
    });
    second_started_rx
        .recv_timeout(WAIT)
        .expect("second caller started before first transaction is released");
    let second_finished_early = second_finished_rx.recv_timeout(DIAGNOSTIC_WAIT).is_ok();

    // These public diagnostics must remain usable while a cache transaction is
    // doing its loader work. Capture the result before unblocking the loader so
    // this also rejects a state guard accidentally held across the callback.
    let diagnostic_cache = Arc::clone(&cache);
    let (diagnostic_tx, diagnostic_rx) = mpsc::channel();
    let diagnostic = thread::spawn(move || {
        let stats = diagnostic_cache.get_statistics();
        let info = diagnostic_cache.cache_info();
        let memory = diagnostic_cache.memory_info();
        let files = diagnostic_cache.get_cached_files();
        let cached = diagnostic_cache.is_cached(std::path::Path::new("same.wav"));
        diagnostic_tx
            .send((stats, info, memory, files, cached))
            .unwrap();
    });
    let diagnostics_completed_during_loader = diagnostic_rx.recv_timeout(DIAGNOSTIC_WAIT);

    // Always unblock before asserting, so a failed preservation check cannot
    // strand worker threads or hang the test process.
    release_loader_tx.send(()).unwrap();
    let first_value = first.join().expect("first cache caller");
    let second_value = second.join().expect("second cache caller");
    diagnostic.join().expect("diagnostic reader");

    let (stats, cache_info, memory_info, files, cached) = diagnostics_completed_during_loader
        .expect("diagnostics must complete while the loader is active");
    assert!(
        !second_finished_early,
        "same-key transaction must not finish before first loader commits"
    );
    assert_eq!(loader_calls.load(Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(&first_value, &second_value));
    assert_eq!(first_value.as_slice(), &[1, 2, 3, 4]);
    assert!(
        files.is_empty(),
        "insert is not visible during the blocked loader"
    );
    assert!(!cached);
    assert_eq!(
        cache_info,
        (0, 4, 0),
        "during load, insertion is not visible yet"
    );
    assert_eq!(
        memory_info.0, 0,
        "during load, accounting is not yet committed"
    );
    assert_eq!(
        stats.total_requests, 1,
        "the active loader has recorded its request"
    );

    let cached_after_two_opens = cache.get_cached_files();
    assert_eq!(cached_after_two_opens.len(), 1);
    assert_eq!(
        cached_after_two_opens[0].2, 2,
        "two successful opens own two references"
    );
    cache.close_named("same.wav");
    cache.close_named("same.wav");
    assert_eq!(cache.get_cached_files()[0].2, 0);

    // Once the old entry is unreferenced, capacity pressure may evict it.
    let replacement = cache
        .get_or_insert_named("replacement.wav", || Some(vec![5, 6, 7, 8]))
        .expect("replacement fits after eviction");
    assert_eq!(replacement.as_slice(), &[5, 6, 7, 8]);
    assert!(!cache.is_cached(std::path::Path::new("same.wav")));
    assert!(cache.is_cached(std::path::Path::new("replacement.wav")));
    assert_eq!(cache.cache_info(), (4, 4, 1));
    assert_eq!(cache.get_statistics().eviction_count, 1);

    // The sibling cache did not alias the primary cache entry.
    let sibling_again = sibling
        .get_or_insert_named("same.wav", || panic!("sibling hit expected"))
        .expect("sibling cache hit");
    assert!(!Arc::ptr_eq(&first_value, &sibling_again));
}

#[test]
fn negative_cache_is_per_instance_and_clear_rearms_only_that_instance() {
    let first = AudioFileCache::new(8);
    let sibling = AudioFileCache::new(8);
    let first_calls = AtomicUsize::new(0);
    let sibling_calls = AtomicUsize::new(0);

    assert!(
        first
            .get_or_insert_named("missing.wav", || {
                first_calls.fetch_add(1, Ordering::SeqCst);
                None
            })
            .is_none()
    );
    assert!(
        first
            .get_or_insert_named("missing.wav", || {
                first_calls.fetch_add(1, Ordering::SeqCst);
                Some(vec![1])
            })
            .is_none()
    );
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);

    assert!(
        sibling
            .get_or_insert_named("missing.wav", || {
                sibling_calls.fetch_add(1, Ordering::SeqCst);
                Some(vec![2])
            })
            .is_some()
    );
    assert_eq!(sibling_calls.load(Ordering::SeqCst), 1);

    first.clear_cache();
    let recovered = first
        .get_or_insert_named("missing.wav", || {
            first_calls.fetch_add(1, Ordering::SeqCst);
            Some(vec![3])
        })
        .expect("clear removes the negative tombstone");
    assert_eq!(recovered.as_slice(), &[3]);
    assert_eq!(first_calls.load(Ordering::SeqCst), 2);
    assert_eq!(first.cache_info(), (1, 8, 1));
    assert_eq!(first.get_statistics().entry_count, 1);
    assert_eq!(first.get_statistics().current_size, 1);
    assert_eq!(sibling.cache_info(), (1, 8, 1));
}

#[test]
fn recent_closed_entry_survives_maintenance_then_explicit_remove_updates_accounting() {
    let cache = AudioFileCache::new(16);
    cache
        .get_or_insert_named("recent.wav", || Some(vec![1, 2, 3]))
        .expect("load");
    cache.close_named("recent.wav");

    // Maintenance removes only zero-reference entries older than five minutes.
    // This public API test checks the non-expired branch without private field
    // mutation or a wall-clock sleep.
    cache.maintenance();
    assert!(cache.is_cached(std::path::Path::new("recent.wav")));
    assert_eq!(cache.cache_info(), (3, 16, 1));

    assert!(cache.remove_file(std::path::Path::new("recent.wav")));
    assert_eq!(cache.cache_info(), (0, 16, 0));
    assert_eq!(cache.get_statistics().entry_count, 0);
    assert_eq!(cache.get_statistics().current_size, 0);
}

#[test]
fn loader_unwind_leaves_diagnostics_readable_and_poisoned_transaction_excluded() {
    let cache = AudioFileCache::new(4);
    let sibling = AudioFileCache::new(4);
    let load = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cache.get_or_insert_named("panic.wav", || panic!("fixture loader unwinds"))
    }));
    assert!(load.is_err());
    assert_eq!(cache.get_statistics().total_requests, 1);
    assert_eq!(cache.cache_info(), (0, 4, 0));
    assert!(cache.get_cached_files().is_empty());
    let next = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cache.get_or_insert_named("later.wav", || Some(vec![1]))
    }));
    assert!(
        next.is_err(),
        "existing poisoned transaction rejects mutation"
    );
    assert_eq!(cache.get_statistics().total_requests, 1);
    assert_eq!(
        sibling
            .get_or_insert_named("panic.wav", || Some(vec![2]))
            .unwrap()
            .as_slice(),
        &[2]
    );
}
