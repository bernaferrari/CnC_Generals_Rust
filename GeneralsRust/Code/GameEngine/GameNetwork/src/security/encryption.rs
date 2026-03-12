//! Authenticated encryption services used across the modernised networking stack.

use crate::error::{NetworkError, NetworkResult};
use parking_lot::RwLock;
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use std::collections::VecDeque;

const MAX_STORED_KEYS: usize = 4;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 4 + 1 + 1 + 8 + NONCE_LEN;
const MAGIC: &[u8; 4] = b"GNET";
const VERSION: u8 = 1;
const FLAG_ENCRYPTED: u8 = 0x01;

/// Ciphertext returned from the encryption provider.
#[derive(Debug, Clone)]
pub struct EncryptedPacket {
    /// Identifier of the key that produced this payload (0 denotes a session key).
    pub key_id: u64,
    /// Nonce used when sealing (AES-GCM requires 96 bits).
    pub nonce: [u8; NONCE_LEN],
    /// Ciphertext with authentication tag appended.
    pub payload: Vec<u8>,
}

impl EncryptedPacket {
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

#[derive(Debug, Clone)]
struct KeyMaterial {
    id: u64,
    key_bytes: [u8; 32],
}

#[derive(Debug, Clone, Default)]
pub struct EncryptionStats {
    pub active_key_id: u64,
    pub total_rotations: u64,
    pub cached_keys: usize,
}

struct KeyStore {
    active: KeyMaterial,
    previous: VecDeque<KeyMaterial>,
    rotations: u64,
    next_key_id: u64,
}

impl KeyStore {
    fn new(initial: KeyMaterial) -> Self {
        let next_id = initial.id + 1;
        Self {
            active: initial,
            previous: VecDeque::new(),
            rotations: 0,
            next_key_id: next_id,
        }
    }

    fn active(&self) -> KeyMaterial {
        self.active.clone()
    }

    fn find(&self, key_id: u64) -> Option<KeyMaterial> {
        if key_id == self.active.id {
            return Some(self.active.clone());
        }
        self.previous
            .iter()
            .find(|material| material.id == key_id)
            .cloned()
    }

    fn rotate_to(&mut self, material: KeyMaterial) {
        let old = std::mem::replace(&mut self.active, material);
        self.previous.push_front(old);
        while self.previous.len() > MAX_STORED_KEYS {
            self.previous.pop_back();
        }
        self.rotations += 1;
    }

    fn stats(&self) -> EncryptionStats {
        EncryptionStats {
            active_key_id: self.active.id,
            total_rotations: self.rotations,
            cached_keys: self.previous.len() + 1,
        }
    }
}

/// Authenticated encryption provider backed by AES-256-GCM.
pub struct EncryptionProvider {
    keys: RwLock<KeyStore>,
    rng: SystemRandom,
}

impl EncryptionProvider {
    /// Create a new provider with a freshly generated key.
    pub fn new() -> NetworkResult<Self> {
        let rng = SystemRandom::new();
        let initial_material = KeyMaterial {
            id: 1,
            key_bytes: generate_key_bytes(&rng)?,
        };

        Ok(Self {
            keys: RwLock::new(KeyStore::new(initial_material)),
            rng,
        })
    }

    /// Encrypt `plaintext`, optionally using a provided session key.
    pub async fn encrypt(
        &self,
        plaintext: &[u8],
        session_key: Option<[u8; 32]>,
    ) -> NetworkResult<EncryptedPacket> {
        let (key_id, key_bytes) = if let Some(session_key) = session_key {
            (0, session_key)
        } else {
            let store = self.keys.read();
            let material = store.active();
            (material.id, material.key_bytes)
        };

        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| NetworkError::security("failed to generate nonce"))?;

        let mut buffer = plaintext.to_vec();
        buffer.reserve(aead::AES_256_GCM.tag_len());

        let key = build_less_safe_key(&key_bytes)?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let stored_nonce = *nonce.as_ref();
        key.seal_in_place_append_tag(nonce, Aad::empty(), &mut buffer)
            .map_err(|_| NetworkError::security("failed to encrypt payload"))?;

        Ok(EncryptedPacket {
            key_id,
            nonce: stored_nonce,
            payload: buffer,
        })
    }

    /// Decrypt a packet using the provider-managed key cache.
    pub async fn decrypt(&self, packet: &EncryptedPacket) -> NetworkResult<Vec<u8>> {
        if packet.payload.len() < aead::AES_256_GCM.tag_len() {
            return Err(NetworkError::security("ciphertext too short"));
        }

        let key_bytes = {
            if packet.key_id == 0 {
                return Err(NetworkError::security(
                    "session key required to decrypt this packet",
                ));
            }

            let store = self.keys.read();
            store
                .find(packet.key_id)
                .map(|material| material.key_bytes)
                .ok_or_else(|| NetworkError::security("unknown encryption key id"))?
        };

        decrypt_with_key(&packet.payload, packet.nonce, &key_bytes)
    }

