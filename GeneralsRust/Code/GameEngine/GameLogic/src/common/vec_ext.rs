//! Vec extension methods
//!
//! Provides additional utility methods for Vec types

use game_engine::rts::player::Player;
use std::sync::{Arc, Mutex};

/// Extension trait for Vec of players to provide game-specific functionality.
pub trait PlayerVecExt {
    /// Return a reference to the neutral player if present.
    fn get_neutral_player(&self) -> Option<&Arc<Mutex<Player>>>;
}

impl PlayerVecExt for Vec<Arc<Mutex<Player>>> {
    fn get_neutral_player(&self) -> Option<&Arc<Mutex<Player>>> {
        // We currently have no explicit neutral marker; fall back to the first entry
        // to preserve C++ call-site expectations until player metadata is extended.
        self.first()
    }
}
