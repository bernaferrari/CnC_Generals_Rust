//! C++ ParticleSys.cpp:3232-3323 ordered system payload and atomic admission.
//!
//! Both Xfer interfaces use one codec. Loading owns a detached system list;
//! neither an invalid header nor a later particle failure replaces live state.

use super::*;
use game_engine::common::system::xfer::XferMode;
use std::collections::HashSet;

/// Proof that these IDs were admitted by one successful native payload load.
/// An ordinary runtime-owner observation cannot manufacture this receipt.
#[derive(Debug)]
pub struct ParticleSystemRestoreReceipt {
    owner: ParticleSystemOwner,
    systems: HashSet<ParticleSystemId>,
}

impl ParticleSystemRestoreReceipt {
    pub fn belongs_to(&self, manager: &ParticleSystemManager) -> bool {
        self.owner == manager.runtime_owner()
    }

    pub fn contains_system(&self, id: ParticleSystemId) -> bool {
        self.systems.contains(&id)
    }
}

struct LoadedSystems {
    next_id: ParticleSystemId,
    systems: HashMap<ParticleSystemId, Box<ParticleSystem>>,
    order: Vec<ParticleSystemId>,
    next_particle_ordinal: u64,
}

impl ParticleSystemManager {
    /// Capture this receiving manager, without consulting a process registry.
    pub fn capture_xfer_bytes(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        let mut xfer = CommonXferSave::new(Cursor::new(&mut bytes), 1);
        self.xfer_owned_state(&mut xfer)?;
        let mut tail = Vec::new();
        for word in self.random_state.seed_words() {
            tail.extend_from_slice(&word.to_le_bytes());
        }
        let ids: Vec<_> = self
            .active_system_ids_in_order()
            .into_iter()
            .filter(|id| {
                self.active_systems
                    .get(id)
                    .is_some_and(|system| !system.is_destroyed() && system.is_saveable())
            })
            .collect();
        tail.extend_from_slice(&(ids.len() as u32).to_le_bytes());
        for id in ids {
            tail.extend_from_slice(&id.to_le_bytes());
            for word in self.active_systems[&id].random_state.seed_words() {
                tail.extend_from_slice(&word.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&tail);
        bytes.extend_from_slice(&(tail.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"NRG1");
        Ok(bytes)
    }

    /// Atomically restore this manager and return proof of the new namespace.
    /// Empty historical placeholders contain no systems and grant no receipt.
    pub fn restore_xfer_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Option<ParticleSystemRestoreReceipt>, String> {
        if bytes.is_empty() || bytes == [1] {
            return Ok(None);
        }
        let (payload, random_tail) = decode_random_tail(bytes)?;
        let mut xfer = CommonXferLoad::new(Cursor::new(payload.to_vec()), 1);
        self.xfer_owned_state_with_capsule(&mut xfer, random_tail)?;
        Ok(Some(ParticleSystemRestoreReceipt {
            owner: self.runtime_owner(),
            systems: self.active_system_ids_in_order().into_iter().collect(),
        }))
    }

    fn xfer_owned_state(&mut self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
        self.xfer_owned_state_with_capsule(xfer, None)
    }
    fn xfer_owned_state_with_capsule(
        &mut self,
        xfer: &mut dyn CommonXfer,
        capsule: Option<([u32; 6], Vec<(u32, [u32; 6])>)>,
    ) -> Result<(), String> {
        let mode = xfer.get_xfer_mode();
        if !matches!(mode, XferMode::Save | XferMode::Load | XferMode::Crc) {
            return Err("particle system state requires a save or load stream".into());
        }
        let mut version = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        let mut next_id = self.next_system_id;
        xfer.xfer_unsigned_int(&mut next_id)
            .map_err(|e| e.to_string())?;
        let mut count = self.active_systems.len() as u32;
        xfer.xfer_unsigned_int(&mut count)
            .map_err(|e| e.to_string())?;

        if mode != XferMode::Load {
            // The C++ linked list, not lookup-map order, defines the payload.
            for id in self.active_system_ids_in_order() {
                let system = self
                    .active_systems
                    .get_mut(&id)
                    .ok_or("missing particle system during save")?;
                let mut name = if system.is_destroyed() || !system.is_saveable() {
                    String::new()
                } else {
                    system.template().name().to_string()
                };
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                if !name.is_empty() {
                    Snapshotable::xfer(&mut **system, xfer)?;
                }
            }
            return Ok(());
        }

        let mut random = self.random_state.clone();
        let mut loaded = (|random: &mut game_engine::common::random_value::RandomState| {
            let mut loaded = LoadedSystems {
                next_id,
                systems: HashMap::new(),
                order: Vec::new(),
                next_particle_ordinal: 1,
            };
            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                if name.is_empty() {
                    continue;
                }
                let template = self
                    .find_template(&name)
                    .ok_or_else(|| format!("unknown particle template '{name}'"))?;
                // Constructor wind/initial-delay observations are staged too.
                // Successful loads retain their existing draw order; failed
                // candidates cannot advance the shared presentation stream.
                let mut system = Box::new(ParticleSystem::new_with_random(
                    template,
                    1,
                    false,
                    &mut |lo, hi| random.next_real(lo, hi),
                ));
                system.xfer_with_particle_random(xfer, random)?;
                let id = system.system_id();
                if id == INVALID_PARTICLE_SYSTEM_ID || loaded.systems.contains_key(&id) {
                    return Err("invalid restored particle system id".into());
                }
                Snapshotable::load_post_process(&mut *system)?;
                loaded.next_id = loaded.next_id.max(id.saturating_add(1));
                for particle in system.particles_mut() {
                    let ordinal = loaded.next_particle_ordinal;
                    loaded.next_particle_ordinal = ordinal
                        .checked_add(1)
                        .ok_or("particle admission ordinal exhausted")?;
                    particle.admission_ordinal = Some(ordinal);
                }
                loaded.systems.insert(id, system);
                loaded.order.push(id);
            }
            Ok::<_, String>(loaded)
        })(&mut random)?;
        if let Some((manager, states)) = capsule {
            let expected: HashSet<_> = states.iter().map(|(id, _)| *id).collect();
            let actual: HashSet<_> = loaded.systems.keys().copied().collect();
            if expected != actual {
                return Err("particle random capsule does not match restored system roster".into());
            }
            for (id, words) in states {
                loaded
                    .systems
                    .get_mut(&id)
                    .expect("validated capsule receiver")
                    .random_state
                    .set_seed_words(words);
            }
            random.set_seed_words(manager);
        }
        self.random_state = random;

        let particle_count = loaded.systems.values().map(|s| s.particle_count()).sum();
        self.renew_system_namespace();
        self.next_system_id = loaded.next_id;
        self.next_particle_ordinal = loaded.next_particle_ordinal;
        self.active_systems = loaded.systems;
        self.active_system_order = loaded.order;
        self.system_count = self.active_systems.len();
        self.particle_count = particle_count;

        Ok(())
    }
}

fn with_receiving_manager<R>(
    operation: impl FnOnce(&mut ParticleSystemManager) -> Result<R, String>,
) -> Result<R, String> {
    let mut guard = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
    if let Some(manager) = guard.as_mut() {
        operation(manager)
    } else {
        let mut candidate = ParticleSystemManager::new();
        let result = operation(&mut candidate)?;
        *guard = Some(candidate);
        Ok(result)
    }
}

pub fn xfer_particle_system_manager_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let mut adapter = crate::core::game_client::RuntimeCommonXferAdapter::new(xfer);
    with_receiving_manager(|manager| manager.xfer_owned_state(&mut adapter))
        .map_err(|_| XferStatus::InvalidData)
}

pub fn load_post_process_particle_system_manager_state() -> Result<(), XferStatus> {
    let mut guard = get_particle_system_manager_mut().map_err(|_| XferStatus::InvalidData)?;
    let Some(manager) = guard.as_mut() else {
        return Ok(());
    };
    for id in manager.active_system_ids_in_order() {
        if let Some(system) = manager.active_systems.get_mut(&id) {
            Snapshotable::load_post_process(&mut **system).map_err(|_| XferStatus::InvalidData)?;
        }
    }
    Ok(())
}

/// C++ CHUNK_ParticleSystem payload; no runtime owner identity is serialized.
pub fn capture_live_particle_system_xfer_bytes() -> Result<Vec<u8>, String> {
    with_receiving_manager(ParticleSystemManager::capture_xfer_bytes)
}

pub fn restore_live_particle_system_from_xfer_bytes(
    bytes: &[u8],
) -> Result<Option<ParticleSystemRestoreReceipt>, String> {
    if bytes.is_empty() || bytes == [1] {
        return Ok(None);
    }
    with_receiving_manager(|manager| manager.restore_xfer_bytes(bytes))
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;

// Runtime capsules deliberately remain outside the original ParticleSys Xfer.
fn decode_random_tail(
    bytes: &[u8],
) -> Result<(&[u8], Option<([u32; 6], Vec<(u32, [u32; 6])>)>), String> {
    if bytes.len() < 8 || &bytes[bytes.len() - 4..] != b"NRG1" {
        return Ok((bytes, None));
    }
    let length =
        u32::from_le_bytes(bytes[bytes.len() - 8..bytes.len() - 4].try_into().unwrap()) as usize;
    let start = bytes
        .len()
        .checked_sub(
            length
                .checked_add(8)
                .ok_or("particle random capsule size overflow")?,
        )
        .ok_or("invalid particle random capsule size")?;
    let tail = &bytes[start..bytes.len() - 8];
    if tail.len() < 28 {
        return Err("truncated particle random capsule".into());
    }
    let words = |chunk: &[u8]| -> [u32; 6] {
        std::array::from_fn(|index| {
            u32::from_le_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap())
        })
    };
    let manager = words(&tail[..24]);
    let count = u32::from_le_bytes(tail[24..28].try_into().unwrap()) as usize;
    if count.checked_mul(28).and_then(|size| size.checked_add(28)) != Some(tail.len()) {
        return Err("invalid particle random capsule roster".into());
    }
    let mut seen = HashSet::new();
    let mut systems = Vec::with_capacity(count);
    for row in tail[28..].chunks_exact(28) {
        let id = u32::from_le_bytes(row[..4].try_into().unwrap());
        if id == INVALID_PARTICLE_SYSTEM_ID || !seen.insert(id) {
            return Err("duplicate particle random capsule ID".into());
        }
        systems.push((id, words(&row[4..])));
    }
    Ok((&bytes[..start], Some((manager, systems))))
}
