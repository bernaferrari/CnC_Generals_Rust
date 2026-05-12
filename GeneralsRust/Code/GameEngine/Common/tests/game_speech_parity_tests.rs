use game_engine::common::audio::game_speech::{Speaker, Speech, SpeechInfo, SpeechManager};
use game_engine::common::random_value::init_random_with_seed;
use std::sync::{Mutex, OnceLock};

fn random_state_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn speech_filename_selection_uses_game_client_random_seed() {
    let _guard = random_state_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut info = SpeechInfo::new();
    info.dialog_event = "TestSpeech".to_string();
    info.dialog_files = vec![
        "SpeechA".to_string(),
        "SpeechB".to_string(),
        "SpeechC".to_string(),
    ];

    init_random_with_seed(0x1234);
    let mut manager = SpeechManager::new();
    manager.add_new_speech(&info);
    let speech = manager
        .get_speech_by_name("TestSpeech")
        .expect("registered speech");
    let first = manager.get_filename_for_play(speech);

    init_random_with_seed(0x1234);
    let repeat = manager.get_filename_for_play(speech);

    assert_eq!(first, repeat);
    assert!(
        matches!(
            first.as_str(),
            "data\\audio\\sounds\\SpeechA.wav"
                | "data\\audio\\sounds\\SpeechB.wav"
                | "data\\audio\\sounds\\SpeechC.wav"
        ),
        "unexpected generated speech path: {first}"
    );
}

#[test]
fn localized_speech_filename_matches_cpp_path_shape() {
    let _guard = random_state_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut info = SpeechInfo::new();
    info.dialog_event = "LocalizedSpeech".to_string();
    info.dialog_files = vec!["$Eva_LowPower".to_string()];

    let mut manager = SpeechManager::new();
    manager.add_new_speech(&info);
    let speech = manager
        .get_speech_by_name("LocalizedSpeech")
        .expect("registered speech");

    assert_eq!(
        manager.get_filename_for_play(speech),
        "data\\audio\\sounds\\english\\Eva_LowPower.wav"
    );
}

#[test]
fn speech_from_info_resets_internal_play_count() {
    let mut info = SpeechInfo::new();
    info.dialog_event = "ResetPlayCount".to_string();
    info.internal_play_count = 12;

    let speech = Speech::from_speech_info(&info);

    assert_eq!(speech.info.internal_play_count, 0);
}

#[test]
fn next_playback_filename_uses_cpp_path_and_game_client_random_start() {
    let _guard = random_state_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut info = SpeechInfo::new();
    info.dialog_event = "RandomStartSpeech".to_string();
    info.dialog_files = vec![
        "SpeechA".to_string(),
        "SpeechB".to_string(),
        "SpeechC".to_string(),
    ];
    info.random_start_index = 1;
    info.sequential_start_index = -1;

    init_random_with_seed(0x5678);
    let mut speech = Speech::from_speech_info(&info);
    let first = SpeechManager::get_next_filename_for_play(&mut speech);

    init_random_with_seed(0x5678);
    let mut repeat_speech = Speech::from_speech_info(&info);
    let repeat = SpeechManager::get_next_filename_for_play(&mut repeat_speech);

    assert_eq!(first, repeat);
    assert_eq!(speech.info.internal_play_count, 1);
    assert!(
        matches!(
            first.as_str(),
            "Data\\Audio\\Sounds\\SpeechB.wav" | "Data\\Audio\\Sounds\\SpeechC.wav"
        ),
        "unexpected generated playback path: {first}"
    );
}

#[test]
fn next_playback_filename_uses_first_dialog_for_sequential_speech() {
    let mut info = SpeechInfo::new();
    info.dialog_event = "SequentialSpeech".to_string();
    info.dialog_files = vec!["SpeechA".to_string(), "SpeechB".to_string()];
    info.sequential_start_index = 0;

    let mut speech = Speech::from_speech_info(&info);

    assert_eq!(
        SpeechManager::get_next_filename_for_play(&mut speech),
        "Data\\Audio\\Sounds\\SpeechA.wav"
    );
    assert_eq!(speech.info.internal_play_count, 1);
}

#[test]
fn speaker_start_records_cpp_playback_filename() {
    let info = SpeechInfo {
        dialog_event: "SpeakerPlayback".to_string(),
        dialog_files: vec!["SpeechA".to_string()],
        sequential_start_index: 0,
        priority: 1,
        ..SpeechInfo::default()
    };
    let speech = Speech::from_speech_info(&info);
    let mut speaker = Speaker::new();
    speaker.say_speech(speech, 1, 0, 0);

    speaker.update();

    assert_eq!(
        speaker.last_opened_filename.as_deref(),
        Some("Data\\Audio\\Sounds\\SpeechA.wav")
    );
    assert_eq!(
        speaker
            .current_speech
            .as_ref()
            .and_then(|item| item.speech.as_ref())
            .map(|speech| speech.info.internal_play_count),
        Some(1)
    );
}
