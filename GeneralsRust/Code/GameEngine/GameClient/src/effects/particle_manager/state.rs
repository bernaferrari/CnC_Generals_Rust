//! C++ ParticleSys.cpp:3232-3323 ordered system payload and atomic admission.
//!
//! Both Xfer interfaces use one codec. Loading owns a detached system list;
//! neither an invalid header nor a later particle failure replaces live state.

use super::*;
use game_engine::common::random_value::with_game_client_random_transaction;
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
}

impl ParticleSystemManager {
    /// Capture this receiving manager, without consulting a process registry.
    pub fn capture_xfer_bytes(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        let mut xfer = CommonXferSave::new(Cursor::new(&mut bytes), 1);
        self.xfer_owned_state(&mut xfer)?;
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
        let mut xfer = CommonXferLoad::new(Cursor::new(bytes.to_vec()), 1);
        self.xfer_owned_state(&mut xfer)?;
        Ok(Some(ParticleSystemRestoreReceipt {
            owner: self.runtime_owner(),
            systems: self.active_system_ids_in_order().into_iter().collect(),
        }))
    }

    fn xfer_owned_state(&mut self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
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

        let loaded = with_game_client_random_transaction(|random| {
            let mut loaded = LoadedSystems {
                next_id,
                systems: HashMap::new(),
                order: Vec::new(),
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
                Snapshotable::xfer(&mut *system, xfer)?;
                let id = system.system_id();
                if id == INVALID_PARTICLE_SYSTEM_ID || loaded.systems.contains_key(&id) {
                    return Err("invalid restored particle system id".into());
                }
                Snapshotable::load_post_process(&mut *system)?;
                loaded.next_id = loaded.next_id.max(id.saturating_add(1));
                loaded.systems.insert(id, system);
                loaded.order.push(id);
            }
            Ok::<_, String>(loaded)
        })?;

        let particle_count = loaded.systems.values().map(|s| s.particle_count()).sum();
        self.renew_system_namespace();
        self.next_system_id = loaded.next_id;
        self.active_systems = loaded.systems;
        self.active_system_order = loaded.order;
        self.system_count = self.active_systems.len();
        self.particle_count = particle_count;
        self.field_particle_count = particle_count;
        self.on_screen_particle_count = 0;
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
