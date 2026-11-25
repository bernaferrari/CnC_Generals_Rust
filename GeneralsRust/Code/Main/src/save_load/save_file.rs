use crate::game_logic::GameLogic;
use crate::save_load::*;
use game_engine::common::system::save_game::GameStateMap as CommonGameStateMap;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "save_file/decoded.rs"]
mod decoded;
#[path = "save_file/map_admission.rs"]
mod map_admission;
pub use decoded::{DecodedGameSave, MapAdmissionIntent};
pub(crate) use decoded::SaveArchive;
use decoded::{SaveArchiveRef, SaveMapArchive};

/// Production saves are a single versioned serde archive compressed with zstd.
/// The archive owns the world snapshot, campaign state, map admission payload,
/// and client companions explicitly; decoding never publishes pieces of one
/// candidate for another restore to take.
pub(crate) const VERSIONED_SAVE_MAGIC: &[u8; 8] = b"GZHRSAV\0";
pub(crate) const VERSIONED_SAVE_ARCHIVE_VERSION: u32 = 4;
const MAX_VERSIONED_SAVE_BYTES: usize = 512 * 1024 * 1024;

/// Stored game-mode integers written by `GameStateMap::xfer` v2
/// (`GameLogic.h`, `GameStateMap.cpp:324-329`).
const SAVED_GAME_MODE_SINGLE_PLAYER: i32 = 0;
const SAVED_GAME_MODE_LAN: i32 = 1;
const SAVED_GAME_MODE_SKIRMISH: i32 = 2;
const SAVED_GAME_MODE_REPLAY: i32 = 3;
const SAVED_GAME_MODE_SHELL: i32 = 4;
const SAVED_GAME_MODE_INTERNET: i32 = 5;
const SAVED_GAME_MODE_NONE: i32 = 6;

fn encode_saved_game_mode(mode: crate::game_logic::GameMode) -> i32 {
    use crate::game_logic::GameMode;
    match mode {
        GameMode::SinglePlayer => SAVED_GAME_MODE_SINGLE_PLAYER,
        GameMode::Lan | GameMode::Multiplayer => SAVED_GAME_MODE_LAN,
        GameMode::Skirmish => SAVED_GAME_MODE_SKIRMISH,
        GameMode::Replay => SAVED_GAME_MODE_REPLAY,
        GameMode::Shell => SAVED_GAME_MODE_SHELL,
        GameMode::Internet => SAVED_GAME_MODE_INTERNET,
        GameMode::None => SAVED_GAME_MODE_NONE,
    }
}

pub fn decode_saved_game_mode(mode: i32) -> Option<crate::game_logic::GameMode> {
    use crate::game_logic::GameMode;
    match mode {
        SAVED_GAME_MODE_SINGLE_PLAYER => Some(GameMode::SinglePlayer),
        SAVED_GAME_MODE_LAN => Some(GameMode::Lan),
        SAVED_GAME_MODE_SKIRMISH => Some(GameMode::Skirmish),
        SAVED_GAME_MODE_REPLAY => Some(GameMode::Replay),
        SAVED_GAME_MODE_SHELL => Some(GameMode::Shell),
        SAVED_GAME_MODE_INTERNET => Some(GameMode::Internet),
        SAVED_GAME_MODE_NONE => Some(GameMode::None),
        _ => None,
    }
}

