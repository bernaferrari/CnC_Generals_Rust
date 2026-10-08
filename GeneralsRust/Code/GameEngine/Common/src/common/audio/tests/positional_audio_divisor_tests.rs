//! C++ minimum-volume comparisons through the actual manager update.
//! Seeded playing state verifies culling policy, not backend playback success.

use super::*;
use crate::common::audio::audio_event_rts::ST_WORLD;
use std::io::Read;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

const CHILD_MODE: &str = "GENERALS_POSITIONAL_AUDIO_DIVISOR_CHILD";
const CHILD_TIMEOUT: Duration = Duration::from_secs(10);

fn read_pipe(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn run_exact_child(test_name: &str) {
    let module = module_path!()
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(module_path!());
    let exact_name = format!("{module}::{test_name}");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(&exact_name)
        .arg("--nocapture")
        .env(CHILD_MODE, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out_reader = thread::spawn(move || read_pipe(stdout).unwrap());
    let err_reader = thread::spawn(move || read_pipe(stderr).unwrap());
    let deadline = Instant::now() + CHILD_TIMEOUT;
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status, false);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break (child.wait().unwrap(), true);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = String::from_utf8_lossy(&out_reader.join().unwrap()).into_owned();
    let stderr = String::from_utf8_lossy(&err_reader.join().unwrap()).into_owned();
    assert!(
        !timed_out,
        "isolated audio regression timed out: {stdout}{stderr}"
    );
    assert!(
        status.success(),
        "isolated audio regression failed: {stdout}{stderr}"
    );
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "child did not run exactly one test: {stdout}{stderr}"
    );
}

fn max_range_event(
    name: &str,
    handle: AudioHandle,
    type_field: u32,
    priority: AudioPriority,
) -> AudioEventRts {
    let mut info = test_info(name, AudioType::SoundEffect, 0, 0);
    info.type_field = type_field | ST_WORLD;
    info.priority = priority;
    info.min_distance = 10.0;
    info.max_distance = 100.0;
    let mut event = event_with(info, 1.0);
    // Playing events already have generatePlayInfo applied (CPP lines358-361).
    event.set_volume_shift(1.0);
    event.set_playing_handle(handle);
    event.set_position(&Coord3D {
        x: 10_000.0,
        y: 0.0,
        z: 0.0,
    });
    event
}

fn assert_isolated_no_hook_and_inert_constructor() -> AudioManager {
    // A process child prevents a hook installed by another unit test from
    // making the result depend on test order. `AudioManager::new` must not
    // publish/register a process-wide playback hook.
    assert!(!crate::common::audio::sound_playback_hook_registered());
    let mut manager = AudioManager::new();
    assert!(!crate::common::audio::sound_playback_hook_registered());
    manager.init();
    assert!(!crate::common::audio::sound_playback_hook_registered());
    manager.audio_settings.min_volume = 0.1;
    manager
}

#[test]
fn max_distance_zero_over_zero_retains_positional_event_cpp_parity() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("max_distance_zero_over_zero_retains_positional_event_cpp_parity");
        return;
    }
    let mut manager = assert_isolated_no_hook_and_inert_constructor();
    manager.set_volume(0.0, AudioAffect::Sound);
    manager.set_volume(0.75, AudioAffect::Sound3D);

    let event = max_range_event("AtMaxRange", 7101, 0, AudioPriority::Normal);
    assert!(event.is_positional_audio());
    assert_eq!(manager.get_effective_volume(&event), 0.0);
    manager.insert_playing_event_for_test(event);
    assert_eq!(manager.active_event_count(), 1);

    // C++ computes 0/0 => NaN; comparison with positive minVolume is false.
    // Current Rust EPSILON clamp returns 0 and releases this event here.
    manager.update();
    assert_eq!(
        manager.active_event_count(),
        1,
        "zero/zero divisor case must not be culled"
    );
    assert!(manager.active_event_mut_for_test(7101).is_some());
}

