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
fn known(tag: &[u8]) -> bool {
    matches!(
        tag,
        b"CPRJ"
            | b"PCOI"
            | b"GMSH"
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
    find_record(bytes, TMAI_MAGIC)
}

/// Walk the actual lifecycle owner framing. Names and opaque payloads may
/// contain a sibling magic; only the end of an owner record admits a tag.
pub(in crate::save_load::snapshot) fn find_record<'a>(
    bytes: &'a [u8],
    target: &[u8; 4],
) -> SaveLoadResult<Option<&'a [u8]>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let mut offset = if bytes.get(..4).is_some_and(known) {
        0
    } else {
        super::super::combat_projectile_persist::prefix_len(bytes)?
    };
    let mut found = None;
    while let Some(tag) = bytes.get(offset..offset.saturating_add(4)) {
        if !known(tag) {
            break;
        }
        let start = offset;
        if tag == b"SPCD" {
            let length = super::super::special_power_cooldown_persist::record_len(&bytes[start..])?;
            advance(bytes, &mut offset, length)?;
        } else {
            offset += 4;
            word(bytes, &mut offset)?; // owner version; its decoder validates policy
            let length = word(bytes, &mut offset)? as usize;
            advance(bytes, &mut offset, length)?;
        }
        if tag == target {
            if found.is_some() {
                return Err(bad("duplicate lifecycle owner"));
            }
            found = Some(&bytes[start + 4..offset]);
        }
    }
    Ok(found)
}
