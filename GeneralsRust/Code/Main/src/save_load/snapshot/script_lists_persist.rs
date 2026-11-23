//! Native save frame for the receiving ScriptEngine's live side lists.
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use gamelogic::scripting::engine::SideScriptContinuation;

const MAGIC: &[u8; 4] = b"SLSR";
const VERSION: u32 = 1;
const MAX_BYTES: usize = 4 * 1024 * 1024;

fn invalid(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("side script continuation: {error}"))
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let engine = world
        .owned_script_engine
        .as_ref()
        .ok_or_else(|| invalid("missing script owner"))?;
    let saved = engine.capture_side_script_continuation().map_err(invalid)?;
    let payload =
        bincode::serde::encode_to_vec(&saved, bincode::config::legacy().with_limit::<MAX_BYTES>())
            .map_err(invalid)?;
    if payload.len() > MAX_BYTES {
        return Err(invalid("payload too large"));
    }
    // Historical lifecycle readers scan uppercase tags. Lowercase hex
    // keeps authored names and nested data outside that namespace.
    let encoded = hex::encode(payload);
    bytes.extend_from_slice(encoded.as_bytes());
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(MAGIC);
    Ok(())
}

fn split(bytes: &[u8]) -> SaveLoadResult<(&[u8], Option<&[u8]>)> {
    let bytes = super::player_team_chunks::lifecycle_prefix(bytes)?;
    if !bytes.ends_with(MAGIC) {
        return Ok((bytes, None));
    }
    let footer = bytes
        .len()
        .checked_sub(12)
        .ok_or_else(|| invalid("truncated footer"))?;
    let version = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
    if version != VERSION || length == 0 || length > MAX_BYTES * 2 || length % 2 != 0 {
        return Err(invalid("invalid version or payload length"));
    }
    let start = footer
        .checked_sub(length)
        .ok_or_else(|| invalid("truncated payload"))?;
    let (prefix, payload) = bytes[..footer].split_at(start);
    Ok((prefix, Some(payload)))
}

pub(super) fn lifecycle_prefix(bytes: &[u8]) -> SaveLoadResult<&[u8]> {
    Ok(split(bytes)?.0)
}

pub(super) fn prepare(
    bytes: &[u8],
    world: &GameLogic,
) -> SaveLoadResult<Option<SideScriptContinuation>> {
    let Some(payload) = split(bytes)?.1 else {
        return Ok(None);
    };
    let decoded = hex::decode(payload).map_err(invalid)?;
    let (saved, consumed) = bincode::serde::decode_from_slice(
        &decoded,
        bincode::config::legacy().with_limit::<MAX_BYTES>(),
    )
    .map_err(invalid)?;
    if consumed != decoded.len() {
        return Err(invalid("trailing payload bytes"));
    }
    let engine = world
        .owned_script_engine
        .as_ref()
        .ok_or_else(|| invalid("missing receiver"))?;
    engine
        .validate_side_script_continuation(&saved)
        .map_err(invalid)?;
    Ok(Some(saved))
}

pub(super) fn restore(saved: &SideScriptContinuation, world: &GameLogic) -> SaveLoadResult<()> {
    let engine = world
        .owned_script_engine
        .as_ref()
        .ok_or_else(|| invalid("missing receiver"))?;
    engine
        .restore_side_script_continuation(saved)
        .map_err(invalid)
}
