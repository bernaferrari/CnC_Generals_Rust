//! Typed Rust world codec for the identical version-23 through 27 positional body.
use super::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferMode};
// Decode through the actual bincode 2 adapter wire configuration. The local
// bincode_legacy Options setters do not enforce trailing-byte rejection.
fn decode_exact<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> SaveLoadResult<T> {
    let (value, consumed) =
        bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::legacy())
            .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
    if consumed != bytes.len() {
        return Err(SaveLoadError::Serialization(
            "Trailing bytes in current Rust snapshot payload".to_string(),
        ));
    }
    Ok(value)
}

pub(crate) fn decode_bincode_world_snapshot(payload: &[u8]) -> SaveLoadResult<WorldSnapshot> {
    // Decode only the fixed-integer little-endian u32 envelope first. Reject
    // unsupported versions before interpreting the positional world body.
    let (version, _): (u32, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy())
            .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
    validate_direct_world_snapshot_version(version)?;
    decode_exact(payload)
}

pub(super) fn xfer_pending_combat(
    xfer: &mut dyn Xfer,
    snapshot: &mut crate::game_logic::combat::PendingCombatSnapshot,
) -> SaveLoadResult<()> {
    let mut bytes = if xfer.get_mode() == XferMode::Load {
        Vec::new()
    } else {
        bincode_legacy::serialize(snapshot)
            .map_err(|error| SaveLoadError::Serialization(error.to_string()))?
    };
    let mut length = u32::try_from(bytes.len()).map_err(|_| {
        SaveLoadError::Serialization("Pending combat snapshot exceeds u32 framing".to_string())
    })?;
    xfer.xfer_u32(&mut length)?;
    if xfer.get_mode() == XferMode::Load {
        // Read incrementally so a corrupt length cannot allocate gigabytes
        // before the reader detects EOF. No arbitrary save-size ceiling.
        let mut remaining = length as usize;
        while remaining != 0 {
            let mut block = vec![0; remaining.min(4096)];
            xfer.xfer_raw(&mut block)?;
            remaining -= block.len();
            bytes.extend_from_slice(&block);
        }
        let decoded = decode_exact(&bytes)?;
        *snapshot = decoded;
    } else {
        xfer.xfer_raw(&mut bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::combat::PendingCombatSnapshot;
    use crate::save_load::{XferLoad, XferSave};
    use std::io::Cursor;

    #[test]
    fn typed_pending_combat_frame_rejects_bad_lengths_and_trailing_payload() {
        let expected = bincode_legacy::serialize(&PendingCombatSnapshot::default()).unwrap();
        let valid_len = u32::try_from(expected.len()).unwrap();
        for (length, payload) in [
            (0, Vec::new()),
            (valid_len - 1, expected[..expected.len() - 1].to_vec()),
            (valid_len + 1, [expected.clone(), vec![0x71]].concat()),
            (u32::MAX, expected.clone()),
        ] {
            let mut input = length.to_le_bytes().to_vec();
            input.extend_from_slice(&payload);
            let mut reader = XferLoad::new(Cursor::new(input));
            let mut destination = PendingCombatSnapshot::default();
            assert!(xfer_pending_combat(&mut reader, &mut destination).is_err());
            assert_eq!(bincode_legacy::serialize(&destination).unwrap(), expected);
        }
    }

    #[test]
    fn typed_pending_combat_frame_preserves_exact_following_record() {
        let mut input = PendingCombatSnapshot::default();
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = XferSave::new(&mut bytes);
            xfer_pending_combat(&mut writer, &mut input).unwrap();
            let mut following = 0xEC91_F07Du32;
            writer.xfer_u32(&mut following).unwrap();
        }
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        let mut destination = PendingCombatSnapshot::default();
        xfer_pending_combat(&mut reader, &mut destination).unwrap();
        let mut following = 0;
        reader.xfer_u32(&mut following).unwrap();
        assert_eq!(following, 0xEC91_F07D);
        assert_eq!(
            bincode_legacy::serialize(&destination).unwrap(),
            bincode_legacy::serialize(&input).unwrap()
        );
    }
}
