//! Host `.sav` companion for leftover `CHUNK_GameClient`.
//!
//! C++ `GameClient::xfer` (`GameClient.cpp:1338-1563`) writes objectless
//! drawables with `objectID = INVALID_ID` and recreates them via
//! `TheThingFactory->newDrawable` on load. Live save used a NullSnapshot
//! placeholder, so PUC beams / lock-on / ropes never came back.

use crate::save_load::{SaveLoadError, SaveLoadResult};

pub const CHUNK_GAME_CLIENT: &str = "CHUNK_GameClient";

pub use game_client::core::game_client::GameClientArchive;

pub fn capture_game_client_archive(
    client: &mut game_client::core::game_client::GameClient,
) -> SaveLoadResult<GameClientArchive> {
    client
        .capture_archive()
        .map_err(SaveLoadError::Serialization)
}

/// Rust owner-managed auxiliaries are persisted by their module's template continuation.
#[cfg(feature = "game_client")]
pub(crate) fn capture_game_client_archive_excluding_owned(
    client: &mut game_client::core::game_client::GameClient,
    owned: &[game_client::core::game_client::NativeAuxiliaryReceipt],
) -> SaveLoadResult<GameClientArchive> {
    client
        .capture_archive_with_owned_auxiliaries(owned)
        .map_err(SaveLoadError::Serialization)
}

/// Validate the typed companion before a staged world can commit. Nested
/// module payload validity remains owned by GameClient's post-commit native
/// reconstruction, exactly as in the positional implementation.
pub fn validate_game_client_archive(archive: &GameClientArchive) -> SaveLoadResult<()> {
    archive
        .validate()
        .map_err(|error| SaveLoadError::Serialization(format!("invalid CHUNK_GameClient: {error}")))
}

pub fn restore_game_client_from_archive(
    client: &mut game_client::core::game_client::GameClient,
    archive: &GameClientArchive,
) -> SaveLoadResult<()> {
    client
        .restore_from_archive(archive)
        .map_err(SaveLoadError::Serialization)
}

pub fn restore_objectless_from_client_drawables(
    visual_world: &gamelogic::helpers::ClientVisualHandle,
    snapshot: &super::ClientDrawableWorldSnapshot,
) {
    for drawable in &snapshot.drawables {
        if drawable.object_id != 0 {
            continue;
        }
        let template = drawable.source_template_name.trim();
        if template.is_empty() || drawable.draw_module_index == 0 {
            continue;
        }
        visual_world.restore_objectless_drawable(
            drawable.draw_module_index,
            &gamelogic::helpers::DrawableState {
                template_name: template.to_string(),
                indicator_color: gamelogic::common::Color::default(),
                position: glam::Vec3::ZERO,
                orientation: 0.0,
                shroud_status_object_id: gamelogic::common::types::INVALID_ID,
                beam_start: None,
                beam_end: None,
                beam_width: None,
                laser_growth_frames: None,
                laser_growth_start_frame: None,
                projectile_stream: None,
                drawable: None,
                expiration_frame: None,
            },
        );
    }
}
