//! Production-linked preservation tests for Rust debug infrastructure.
//! These contracts must pass before and after changing private ownership; they
//! are not an original-C++ logging parity oracle. No global debug owner or panic
//! hook is installed, and every filesystem side effect stays in a TempDir.

use generals_main::debug_system::{DebugConfig, DebugSystem, PerformanceTimer, ScopedTimer};
use std::collections::BTreeSet;
use std::fs;
use std::sync::{Arc, Barrier};
use tempfile::TempDir;

fn config(directory: &TempDir) -> DebugConfig {
    DebugConfig {
        log_to_file: true,
        log_to_console: false,
        log_file_path: directory.path().join("debug.log"),
        crash_dump_path: directory.path().join("crashes"),
        enable_crash_handler: false,
        performance_logging: true,
        flush_frequency: 100,
        ..DebugConfig::default()
    }
}

fn messages(contents: &str) -> Vec<&str> {
    assert!(contents.is_empty() || contents.ends_with('\n'));
    contents
        .lines()
        .map(|line| {
            let (timestamp, message) = line.split_once("] [INFO] ").unwrap();
            let digits = timestamp.strip_prefix('[').unwrap();
            assert!(!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()));
            message
        })
        .collect()
}

#[test]
fn outer_owner_clones_share_state_and_return_independent_snapshots() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<DebugSystem>();
    send_sync::<ScopedTimer>();

    let directory = TempDir::new().unwrap();
    let settings = config(&directory);
    let path = settings.log_file_path.clone();
    let owner = Arc::new(DebugSystem::new(settings).unwrap());
    let other = Arc::clone(&owner);
    assert!(Arc::ptr_eq(&owner, &other));
    owner.log_to_file("info", "first owner").unwrap();
    other.log_to_file("info", "second owner").unwrap();
    owner.start_timer("shared");
    assert!(other.get_performance_stats()["shared"].samples.is_empty());
    other.stop_timer("shared");
    other.flush().unwrap();

    let contents = fs::read_to_string(path).unwrap();
    assert_eq!(messages(&contents), ["first owner", "second owner"]);
    let mut stats = owner.get_stats();
    assert_eq!(stats.log_entries_written, 2);
    assert_eq!(stats.total_log_size, contents.len() as u64);
    stats.log_entries_written = 900;
    assert_eq!(other.get_stats().log_entries_written, 2);
    let mut timers = other.get_performance_stats();
    assert_eq!(timers["shared"].samples.len(), 1);
    timers.get_mut("shared").unwrap().samples.clear();
    assert_eq!(owner.get_performance_stats()["shared"].samples.len(), 1);
}

#[test]
fn scoped_timer_keeps_owner_alive_and_records_on_drop_across_threads() {
    let directory = TempDir::new().unwrap();
    let owner = Arc::new(DebugSystem::new(config(&directory)).unwrap());
    let weak = Arc::downgrade(&owner);
    let timer = ScopedTimer::new(Arc::clone(&owner), "retained".into());
    drop(owner);
    assert_eq!(weak.strong_count(), 1);
    let observer = weak.upgrade().expect("the scoped timer retains the owner");
    assert!(
        observer.get_performance_stats()["retained"]
            .samples
            .is_empty()
    );
    std::thread::spawn(move || drop(timer)).join().unwrap();
    assert_eq!(
        observer.get_performance_stats()["retained"].samples.len(),
        1
    );
    drop(observer);
    assert!(weak.upgrade().is_none());

    let owner = Arc::new(DebugSystem::new(config(&directory)).unwrap());
    let weak = Arc::downgrade(&owner);
    let timer = ScopedTimer::new(owner, "final owner".into());
    assert_eq!(weak.strong_count(), 1);
    drop(timer);
    assert!(weak.upgrade().is_none());
}

