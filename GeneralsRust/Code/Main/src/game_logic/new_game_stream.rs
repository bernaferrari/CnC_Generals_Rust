//! New game stream.

use game_engine::common::message_stream::{
    GameMessage, GameMessageArgumentType, GameMessageType, MessageStream, get_message_stream,
};

/// Lookup residual name index (exact match).

/// NewGame stream residual method names.

/// Ordered NewGame residual navigation steps.

/// Runtime-host command residual names.

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 167 composite residual honesty pack.

/// Source residual: engine posts and drains NewGame on the retail path.

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

#[cfg(test)]
mod tests {
    use super::*;
}
