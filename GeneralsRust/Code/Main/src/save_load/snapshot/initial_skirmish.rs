//! One detached resolved start capsule outside the historical PTSC envelope.
//! No positional World/Player/Team records change. Hex prevents nested owner
//! payloads being mistaken for historical lifecycle tags.
use super::WorldSnapshot;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use crate::skirmish_config::saved_admission::SavedSkirmishAdmission;
const TAG: &[u8; 4] = b"SINI";
const LIMIT: usize = 1024 * 1024;
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) enum InitialAdmissionRecord {
    Native(SavedSkirmishAdmission),
    Original(game_engine::common::system::skirmish_game_info::SkirmishGameInfo),
}
impl InitialAdmissionRecord {
    fn validate(&self) -> SaveLoadResult<()> {
        match self {
            Self::Native(value) => value.validate().map_err(invalid),
            Self::Original(value) => value
                .encode()
                .map(|_| ())
                .map_err(|e| invalid(e.to_string())),
        }
    }
}

fn invalid(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

pub(super) fn split(bytes: &[u8]) -> SaveLoadResult<(&[u8], Option<InitialAdmissionRecord>)> {
    if !bytes.ends_with(TAG) {
        return Ok((bytes, None));
    }
    let footer = bytes
        .len()
        .checked_sub(8)
        .ok_or_else(|| invalid("truncated initial Skirmish admission footer"))?;
    let size = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap()) as usize;
    if size > LIMIT * 2 || size % 2 != 0 {
        return Err(invalid("invalid initial Skirmish admission length"));
    }
    let start = footer
        .checked_sub(size)
        .ok_or_else(|| invalid("truncated initial Skirmish admission"))?;
    let mut decoded = Vec::with_capacity(size / 2);
    for pair in bytes[start..footer].chunks_exact(2) {
        let digit = |n| match n {
            b'0'..=b'9' => Ok(n - b'0'),
            b'a'..=b'f' => Ok(n - b'a' + 10),
            _ => Err(invalid("invalid initial Skirmish admission hex")),
        };
        decoded.push((digit(pair[0])? << 4) | digit(pair[1])?);
    }
    let ((version, admission), consumed): ((u32, InitialAdmissionRecord), usize) =
        bincode::serde::decode_from_slice(
            &decoded,
            bincode::config::legacy().with_limit::<LIMIT>(),
        )
        .map_err(|e| invalid(format!("invalid initial Skirmish admission: {e}")))?;
    if consumed != decoded.len() {
        return Err(invalid("trailing initial Skirmish admission bytes"));
    }
    if version != 1 {
        return Err(invalid(format!(
            "unsupported initial Skirmish admission version {version}"
        )));
    }
    admission.validate()?;
    Ok((&bytes[..start], Some(admission)))
}

pub(crate) fn read(world: &WorldSnapshot) -> SaveLoadResult<Option<Box<SavedSkirmishAdmission>>> {
    Ok(match split(&world.lifecycle_tail)?.1 {
        Some(InitialAdmissionRecord::Native(value)) => Some(Box::new(value)),
        _ => None,
    })
}
/// The actual receiving admission boundary, never called by archive decoding.
pub(crate) fn read_for_restore(
    world: &WorldSnapshot,
) -> SaveLoadResult<Option<Box<SavedSkirmishAdmission>>> {
    match split(&world.lifecycle_tail)?.1 {
        Some(InitialAdmissionRecord::Native(value)) => Ok(Some(Box::new(value))),
        Some(InitialAdmissionRecord::Original(source)) => {
            SavedSkirmishAdmission::from_original(source)
                .map(|v| Some(Box::new(v)))
                .map_err(invalid)
        }
        None => Ok(None),
    }
}
pub(crate) fn source(
    world: &WorldSnapshot,
) -> SaveLoadResult<Option<game_engine::common::system::skirmish_game_info::SkirmishGameInfo>> {
    Ok(match split(&world.lifecycle_tail)?.1 {
        Some(InitialAdmissionRecord::Native(value)) => value.source,
        Some(InitialAdmissionRecord::Original(value)) => Some(value),
        None => None,
    })
}
pub(crate) fn append_original(
    bytes: &mut Vec<u8>,
    source: game_engine::common::system::skirmish_game_info::SkirmishGameInfo,
) -> SaveLoadResult<()> {
    append_record(bytes, &InitialAdmissionRecord::Original(source))
}
pub(crate) fn append(
    bytes: &mut Vec<u8>,
    admission: &SavedSkirmishAdmission,
) -> SaveLoadResult<()> {
    append_record(bytes, &InitialAdmissionRecord::Native(admission.clone()))
}
pub(super) fn append_record(
    bytes: &mut Vec<u8>,
    admission: &InitialAdmissionRecord,
) -> SaveLoadResult<()> {
    admission.validate()?;
    let encoded =
        bincode_legacy::serialize(&(1u32, admission)).map_err(|e| invalid(e.to_string()))?;
    if encoded.len() > LIMIT {
        return Err(invalid("initial Skirmish admission exceeds 1 MiB"));
    }
    let (prefix, _) = split(bytes)?;
    let mut next = prefix.to_vec();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in &encoded {
        next.push(HEX[(byte >> 4) as usize]);
        next.push(HEX[(byte & 15) as usize]);
    }
    next.extend_from_slice(&((encoded.len() * 2) as u32).to_le_bytes());
    next.extend_from_slice(TAG);
    *bytes = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_outer_capsule_rejects_without_replacing_existing_tail() {
        let original = b"unrelated exact lifecycle prefix".to_vec();
        for bytes in [
            b"SINI".to_vec(),
            b"01000000\xff\xff\xff\xffSINI".to_vec(),
            b"z002000000SINI".to_vec(),
        ] {
            assert!(split(&bytes).is_err());
        }
        let mut world = crate::game_logic::GameLogic::new();
        let source = game_engine::common::system::skirmish_game_info::SkirmishGameInfo::default();
        let mut bytes = original.clone();
        append_original(&mut bytes, source.clone()).unwrap();
        let (prefix, record) = split(&bytes).unwrap();
        assert_eq!(prefix, &original);
        assert!(matches!(record,Some(InitialAdmissionRecord::Original(value)) if value==source));
        let mut bad = source;
        bad.map_name = "x".repeat(256);
        let before = bytes.clone();
        assert!(append_original(&mut bytes, bad).is_err());
        assert_eq!(bytes, before);
        assert!(world.initial_skirmish_admission.is_none());
        world.clear_all_players();
        assert!(world.initial_skirmish_admission.is_none());
    }
}
