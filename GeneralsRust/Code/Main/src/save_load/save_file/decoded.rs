//! A single decoded archive owns its logical and client payloads together.
//! C++ GameState::xferSaveData walks named blocks on one load operation
//! (GameState.cpp:1385–1458). No payload is published for a later load to take.
use crate::save_load::{SaveGameInfo, snapshot::WorldSnapshot};

/// Admission requirement captured from the world that produced this archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAdmissionIntent {
    Mapless,
    Authored,
}

#[derive(Debug)]
pub struct DecodedGameSave {
    pub snapshot: WorldSnapshot,
    /// Detached CHUNK_Campaign payload; only this archive's successful admission
    /// may apply it. Failed or interleaved decodes never publish pending state.
    pub campaign_state: Option<game_engine::System::CampaignManagerXferState>,
    pub save_info: SaveGameInfo,
    /// Archives without this host extension are conservatively authored.
    pub map_admission_intent: MapAdmissionIntent,
    /// Original GameStateMap v2 game-mode integer. Version 1 and omitted map
    /// chunks leave the receiving mode unchanged; unknown values retain the
    /// existing admission fallback rather than inventing an enum variant.
    pub saved_game_mode: Option<i32>,
    /// Absent for the historical one-byte NullSnapshot or omitted client chunk.
    /// Kept detached until the receiving host admits the candidate world.
    pub game_client_xfer: Option<Vec<u8>>,
    /// Paired with this archive, never published for another restore to take.
    pub terrain_visual_xfer: Option<Vec<u8>>,
}
