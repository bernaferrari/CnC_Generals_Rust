/// Tracks the local player index so translators can stamp commands correctly.
/// Owned on GameClient.local_player_id.

#[cfg(test)]
thread_local! {
    static TEST_LOCAL_PLAYER_ID: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Set the local player id (usually once when a game session starts).
pub fn set_local_player_id(player_id: i32) {
    if let Some(()) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.set_local_player_id(player_id);
    }) {
        return;
    }
    #[cfg(test)]
    {
        TEST_LOCAL_PLAYER_ID.with(|cell| cell.set(player_id));
        return;
    }
    #[cfg(not(test))]
    {
        let _ = player_id;
    }
}

/// Retrieve the currently configured local player id.
pub fn get_local_player_id() -> i32 {
    if let Some(id) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.local_player_id()
    }) {
        return id;
    }
    #[cfg(test)]
    {
        return TEST_LOCAL_PLAYER_ID.with(|cell| cell.get());
    }
    #[cfg(not(test))]
    {
        0
    }
}
