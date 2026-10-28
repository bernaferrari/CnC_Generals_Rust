//! Immutable receiving Body rules continuation. Framed after A10F and before
//! APRO. Each owner asks the next frame for its boundary; payloads are never scanned.
use crate::game_logic::GameLogic;
use crate::game_logic::body_damage_rules::{BodyDamagePolicy, BodyDamageRules};
use crate::save_load::{SaveLoadError, SaveLoadResult};
const MAGIC: &[u8; 4] = b"BDRF";
const VERSION: u32 = 1;
const PAYLOAD_BYTES: usize = 9;
fn corrupt(message: &str) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) {
    append_policy(bytes, world.body_damage_policy());
}
pub(super) fn append_policy(bytes: &mut Vec<u8>, policy: BodyDamagePolicy) {
    let (receiving, rules) = match policy {
        BodyDamagePolicy::StandaloneRetail => (0u8, BodyDamageRules::standalone_retail()),
        BodyDamagePolicy::Receiving(rules) => (1u8, rules),
    };
    let (damaged, really_damaged) = rules.thresholds();
    bytes.push(receiving);
    bytes.extend_from_slice(&damaged.to_le_bytes());
    bytes.extend_from_slice(&really_damaged.to_le_bytes());
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(PAYLOAD_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(MAGIC);
}
pub(super) fn prepare(
    bytes: &[u8],
    world_version: u32,
) -> SaveLoadResult<Option<BodyDamagePolicy>> {
    let (start, end) = frame(bytes)?;
    if start == end {
        if world_version >= 31 {
            return Err(corrupt("world31 requires receiving Body rules"));
        }
        // Versions23–30 did not save these rules. Their policy comes from the
        // receiving application/map content; custom continuation was not encoded.
        return Ok(None);
    }
    let payload = &bytes[start..end - 12];
    let damaged = f32::from_le_bytes(payload[1..5].try_into().unwrap());
    let really_damaged = f32::from_le_bytes(payload[5..9].try_into().unwrap());
    let rules = BodyDamageRules::from_thresholds(damaged, really_damaged)
        .map_err(|error| corrupt(&error))?;
    match payload[0] {
        0 if rules == BodyDamageRules::standalone_retail() => {
            Ok(Some(BodyDamagePolicy::StandaloneRetail))
        }
        1 => Ok(Some(BodyDamagePolicy::Receiving(rules))),
        _ => Err(corrupt("invalid Body rules policy provenance")),
    }
}
pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let (start, end) = frame(bytes)?;
    if start != end {
        prepare(bytes, 31)?;
    }
    Ok(start)
}
fn frame(bytes: &[u8]) -> SaveLoadResult<(usize, usize)> {
    let end = super::garrison_team_identity_persist::suffix_start(bytes)?;
    let prefix = bytes
        .get(..end)
        .ok_or_else(|| corrupt("Body rules boundary invalid"))?;
    if !prefix.ends_with(MAGIC) {
        return Ok((end, end));
    }
    let footer = end
        .checked_sub(12)
        .ok_or_else(|| corrupt("Body rules footer truncated"))?;
    let version = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap());
    let len = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
    if version != VERSION || len != PAYLOAD_BYTES {
        return Err(corrupt("invalid Body rules version/length"));
    }
    let start = footer
        .checked_sub(len)
        .ok_or_else(|| corrupt("Body rules payload truncated"))?;
    if bytes[..start].ends_with(MAGIC) {
        return Err(corrupt("duplicate Body rules frame"));
    }
    Ok((start, end))
}

#[cfg(test)]
#[path = "body_rules_persist_tests.rs"]
mod tests;
