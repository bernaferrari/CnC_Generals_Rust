#![cfg(feature = "internal")]
/*
**  Command & Conquer Generals Zero Hour(tm)
**  Copyright 2025 Electronic Arts Inc.
*/

//! Integration tests for Common component features

extern crate game_engine as ge;

use ge::common::*;

#[test]
fn test_global_data_comprehensive() {
    let mut global = global_data::GlobalData::default();

    // Test basic fields
    assert!(global.use_trees);
    assert_eq!(global.max_shell_screens, 8);
    assert_eq!(global.camera_height, 150.0);

    // Test time of day
    assert!(global.set_time_of_day(global_data::TimeOfDay::Night));
    assert_eq!(global.time_of_day, global_data::TimeOfDay::Night);

    // Test overrides
    global.set_override("test_key", global_data::GlobalValue::Int(42));
    assert_eq!(
        global.get_override("test_key"),
        Some(&global_data::GlobalValue::Int(42))
    );

    // Test init and reset
    global.init();
    assert!(global.get_override("test_key").is_none());
}

#[test]
fn test_preferences_skirmish() {
    let mut prefs = preferences::SkirmishPreferences::new();

    // Test defaults
    assert_eq!(prefs.num_ai_opponents, 1);
    assert_eq!(prefs.player_faction, "USA");

    // Test modification
    prefs.num_ai_opponents = 3;
    prefs.player_faction = String::from("China");

    // Test save/load
    let saved = prefs.save();
    let mut loaded = preferences::SkirmishPreferences::new();
    loaded.load(saved);

    assert_eq!(loaded.num_ai_opponents, 3);
    assert_eq!(loaded.player_faction, "China");
}

#[test]
fn test_preferences_all_types() {
    // Test all preference types can be created
    let _skirmish = preferences::SkirmishPreferences::new();
    let _ladder = preferences::LadderPreferences::new();
    let _quickmatch = preferences::QuickmatchPreferences::new();
    let _custom = preferences::CustomMatchPreferences::new();
    let mut ignore = preferences::IgnorePreferences::new();

    // Test ignore list functionality
    ignore.add_player("BadPlayer123".to_string());
    assert!(ignore.is_ignored("BadPlayer123"));
    assert!(!ignore.is_ignored("GoodPlayer456"));

    ignore.remove_player("BadPlayer123");
    assert!(!ignore.is_ignored("BadPlayer123"));

    ignore.clear_all();
    assert_eq!(ignore.ignored_players.len(), 0);
}

