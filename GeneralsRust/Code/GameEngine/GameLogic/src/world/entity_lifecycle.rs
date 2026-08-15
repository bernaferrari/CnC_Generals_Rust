//! Shared Entity↔Main Object lifecycle/state envelope.
//!
//! Wire format mirrors C++ `Object::xfer` v9 module blocks
//! (`Object.cpp:4264-4356`): `UnsignedShort` count, per module a tag string,
//! then a length-delimited payload (`beginBlock`/`dataSize`). Unknown tags are
//! skipped by consuming `payload_len` bytes so the stream offset stays valid.
//! Cached C++ interfaces (body/contain/AI/physics) are not in this envelope —
//! they are reconstructed by ctor, same as retail.
//!
//! Destroy timing follows `GameLogic::destroyObject` (`GameLogic.cpp:3932-3967`):
//! mark destroyed and record the frame; removal happens later on the destroy list.

use std::fmt;

/// Current envelope schema version. `0` / absent bytes decode as default.
pub const ENTITY_LIFECYCLE_ENVELOPE_VERSION: u8 = 1;

/// One ordered module-tag + length-delimited payload (C++ xfer v9 block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityModuleState {
    pub tag: String,
    pub payload: Vec<u8>,
}

/// Versioned Entity↔Main Object lifecycle envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityLifecycleEnvelope {
    pub version: u8,
    pub entity_id: u32,
    pub destroyed: bool,
    pub destroyed_at_frame: u32,
    pub module_states: Vec<EntityModuleState>,
}

impl Default for EntityLifecycleEnvelope {
    fn default() -> Self {
        Self {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: 0,
            destroyed: false,
            destroyed_at_frame: 0,
            module_states: Vec::new(),
        }
    }
}

/// Codec failure. Truncation is an error, never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityLifecycleCodecError {
    UnexpectedEof { context: &'static str },
    InvalidUtf8,
}

impl fmt::Display for EntityLifecycleCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { context } => {
                write!(f, "truncated entity lifecycle envelope at {context}")
            }
            Self::InvalidUtf8 => write!(f, "module tag is not valid UTF-8"),
        }
    }
}

impl std::error::Error for EntityLifecycleCodecError {}

pub type EntityLifecycleCodecResult<T> = Result<T, EntityLifecycleCodecError>;

impl EntityLifecycleEnvelope {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.version);
        out.extend_from_slice(&self.entity_id.to_le_bytes());
        out.push(u8::from(self.destroyed));
        out.extend_from_slice(&self.destroyed_at_frame.to_le_bytes());
        let count = u16::try_from(self.module_states.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&count.to_le_bytes());
        for module in self.module_states.iter().take(count as usize) {
            encode_tag(&mut out, &module.tag);
            let len = u32::try_from(module.payload.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&len.to_le_bytes());
            let take = len as usize;
            out.extend_from_slice(&module.payload[..take]);
        }
        out
    }

    /// Decode a stream. Empty / version-0-absent bytes yield the default envelope.
    pub fn decode(bytes: &[u8]) -> EntityLifecycleCodecResult<Self> {
        if bytes.is_empty() || bytes[0] == 0 {
            return Ok(Self::default());
        }
        let mut cur = Cursor::new(bytes);
        let version = cur.read_u8("version")?;
        let entity_id = cur.read_u32("entity_id")?;
        let destroyed = cur.read_u8("destroyed")? != 0;
        let destroyed_at_frame = cur.read_u32("destroyed_at_frame")?;
        let count = cur.read_u16("module_count")?;
        let mut module_states = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let tag = cur.read_tag()?;
            let payload_len = cur.read_u32("payload_len")?;
            let payload = cur.read_exact(payload_len as usize, "payload")?;
            module_states.push(EntityModuleState { tag, payload });
        }
        Ok(Self {
            version,
            entity_id,
            destroyed,
            destroyed_at_frame,
            module_states,
        })
    }
}

