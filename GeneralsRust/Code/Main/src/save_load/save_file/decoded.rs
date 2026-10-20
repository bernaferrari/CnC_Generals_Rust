//! A single decoded archive owns its logical and client payloads together.
//! C++ GameState::xferSaveData walks named blocks on one load operation
//! (GameState.cpp:1385–1458). No payload is published for a later load to take.
use crate::save_load::{SaveGameInfo, snapshot::WorldSnapshot};

#[derive(Debug)]
pub struct DecodedGameSave {
    pub snapshot: WorldSnapshot,
    pub save_info: SaveGameInfo,
    /// Absent for the historical one-byte NullSnapshot or omitted client chunk.
    /// Kept detached until the receiving host admits the candidate world.
    pub game_client_xfer: Option<Vec<u8>>,
}