#[test]
fn positive_sound_denominator_culls_zero_effective_positional_event() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("positive_sound_denominator_culls_zero_effective_positional_event");
        return;
    }
    let mut manager = assert_isolated_no_hook_and_inert_constructor();
    manager.set_volume(0.5, AudioAffect::Sound);
    manager.set_volume(0.75, AudioAffect::Sound3D);

    let event = max_range_event(
        "AtMaxRangePositiveDenominator",
        7102,
        0,
        AudioPriority::Normal,
    );
    assert!(manager.sound_volume > 0.0);
    assert!(manager.sound_3d_volume > 0.0);
    assert_eq!(manager.get_effective_volume(&event), 0.0);
    manager.insert_playing_event_for_test(event);
    assert_eq!(manager.active_event_count(), 1);

    // 0 / positive = 0, which is below minVolume: ordinary event is released.
    manager.update();
    assert_eq!(manager.active_event_count(), 0);
    assert!(manager.active_event_mut_for_test(7102).is_none());
}

#[test]
fn global_and_critical_max_range_events_keep_cpp_exemptions() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("global_and_critical_max_range_events_keep_cpp_exemptions");
        return;
    }
    let mut manager = assert_isolated_no_hook_and_inert_constructor();
    manager.set_volume(0.5, AudioAffect::Sound);
    manager.set_volume(0.75, AudioAffect::Sound3D);

    assert!(manager.audio_settings.global_max_range < 10_000);
    let global = max_range_event("GlobalAtMaxRange", 7103, ST_GLOBAL, AudioPriority::Normal);
    let critical = max_range_event("CriticalAtMaxRange", 7104, 0, AudioPriority::Critical);
    assert_eq!(manager.get_effective_volume(&global), 0.0);
    assert_eq!(manager.get_effective_volume(&critical), 0.0);
    manager.insert_playing_event_for_test(global);
    manager.insert_playing_event_for_test(critical);
    assert_eq!(manager.active_event_count(), 2);

    manager.update();
    assert_eq!(manager.active_event_count(), 2);
    assert!(manager.active_event_mut_for_test(7103).is_some());
    assert!(manager.active_event_mut_for_test(7104).is_some());
}

#[test]
fn tiny_sound_denominator_keeps_original_threshold_comparison() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("tiny_sound_denominator_keeps_original_threshold_comparison");
        return;
    }
    let mut manager = assert_isolated_no_hook_and_inert_constructor();
    manager.audio_settings.min_volume = 0.005;
    manager.set_volume(1e-8, AudioAffect::Sound);
    manager.set_volume(0.75, AudioAffect::Sound3D);
    let mut event = max_range_event("NearTinyVolume", 7105, 0, AudioPriority::Normal);
    event.set_position(&Coord3D {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    });
    event.set_volume(1e-10);
    let effective = manager.get_effective_volume(&event);
    assert!(effective > 0.0);
    assert!(effective / manager.sound_volume > manager.audio_settings.min_volume);
    assert!(effective / f32::EPSILON < manager.audio_settings.min_volume);
    manager.insert_playing_event_for_test(event);
    manager.update();
    assert_eq!(
        manager.active_event_count(),
        1,
        "raw tiny divisor must preserve CPP threshold result"
    );
}

#[test]
fn zero_sound_3d_uses_unit_divisor_and_culls_ordinary_positional_event() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("zero_sound_3d_uses_unit_divisor_and_culls_ordinary_positional_event");
        return;
    }
    let mut manager = assert_isolated_no_hook_and_inert_constructor();
    manager.set_volume(0.0, AudioAffect::Sound);
    manager.set_volume(0.0, AudioAffect::Sound3D);
    let event = max_range_event("BothSlidersZero", 7106, 0, AudioPriority::Normal);
    assert!(event.is_positional_audio());
    assert_eq!(manager.get_effective_volume(&event), 0.0);
    manager.insert_playing_event_for_test(event);
    manager.update();
    assert_eq!(manager.active_event_count(), 0);
}
