//! Locate TMAI at real owner boundaries, never in an opaque string or u32 ID.
use super::*;
fn bad(message: &str) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("TMAI framing: {message}"))
}
fn word(bytes: &[u8], offset: &mut usize) -> SaveLoadResult<u32> {
    let end = offset.checked_add(4).ok_or_else(|| bad("word overflow"))?;
    let value = bytes
        .get(*offset..end)
        .ok_or_else(|| bad("truncated word"))?;
    *offset = end;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn advance(bytes: &[u8], offset: &mut usize, length: usize) -> SaveLoadResult<()> {
    *offset = offset
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| bad("truncated payload"))?;
    Ok(())
}
fn skip_spcd_table(bytes: &[u8], offset: &mut usize) -> SaveLoadResult<()> {
    let count = word(bytes, offset)? as usize;
    if count > 1 << 20 || count > bytes.len().saturating_sub(*offset) / 8 {
        return Err(bad("SPCD table count"));
    }
    for _ in 0..count {
        word(bytes, offset)?;
        let timers = word(bytes, offset)? as usize;
        if timers > 1 << 20 || timers > bytes.len().saturating_sub(*offset) / 8 {
            return Err(bad("SPCD timer count"));
        }
        for _ in 0..timers {
            let length = word(bytes, offset)? as usize;
            advance(bytes, offset, length)?;
            advance(bytes, offset, 4)?;
        }
    }
    Ok(())
}
fn known(tag: &[u8]) -> bool {
    matches!(
        tag,
        b"CPRJ"
            | b"BDKD"
            | b"NLFC"
            | b"ODIS"
            | b"RDCK"
            | b"OLST"
            | b"NDIE"
            | b"SPCD"
            | b"BPPL"
            | b"SUBD"
            | b"HSQD"
            | b"BTRY"
            | b"CBPD"
            | b"PDRP"
            | b"DZRP"
            | b"RHBH"
            | b"WSFL"
            | b"SABL"
            | b"TMAI"
    )
}
pub(super) fn find(bytes: &[u8]) -> SaveLoadResult<Option<&[u8]>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let mut offset = if bytes.get(..4).is_some_and(known) {
        0
    } else {
        super::super::combat_projectile_persist::prefix_len(bytes)?
    };
    while let Some(tag) = bytes.get(offset..offset.saturating_add(4)) {
        if !known(tag) {
            return Ok(None);
        }
        let start = offset;
        offset += 4;
        let version = word(bytes, &mut offset)?;
        if tag == b"SPCD" {
            if version == 0 || version > 2 {
                return Err(bad("unsupported SPCD predecessor"));
            }
            skip_spcd_table(bytes, &mut offset)?;
            skip_spcd_table(bytes, &mut offset)?;
            let has_pauses = version == 2
                || bytes
                    .get(offset..offset.saturating_add(4))
                    .is_some_and(|value| u32::from_le_bytes(value.try_into().unwrap()) <= 1 << 16);
            if has_pauses {
                skip_spcd_table(bytes, &mut offset)?;
            }
            continue;
        }
        let length = word(bytes, &mut offset)? as usize;
        advance(bytes, &mut offset, length)?;
        if tag == TMAI_MAGIC {
            if bytes.get(offset..offset.saturating_add(4)) == Some(TMAI_MAGIC.as_slice()) {
                return Err(bad("duplicate TMAI owner"));
            }
            return Ok(Some(&bytes[start + 4..offset]));
        }
    }
    Ok(None)
}
