#![cfg(not(target_arch = "wasm32"))]

use super::*;
use rodio_compat::Source;
use std::collections::HashMap;
use std::io::Read;
use std::num::{NonZeroU16, NonZeroU32};
use std::process::Stdio;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_MODE: &str = "GENERALS_RODIO_VOICE_OWNER_CHILD";
const CHILD_TIMEOUT: Duration = Duration::from_secs(20);

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
        "offline Rodio voice test timed out: {stdout}{stderr}"
    );
    assert!(
        status.success(),
        "offline Rodio voice test failed: {stdout}{stderr}"
    );
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "child did not run exactly one test: {stdout}{stderr}"
    );
}

fn empty_hook() -> RodioPlaybackHook {
    RodioPlaybackHook {
        sinks: Mutex::new(HashMap::new()),
        listener_position: Mutex::new(Coord3D::ZERO),
        listener_orientation: Mutex::new(Coord3D {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }),
    }
}

fn offline_mixer() -> (rodio::mixer::Mixer, rodio::mixer::MixerSource) {
    rodio::mixer::mixer(
        NonZeroU16::new(2).unwrap(),
        NonZeroU32::new(44_100).unwrap(),
    )
}

fn finite_pcm(sample: f32, frames: usize) -> rodio_compat::SamplesBuffer {
    rodio_compat::samples_buffer(1, 44_100, vec![sample; frames])
}

#[test]
fn offline_flat_voice_preserves_pause_volume_completion_and_registry_removal() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child(
            "offline_flat_voice_preserves_pause_volume_completion_and_registry_removal",
        );
        return;
    }

    // A real Player is driven by an in-memory Rodio mixer, not an output device.
    // The assertion checks generated PCM, not that sound was heard.
    let (mixer, mut output) = offline_mixer();
    let sink = Sink::connect_new(&mixer);
    sink.append(finite_pcm(0.4, 4_096));
    let hook = Arc::new(empty_hook());
    hook.sinks.lock().unwrap().insert(
        1_001,
        RodioSinkState::new(
            RodioVoice::Flat(sink),
            0.5,
            None,
            (10.0, 100.0),
            false,
            Some(4_096.0 / 44.1),
        ),
    );

    hook.set_sink_volume(1_001, 0.5);
    assert!(hook.is_playing(1_001));
    hook.pause(1_001);
    assert!(hook.is_sink_paused(1_001));
    let paused = output.by_ref().take(256).collect::<Vec<_>>();
    assert_eq!(paused.len(), 256);
    assert!(paused.iter().all(|sample| sample.abs() <= 1.0e-7));

    hook.resume(1_001);
    assert!(!hook.is_sink_paused(1_001));
    let audible = output.by_ref().take(2_048).collect::<Vec<_>>();
    let peak = audible
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, Real::max);
    assert!(
        peak > 0.15 && peak < 0.25,
        "volume-scaled PCM peak was {peak}"
    );

    // Pump the finite source to completion on the test thread.
    for _ in output.by_ref().take(20_000) {}
    assert!(!hook.is_playing(1_001));
}

#[test]
fn offline_spatial_voice_preserves_listener_pan_and_volume_controls() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("offline_spatial_voice_preserves_listener_pan_and_volume_controls");
        return;
    }

    let (mixer, mut output) = offline_mixer();
    let sink =
        SpatialSink::connect_new(&mixer, [0.0, 0.0, -1.0], [-0.1, 0.0, 0.0], [0.1, 0.0, 0.0]);
    sink.append(finite_pcm(0.4, 8_192));
    let hook = Arc::new(empty_hook());
    hook.sinks.lock().unwrap().insert(
        1_002,
        RodioSinkState::new(
            RodioVoice::Spatial(sink),
            0.5,
            Some(Coord3D {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            (10.0, 100.0),
            false,
            Some(8_192.0 / 44.1),
        ),
    );
    hook.set_listener_position(&Coord3D::ZERO);
    hook.set_listener_orientation(&Coord3D {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    });
    hook.set_sink_volume(1_002, 0.5);

    let samples = output.by_ref().take(16_384).collect::<Vec<_>>();
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, Real::max);
    assert!(
        peak > 0.05 && peak < 0.25,
        "spatial volume-scaled peak was {peak}"
    );
    let (left, right) = samples
        .chunks_exact(2)
        .fold((0.0, 0.0), |(left, right), pair| {
            (left + pair[0].abs(), right + pair[1].abs())
        });
    let total = left + right;
    assert!(
        total > 1.0,
        "offline spatial source produced too little PCM: {total}"
    );
    assert!(
        (left - right).abs() > total * 0.05,
        "listener pan did not measurably change channel balance: left={left}, right={right}"
    );
}

#[test]
fn offline_voice_registry_serializes_concurrent_hook_controls() {
    if std::env::var_os(CHILD_MODE).is_none() {
        run_exact_child("offline_voice_registry_serializes_concurrent_hook_controls");
        return;
    }

    let (mixer, mut output) = offline_mixer();
    let sink = Sink::connect_new(&mixer);
    sink.append(finite_pcm(0.4, 128).repeat_infinite());
    let hook = Arc::new(empty_hook());
    hook.sinks.lock().unwrap().insert(
        1_003,
        RodioSinkState::new(
            RodioVoice::Flat(sink),
            0.5,
            None,
            (10.0, 100.0),
            false,
            None,
        ),
    );

    let rendezvous = Arc::new(Barrier::new(3));
    let first_barrier = rendezvous.clone();
    let second_barrier = rendezvous.clone();
    let render_thread = thread::spawn(move || {
        for _ in 0..256 {
            rendezvous.wait();
            // Drive real Player callback state while other threads change controls.
            assert_eq!(output.by_ref().take(32).count(), 32);
        }
        output
    });
    let first = hook.clone();
    let first_thread = thread::spawn(move || {
        for index in 0..256 {
            first_barrier.wait();
            first.pause(1_003);
            first.resume(1_003);
            first.set_sink_volume(1_003, (index % 10) as Real / 10.0);
            assert!(first.is_playing(1_003));
        }
    });
    let second = hook.clone();
    let second_thread = thread::spawn(move || {
        for index in 0..256 {
            second_barrier.wait();
            second.set_listener_position(&Coord3D {
                x: index as Real,
                y: 0.0,
                z: 1.0,
            });
            second.set_listener_orientation(&Coord3D {
                x: 0.0,
                y: 1.0,
                z: (index % 5) as Real,
            });
            second.pause(1_003);
            second.resume(1_003);
            assert!(second.is_playing(1_003));
        }
    });
    first_thread.join().unwrap();
    second_thread.join().unwrap();
    let mut output = render_thread.join().unwrap();

    // Set deterministic final values after concurrent controls, then consume
    // real samples from the offline mixer while the source remains infinite.
    hook.set_sink_volume(1_003, 0.5);
    hook.pause(1_003);
    // Rodio applies controls every5ms; flush one full control interval.
    for _ in output.by_ref().take(512) {}
    let paused = output.by_ref().take(128).collect::<Vec<_>>();
    assert_eq!(paused.len(), 128);
    assert!(paused.iter().all(|sample| sample.abs() <= 1.0e-7));
    hook.resume(1_003);
    let audible = output.by_ref().take(2_048).collect::<Vec<_>>();
    let peak = audible
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, Real::max);
    assert!(
        peak > 0.15 && peak < 0.25,
        "concurrent voice PCM peak was {peak}"
    );

    hook.stop(1_003);
    assert!(!hook.is_playing(1_003));
}
