#![cfg(feature = "internal")]

use game_client_rust::input::{KeyCode, KeyModifiers};
use game_client_rust::message_stream::{GameMessageType, InputEvent, InputProcessor};
use std::time::Instant;

fn has_message(
    messages: &[game_client_rust::message_stream::GameMessage],
    expected: GameMessageType,
) -> bool {
    messages
        .iter()
        .any(|message| *message.get_type() == expected)
}

#[test]
fn retail_modifier_keys_emit_matching_modes() {
    let mut processor = InputProcessor::with_default_config();

    let messages = processor.process_input_event(InputEvent::KeyDown {
        key: KeyCode::LeftCtrl,
        modifiers: KeyModifiers::CTRL,
        timestamp: Instant::now(),
    });
    assert!(has_message(
        &messages,
        GameMessageType::MetaBeginForceAttack
    ));

    let messages = processor.process_input_event(InputEvent::KeyUp {
        key: KeyCode::LeftCtrl,
        modifiers: KeyModifiers::empty(),
        timestamp: Instant::now(),
    });
    assert!(has_message(&messages, GameMessageType::MetaEndForceAttack));

    let messages = processor.process_input_event(InputEvent::KeyDown {
        key: KeyCode::LeftAlt,
        modifiers: KeyModifiers::ALT,
        timestamp: Instant::now(),
    });
    assert!(has_message(&messages, GameMessageType::MetaBeginWaypoints));

    let messages = processor.process_input_event(InputEvent::KeyUp {
        key: KeyCode::LeftAlt,
        modifiers: KeyModifiers::empty(),
        timestamp: Instant::now(),
    });
    assert!(has_message(&messages, GameMessageType::MetaEndWaypoints));

    let messages = processor.process_input_event(InputEvent::KeyDown {
        key: KeyCode::LeftShift,
        modifiers: KeyModifiers::SHIFT,
        timestamp: Instant::now(),
    });
    assert!(has_message(
        &messages,
        GameMessageType::MetaBeginPreferSelection
    ));

    let messages = processor.process_input_event(InputEvent::KeyUp {
        key: KeyCode::LeftShift,
        modifiers: KeyModifiers::empty(),
        timestamp: Instant::now(),
    });
    assert!(has_message(
        &messages,
        GameMessageType::MetaEndPreferSelection
    ));
}
