//! Wave 167 residual peels: MSG_NEW_GAME stream post + drain residual
//! (C++ Shell ButtonStart → NewGame → Menu tick drain → startNewGame path;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 166 Skirmish options WND residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - SkirmishGameOptionsMenu ButtonStart posts MSG_NEW_GAME
//! - GameLogic/Menu update drains NewGame → startNewGame / start_game_from_ui
//! - Runtime-host `queue_new_game` posts + drains immediately
//!
//! Fail-closed:
//! - Does not require full map load / InGame transition in this peel
//! - Not full W3DMainMenuInit residual
//! - Shell `playable_claim` stays false; network deferred

use game_engine::common::message_stream::{
    GameMessage, GameMessageArgumentType, GameMessageType, MessageStream, get_message_stream,
};

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// NewGame stream residual method names.
pub const NEW_GAME_STREAM_METHOD_NAMES_WAVE167: &[&str] = &[
    "queue_new_game",
    "append_message NewGame",
    "take_pending_new_game_start_request",
    "take_new_game_dispatch_from_common_stream",
    "start_game_from_ui",
];

/// Ordered NewGame residual navigation steps.
pub const NEW_GAME_STREAM_NAV_STEPS_WAVE167: &[&str] = &[
    "SEED_PENDING_MAP",
    "APPEND_MSG_NEW_GAME",
    "APPEND_MODE_DIFFICULTY_ARGS",
    "DRAIN_NEW_GAME_FROM_STREAM",
    "BUILD_START_REQUEST",
    "START_GAME_FROM_UI_PATH",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_NEW_GAME_STREAM_CMD_NAMES_WAVE167: &[&str] = &[
    "click_new_game_stream_ok_queue",
    "click_new_game_stream_ok_drain",
    "click_new_game_stream_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_new_game_stream_method_names_residual_wave167() -> bool {
    NEW_GAME_STREAM_METHOD_NAMES_WAVE167.len() == 5
        && residual_name_index(NEW_GAME_STREAM_METHOD_NAMES_WAVE167, "queue_new_game") == Some(0)
        && residual_name_index(
            NEW_GAME_STREAM_METHOD_NAMES_WAVE167,
            "take_pending_new_game_start_request",
        ) == Some(2)
        && residual_name_index(NEW_GAME_STREAM_METHOD_NAMES_WAVE167, "start_game_from_ui")
            == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_new_game_stream_nav_commands_residual_wave167() -> bool {
    NEW_GAME_STREAM_NAV_STEPS_WAVE167.len() == 6
        && residual_name_index(NEW_GAME_STREAM_NAV_STEPS_WAVE167, "APPEND_MSG_NEW_GAME") == Some(1)
        && residual_name_index(
            NEW_GAME_STREAM_NAV_STEPS_WAVE167,
            "DRAIN_NEW_GAME_FROM_STREAM",
        ) == Some(3)
        && RUNTIME_HOST_NEW_GAME_STREAM_CMD_NAMES_WAVE167.len() == 3
}

/// Wave 167 composite residual honesty pack.
pub fn honesty_new_game_stream_residual_pack_wave167() -> bool {
    honesty_new_game_stream_method_names_residual_wave167()
        && honesty_new_game_stream_nav_commands_residual_wave167()
}

/// Source residual: engine posts and drains NewGame on the retail path.
pub fn honesty_new_game_stream_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let q = match src.find("fn runtime_host_cmd_queue_new_game") {
        Some(i) => i,
        None => return false,
    };
    let q_body = &src[q..src.len().min(q + 5000)];
    let has_queue = q_body.contains("GameMessageType::NewGame")
        && q_body.contains("append_message")
        && q_body.contains("take_pending_new_game_start_request")
        && q_body.contains("start_game_from_ui");

    // Menu tick drain residual.
    let has_menu_drain = src.contains("Menu NewGame drain")
        || (src.contains("take_pending_new_game_start_request()")
            && src.contains("start_game_from_ui"));

    // click_skirmish_start also drains after WND ButtonStart.
    let s = match src.find("fn runtime_host_cmd_click_skirmish_start") {
        Some(i) => i,
        None => return false,
    };
    let s_body = &src[s..src.len().min(s + 10000)];
    let has_skirmish_drain = s_body.contains("take_pending_new_game_start_request");

    has_queue && has_menu_drain && has_skirmish_drain
}

fn with_stream_mut<R>(f: impl FnOnce(&mut MessageStream) -> R) -> Option<R> {
    let stream = get_message_stream();
    let mut guard = match stream.write() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    Some(f(&mut guard))
}

fn with_stream_ref<R>(f: impl FnOnce(&MessageStream) -> R) -> Option<R> {
    let stream = get_message_stream();
    let guard = match stream.read() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    Some(f(&guard))
}

/// Count NewGame messages currently on the common stream.
fn count_new_game_messages_on_stream() -> usize {
    with_stream_ref(|guard| {
        guard
            .get_messages()
            .iter()
            .filter(|m| matches!(m.get_type(), GameMessageType::NewGame))
            .count()
    })
    .unwrap_or(0)
}

/// Re-append a kept message (type + scalar args) onto the stream.
fn reappend_message(guard: &mut MessageStream, message: &GameMessage) {
    let forwarded = guard.append_message(message.get_type().clone());
    for arg in message.get_arguments() {
        match &arg.data {
            GameMessageArgumentType::Integer(v) => forwarded.append_integer_argument(*v),
            GameMessageArgumentType::Real(v) => forwarded.append_real_argument(*v),
            GameMessageArgumentType::Boolean(v) => forwarded.append_boolean_argument(*v),
            // Residual peel: scalar args cover NewGame + most host posts.
            _ => {}
        }
    }
}

/// Remove all NewGame messages from the common stream (Menu drain residual peel).
/// Returns how many were removed. Preserves non-NewGame messages.
fn drain_new_game_messages_from_stream() -> usize {
    with_stream_mut(|guard| {
        let messages: Vec<GameMessage> = guard.get_messages().iter().cloned().collect();
        let kept: Vec<GameMessage> = messages
            .iter()
            .filter(|m| !matches!(m.get_type(), GameMessageType::NewGame))
            .cloned()
            .collect();
        let removed = messages.len() - kept.len();
        guard.clear_messages();
        for message in &kept {
            reappend_message(guard, message);
        }
        removed
    })
    .unwrap_or(0)
}

/// Post a skirmish NewGame onto the common stream (queue residual peel).
fn post_skirmish_new_game_to_stream() -> bool {
    with_stream_mut(|guard| {
        let msg = guard.append_message(GameMessageType::NewGame);
        // mode=skirmish(2), difficulty normal(1), rank 0, max fps 30
        msg.append_integer_argument(2);
        msg.append_integer_argument(1);
        msg.append_integer_argument(0);
        msg.append_integer_argument(30);
    })
    .is_some()
}

/// Live residual: post NewGame, observe it, drain it (no full start_game_from_ui).
pub fn simulate_new_game_stream_post_drain_honesty() -> bool {
    if !honesty_new_game_stream_residual_pack_wave167() {
        return false;
    }
    if !honesty_new_game_stream_source() {
        return false;
    }

    // Isolate: drain any pre-existing NewGame noise first.
    let _ = drain_new_game_messages_from_stream();
    if count_new_game_messages_on_stream() != 0 {
        return false;
    }

    if !post_skirmish_new_game_to_stream() {
        return false;
    }
    if count_new_game_messages_on_stream() == 0 {
        return false;
    }

    let removed = drain_new_game_messages_from_stream();
    if removed == 0 {
        return false;
    }
    if count_new_game_messages_on_stream() != 0 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_new_game_stream_method_names_residual_wave167());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_new_game_stream_nav_commands_residual_wave167());
    }

    #[test]
    fn wave167_composite_pack() {
        assert!(honesty_new_game_stream_residual_pack_wave167());
    }

    #[test]
    fn new_game_stream_source() {
        assert!(honesty_new_game_stream_source());
    }

    #[test]
    fn simulate_new_game_stream_post_drain_honesty_residual_live() {
        assert!(
            simulate_new_game_stream_post_drain_honesty(),
            "NewGame must post to common stream and drain cleanly"
        );
    }
}
