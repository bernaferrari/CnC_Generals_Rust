//! Add owned filter/focus state without changing the original positional record.
use super::*;
// A historical record starts with Option<Vec3>'s 0/1 discriminant. FF cannot
// prefix a valid old record, so malformed envelopes never fall back to legacy.
const MAGIC: &[u8; 4] = b"\xffLCF";
const VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
struct Envelope {
    legacy_record: Vec<u8>,
    // Opaque bounded filter record keeps envelope shape stable without Client.
    filter: Option<Vec<u8>>,
    pending_focus_previous_target: Option<Vec3>,
}
pub(super) fn encode(state: &LogicCameraContinuation) -> Result<Vec<u8>, String> {
    let envelope = Envelope {
        legacy_record: codec().serialize(state)?,
        #[cfg(feature = "game_client")]
        filter: Some(codec().serialize(&state.owned_extras.filter)?),
        #[cfg(not(feature = "game_client"))]
        filter: None,
        pending_focus_previous_target: state.owned_extras.pending_focus_previous_target,
    };
    let payload = codec().serialize(&envelope)?;
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&payload);
    if bytes.len() > MAX_BYTES {
        return Err("camera continuation exceeds 4 MiB".into());
    }
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8]) -> Result<LogicCameraContinuation, String> {
    if bytes.first() != Some(&0xff) {
        return codec().deserialize(bytes);
    }
    if bytes.len() < 8 || &bytes[..4] != MAGIC {
        return Err("invalid logic camera owner envelope".into());
    }
    if u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != VERSION {
        return Err("unknown logic camera owner envelope version".into());
    }
    let envelope: Envelope = codec().deserialize(&bytes[8..])?;
    let mut state: LogicCameraContinuation = codec().deserialize(&envelope.legacy_record)?;
    state.owned_extras.pending_focus_previous_target = envelope.pending_focus_previous_target;
    #[cfg(feature = "game_client")]
    {
        state.owned_extras.filter = match envelope.filter {
            Some(bytes) => codec().deserialize(&bytes)?,
            None => Default::default(),
        };
    }
    #[cfg(not(feature = "game_client"))]
    if envelope.filter.is_some() {
        return Err("saved camera filter requires the GameClient feature".into());
    }
    Ok(state)
}
