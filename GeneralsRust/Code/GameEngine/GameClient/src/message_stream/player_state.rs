use std::sync::atomic::{AtomicI32, Ordering};

/// Tracks the local player index so translators can stamp commands correctly.
static LOCAL_PLAYER_ID: AtomicI32 = AtomicI32::new(0);

/// Set the local player id (usually once when a game session starts).
pub fn set_local_player_id(player_id: i32) {
    LOCAL_PLAYER_ID.store(player_id, Ordering::SeqCst);
}

/// Retrieve the currently configured local player id.
pub fn get_local_player_id() -> i32 {
    LOCAL_PLAYER_ID.load(Ordering::SeqCst)
}
