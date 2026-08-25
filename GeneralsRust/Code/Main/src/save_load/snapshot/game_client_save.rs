//! Host `.sav` companion for leftover `CHUNK_GameClient`.
//!
//! C++ `GameClient::xfer` (`GameClient.cpp:1338-1563`) writes objectless
//! drawables with `objectID = INVALID_ID` and recreates them via
//! `TheThingFactory->newDrawable` on load. Live save used a NullSnapshot
//! placeholder, so PUC beams / lock-on / ropes never came back.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::sync::Mutex;

pub const CHUNK_GAME_CLIENT: &str = "CHUNK_GameClient";

static PENDING_GAME_CLIENT_XFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn capture_game_client_xfer_bytes() -> SaveLoadResult<Vec<u8>> {
    game_client::core::capture_live_game_client_xfer_bytes().map_err(SaveLoadError::Serialization)
}

pub fn stash_loaded_game_client_xfer(bytes: Vec<u8>) {
    if let Ok(mut slot) = PENDING_GAME_CLIENT_XFER.lock() {
        *slot = Some(bytes);
    }
}

pub fn take_loaded_game_client_xfer() -> Option<Vec<u8>> {
    PENDING_GAME_CLIENT_XFER
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub fn restore_game_client_from_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    game_client::core::restore_live_game_client_from_xfer_bytes(bytes)
        .map_err(SaveLoadError::Serialization)
}

pub fn restore_objectless_from_client_drawables(snapshot: &super::ClientDrawableWorldSnapshot) {
    let Some(client) = gamelogic::helpers::TheGameClient::get() else {
        return;
    };
    for drawable in &snapshot.drawables {
        if drawable.object_id != 0 {
            continue;
        }
        let template = drawable.source_template_name.trim();
        if template.is_empty() || drawable.draw_module_index == 0 {
            continue;
        }
        client.restore_objectless_drawable(
            drawable.draw_module_index,
            &gamelogic::helpers::DrawableState {
                template_name: template.to_string(),
                indicator_color: gamelogic::common::Color::default(),
                position: gamelogic::common::Coord3D::ZERO,
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