#[test]
fn concurrent_logging_and_flush_preserve_every_complete_line_and_byte_count() {
    const WRITERS: usize = 4;
    const ENTRIES: usize = 64;
    let directory = TempDir::new().unwrap();
    let mut settings = config(&directory);
    settings.flush_frequency = 7;
    let path = settings.log_file_path.clone();
    let owner = Arc::new(DebugSystem::new(settings).unwrap());
    let ready = Arc::new(Barrier::new(WRITERS + 1));
    let payload = |writer: usize, entry: usize| {
        // UTF-8 and a write larger than BufWriter's buffer exercise byte counts
        // and complete writes without depending on thread scheduling or time.
        format!("writer={writer};entry={entry};{}", "é".repeat(5000))
    };
    let expected: BTreeSet<_> = (0..WRITERS)
        .flat_map(|writer| (0..ENTRIES).map(move |entry| payload(writer, entry)))
        .collect();
    let mut workers = Vec::new();
    for writer in 0..WRITERS {
        let owner = Arc::clone(&owner);
        let ready = Arc::clone(&ready);
        workers.push(std::thread::spawn(move || {
            ready.wait();
            for entry in 0..ENTRIES {
                owner.log_to_file("info", &payload(writer, entry)).unwrap();
            }
        }));
    }
    let flushing_owner = Arc::clone(&owner);
    workers.push(std::thread::spawn(move || {
        ready.wait();
        for _ in 0..ENTRIES {
            flushing_owner.flush().unwrap();
            std::thread::yield_now();
        }
    }));
    for worker in workers {
        worker.join().unwrap();
    }
    owner.flush().unwrap();
    let contents = fs::read_to_string(path).unwrap();
    let lines = messages(&contents);
    assert_eq!(lines.len(), WRITERS * ENTRIES);
    let actual: BTreeSet<_> = lines.iter().map(|line| (*line).to_owned()).collect();
    assert_eq!(actual.len(), lines.len(), "no duplicate entries");
    assert_eq!(actual, expected);
    let stats = owner.get_stats();
    assert_eq!(stats.log_entries_written, (WRITERS * ENTRIES) as u64);
    assert_eq!(stats.total_log_size, contents.len() as u64);
}

#[test]
fn disabled_file_and_performance_logging_remain_no_ops() {
    let directory = TempDir::new().unwrap();
    let mut settings = config(&directory);
    settings.log_to_file = false;
    settings.performance_logging = false;
    settings.log_file_path = directory.path().join("absent-parent/debug.log");
    let path = settings.log_file_path.clone();
    let crash_path = settings.crash_dump_path.clone();
    let owner = Arc::new(DebugSystem::new(settings).unwrap());
    let initial = owner.get_stats();
    owner.log_to_file("info", "discarded").unwrap();
    owner.flush().unwrap();
    owner.start_timer("disabled");
    owner.stop_timer("disabled");
    drop(ScopedTimer::new(
        Arc::clone(&owner),
        "disabled scope".into(),
    ));
    assert!(!path.exists());
    assert!(!path.parent().unwrap().exists());
    assert!(
        crash_path.is_dir(),
        "constructor still creates crash directory"
    );
    assert!(owner.get_performance_stats().is_empty());
    let stats = owner.get_stats();
    assert_eq!(stats.log_entries_written, 0);
    assert_eq!(stats.total_log_size, 0);
    assert_eq!(stats.crashes_handled, 0);
    assert_eq!(stats.performance_samples, 0);
    assert_eq!(stats.last_flush_time, initial.last_flush_time);
}

#[test]
fn configured_periodic_and_explicit_flush_make_buffered_entries_visible() {
    let directory = TempDir::new().unwrap();
    let mut settings = config(&directory);
    settings.flush_frequency = 2;
    let path = settings.log_file_path.clone();
    let owner = DebugSystem::new(settings).unwrap();
    let initial = owner.get_stats();
    owner.log_to_file("info", "one").unwrap();
    assert!(fs::read(&path).unwrap().is_empty());
    assert_eq!(owner.get_stats().last_flush_time, initial.last_flush_time);
    owner.log_to_file("info", "two").unwrap();
    let periodic = fs::read_to_string(&path).unwrap();
    assert_eq!(messages(&periodic), ["one", "two"]);
    owner.log_to_file("info", "three").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), periodic);
    owner.flush().unwrap();
    let explicit = fs::read_to_string(&path).unwrap();
    assert_eq!(messages(&explicit), ["one", "two", "three"]);
    assert_eq!(owner.get_stats().total_log_size, explicit.len() as u64);
}

#[test]
fn buffered_writer_is_flushed_when_the_last_outer_owner_is_dropped() {
    let directory = TempDir::new().unwrap();
    let settings = config(&directory);
    let path = settings.log_file_path.clone();
    let owner = Arc::new(DebugSystem::new(settings).unwrap());
    let other = Arc::clone(&owner);
    owner.log_to_file("info", "last owner").unwrap();
    drop(owner);
    assert!(fs::read(&path).unwrap().is_empty());
    drop(other);
    assert_eq!(messages(&fs::read_to_string(path).unwrap()), ["last owner"]);
}

