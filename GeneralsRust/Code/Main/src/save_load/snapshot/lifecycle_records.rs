//! Index the tagged records between the structural lifecycle prefix and footer owners.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(super) struct LifecycleRecord<'a> {
    tag: [u8; 4],
    offset: usize,
    bytes: &'a [u8],
    footer_len: usize,
}

impl<'a> LifecycleRecord<'a> {
    pub(super) fn range(self) -> Range<usize> {
        self.offset..self.offset + self.bytes.len() + self.footer_len
    }

    /// The bytes following the tag, including the record's version and header.
    pub(super) fn body(self) -> &'a [u8] {
        &self.bytes[4..]
    }

    pub(super) fn version(self) -> u32 {
        u32::from_le_bytes(self.bytes[4..8].try_into().unwrap())
    }

    /// Payload of a regular length-prefixed record.
    pub(super) fn payload(self) -> &'a [u8] {
        &self.bytes[12..]
    }

    pub(super) fn decode<T: DeserializeOwned>(self, version: u32) -> SaveLoadResult<T> {
        if self.version() != version {
            return Err(SaveLoadError::Corrupted(format!(
                "unknown {} suffix version {}",
                String::from_utf8_lossy(&self.tag),
                self.version(),
            )));
        }
        self.decode_payload()
    }

    pub(super) fn decode_payload<T: DeserializeOwned>(self) -> SaveLoadResult<T> {
        use bincode_legacy::Options;
        bincode_legacy::DefaultOptions::new()
            .deserialize(self.payload())
            .map_err(|error| corrupted(self.tag, error))
    }
}

#[derive(Default)]
pub(super) struct LifecycleRecords<'a> {
    by_tag: HashMap<[u8; 4], LifecycleRecord<'a>>,
}

impl<'a> LifecycleRecords<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> SaveLoadResult<Self> {
        let (mut start, end) = if bytes.get(..4).is_some_and(is_tagged_record) {
            // Detached module encoders can start directly with a tagged record.
            (0, bytes.len())
        } else {
            (
                super::combat_projectile_persist::prefix_len(bytes)?,
                super::named_unit_history_persist::suffix_start(bytes)?,
            )
        };
        let through_records = bytes.get(start..end).ok_or_else(|| {
            SaveLoadError::Corrupted("lifecycle record boundaries overlap".into())
        })?;
        let mut rest = through_records;
        let mut records = Self::default();
        while let Some(tag) = rest.get(..4).filter(|tag| is_tagged_record(tag)) {
            let tag: [u8; 4] = tag.try_into().unwrap();
            let (len, footer_len) = record_lengths(tag, rest)?;
            let (record, next) = rest.split_at(len + footer_len);
            let entry = LifecycleRecord {
                tag,
                offset: start,
                bytes: &record[..len],
                footer_len,
            };
            if records.by_tag.insert(tag, entry).is_some() {
                return Err(corrupted(tag, "duplicate lifecycle record"));
            }
            start += record.len();
            rest = next;
        }
        Ok(records)
    }

    pub(super) fn get(&self, tag: &[u8; 4]) -> Option<LifecycleRecord<'a>> {
        self.by_tag.get(tag).copied()
    }
}

fn record_lengths(tag: [u8; 4], rest: &[u8]) -> SaveLoadResult<(usize, usize)> {
    // SPCD has counted tables instead of the regular payload length.
    if &tag == b"SPCD" {
        let len = super::special_power_cooldown_persist::record_len(rest)?;
        return Ok((len, 0));
    }

    let header = rest
        .get(4..12)
        .ok_or_else(|| corrupted(tag, "truncated header"))?;
    let version = u32::from_le_bytes(header[..4].try_into().unwrap());
    let payload_len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    let len = 12usize
        .checked_add(payload_len)
        .filter(|len| *len <= rest.len())
        .ok_or_else(|| {
            SaveLoadError::Corrupted(format!(
                "lifecycle record payload truncated ({})",
                String::from_utf8_lossy(&tag),
            ))
        })?;
    if &tag != b"OXFR" || version < 9 {
        return Ok((len, 0));
    }

    // The OXFR locator belongs to the record and must be skipped by every reader.
    let footer = rest[len..]
        .get(..8)
        .ok_or_else(|| corrupted(tag, "truncated locator"))?;
    if footer[..4] != header[4..] || &footer[4..] != b"OXFE" {
        return Err(corrupted(tag, "invalid length or footer locator"));
    }
    Ok((len, footer.len()))
}

fn is_tagged_record(tag: &[u8]) -> bool {
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
            | b"OLYR"
            | b"NDIE"
            | b"SPCD"
            | b"PGNM"
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
            | b"DCKQ"
            | b"MSRT"
            | b"DPLS"
            | b"OXFR"
            | b"ADPS"
            | b"OCLT"
            | b"JTAI"
            | b"CHNK"
            | b"HKIN"
            | b"WHCR"
            | b"FSGM"
            | b"MCRT"
            | b"GPSG"
            | b"DSCR"
            | b"AMOB"
            | b"PPRD"
            | b"CLHA"
            | b"PDLS"
            | b"PJST"
            | b"TXEB"
            | b"BRBH"
            | b"OXOB"
            | b"INFR"
            | b"FWAL"
            | b"NMSD"
            | b"TRAI"
            | b"STLG"
            | b"WLCH"
            | b"SCKP"
            | b"GFPT"
            | b"SDEN"
            | b"AITQ"
    )
}

fn corrupted(tag: [u8; 4], message: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!(
        "{} lifecycle record: {message}",
        String::from_utf8_lossy(&tag)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append(bytes: &mut Vec<u8>, tag: &[u8; 4], version: u32, payload: &[u8]) {
        bytes.extend_from_slice(tag);
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
    }

    #[test]
    fn object_module_locator_preserves_the_following_record_boundary() {
        let mut bytes = Vec::new();
        append(&mut bytes, b"OXFR", 9, b"STLG");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(b"OXFE");
        let score_offset = bytes.len();
        append(&mut bytes, b"SCKP", 1, b"PPRD");
        let records = LifecycleRecords::parse(&bytes).unwrap();
        let module = records.get(b"OXFR").unwrap();
        assert_eq!(module.payload(), b"STLG");
        assert_eq!(module.range(), 0..score_offset);
        assert_eq!(records.get(b"SCKP").unwrap().payload(), b"PPRD");
        assert!(records.get(b"STLG").is_none());
        assert!(records.get(b"PPRD").is_none());

        bytes[score_offset - 1] = b'!';
        assert!(LifecycleRecords::parse(&bytes).is_err());
    }
}
