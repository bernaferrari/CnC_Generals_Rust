//! A single decoded archive owns its logical and client payloads together.
//! No payload is published for a later load to take.
use crate::save_load::{SaveGameInfo, snapshot::WorldSnapshot};
use serde::{Deserialize, Serialize};

/// Admission requirement captured from the world that produced this archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Structured map admission payload replacing the C++ `CHUNK_GameStateMap`
/// positional block. `map_bytes` remains raw because a `.map` file is itself
/// an asset payload, not a save-format structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMapArchive {
    pub save_game_map: String,
    pub portable_pristine_map_name: String,
    pub game_mode: i32,
    pub map_bytes: Vec<u8>,
    pub next_object_id: u32,
    pub next_drawable_id: u32,
    pub source_game_info: Option<game_engine::common::system::skirmish_game_info::SkirmishGameInfo>,
}

/// Owned versioned save archive. Companion byte fields are temporary capsules
/// for behavior systems that are still being converted to typed snapshots;
/// unlike the old chunk container, their ownership and ordering are explicit.
#[derive(Debug, Deserialize)]
pub struct SaveArchive {
    pub version: u32,
    pub save_info: SaveGameInfo,
    pub pristine_map_name: Option<String>,
    pub snapshot: WorldSnapshot,
    pub campaign_state: Option<game_engine::System::CampaignManagerXferState>,
    pub map_admission_intent: MapAdmissionIntent,
    pub saved_game_mode: Option<i32>,
    pub game_client_xfer: Option<Vec<u8>>,
    pub terrain_visual_xfer: Option<Vec<u8>>,
    pub map: Option<SaveMapArchive>,
}

/// Borrowed serialization form avoids cloning a multi-hundred-megabyte world
/// merely to wrap it in an archive envelope.
#[derive(Serialize)]
pub struct SaveArchiveRef<'a> {
    pub version: u32,
    pub save_info: &'a SaveGameInfo,
    pub pristine_map_name: Option<&'a str>,
    pub snapshot: &'a WorldSnapshot,
    pub campaign_state: Option<&'a game_engine::System::CampaignManagerXferState>,
    pub map_admission_intent: MapAdmissionIntent,
    pub saved_game_mode: Option<i32>,
    pub game_client_xfer: Option<&'a [u8]>,
    pub terrain_visual_xfer: Option<&'a [u8]>,
    pub map: Option<&'a SaveMapArchive>,
}