#[test]
fn startup_rotation_preserves_backup_order_and_starts_fresh_stats() {
    let directory = TempDir::new().unwrap();
    let mut settings = config(&directory);
    settings.max_log_file_size = 4;
    settings.max_log_files = 3;
    let path = settings.log_file_path.clone();
    fs::write(&path, "current").unwrap();
    for (index, content) in [(1, "previous one"), (2, "previous two"), (3, "oldest")] {
        fs::write(path.with_extension(format!("log.{index}")), content).unwrap();
    }
    let owner = DebugSystem::new(settings).unwrap();
    assert!(fs::read(&path).unwrap().is_empty());
    for (index, content) in [(1, "current"), (2, "previous one"), (3, "previous two")] {
        assert_eq!(
            fs::read_to_string(path.with_extension(format!("log.{index}"))).unwrap(),
            content
        );
    }
    assert!(!path.with_extension("log.4").exists());
    assert_eq!(owner.get_stats().log_entries_written, 0);
    assert_eq!(owner.get_stats().total_log_size, 0);
    owner.log_to_file("info", "fresh").unwrap();
    owner.flush().unwrap();
    assert_eq!(messages(&fs::read_to_string(path).unwrap()), ["fresh"]);
}

#[test]
fn startup_appends_at_or_below_rotation_threshold_and_does_not_rotate_per_write() {
    for size in [3, 4] {
        let directory = TempDir::new().unwrap();
        let mut settings = config(&directory);
        settings.max_log_file_size = 4;
        let path = settings.log_file_path.clone();
        let original = "x".repeat(size);
        fs::write(&path, &original).unwrap();
        let owner = DebugSystem::new(settings).unwrap();
        owner.log_to_file("info", "appended").unwrap();
        owner.flush().unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            messages(contents.strip_prefix(&original).unwrap()),
            ["appended"]
        );
        assert!(!path.with_extension("log.1").exists());
        assert_eq!(owner.get_stats().log_entries_written, 1);
        assert_eq!(
            owner.get_stats().total_log_size,
            (contents.len() - size) as u64
        );
    }
}

#[test]
fn performance_timers_keep_bounded_samples_and_ignore_unknown_stops() {
    let directory = TempDir::new().unwrap();
    let owner = DebugSystem::new(config(&directory)).unwrap();
    owner.stop_timer("unknown");
    assert!(owner.get_performance_stats().is_empty());
    for _ in 0..1005 {
        owner.start_timer("bounded");
        owner.stop_timer("bounded");
    }
    let timers = owner.get_performance_stats();
    let timer = &timers["bounded"];
    assert_eq!(timer.name, "bounded");
    assert_eq!(timer.samples.len(), 1000);
    assert!(
        timer
            .samples
            .iter()
            .all(|sample| sample.is_finite() && *sample >= 0.0)
    );
    assert_eq!(timer.avg_time, timer.samples.iter().sum::<f64>() / 1000.0);
    assert!(
        timer
            .samples
            .iter()
            .all(|sample| *sample >= timer.min_time && *sample <= timer.max_time)
    );
    let mut direct = PerformanceTimer::new("direct".into());
    direct.start();
    direct.stop();
    assert_eq!(direct.samples.len(), 1);
    assert_eq!(direct.avg_time, direct.samples[0]);
    assert_eq!(direct.min_time, direct.samples[0]);
    assert_eq!(direct.max_time, direct.samples[0]);
}

#[test]
fn critical_error_writes_complete_report_and_shares_accounting() {
    let directory = TempDir::new().unwrap();
    let settings = config(&directory);
    let crash_path = settings.crash_dump_path.clone();
    let owner = Arc::new(DebugSystem::new(settings).unwrap());
    let other = Arc::clone(&owner);
    other
        .handle_critical_error("test failure", Some("test details"))
        .unwrap();
    let files = fs::read_dir(crash_path)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    let report = fs::read_to_string(files[0].path()).unwrap();
    assert!(report.starts_with("Command & Conquer Generals Zero Hour - Critical Error\n"));
    assert!(report.contains("\nError: test failure\nDetails: test details\n"));
    assert!(report.contains(&format!("\nPlatform: {}\n", std::env::consts::OS)));
    assert!(report.ends_with(&format!("Architecture: {}\n", std::env::consts::ARCH)));
    assert_eq!(owner.get_stats().crashes_handled, 1);
    assert_eq!(owner.get_stats().log_entries_written, 0);
}