/// C++ `GameState::xfer` writes `GetLocalTime` fields (`GameState.cpp:1562-1582`).
/// Leftover `SaveDate::from_local_time` already matches that calendar.
fn map_leaf_name(path: &str) -> String {
    path.replace('/', "\\")
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn campaign_state_requires_owner(
    state: &game_engine::System::CampaignManagerXferState,
) -> bool {
    !state.campaign.trim().is_empty() || !state.mission.trim().is_empty() || state.is_challenge
}

/// Test-only campaign owner. Production capture/apply goes through the receiving
/// `GameClient`; GameClient's `get_campaign_manager` is cfg(test) on that crate and
/// is configured out when Main depends on it, so Main keeps a local fixture.
#[cfg(test)]
mod campaign_test_fixture {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static FIXTURE: OnceLock<Mutex<game_client::gui::campaign_manager::CampaignManager>> =
        OnceLock::new();

    fn lock() -> std::sync::MutexGuard<'static, game_client::gui::campaign_manager::CampaignManager>
    {
        FIXTURE
            .get_or_init(|| Mutex::new(game_client::gui::campaign_manager::CampaignManager::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn apply(state: game_engine::System::CampaignManagerXferState) {
        lock().apply_logic_chunk_state(state);
    }

    pub(super) fn capture() -> game_engine::System::CampaignManagerXferState {
        lock().capture_logic_chunk_state()
    }
}

#[cfg(test)]
fn apply_campaign_manager_state(state: game_engine::System::CampaignManagerXferState) {
    campaign_test_fixture::apply(state);
}

#[cfg(test)]
pub(crate) fn capture_live_campaign_state() -> game_engine::System::CampaignManagerXferState {
    campaign_test_fixture::capture()
}

/// Apply only the payload belonging to the admitted archive. Decoding another
/// archive cannot replace or discard it.
#[cfg(test)]
pub(crate) fn commit_campaign_state(state: Option<game_engine::System::CampaignManagerXferState>) {
    if let Some(state) = state {
        apply_campaign_manager_state(state);
    }
}

/// C++ `GameState::loadGame` clearGameData on xfer/loadPostProcess failure.
#[cfg(test)]
fn rollback_campaign_after_failed_load(prior: game_engine::System::CampaignManagerXferState) {
    apply_campaign_manager_state(prior);
}

fn campaign_difficulty(state: &game_engine::System::CampaignManagerXferState) -> GameDifficulty {
    match state.difficulty {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Medium,
    }
}


#[cfg(test)]
pub(crate) fn rewrite_versioned_save_archive(
    path: &Path,
    change: impl FnOnce(&mut SaveArchive),
) {
    let input = std::fs::read(path).expect("read archive to mutate");
    let mut archive = decode_versioned_save_archive(&input).expect("decode archive to mutate");
    change(&mut archive);
    let encoded = encode_versioned_save_archive(&SaveArchiveRef {
        version: archive.version,
        save_info: &archive.save_info,
        pristine_map_name: archive.pristine_map_name.as_deref(),
        snapshot: &archive.snapshot,
        campaign_state: archive.campaign_state.as_ref(),
        map_admission_intent: archive.map_admission_intent,
        saved_game_mode: archive.saved_game_mode,
        game_client_xfer: archive.game_client_xfer.as_deref(),
        terrain_visual_archive: archive.terrain_visual_archive.as_ref(),
        ghost_object_archive: archive.snapshot.w3d_ghost_archive.as_ref(),
        particle_system_xfer: archive.snapshot.particle_system_companion.as_deref(),
        map: archive.map.as_ref(),
    })
    .expect("encode mutated archive");
    std::fs::write(path, encoded).expect("write mutated archive");
}

pub(crate) fn parse_named_chunk_save_info(data: &[u8]) -> SaveLoadResult<SaveGameInfo> {
    if data.len() >= 8 && &data[..8] == VERSIONED_SAVE_MAGIC {
        let archive = decode_versioned_save_archive(data)?;
        let mut info = archive.save_info;
        info.pristine_map_name = archive.pristine_map_name;
        if let Some(campaign) = archive.campaign_state.as_ref() {
            info.difficulty = campaign_difficulty(campaign);
        }
        return Ok(info);
    }
    Err(SaveLoadError::InvalidFormat)
}

fn capture_save_map_archive(
    save_info: &SaveGameInfo,
    save_directory: &Path,
    source: Option<&game_engine::common::system::skirmish_game_info::SkirmishGameInfo>,
    game_mode: i32,
) -> SaveMapArchive {
    let leaf = map_leaf_name(&save_info.map_name);
    let pristine_source = save_info
        .pristine_map_name
        .as_deref()
        .unwrap_or(&save_info.map_name);
    let user_data = game_engine::common::global_data::read()
        .get_user_data_dir()
        .to_string();
    let portable_pristine =
        super::map_paths::to_portable(pristine_source, save_directory, &user_data);
    let mut map_bytes = Vec::new();
    if !save_info.map_name.is_empty() {
        if let Some(path) = crate::game_logic::script_loader::find_map_file(&save_info.map_name) {
            map_bytes = std::fs::read(path).unwrap_or_default();
        } else if Path::new(&save_info.map_name).is_file() {
            map_bytes = std::fs::read(&save_info.map_name).unwrap_or_default();
        }
    }
    let source_game_info = source.map(|source| {
        let mut source = source.clone();
        source.map_name =
            super::map_paths::to_portable(&source.map_name, save_directory, &user_data);
        source
    });
    SaveMapArchive {
        save_game_map: format!("Save\\{leaf}"),
        portable_pristine_map_name: portable_pristine,
        game_mode,
        map_bytes,
        next_object_id: 1,
        next_drawable_id: 1,
        source_game_info,
    }
}

fn extract_archived_map(
    map: &SaveMapArchive,
    save_dir: &Path,
    user_data: &str,
) -> Option<(PathBuf, String)> {
    if map.map_bytes.is_empty() {
        return None;
    }
    let leaf = map_leaf_name(&map.save_game_map);
    if leaf.is_empty() {
        return None;
    }
    let pristine =
        super::map_paths::from_portable(&map.portable_pristine_map_name, save_dir, user_data);
    let _ = std::fs::create_dir_all(save_dir);
    let dest = save_dir.join(leaf);
    std::fs::write(&dest, &map.map_bytes).ok()?;
    Some((dest, pristine))
}

fn encode_versioned_save_archive(archive: &SaveArchiveRef<'_>) -> SaveLoadResult<Vec<u8>> {
    let plain = bincode_legacy::serialize(archive)
        .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
    let compressed = zstd::bulk::compress(&plain, 3)
        .map_err(|error| SaveLoadError::Compression(error.to_string()))?;
    let mut out = Vec::with_capacity(compressed.len() + 16);
    out.extend_from_slice(VERSIONED_SAVE_MAGIC);
    out.extend_from_slice(&(plain.len() as u64).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

fn decode_versioned_save_archive(data: &[u8]) -> SaveLoadResult<SaveArchive> {
    if data.len() < 16 || &data[..8] != VERSIONED_SAVE_MAGIC {
        return Err(SaveLoadError::InvalidFormat);
    }
    let plain_len = u64::from_le_bytes(data[8..16].try_into().expect("archive size field"));
    let plain_len = usize::try_from(plain_len).map_err(|_| {
        SaveLoadError::Corrupted("versioned save size does not fit this platform".into())
    })?;
    if plain_len > MAX_VERSIONED_SAVE_BYTES {
        return Err(SaveLoadError::Corrupted(format!(
            "versioned save is too large: {plain_len} bytes"
        )));
    }
    let plain = zstd::bulk::decompress(&data[16..], plain_len)
        .map_err(|error| SaveLoadError::Compression(error.to_string()))?;
    if plain.len() != plain_len {
        return Err(SaveLoadError::Corrupted(format!(
            "versioned save size mismatch: expected {plain_len}, got {}",
            plain.len()
        )));
    }
    let mut archive: SaveArchive = bincode_legacy::deserialize(&plain)
        .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
    if archive.version != VERSIONED_SAVE_ARCHIVE_VERSION {
        return Err(SaveLoadError::VersionMismatch {
            expected: VERSIONED_SAVE_ARCHIVE_VERSION,
            actual: archive.version,
        });
    }
    validate_direct_world_snapshot_version(archive.snapshot.version)?;
    // Companions are archive-owned on disk but snapshot-owned at runtime.
    // Normalize the historical empty/one-byte NullSnapshot forms to absence.
    fn companion(bytes: Option<Vec<u8>>) -> Option<Vec<u8>> {
        bytes.filter(|bytes| !bytes.is_empty() && bytes.as_slice() != [1])
    }
    archive.snapshot.w3d_ghost_archive = archive.ghost_object_archive.take();
    archive.snapshot.particle_system_companion = companion(archive.particle_system_xfer.take());
    crate::save_load::snapshot::validate_decoded_world_snapshot(&mut archive.snapshot)?;
    if let Some(ghosts) = archive.snapshot.w3d_ghost_archive.as_ref() {
        decode_w3d_ghost_archive(ghosts)?;
    }
    Ok(archive)
}

/// Save file section types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveFileSection {
    Header,
    GameInfo,
    WorldState,
    PlayerStates,
    AIStates,
    MapState,
    Scripts,
    EndMarker,
}

/// Save file manager
pub struct SaveFileManager {
    save_directory: PathBuf,
    temp_directory: PathBuf,
    auto_save_interval: std::time::Duration,
    max_save_files: usize,
    last_auto_save: SystemTime,
    rejected_logical_worlds: Vec<Box<GameLogic>>,
}

impl SaveFileManager {
    pub(crate) fn retain_rejected_logical_world(&mut self, world: Box<GameLogic>) {
        self.rejected_logical_worlds.push(world);
    }
    pub(crate) fn transfer_rejected_logical_worlds_to(&mut self, receiving: &mut Self) {
        receiving
            .rejected_logical_worlds
            .append(&mut self.rejected_logical_worlds);
    }
    pub(crate) fn validate_logical_restore_admission(&self) -> Result<(), String> {
        if self.rejected_logical_worlds.is_empty() {
            Ok(())
        } else {
            Err("terminal logical restore candidate retirement".into())
        }
    }

    pub fn new() -> Self {
        let save_dir = SaveLoadManager::default_save_directory();
        Self::with_save_directory(save_dir)
    }

    pub fn with_save_directory(save_directory: impl Into<PathBuf>) -> Self {
        let save_dir = save_directory.into();
        let mut temp_dir = save_dir.clone();
        temp_dir.push("temp");

        Self {
            save_directory: save_dir,
            temp_directory: temp_dir,
            auto_save_interval: std::time::Duration::from_secs(300), // 5 minutes
            max_save_files: MAX_SAVE_SLOTS,
            last_auto_save: SystemTime::now(),
            rejected_logical_worlds: Vec::new(),
        }
    }

    /// Directory used for list/save/load. Default is `UserData/Save` (Popup + host).
    pub fn save_directory(&self) -> &Path {
        &self.save_directory
    }

    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create directories if they don't exist
        std::fs::create_dir_all(&self.save_directory)?;
        std::fs::create_dir_all(&self.temp_directory)?;

        // Clean up old temporary files
        self.cleanup_temp_files()?;

        Ok(())
    }

    /// Between missions only the campaign and header survive the outgoing world.
    pub(crate) fn save_campaign_checkpoint(
        &self,
        next_map: &str,
        description: &str,
        state: &game_engine::System::CampaignManagerXferState,
    ) -> (PathBuf, SaveLoadResult<()>) {
        let (campaign_side, mission_number) = campaign_header_from_campaign_manager(Some(state));
        let info = SaveGameInfo {
            filename: String::new(),
            display_name: description.to_owned(),
            description: description.to_owned(),
            map_name: next_map.to_owned(),
            pristine_map_name: None,
            campaign_side,
            mission_number,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_owned(),
            play_time: std::time::Duration::ZERO,
            difficulty: campaign_difficulty(state),
            save_type: SaveFileType::Mission,
        };
        let mut destination = self.save_directory.clone();
        let result = (|| {
            let encoded = Self::encode_mission_checkpoint_bytes(&info, state)?;
            self.publish_numbered_checkpoint(&encoded, &mut destination)
        })();
        (destination, result)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn publish_numbered_checkpoint(
        &self,
        bytes: &[u8],
        destination: &mut PathBuf,
    ) -> SaveLoadResult<()> {
        std::fs::create_dir_all(&self.save_directory)?;
        let mut staged = tempfile::NamedTempFile::new_in(&self.save_directory)?;
        staged.write_all(bytes)?;
        // GameState.cpp:416-507 chooses the lowest free eight-digit save name.
        for number in 0..=99_999_999 {
            *destination = self.get_save_path(&format!("{number:08}"));
            match staged.persist_noclobber(&*destination) {
                Ok(_) => return Ok(()),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    staged = error.file;
                }
                Err(error) => return Err(error.error.into()),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "all numbered save slots are occupied",
        )
        .into())
    }

    #[cfg(target_arch = "wasm32")]
    fn publish_numbered_checkpoint(&self, _: &[u8], _: &mut PathBuf) -> SaveLoadResult<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported).into())
    }

    /// Save game to file
    pub fn save_game(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        // Non-host callers deliberately remain logic-only.  The authoritative
        // CnCGameEngine path captures its renderer-owned companion explicitly
        // through the companion-aware API below.
        self.save_game_with_client_drawable_snapshot(
            filename,
            game_logic,
            ClientDrawableWorldSnapshot::default(),
            save_info,
        )
    }

    /// Save a WorldSnapshot with an explicitly captured renderer companion.
    ///
    /// SnapshotBuilder owns only GameLogic, so it must not reach into live
    /// renderer/global state.  The host captures the DTO at its authority
    /// boundary, passes it here, and this method attaches it before the normal
    /// Common `.sav` writer serializes the exact v4 positional record.
    pub fn save_game_with_client_drawable_snapshot(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            None,
            save_info,
            crate::save_load::snapshot::ClientSaveState::default(),
            None,
            None,
        )
    }

    /// Save the committed host world's visual companion from its owning client.
    pub fn save_game_with_client_state(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            crate::save_load::snapshot::ClientSaveState::default(),
            None,
            None,
        )
    }

    /// Save a view captured by the same engine that owns this world/client.
    pub fn save_game_with_client_state_and_camera(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        camera: crate::save_load::snapshot::CameraPersist,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            crate::save_load::snapshot::ClientSaveState {
                camera: Some(camera),
                camera_continuation: None,
                control_groups: None,
            },
            None,
            None,
        )
    }

    /// Capture camera and control groups from the same driving engine.
    pub fn save_game_with_client_save_state(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            None,
            None,
        )
    }

    /// Native caller supplies its actual scorch observation explicitly.
    pub fn save_game_with_client_save_state_and_terrain_scorches(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            scorches,
            None,
        )
    }

    /// Native source owner for water-render state, scorch marks and TREE records.
    pub fn save_game_with_client_save_state_and_native_terrain(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        visual: Option<&game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            visual.map(|v| v.scorch_marks()),
            visual,
        )
    }

    fn save_game_with_client_state_impl(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        mut client: Option<&mut game_client::core::game_client::GameClient>,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        terrain_scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
        native_terrain: Option<&game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) -> SaveLoadResult<()> {
        if let Some(camera) = state.camera {
            camera.validate()?;
        }
        if let Some(groups) = &state.control_groups {
            groups.validate()?;
            for player_id in groups.players.keys() {
                let player_id = u32::try_from(*player_id).map_err(|_| {
                    SaveLoadError::Corrupted("invalid control-group owner identity".into())
                })?;
                if game_logic.get_player(player_id).is_none() {
                    return Err(SaveLoadError::Corrupted(format!(
                        "control-group owner {player_id} absent from receiving roster"
                    )));
                }
            }
        }
        let native_camera = state
            .camera_continuation
            .as_deref()
            .map(crate::cnc_game_engine::CnCGameEngine::decode_camera_continuation)
            .transpose()
            .map_err(SaveLoadError::Corrupted)?;
        // Full native continuation is canonical when supplied. Derive the
        // historical View/v18 pose from that same owner, never a second input.
        let camera = native_camera
            .as_ref()
            .map(|camera| camera.camera_persist())
            .or(state.camera);
        let save_path = self.get_save_path(filename);
        let temp_path = self.get_temp_path(&format!("{}_temp", filename));

        // Create snapshot of current game state
        let snapshot_builder = SnapshotBuilder::new();
        let mut world_snapshot = if let Some(client) = client.as_deref() {
            snapshot_builder.create_world_snapshot_with_client(game_logic, client)?
        } else {
            snapshot_builder.create_world_snapshot(game_logic)?
        };
        world_snapshot.client_drawables = client_drawables;
        if let Some(camera) = camera {
            camera.write_to(&mut world_snapshot.persist_v18);
        }
        if let Some(groups) = &state.control_groups {
            crate::save_load::snapshot::write_control_groups(
                &mut world_snapshot.lifecycle_tail,
                groups,
            )?;
        }
        if let Some(bytes) = state.camera_continuation.as_deref() {
            crate::save_load::snapshot::write_native_camera_continuation(
                &mut world_snapshot.lifecycle_tail,
                bytes,
            )?;
        }
        if save_info.save_type != SaveFileType::Mission {
            let persist = &mut world_snapshot.persist_v18;
            persist.tactical_view_camera = Some(
                crate::save_load::snapshot::persist_v18::TacticalViewPersist {
                    angle: persist.camera_angle,
                    position: [
                        persist.camera_target[0],
                        persist.camera_target[2],
                        persist.camera_target[1],
                    ],
                },
            );
        }
        // C++ GameState writes the logic chunk before GameClient::xfer.
        let campaign_state = if let Some(client) = client.as_deref() {
            client.campaign_manager().capture_logic_chunk_state()
        } else if game_logic.has_campaign_session() {
            return Err(SaveLoadError::Serialization(
                "active campaign save has no receiving campaign owner".into(),
            ));
        } else {
            game_engine::System::CampaignManagerXferState::default()
        };
        if client.is_some()
            && campaign_state_requires_owner(&campaign_state) != game_logic.has_campaign_session()
        {
            return Err(SaveLoadError::Serialization(
                "receiving campaign owner and admitted session kind disagree".into(),
            ));
        }
        let client_xfer_bytes = if let Some(client) = client.as_deref_mut() {
            #[cfg(feature = "game_client")]
            {
                crate::save_load::snapshot::capture_game_client_xfer_bytes_excluding_owned(
                    client,
                    &game_logic.owned_jet_lockon_receipts(),
                )?
            }
            #[cfg(not(feature = "game_client"))]
            {
                capture_game_client_xfer_bytes(client)?
            }
        } else {
            Vec::new()
        };
        // C++ named phases: GameLogic, GameClient, then ParticleSystem. Capture
        // once on this new save candidate; the writer never consults a later
        // live manager for a decoded or absent companion.
        world_snapshot.particle_system_companion = if save_info.save_type == SaveFileType::Mission {
            // C++ mission saves contain only GameState and Campaign blocks.
            None
        } else {
            client
                .as_deref_mut()
                .map(|client| capture_particle_system_xfer_bytes(client.particle_systems_mut()))
                .transpose()?
        };

        // C++ GameState::saveGame (GameState.cpp:534) makes sure the save
        // directory exists at save time; our atomic flow writes into
        // save_directory/temp first, so that directory must exist too.
        std::fs::create_dir_all(&self.temp_directory)?;

        // Save to temporary file first
        let mut effective_info = save_info.clone();
        if effective_info.pristine_map_name.is_none() {
            effective_info.pristine_map_name = game_logic.map_definition_source.clone();
        }
        // The owner, not a UI label or filesystem lookup, determines whether
        // this archive needs an authored map on admission.
        let map_intent = if game_logic.map_definition_source.is_some() {
            effective_info.map_name = game_logic.get_current_map_name().to_owned();
            MapAdmissionIntent::Authored
        } else {
            MapAdmissionIntent::Mapless
        };
        let native_water = native_terrain.map(|visual| visual.capture_water_state());
        let tree_records =
            native_terrain.map_or_else(Vec::new, |visual| visual.capture_tree_records());
        let terrain_visual_archive =
            crate::save_load::snapshot::capture_receiving_terrain_visual_archive(
                game_logic,
                native_water.as_ref(),
                terrain_scorches,
                tree_records,
            )?;
        let write_result = self.save_to_file(
            &temp_path,
            &world_snapshot,
            &effective_info,
            &client_xfer_bytes,
            Some(&terrain_visual_archive),
            encode_saved_game_mode(game_logic.game_mode()),
            &campaign_state,
            map_intent,
        );
        write_result?;

        // Atomically move temp file to final location
        std::fs::rename(&temp_path, &save_path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            SaveLoadError::Io(e)
        })?;

        // C++ GameState::saveGame never deletes existing saves.
        log::info!("Game saved successfully to: {}", save_path.display());
        Ok(())
    }

    /// Load game from file
    /// Logical-only API. Native applications use the companion-aware route.
    pub fn load_game(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<SaveGameInfo> {
        let (info, _native_effects) = self.load_game_with_terrain_visual(filename, game_logic)?;
        // Explicit absence of a native receiver: logical state only, matching
        // this API's existing omission of the GameClient companion.
        Ok(info)
    }

    pub fn load_game_with_terrain_visual(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<(SaveGameInfo, Option<PreparedTerrainVisual>)> {
        self.validate_logical_restore_admission()
            .map_err(SaveLoadError::Serialization)?;
        let mut decoded = self.load_game_snapshot(filename)?;
        if decoded
            .campaign_state
            .as_ref()
            .is_some_and(campaign_state_requires_owner)
        {
            return Err(SaveLoadError::Serialization(
                "campaign save requires a receiving campaign owner".into(),
            ));
        }
        // Mission metadata launches a later mission-start operation, as before.
        if decoded.save_info.save_type == SaveFileType::Mission {
            // Logical-only API has no active campaign receiver. Its archive
            // payload remains detached and cannot mutate another application.
            decoded.campaign_state.take();
            return Ok((decoded.save_info, None));
        }
        match crate::cnc_game_engine::CnCGameEngine::restore_decoded_logical_save(
            self, filename, decoded, game_logic,
        )
        .map_err(SaveLoadError::Serialization)
        {
            Ok(restored) => Ok(restored),
            Err(error) => Err(error),
        }
    }

    /// Decode a save without mutating a live `GameLogic` instance.
    ///
    /// The runtime host uses this to load the saved map into a staging world
    /// before it restores the snapshot.  Keeping the decode separate from the
    /// restore means a bad/missing map or corrupted snapshot cannot partially
    /// overwrite the currently playable match.
    pub fn load_game_snapshot(&self, filename: &str) -> SaveLoadResult<DecodedGameSave> {
        let save_path = self.get_save_path(filename);
        if !save_path.exists() {
            return Err(SaveLoadError::FileNotFound(filename.to_string()));
        }

        self.load_from_file(&save_path)
    }

    /// Restore a previously decoded snapshot into the supplied world.
    ///
    /// Callers that need transactional map identity handling should restore
    /// into a fresh staging world and install it only after this succeeds.
    pub fn restore_game_snapshot(
        &self,
        world_snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let snapshot_builder = SnapshotBuilder::new();
        snapshot_builder.restore_from_snapshot(world_snapshot, game_logic)
    }

    /// Quick save to slot 0
    pub fn quick_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let save_info = SaveGameInfo {
            pristine_map_name: None,
            filename: "quicksave".to_string(),
            display_name: "Quick Save".to_string(),
            description: "Quick save".to_string(),
            map_name: game_logic
                .map_definition_source
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0), // Would track actual play time
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::QuickSave,
        };

        self.save_game("quicksave", game_logic, &save_info)
    }

    /// Auto save if enough time has passed
    pub fn try_auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<bool> {
        let now = SystemTime::now();
        if now.duration_since(self.last_auto_save).unwrap_or_default() >= self.auto_save_interval {
            self.auto_save(game_logic)?;
            self.last_auto_save = now;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Auto save
    pub fn auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let save_info = SaveGameInfo {
            pristine_map_name: None,
            filename: format!("autosave_{}", timestamp),
            display_name: "Auto Save".to_string(),
            description: format!("Automatic save at {}", timestamp),
            map_name: game_logic
                .map_definition_source
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::AutoSave,
        };

        let filename = &save_info.filename;
        self.save_game(filename, game_logic, &save_info)?;

        Ok(())
    }

    /// Delete save file
    pub fn delete_save(&self, filename: &str) -> SaveLoadResult<()> {
        match self.delete_existing_save(filename) {
            Err(SaveLoadError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    /// Delete a single save slot, reporting a missing file as a failure.
    pub fn delete_existing_save(&self, filename: &str) -> SaveLoadResult<()> {
        use std::path::Component;
        let mut components = Path::new(filename).components();
        if filename.is_empty()
            || filename.contains(['/', '\\'])
            || !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(SaveLoadError::InvalidFormat);
        }
        let save_path = self.get_save_path(filename);
        std::fs::remove_file(&save_path)?;
        log::info!("Deleted save file: {}", save_path.display());
        Ok(())
    }

    /// Check if save file exists
    pub fn save_exists(&self, filename: &str) -> bool {
        self.get_save_path(filename).exists()
    }

    /// Get save file info without loading the entire file
    pub fn get_save_info(&self, filename: &str) -> SaveLoadResult<SaveGameInfo> {
        let save_path = self.get_save_path(filename);
        self.get_save_info_from_path(&save_path)
    }

    fn get_save_info_from_path(&self, save_path: &Path) -> SaveLoadResult<SaveGameInfo> {
        let mut file = File::open(save_path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;
        if all.len() >= 8 && &all[..8] == VERSIONED_SAVE_MAGIC {
            let archive = decode_versioned_save_archive(&all)?;
            let mut info = archive.save_info;
            info.pristine_map_name = archive.pristine_map_name;
            if let Some(campaign) = archive.campaign_state.as_ref() {
                info.difficulty = campaign_difficulty(campaign);
            }
            return Ok(info);
        }
        Err(SaveLoadError::InvalidFormat)
    }

    /// List all available save files
    pub fn list_saves(&self) -> SaveLoadResult<Vec<AvailableGameInfo>> {
        let mut saves = Vec::new();

        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == SAVE_EXTENSION {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        match self.get_save_info(filename) {
                            Ok(save_info) => {
                                saves.push(AvailableGameInfo {
                                    filename: filename.to_string(),
                                    save_info,
                                });
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to read save info from {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        // Sort by save date, newest first
        saves.sort_by(|a, b| b.save_info.save_date.cmp(&a.save_info.save_date));

        Ok(saves)
    }

    /// Save data as one versioned, compressed serde archive.
    fn save_to_file(
        &self,
        path: &Path,
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        client_xfer_bytes: &[u8],
        terrain_visual_archive: Option<&crate::save_load::snapshot::TerrainVisualArchive>,
        supplied_game_mode: i32,
        campaign_state: &game_engine::System::CampaignManagerXferState,
        map_intent: MapAdmissionIntent,
    ) -> SaveLoadResult<()> {
        let mission = save_info.save_type == SaveFileType::Mission;
        let empty_snapshot;
        let mut map_archive = None;
        let mut saved_game_mode = None;
        let snapshot = if mission {
            empty_snapshot = WorldSnapshot::default();
            &empty_snapshot
        } else {
            validate_direct_world_snapshot_version(world_snapshot.version)?;
            crate::save_load::snapshot::validate_world_path_links(world_snapshot)?;
            if let Some(ghosts) = world_snapshot.w3d_ghost_archive.as_ref() {
                decode_w3d_ghost_archive(ghosts)?;
            }
            let initial = crate::save_load::snapshot::initial_skirmish::read(world_snapshot)?;
            let source_game_info =
                crate::save_load::snapshot::initial_skirmish::source(world_snapshot)?;
            let mode = if initial.is_some() || source_game_info.is_some() {
                SAVED_GAME_MODE_SKIRMISH
            } else {
                supplied_game_mode
            };
            map_admission::validate_native(
                world_snapshot,
                mode,
                source_game_info.as_ref(),
                str::to_owned,
            )?;
            let player_team_chunks = player_team_chunks_from_world(world_snapshot)?;
            validate_host_alliances(world_snapshot, &player_team_chunks)?;
            saved_game_mode = Some(mode);
            map_archive = Some(capture_save_map_archive(
                save_info,
                &self.save_directory,
                source_game_info.as_ref(),
                mode,
            ));
            world_snapshot
        };

        let archive = SaveArchiveRef {
            version: VERSIONED_SAVE_ARCHIVE_VERSION,
            save_info,
            pristine_map_name: save_info.pristine_map_name.as_deref(),
            snapshot,
            campaign_state: Some(campaign_state),
            map_admission_intent: map_intent,
            saved_game_mode,
            game_client_xfer: (!client_xfer_bytes.is_empty()).then_some(client_xfer_bytes),
            terrain_visual_archive,
            ghost_object_archive: snapshot.w3d_ghost_archive.as_ref(),
            particle_system_xfer: snapshot.particle_system_companion.as_deref(),
            map: map_archive.as_ref(),
        };
        let encoded = encode_versioned_save_archive(&archive)?;
        std::fs::write(path, encoded)?;
        Ok(())
    }

    /// Load the current versioned serde `.sav` container.
    fn load_from_file(&self, path: &Path) -> SaveLoadResult<DecodedGameSave> {
        // Preserve the useful operational behavior without preserving the C++
        // byte container: scratch maps are cleared before admitting a new one.
        {
            let scratch = CommonGameStateMap::new(self.save_directory.clone());
            if let Err(err) = scratch.clear_scratch_pad_maps() {
                log::warn!("Error clearing scratch-pad maps before load: {err}");
            }
        }

        let data = std::fs::read(path)?;
        let archive = decode_versioned_save_archive(&data)?;
        let mut save_info = archive.save_info;
        save_info.pristine_map_name = archive.pristine_map_name;
        if let Some(campaign) = archive.campaign_state.as_ref() {
            save_info.difficulty = campaign_difficulty(campaign);
            if !campaign.campaign.is_empty() {
                save_info.campaign_side = Some(campaign.campaign.clone());
            }
        }

        let mut world_snapshot = archive.snapshot;
        let mut saved_game_mode = archive.saved_game_mode;
        if save_info.save_type != SaveFileType::Mission {
            let player_team_chunks = player_team_chunks_from_world(&world_snapshot)?;
            validate_host_alliances(&world_snapshot, &player_team_chunks)?;
            let map = archive.map.as_ref().ok_or_else(|| {
                SaveLoadError::Corrupted("structured save is missing map admission data".into())
            })?;
            map_admission::bind_original_if_needed(
                &mut world_snapshot,
                map.source_game_info.as_ref(),
            )?;
            let user_data = game_engine::common::global_data::read()
                .get_user_data_dir()
                .to_string();
            map_admission::validate_native(
                &world_snapshot,
                map.game_mode,
                map.source_game_info.as_ref(),
                |map_name| {
                    super::map_paths::to_portable(map_name, &self.save_directory, &user_data)
                },
            )?;
            if let Some((extracted, pristine)) =
                extract_archived_map(map, &self.save_directory, &user_data)
            {
                save_info.map_name = extracted.to_string_lossy().into_owned();
                save_info.pristine_map_name = (!pristine.is_empty()).then_some(pristine);
            }
            saved_game_mode = saved_game_mode.or(Some(map.game_mode));
        }

        Ok(DecodedGameSave {
            snapshot: world_snapshot,
            campaign_state: archive.campaign_state,
            save_info,
            map_admission_intent: archive.map_admission_intent,
            game_client_xfer: archive.game_client_xfer,
            terrain_visual_archive: archive.terrain_visual_archive,
            saved_game_mode,
        })
    }

    fn encode_mission_checkpoint_bytes(
        save_info: &SaveGameInfo,
        campaign_state: &game_engine::System::CampaignManagerXferState,
    ) -> SaveLoadResult<Vec<u8>> {
        let empty_world = WorldSnapshot::default();
        let archive = SaveArchiveRef {
            version: VERSIONED_SAVE_ARCHIVE_VERSION,
            save_info,
            pristine_map_name: save_info.pristine_map_name.as_deref(),
            snapshot: &empty_world,
            campaign_state: Some(campaign_state),
            map_admission_intent: MapAdmissionIntent::Authored,
            saved_game_mode: None,
            game_client_xfer: None,
            terrain_visual_archive: None,
            ghost_object_archive: None,
            particle_system_xfer: None,
            map: None,
        };
        encode_versioned_save_archive(&archive)
    }

    pub fn read_campaign_state(
        &self,
        filename: &str,
    ) -> SaveLoadResult<game_engine::System::CampaignManagerXferState> {
        let save_path = self.get_save_path(filename);
        let data = std::fs::read(&save_path)?;
        decode_versioned_save_archive(&data)?
            .campaign_state
            .ok_or_else(|| SaveLoadError::Corrupted("campaign state missing from save".into()))
    }

    pub fn get_save_path(&self, filename: &str) -> PathBuf {
        let mut path = self.save_directory.clone();
        path.push(format!("{}.{}", filename, SAVE_EXTENSION));
        path
    }

    /// Get temporary file path
    fn get_temp_path(&self, filename: &str) -> PathBuf {
        let mut path = self.temp_directory.clone();
        path.push(format!("{}.tmp", filename));
        path
    }

    /// Clean up temporary files
    fn cleanup_temp_files(&self) -> SaveLoadResult<()> {
        if !self.temp_directory.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.temp_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "tmp" {
                    if let Err(e) = std::fs::remove_file(&path) {
                        log::warn!("Failed to remove temp file {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old auto save files
    fn cleanup_old_auto_saves(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        let auto_saves: Vec<_> = saves
            .into_iter()
            .filter(|s| s.save_info.save_type == SaveFileType::AutoSave)
            .collect();

        // Keep only the 5 most recent auto saves
        if auto_saves.len() > 5 {
            for old_save in &auto_saves[5..] {
                if let Err(e) = self.delete_save(&old_save.filename) {
                    log::warn!(
                        "Failed to delete old auto save {}: {}",
                        old_save.filename,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    fn enforce_save_limit(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        if saves.len() <= self.max_save_files {
            return Ok(());
        }

        for old_save in saves.iter().skip(self.max_save_files) {
            if let Err(e) = self.delete_save(&old_save.filename) {
                log::warn!(
                    "Failed to delete excess save {} while enforcing limit: {}",
                    old_save.filename,
                    e
                );
            }
        }

        Ok(())
    }
}

impl Default for SaveFileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod rejected_logical_candidate_tests {
    use super::*;
    use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
    #[test]
    fn terminal_logical_candidate_is_retained_and_blocks_load_before_decode() {
        let mut manager = SaveFileManager::with_save_directory("unused-terminal-candidate");
        let mut failed = GameLogic::new();
        failed.add_object(Object::new(
            ThingTemplate::new("FailedOwner"),
            ObjectId(7),
            Team::USA,
        ));
        failed.frame = 91;
        let mut peer = GameLogic::new();
        peer.add_object(Object::new(
            ThingTemplate::new("Peer"),
            ObjectId(7),
            Team::USA,
        ));
        manager.retain_rejected_logical_world(Box::new(failed));
        let error = manager.load_game("not-decoded", &mut peer).unwrap_err();
        assert!(error
            .to_string()
            .contains("terminal logical restore candidate"));
        assert_eq!(manager.rejected_logical_worlds.len(), 1);
        assert_eq!(manager.rejected_logical_worlds[0].frame, 91);
        assert!(manager.rejected_logical_worlds[0]
            .host_object(ObjectId(7))
            .is_some());
        assert!(peer.host_object(ObjectId(7)).is_some());
        assert_eq!(peer.frame, 0);
    }
}
