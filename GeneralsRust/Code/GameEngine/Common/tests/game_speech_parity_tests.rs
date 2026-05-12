use game_engine::common::audio::game_speech::{SpeechInfo, SpeechManager};
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
