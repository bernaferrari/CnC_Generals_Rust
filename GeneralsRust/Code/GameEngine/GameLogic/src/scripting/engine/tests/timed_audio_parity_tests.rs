//! ScriptEngine.cpp:7268-7330: front insertion and unsigned frame arithmetic.
use super::*;

#[cfg(not(target_arch = "wasm32"))]
fn in_child(test: &str) -> bool {
    let module = module_path!();
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    matches!(
        crate::test_process::run_bounded(
            &format!("{}::{test}", module.strip_prefix(prefix).unwrap_or(module)),
            "GENERALS_CANONICAL_AUDIO_TIMER_CHILD",
        ),
        crate::test_process::TestProcess::Child
    )
}

#[test]
fn timer_frame_conversion_uses_cpp_milliseconds_per_frame_division() {
    // GameCommon.h computes this in Real (f32). Reassociating to /1000*30
    // loses a frame at this exact boundary on the old Rust implementation.
    let one_frame_ms = game_engine::common::game_common::MSEC_PER_LOGICFRAME_REAL;
    assert_eq!(one_frame_ms.to_bits(), 0x4205_5555);
    assert_eq!(
        ScriptEngine::timed_audio_frames_from_length_ms(one_frame_ms),
        1
    );
    assert_eq!(
        ScriptEngine::timed_audio_frames_from_length_ms(f32::from_bits(one_frame_ms.to_bits() - 1)),
        0
    );
    assert_eq!(ScriptEngine::timed_audio_frames_from_length_ms(100.0), 3);
    assert_eq!(ScriptEngine::timed_audio_frames_from_length_ms(1_000.0), 30);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn first_canonical_queries_insert_at_the_front_without_reordering_existing_rows() {
    if !in_child("first_canonical_queries_insert_at_the_front_without_reordering_existing_rows") {
        return;
    }
    let mut first = ScriptEngine::new().unwrap();
    first.new_map();
    let foreign = ScriptEngine::new().unwrap();
    let names = ["TimerParity_Missing_First", "TimerParity_Missing_Second"];
    for (slot, name) in names.iter().enumerate() {
        let frame = 10 + slot as u32;
        assert!(first.is_speech_complete_at_frame(name, false, frame));
        assert!(first.is_audio_complete_at_frame(name, false, frame));
    }
    let expected = vec![(names[1].into(), 11), (names[0].into(), 10)];
    assert_eq!(first.snapshot_xfer_tail().testing_speech, expected);
    assert_eq!(first.snapshot_xfer_tail().testing_audio, expected);
    // Actual version-6 engine wire must keep both list order and deadlines.
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let random_before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let mut saved = Cursor::new(Vec::new());
    first.xfer(&mut XferSave::new(&mut saved, 1)).unwrap();
    let saved = saved.into_inner();
    let mut restored = ScriptEngine::new().unwrap();
    restored
        .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
        .unwrap();
    assert_eq!(restored.snapshot_xfer_tail().testing_speech, expected);
    assert_eq!(restored.snapshot_xfer_tail().testing_audio, expected);
    let mut resaved = Cursor::new(Vec::new());
    restored.xfer(&mut XferSave::new(&mut resaved, 1)).unwrap();
    assert_eq!(resaved.into_inner(), saved);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        random_before
    );
    // Querying an existing row keeps its position and deadline.
    assert!(first.is_speech_complete_at_frame(names[0], false, 12));
    assert_eq!(first.snapshot_xfer_tail().testing_speech, expected);
    assert!(first.is_audio_complete_at_frame(names[0], true, 12));
    assert_eq!(
        first.snapshot_xfer_tail().testing_audio,
        [(names[1].into(), 11)]
    );
    assert!(foreign.snapshot_xfer_tail().testing_speech.is_empty());
    assert!(foreign.snapshot_xfer_tail().testing_audio.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
struct AudioFixtureDirectory {
    path: std::path::PathBuf,
    original: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioFixtureDirectory {
    fn new() -> Self {
        let original = std::env::current_dir().unwrap();
        let path =
            std::env::temp_dir().join(format!("generals-audio-timer-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        // This helper is used only in a bounded, exact-test child. No other
        // test or game's asset resolution can observe its temporary directory.
        std::env::set_current_dir(&path).unwrap();
        Self { path, original }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for AudioFixtureDirectory {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original).unwrap();
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn canonical_audio_deadlines_wrap_like_cpp_unsigned_frames() {
    if !in_child("canonical_audio_deadlines_wrap_like_cpp_unsigned_frames") {
        return;
    }
    let _directory = AudioFixtureDirectory::new();
    use game_engine::common::audio::{AudioEventInfo, AudioEventRts, AudioType};
    let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
    let name = "TimerParity_Rollover_100ms";
    let info = std::sync::Arc::new(AudioEventInfo {
        audio_name: name.into(),
        sound_type: AudioType::Music,
        sound_type_field: AudioType::Music,
        filename: "TimerParity_Rollover_100ms.wav".into(),
        ..Default::default()
    });
    manager
        .lock()
        .unwrap()
        .register_audio_event_info((*info).clone());
    let mut event = AudioEventRts::with_event_name(name);
    event.set_audio_event_info(info);
    {
        // Match the actual get_audio_length_ms call: it generates the filename
        // while the existing AudioManager guard is held.
        let _audio = manager.lock().unwrap();
        event.generate_filename();
    }
    let filename = std::path::Path::new(event.get_filename());
    assert!(
        !filename.is_absolute(),
        "fixture stays inside the temporary child directory"
    );
    if let Some(parent) = filename
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).unwrap();
    }
    // Actual PCM decoder: 800 mono 16-bit samples at 8000 Hz = 100ms.
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&1636_u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&8000_u32.to_le_bytes());
    wav.extend_from_slice(&16000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&1600_u32.to_le_bytes());
    wav.resize(1644, 0);
    std::fs::write(filename, wav).unwrap();
    let leftover = crate::common::audio::AudioEventRts::new(name);
    assert_eq!(
        TheAudio::get().unwrap().get_audio_length_ms(&leftover),
        100.0
    );
    let engine = ScriptEngine::new().unwrap();
    let now = u32::MAX - 1;
    // C++ adds 3 with unsigned wrap, then immediately compares now >= 1.
    assert!(engine.is_speech_complete_at_frame(name, false, now));
    assert!(engine.is_audio_complete_at_frame(name, false, now));
    assert_eq!(
        engine.snapshot_xfer_tail().testing_speech,
        [(name.into(), 1)]
    );
    assert_eq!(
        engine.snapshot_xfer_tail().testing_audio,
        [(name.into(), 1)]
    );
    assert!(!engine.is_speech_complete_at_frame(name, true, 0));
    assert!(!engine.is_audio_complete_at_frame(name, true, 0));
    assert!(engine.is_speech_complete_at_frame(name, true, 1));
    assert!(engine.is_audio_complete_at_frame(name, true, 1));
    assert!(engine.snapshot_xfer_tail().testing_speech.is_empty());
    assert!(engine.snapshot_xfer_tail().testing_audio.is_empty());
}

#[test]
fn canonical_timer_reset_and_new_map_clear_only_the_driving_engine() {
    let _guard = crate::test_sync::lock();
    let mut first = ScriptEngine::new().unwrap();
    let mut second = ScriptEngine::new().unwrap();
    first.with_inner_mut(|inner| {
        inner.testing_speech = vec![("SameName".into(), 20)];
        inner.testing_audio = vec![("SameName".into(), 30)];
    });
    second.with_inner_mut(|inner| {
        inner.testing_speech = vec![("SameName".into(), 90)];
        inner.testing_audio = vec![("SameName".into(), 100)];
    });
    first.reset();
    assert!(first.snapshot_xfer_tail().testing_speech.is_empty());
    assert!(first.snapshot_xfer_tail().testing_audio.is_empty());
    assert!(!second.is_speech_complete_at_frame("SameName", false, 20));
    assert!(!second.is_audio_complete_at_frame("SameName", false, 30));
    second.new_map();
    assert!(second.snapshot_xfer_tail().testing_speech.is_empty());
    assert!(second.snapshot_xfer_tail().testing_audio.is_empty());
}
