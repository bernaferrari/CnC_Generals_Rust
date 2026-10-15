//! Native camera continuation at the structural lifecycle boundary. Original
//! View v1 and native pose v18 remain byte compatible. Hex keeps arbitrary
//! owner payloads from masquerading as historical uppercase suffix tags.
use crate::cnc_game_engine::{CnCGameEngine, NativeCameraContinuation};
use crate::game_logic::{GameLogic, LogicCameraContinuation};
use crate::save_load::{SaveLoadError, SaveLoadResult};

const TAG: &[u8; 4] = b"CAMC";
const VERSION: u32 = 1;
const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

pub(super) struct Decoded {
    pub logic: LogicCameraContinuation,
    pub native: Option<NativeCameraContinuation>,
}
struct Record {
    start: usize,
    end: usize,
    logic: Vec<u8>,
    native: Option<Vec<u8>>,
}
fn invalid(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("camera continuation: {}", message.into()))
}
fn word(bytes: &[u8], cursor: &mut usize) -> SaveLoadResult<usize> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| invalid("word overflow"))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| invalid("truncated word"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(value.try_into().unwrap()) as usize)
}
fn record(bytes: &[u8]) -> SaveLoadResult<Option<Record>> {
    let start = super::combat_projectile_persist::historical_prefix_len(bytes)?;
    if !bytes[start..].starts_with(TAG) {
        return Ok(None);
    }
    let mut cursor = start + 4;
    if word(bytes, &mut cursor)? != VERSION as usize {
        return Err(invalid("unsupported version"));
    }
    let size = word(bytes, &mut cursor)?;
    if size > MAX_PAYLOAD * 2 || size % 2 != 0 {
        return Err(invalid("invalid payload length"));
    }
    let end = cursor
        .checked_add(size)
        .ok_or_else(|| invalid("length overflow"))?;
    let encoded = bytes
        .get(cursor..end)
        .ok_or_else(|| invalid("truncated payload"))?;
    let nibble = |v| match v {
        b'0'..=b'9' => Ok(v - b'0'),
        b'a'..=b'f' => Ok(v - b'a' + 10),
        _ => Err(invalid("invalid hex payload")),
    };
    let mut payload = Vec::with_capacity(size / 2);
    for pair in encoded.chunks_exact(2) {
        payload.push(nibble(pair[0])? * 16 + nibble(pair[1])?);
    }
    let mut offset = 0;
    let logic_len = word(&payload, &mut offset)?;
    let logic_end = offset
        .checked_add(logic_len)
        .ok_or_else(|| invalid("logic length overflow"))?;
    let logic = payload
        .get(offset..logic_end)
        .ok_or_else(|| invalid("truncated logic"))?
        .to_vec();
    offset = logic_end;
    let native_len = word(&payload, &mut offset)?;
    let native_end = offset
        .checked_add(native_len)
        .ok_or_else(|| invalid("native length overflow"))?;
    let native = payload
        .get(offset..native_end)
        .ok_or_else(|| invalid("truncated native state"))?;
    if native_end != payload.len() {
        return Err(invalid("trailing payload"));
    }
    if bytes[end..].starts_with(TAG) {
        return Err(invalid("duplicate record"));
    }
    Ok(Some(Record {
        start,
        end,
        logic,
        native: (!native.is_empty()).then(|| native.to_vec()),
    }))
}
pub(super) fn prefix_len(bytes: &[u8]) -> SaveLoadResult<usize> {
    Ok(match record(bytes)? {
        Some(record) => record.end,
        None => super::combat_projectile_persist::historical_prefix_len(bytes)?,
    })
}
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Option<Decoded>> {
    record(bytes)?
        .map(|record| {
            Ok(Decoded {
                logic: GameLogic::decode_camera_continuation(&record.logic).map_err(invalid)?,
                native: record
                    .native
                    .as_deref()
                    .map(CnCGameEngine::decode_camera_continuation)
                    .transpose()
                    .map_err(invalid)?,
            })
        })
        .transpose()
}
fn frame(logic: &[u8], native: Option<&[u8]>) -> SaveLoadResult<Vec<u8>> {
    let native = native.unwrap_or_default();
    let size = logic
        .len()
        .checked_add(native.len())
        .and_then(|v| v.checked_add(8))
        .ok_or_else(|| invalid("payload overflow"))?;
    if size > MAX_PAYLOAD {
        return Err(invalid("payload exceeds limit"));
    }
    let mut payload = Vec::with_capacity(size);
    payload.extend_from_slice(&(logic.len() as u32).to_le_bytes());
    payload.extend_from_slice(logic);
    payload.extend_from_slice(&(native.len() as u32).to_le_bytes());
    payload.extend_from_slice(native);
    let mut bytes = Vec::with_capacity(size * 2 + 12);
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&((size * 2) as u32).to_le_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for value in payload {
        bytes.push(HEX[(value >> 4) as usize]);
        bytes.push(HEX[(value & 15) as usize]);
    }
    Ok(bytes)
}
pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let logic = world.capture_camera_continuation().map_err(invalid)?;
    let boundary = super::combat_projectile_persist::historical_prefix_len(bytes)?;
    if boundary != bytes.len() {
        return Err(invalid("capture requires historical prefix"));
    }
    bytes.extend(frame(&logic, None)?);
    Ok(())
}
pub(crate) fn write_native_camera_continuation(
    bytes: &mut Vec<u8>,
    native: &[u8],
) -> SaveLoadResult<()> {
    CnCGameEngine::decode_camera_continuation(native).map_err(invalid)?;
    let record = record(bytes)?.ok_or_else(|| invalid("missing logic record"))?;
    GameLogic::decode_camera_continuation(&record.logic).map_err(invalid)?;
    let replacement = frame(&record.logic, Some(native))?;
    bytes.splice(record.start..record.end, replacement);
    Ok(())
}
pub(crate) fn read_native_camera_continuation(
    bytes: &[u8],
) -> SaveLoadResult<Option<NativeCameraContinuation>> {
    Ok(decode(bytes)?.and_then(|decoded| decoded.native))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prefix() -> Vec<u8> {
        super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default())
    }
    #[test]
    fn camera_continuation_framing_keeps_suffix_tags_and_owner_bytes_distinct() {
        let mut bytes = prefix();
        let start = bytes.len();
        bytes.extend(frame(b"HSQDCPRJPTSC", Some(b"CAMC")).unwrap());
        let end = bytes.len();
        bytes.extend_from_slice(b"CPRJremaining terminal capsules");
        let decoded = record(&bytes).unwrap().unwrap();
        assert_eq!(decoded.logic, b"HSQDCPRJPTSC");
        assert_eq!(decoded.native.as_deref(), Some(b"CAMC".as_slice()));
        assert_eq!((decoded.start, decoded.end), (start, end));
        assert_eq!(prefix_len(&bytes).unwrap(), end);
        assert!(!bytes[start + 12..end].windows(4).any(|s| s == b"HSQD"));
        assert_eq!(&bytes[end..], b"CPRJremaining terminal capsules");
    }
    #[test]
    fn camera_continuation_framing_rejects_truncation_versions_duplicates_and_limits() {
        let initial = prefix();
        let header = initial.len();
        let mut good = initial.clone();
        good.extend(frame(b"logic", None).unwrap());
        for cut in header + 4..good.len() {
            assert!(record(&good[..cut]).is_err(), "cut={cut}");
        }
        let mut bad = good.clone();
        bad[header + 4..header + 8].copy_from_slice(&99u32.to_le_bytes());
        assert!(record(&bad).is_err());
        let mut bad = good.clone();
        bad[header + 8..header + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(record(&bad).is_err());
        let mut bad = good.clone();
        bad[header + 12] = b'G';
        assert!(record(&bad).is_err());
        let mut bad = good.clone();
        bad.extend_from_slice(&good[header..]);
        assert!(record(&bad).is_err());
        assert!(decode(&good).is_err()); // structural framing alone cannot admit an invalid owner.
        assert_eq!(prefix_len(&initial).unwrap(), header);
    }
    #[test]
    fn camera_continuation_invalid_candidate_preserves_live_owner() {
        let mut source = GameLogic::new();
        source.queue_pending_camera_zoom(0.75, 3.0);
        let mut snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        let start = super::super::combat_projectile_persist::historical_prefix_len(
            &snapshot.lifecycle_tail,
        )
        .unwrap();
        snapshot.lifecycle_tail[start + 4..start + 8].copy_from_slice(&99u32.to_le_bytes());
        let mut receiver = GameLogic::new();
        receiver.queue_pending_camera_zoom(1.5, 7.0);
        let before = receiver.capture_camera_continuation().unwrap();
        assert!(
            super::super::SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut receiver)
                .is_err()
        );
        assert_eq!(receiver.capture_camera_continuation().unwrap(), before);
    }
    #[test]
    fn camera_continuation_survives_direct_world_bincode_without_new_layout_fields() {
        let world = GameLogic::new();
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap();
        let expected = world.capture_camera_continuation().unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot: super::super::WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        assert_eq!(
            record(&snapshot.lifecycle_tail).unwrap().unwrap().logic,
            expected
        );
        let mut restored = GameLogic::new();
        super::super::SnapshotBuilder::new()
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        assert_eq!(restored.capture_camera_continuation().unwrap(), expected);
    }
}