struct Cursor<'a> {
    rest: &'a [u8],
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    fn read_exact(
        &mut self,
        n: usize,
        context: &'static str,
    ) -> EntityLifecycleCodecResult<Vec<u8>> {
        if self.rest.len() < n {
            return Err(EntityLifecycleCodecError::UnexpectedEof { context });
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Ok(head.to_vec())
    }

    fn read_u8(&mut self, context: &'static str) -> EntityLifecycleCodecResult<u8> {
        let bytes = self.read_exact(1, context)?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self, context: &'static str) -> EntityLifecycleCodecResult<u16> {
        let bytes = self.read_exact(2, context)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self, context: &'static str) -> EntityLifecycleCodecResult<u32> {
        let bytes = self.read_exact(4, context)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_tag(&mut self) -> EntityLifecycleCodecResult<String> {
        let len = self.read_u16("tag_len")?;
        let raw = self.read_exact(len as usize, "tag")?;
        String::from_utf8(raw).map_err(|_| EntityLifecycleCodecError::InvalidUtf8)
    }
}

fn encode_tag(out: &mut Vec<u8>, tag: &str) {
    let bytes = tag.as_bytes();
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&bytes[..len as usize]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_envelope() -> EntityLifecycleEnvelope {
        EntityLifecycleEnvelope {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: 42,
            destroyed: true,
            destroyed_at_frame: 17,
            module_states: vec![
                EntityModuleState {
                    tag: "SlowDeath".to_string(),
                    payload: vec![1, 2, 3],
                },
                EntityModuleState {
                    tag: "LifetimeUpdate".to_string(),
                    payload: vec![9],
                },
            ],
        }
    }

    #[test]
    fn encode_decode_roundtrip_preserves_header_and_modules() {
        let encoded = sample_envelope().encode();
        let decoded = EntityLifecycleEnvelope::decode(&encoded).expect("decode");
        assert_eq!(decoded, sample_envelope());
    }

    #[test]
    fn unknown_tag_keeps_offset_for_following_known_tag() {
        let env = EntityLifecycleEnvelope {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: 7,
            destroyed: false,
            destroyed_at_frame: 0,
            module_states: vec![
                EntityModuleState {
                    tag: "FutureUnknownModule".to_string(),
                    payload: vec![0xAA; 8],
                },
                EntityModuleState {
                    tag: "PoisonedBehavior".to_string(),
                    payload: vec![0x10, 0x20],
                },
            ],
        };
        let decoded = EntityLifecycleEnvelope::decode(&env.encode()).expect("decode");
        assert_eq!(decoded.module_states.len(), 2);
        assert_eq!(decoded.module_states[0].tag, "FutureUnknownModule");
        assert_eq!(decoded.module_states[0].payload.len(), 8);
        assert_eq!(decoded.module_states[1].tag, "PoisonedBehavior");
        assert_eq!(decoded.module_states[1].payload, vec![0x10, 0x20]);
    }

    #[test]
    fn truncated_payload_is_error_not_panic() {
        let mut bytes = sample_envelope().encode();
        bytes.pop();
        let err = EntityLifecycleEnvelope::decode(&bytes).expect_err("truncated");
        assert!(matches!(
            err,
            EntityLifecycleCodecError::UnexpectedEof { .. }
        ));
    }

    #[test]
    fn empty_or_v0_absent_decodes_as_default() {
        assert_eq!(
            EntityLifecycleEnvelope::decode(&[]).expect("empty"),
            EntityLifecycleEnvelope::default()
        );
        assert_eq!(
            EntityLifecycleEnvelope::decode(&[0]).expect("v0"),
            EntityLifecycleEnvelope::default()
        );
    }

    #[test]
    fn destroyed_timing_matches_deferred_destroy_mark_frame() {
        let env = sample_envelope();
        assert!(env.destroyed);
        assert_eq!(env.destroyed_at_frame, 17);
    }
}