#[test]
fn test_threading_scoped_mutex() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::thread;
    use threading::ScopedMutex;

    let mutex = ScopedMutex::new(0);
    let counter = Arc::new(AtomicU32::new(0));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let mutex = mutex.clone();
            let counter = Arc::clone(&counter);
            thread::spawn(move || {
                let mut guard = mutex.lock();
                *guard += 1;
                counter.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let guard = mutex.lock();
    assert_eq!(*guard, 10);
    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

#[test]
fn test_threading_rwlock() {
    use threading::ScopedReadWriteLock;

    let rwlock = ScopedReadWriteLock::new(vec![1, 2, 3]);

    // Test read access
    {
        let read_guard = rwlock.read();
        assert_eq!(read_guard.len(), 3);
    }

    // Test write access
    {
        let mut write_guard = rwlock.write();
        write_guard.push(4);
    }

    // Test read after write
    {
        let read_guard = rwlock.read();
        assert_eq!(read_guard.len(), 4);
        assert_eq!(read_guard[3], 4);
    }
}

#[test]
fn test_threading_thread_pool() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use threading::ThreadPool;

    let pool = ThreadPool::new(4);
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..20 {
        let counter = Arc::clone(&counter);
        pool.execute(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    pool.shutdown();
    assert_eq!(counter.load(Ordering::SeqCst), 20);
}

#[test]
fn test_system_info_detection() {
    let info = system_info::SystemInfo::detect();

    // Basic sanity checks
    assert!(info.cpu.logical_cores > 0);
    assert!(info.cpu.physical_cores > 0);
    assert!(info.cpu.physical_cores <= info.cpu.logical_cores);

    // Memory should be detected (or at least have defaults)
    assert!(info.memory.total_bytes >= 0);

    // OS info should be populated
    assert!(!info.os.name.is_empty());
    assert!(!info.os.arch.is_empty());

    // Test report generation
    let report = info.generate_report();
    assert!(report.contains("System Information Report"));
    assert!(report.contains("CPU:"));
    assert!(report.contains("Memory:"));
}

#[test]
fn test_system_info_global() {
    let info1 = system_info::get_system_info();
    let info2 = system_info::get_system_info();

    // Should return consistent information
    assert_eq!(info1.cpu.logical_cores, info2.cpu.logical_cores);
    assert_eq!(info1.cpu.physical_cores, info2.cpu.physical_cores);
}

#[test]
fn test_os_display_modes() {
    let mode = os_display::DisplayMode::new(1920, 1080, 32, 60);
    assert_eq!(mode.width, 1920);
    assert_eq!(mode.height, 1080);
    assert!(mode.is_16_9());
    assert!(!mode.is_4_3());

    let mode_4_3 = os_display::DisplayMode::new(1024, 768, 32, 60);
    assert!(mode_4_3.is_4_3());
    assert!(!mode_4_3.is_16_9());
}

#[test]
fn test_os_display_manager() {
    let manager = os_display::DisplayManager::new();

    // Should have at least one monitor
    assert!(!manager.get_monitors().is_empty());
    assert!(manager.get_primary_monitor().is_some());

    // Test report generation
    let report = manager.generate_report();
    assert!(report.contains("Display Configuration Report"));
}

#[test]
fn test_os_display_configuration() {
    let mut manager = os_display::DisplayManager::new();

    // Test window mode changes
    assert!(manager
        .set_window_mode(os_display::WindowMode::Fullscreen)
        .is_ok());
    assert_eq!(
        manager.get_config().window_mode,
        os_display::WindowMode::Fullscreen
    );

    // Test resolution changes
    assert!(manager.set_resolution(1920, 1080).is_ok());
    assert_eq!(manager.get_config().target_mode.width, 1920);
    assert_eq!(manager.get_config().target_mode.height, 1080);

    // Test gamma/brightness
    manager.set_gamma(1.5);
    assert_eq!(manager.get_config().gamma, 1.5);

    manager.set_brightness(0.8);
    assert_eq!(manager.get_config().brightness, 0.8);

    // Test clamping
    manager.set_gamma(10.0);
    assert_eq!(manager.get_config().gamma, 2.0); // Should be clamped
}

#[test]
fn test_stats_collector_basic() {
    let mut stats = stats_collector::StatsCollector::new();

    // Test basic stats
    stats.increment_stat(stats_collector::StatType::UnitsBuilt, 5);
    assert_eq!(stats.get_stat(&stats_collector::StatType::UnitsBuilt), 5);

    stats.increment_stat(stats_collector::StatType::UnitsBuilt, 3);
    assert_eq!(stats.get_stat(&stats_collector::StatType::UnitsBuilt), 8);

    // Test set_stat
    stats.set_stat(stats_collector::StatType::ResourcesGathered, 1000);
    assert_eq!(
        stats.get_stat(&stats_collector::StatType::ResourcesGathered),
        1000
    );
}

#[test]
fn test_stats_collector_per_player() {
    let mut stats = stats_collector::StatsCollector::new();

    // Register players
    stats.register_player(1);
    stats.register_player(2);

    // Track per-player stats
    stats.increment_player_stat(1, stats_collector::StatType::UnitsBuilt, 10);
    stats.increment_player_stat(2, stats_collector::StatType::UnitsBuilt, 5);

    assert_eq!(
        stats.get_player_stat(1, &stats_collector::StatType::UnitsBuilt),
        10
    );
    assert_eq!(
        stats.get_player_stat(2, &stats_collector::StatType::UnitsBuilt),
        5
    );

    // Test report generation
    let report = stats.generate_report();
    assert!(report.contains("Player 1"));
    assert!(report.contains("Player 2"));
}

#[test]
fn test_stats_collector_performance() {
    let mut stats = stats_collector::StatsCollector::new();

    // Record some frame times
    stats.record_frame_time(16.67); // ~60 FPS
    stats.record_frame_time(16.67);
    stats.record_frame_time(16.67);

    let avg_fps = stats.get_average_fps();
    assert!(avg_fps > 55.0 && avg_fps < 65.0); // Should be around 60

    // Test summary generation
    let summary = stats.generate_summary();
    assert!(summary.contains("FPS"));
    assert!(summary.contains("Frame Time"));
}

#[test]
fn test_stats_collector_reset() {
    let mut stats = stats_collector::StatsCollector::new();

    stats.increment_stat(stats_collector::StatType::UnitsBuilt, 100);
    stats.register_player(1);
    stats.increment_player_stat(1, stats_collector::StatType::UnitsKilled, 50);

    stats.reset_stats();

    assert_eq!(stats.get_stat(&stats_collector::StatType::UnitsBuilt), 0);
    assert_eq!(stats.get_frame_count(), 0);
    assert!(stats.get_all_player_stats().is_empty());
}

#[test]
fn test_performance_tracking_integration() {
    use stats_collector::StatsCollector;
    use std::thread;
    use std::time::Duration;

    let mut collector = StatsCollector::new();

    // Simulate a few frames
    for _ in 0..10 {
        collector.update();
        thread::sleep(Duration::from_millis(10));
    }

    // Should have tracked frames
    assert!(collector.get_frame_count() > 0);
    assert!(collector.get_average_frame_time() > 0.0);

    // Generate comprehensive report
    let report = collector.generate_report();
    assert!(report.contains("Game Statistics Report"));
    assert!(report.contains("Total Frames"));
    assert!(report.contains("Average FPS"));
}

#[test]
fn test_all_components_integration() {
    // Test that all major components work together

    // 1. System Info
    let sys_info = system_info::get_system_info();
    assert!(sys_info.cpu.logical_cores > 0);

    // 2. Display Manager
    let display = os_display::DisplayManager::new();
    assert!(display.get_primary_monitor().is_some());

    // 3. Stats Collector
    let mut stats = stats_collector::StatsCollector::new();
    stats.increment_stat(stats_collector::StatType::UnitsBuilt, 1);
    assert_eq!(stats.get_stat(&stats_collector::StatType::UnitsBuilt), 1);

    // 4. Global Data
    let mut global = global_data::GlobalData::default();
    global.set_override("integration_test", global_data::GlobalValue::Bool(true));
    assert!(global
        .get_override("integration_test")
        .unwrap()
        .as_bool()
        .unwrap());

    // 5. Preferences
    let prefs = preferences::SkirmishPreferences::new();
    assert!(!prefs.map_name.is_empty());

    // All components initialized successfully
}
