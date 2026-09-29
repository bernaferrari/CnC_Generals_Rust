//! Host `.sav` companion for leftover `CHUNK_GameClient`.
//!
//! C++ `GameClient::xfer` (`GameClient.cpp:1338-1563`) writes objectless
//! drawables with `objectID = INVALID_ID` and recreates them via
//! `TheThingFactory->newDrawable` on load. Live save used a NullSnapshot
//! placeholder, so PUC beams / lock-on / ropes never came back.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::io::{Cursor, Read};
use std::collections::HashSet;
use std::sync::Mutex;

pub const CHUNK_GAME_CLIENT: &str = "CHUNK_GameClient";

static PENDING_GAME_CLIENT_XFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn capture_game_client_xfer_bytes(
    client: &mut game_client::core::game_client::GameClient,
) -> SaveLoadResult<Vec<u8>> {
    client.capture_xfer_bytes().map_err(SaveLoadError::Serialization)
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

/// Check the C++ v3 GameClient envelope before a staged world can commit.
/// Drawable payloads are length-delimited and remain decoded by GameClient
/// after commit; this rejects malformed versions, counts, and truncation
/// without constructing presentation objects or touching the live client.
pub fn validate_game_client_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    fn invalid(what: &str) -> SaveLoadError {
        SaveLoadError::Serialization(format!("invalid CHUNK_GameClient: {what}"))
    }
    fn read_array<const N: usize>(cursor: &mut Cursor<&[u8]>) -> SaveLoadResult<[u8; N]> {
        let mut buf = [0u8; N];
        cursor
            .read_exact(&mut buf)
            .map_err(|_| invalid("truncated payload"))?;
        Ok(buf)
    }
    fn skip(cursor: &mut Cursor<&[u8]>, len: usize) -> SaveLoadResult<()> {
        let end = (cursor.position() as usize)
            .checked_add(len)
            .ok_or_else(|| invalid("length overflow"))?;
        if end > cursor.get_ref().len() {
            return Err(invalid("truncated payload"));
        }
        cursor.set_position(end as u64);
        Ok(())
    }

    let mut cursor = Cursor::new(bytes);
    if read_array::<1>(&mut cursor)?[0] != 3 {
        return Err(invalid("unsupported GameClient version"));
    }
    let _frame = u32::from_le_bytes(read_array::<4>(&mut cursor)?);
    if read_array::<1>(&mut cursor)?[0] != 1 {
        return Err(invalid("unsupported Drawable TOC version"));
    }
    let toc_count = u32::from_le_bytes(read_array::<4>(&mut cursor)?) as usize;
    let remaining = bytes.len() - cursor.position() as usize;
    if toc_count > remaining / 3 {
        return Err(invalid("Drawable TOC count exceeds payload"));
    }
    let mut toc_ids = HashSet::with_capacity(toc_count);
    for _ in 0..toc_count {
        let name_len = read_array::<1>(&mut cursor)?[0] as usize;
        skip(&mut cursor, name_len)?;
        let id = u16::from_le_bytes(read_array::<2>(&mut cursor)?);
        if !toc_ids.insert(id) {
            return Err(invalid("duplicate Drawable TOC id"));
        }
    }
    let drawable_count = u16::from_le_bytes(read_array::<2>(&mut cursor)?) as usize;
    let remaining = bytes.len() - cursor.position() as usize;
    if drawable_count > remaining / 6 {
        return Err(invalid("Drawable count exceeds payload"));
    }
    for _ in 0..drawable_count {
        let toc_id = u16::from_le_bytes(read_array::<2>(&mut cursor)?);
        if !toc_ids.contains(&toc_id) {
            return Err(invalid("Drawable references missing TOC id"));
        }
        let block_len = i32::from_le_bytes(read_array::<4>(&mut cursor)?);
        if block_len < 4 {
            return Err(invalid("Drawable block is shorter than object ID"));
        }
        skip(&mut cursor, block_len as usize)?;
    }
    let briefing_count = i32::from_le_bytes(read_array::<4>(&mut cursor)?);
    if briefing_count < 0 || briefing_count as usize > bytes.len() - cursor.position() as usize {
        return Err(invalid("briefing count exceeds payload"));
    }
    for _ in 0..briefing_count {
        let text_len = read_array::<1>(&mut cursor)?[0] as usize;
        skip(&mut cursor, text_len)?;
    }
    if cursor.position() as usize != bytes.len() {
        return Err(invalid("trailing bytes"));
    }
    Ok(())
}

pub fn restore_game_client_from_xfer_bytes(
    client: &mut game_client::core::game_client::GameClient,
    bytes: &[u8],
) -> SaveLoadResult<()> {
    client
        .restore_from_xfer_bytes(bytes)
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

#[cfg(test)]
mod client_xfer_tests {
    use super::*;
    use game_client::drawable::Drawable;

    #[test]
    fn explicit_client_xfer_round_trips_and_preflight_rejects_truncation() {
        let mut source = game_client::core::game_client::GameClient::new().expect("source");
        source.set_frame(41);
        let bytes = capture_game_client_xfer_bytes(&mut source).expect("save client");
        validate_game_client_xfer_bytes(&bytes).expect("valid client envelope");

        let mut restored = game_client::core::game_client::GameClient::new().expect("target");
        restored.set_frame(777);
        restore_game_client_from_xfer_bytes(&mut restored, &bytes).expect("restore client");
        assert_eq!(restored.get_frame(), 41);

        for cut in 0..bytes.len() {
            assert!(
                validate_game_client_xfer_bytes(&bytes[..cut]).is_err(),
                "truncated client payload at byte {cut} must fail before commit"
            );
        }
        let mut wrong_version = bytes.clone();
        wrong_version[0] = 99;
        assert!(validate_game_client_xfer_bytes(&wrong_version).is_err());
        let mut impossible_toc = bytes.clone();
        impossible_toc[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_game_client_xfer_bytes(&impossible_toc).is_err());

        // Exercise the length-delimited Drawable block as well as the empty
        // client envelope; live saves commonly contain many such blocks.
        let mut populated = game_client::core::game_client::GameClient::new()
            .expect("client with drawable");
        let mut drawable = game_client::drawable::BasicDrawable::new(
            game_client::drawable::DrawableId::INVALID,
        );
        drawable.set_object_id(Some(73));
        populated
            .register_drawable_with_template(Box::new(drawable), Some("XferTest".into()))
            .expect("register saved drawable");
        let populated_bytes = capture_game_client_xfer_bytes(&mut populated)
            .expect("save populated client");
        validate_game_client_xfer_bytes(&populated_bytes)
            .expect("valid Drawable block envelope");
    }
}