    /// Decrypt a packet that was sealed with an explicit session key.
    pub async fn decrypt_with_session(
        &self,
        packet: &EncryptedPacket,
        session_key: &[u8; 32],
    ) -> NetworkResult<Vec<u8>> {
        decrypt_with_key(&packet.payload, packet.nonce, session_key)
    }

    /// Force a rotation of the active key, retaining a small cache for in-flight packets.
    pub fn force_key_rotation(&self) -> NetworkResult<()> {
        let mut store = self.keys.write();
        let new_material = KeyMaterial {
            id: store.next_key_id,
            key_bytes: generate_key_bytes(&self.rng)?,
        };
        store.next_key_id += 1;
        store.rotate_to(new_material);
        Ok(())
    }

    /// Fetch simple statistics about the key cache.
    pub async fn get_stats(&self) -> EncryptionStats {
        let store = self.keys.read();
        store.stats()
    }
}

impl Default for EncryptionProvider {
    fn default() -> Self {
        Self::new().expect("encryption provider initialisation")
    }
}

/// Envelope decoded from the wire format.
pub enum Envelope<'a> {
    Plain(&'a [u8]),
    Encrypted {
        key_id: u64,
        nonce: [u8; NONCE_LEN],
        payload: &'a [u8],
    },
}

/// Encode a plaintext payload without encryption.
pub fn encode_plain_envelope(payload: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(HEADER_LEN + payload.len());
    buffer.extend_from_slice(MAGIC);
    buffer.push(VERSION);
    buffer.push(0);
    buffer.extend_from_slice(&0u64.to_le_bytes());
    buffer.extend_from_slice(&[0u8; NONCE_LEN]);
    buffer.extend_from_slice(payload);
    buffer
}

/// Encode an encrypted packet for transport over the wire.
pub fn encode_encrypted_envelope(packet: &EncryptedPacket) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(HEADER_LEN + packet.payload.len());
    buffer.extend_from_slice(MAGIC);
    buffer.push(VERSION);
    buffer.push(FLAG_ENCRYPTED);
    buffer.extend_from_slice(&packet.key_id.to_le_bytes());
    buffer.extend_from_slice(&packet.nonce);
    buffer.extend_from_slice(&packet.payload);
    buffer
}

/// Decode a transport payload, returning either plaintext bytes or the envelope details.
pub fn decode_envelope(bytes: &[u8]) -> NetworkResult<Envelope<'_>> {
    if bytes.len() < HEADER_LEN {
        return Err(NetworkError::security("packet too small"));
    }

    if &bytes[..4] != MAGIC {
        return Err(NetworkError::security("invalid packet magic"));
    }

    if bytes[4] != VERSION {
        return Err(NetworkError::security("unsupported packet version"));
    }

    let flags = bytes[5];
    let mut key_bytes = [0u8; 8];
    key_bytes.copy_from_slice(&bytes[6..14]);
    let key_id = u64::from_le_bytes(key_bytes);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[14..14 + NONCE_LEN]);
    let payload = &bytes[HEADER_LEN..];

    if (flags & FLAG_ENCRYPTED) != 0 {
        Ok(Envelope::Encrypted {
            key_id,
            nonce,
            payload,
        })
    } else {
        Ok(Envelope::Plain(payload))
    }
}

fn decrypt_with_key(
    payload: &[u8],
    nonce_bytes: [u8; NONCE_LEN],
    key_bytes: &[u8; 32],
) -> NetworkResult<Vec<u8>> {
    let key = build_less_safe_key(key_bytes)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut data = payload.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut data)
        .map_err(|_| NetworkError::security("failed to decrypt payload"))?;
    let plain_len = payload.len().saturating_sub(aead::AES_256_GCM.tag_len());
    Ok(plaintext[..plain_len].to_vec())
}

fn generate_key_bytes(rng: &SystemRandom) -> NetworkResult<[u8; 32]> {
    let mut key = [0u8; 32];
    rng.fill(&mut key)
        .map_err(|_| NetworkError::security("failed to generate encryption key"))?;
    Ok(key)
}

fn build_less_safe_key(bytes: &[u8; 32]) -> NetworkResult<LessSafeKey> {
    let unbound = UnboundKey::new(&aead::AES_256_GCM, bytes)
        .map_err(|_| NetworkError::security("invalid encryption key material"))?;
    Ok(LessSafeKey::new(unbound))
}
