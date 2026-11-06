#![cfg(feature = "internal")]

use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::input::{KeyCode, KeyModifiers};
use game_client_rust::message_stream::{
    GameMessage, GameMessageType, InputEvent, InputProcessor, ReceivingUi,
};
use std::time::Instant;

fn no_window_query(_x: i32, _y: i32, _ui: &mut ReceivingUi<'_>) -> bool {
    false
}

/// Drive one raw input event through the processor with a fresh receiving UI.
fn process(processor: &mut InputProcessor, event: InputEvent) -> Vec<GameMessage> {
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, 0, 0);
    processor.process_input_event(event, &mut no_window_query, &mut ui)
}

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

    let messages = process(
        &mut processor,
        InputEvent::KeyDown {
            key: KeyCode::LeftCtrl,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(
        &messages,
        GameMessageType::MetaBeginForceAttack
    ));

    let messages = process(
        &mut processor,
        InputEvent::KeyUp {
            key: KeyCode::LeftCtrl,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(&messages, GameMessageType::MetaEndForceAttack));

    let messages = process(
        &mut processor,
        InputEvent::KeyDown {
            key: KeyCode::LeftAlt,
            modifiers: KeyModifiers::ALT,
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(&messages, GameMessageType::MetaBeginWaypoints));

    let messages = process(
        &mut processor,
        InputEvent::KeyUp {
            key: KeyCode::LeftAlt,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(&messages, GameMessageType::MetaEndWaypoints));

    let messages = process(
        &mut processor,
        InputEvent::KeyDown {
            key: KeyCode::LeftShift,
            modifiers: KeyModifiers::SHIFT,
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(
        &messages,
        GameMessageType::MetaBeginPreferSelection
    ));

    let messages = process(
        &mut processor,
        InputEvent::KeyUp {
            key: KeyCode::LeftShift,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        },
    );
    assert!(has_message(
        &messages,
        GameMessageType::MetaEndPreferSelection
    ));
}
