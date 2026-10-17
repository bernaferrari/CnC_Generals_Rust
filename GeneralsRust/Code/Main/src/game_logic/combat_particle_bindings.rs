//! Runtime receipts connect canonical host entries to the actual receiving
//! client manager namespace. Saved numeric IDs alone never confer ownership.
use super::*;

#[derive(Debug)]
pub(super) struct ClientParticleBinding {
    pub(super) id: u32,
    #[cfg(feature = "game_client")]
    owner: game_client::effects::particle_manager::ParticleSystemOwner,
}

impl Clone for CombatParticleRegistry {
    fn clone(&self) -> Self {
        Self {
            next_id: self.next_id,
            systems: self.systems.clone(),
            // A query/ordinary clone has historical observations, no authority
            // to retire the original registry's live resources.
            client_bindings: HashMap::new(),
            destroyed_this_frame: self.destroyed_this_frame.clone(),
            spawned_this_frame: self.spawned_this_frame.clone(),
            muzzle_flash_until: self.muzzle_flash_until.clone(),
            current_frame: self.current_frame,
        }
    }
}

#[cfg(feature = "game_client")]
pub(crate) fn with_client_particle_manager<R>(
    operation: impl FnOnce(&mut game_client::effects::ParticleSystemManager) -> R,
) -> Option<R> {
    use game_client::effects::{
        get_particle_system_manager_mut, initialize_particle_system_manager,
        register_particle_system_manager_bridge,
    };
    register_particle_system_manager_bridge();
    let missing = {
        let guard = get_particle_system_manager_mut().ok()?;
        guard.is_none()
    };
    if missing {
        let _ = initialize_particle_system_manager();
    }
    let mut guard = get_particle_system_manager_mut().ok()?;
    let manager = guard.as_mut()?;
    Some(operation(manager))
}

#[cfg(feature = "game_client")]
fn create_binding(
    manager: &mut game_client::effects::ParticleSystemManager,
    entry: &CombatParticleSystemEntry,
) -> Option<ClientParticleBinding> {
    let id = if let Some(attachment) = &entry.attachment {
        let template = manager.find_template(&entry.template_name)?;
        let id = manager.create_particle_system(&template, true).ok()?;
        let system = manager.find_particle_system_mut(id)?;
        let local = host_local_to_cpp(attachment.local_offset);
        system.set_position(Vec3::new(local.x, local.y, local.z));
        if attachment.local_z_rotation != 0.0 {
            system.rotate_local_transform_z(attachment.local_z_rotation);
        }
        system.attach_to_object(attachment.owner.0);
        if let Some(lifetime) = attachment.lifetime {
            system.set_system_lifetime(lifetime);
        }
        id
    } else {
        let position = host_local_to_cpp(entry.position);
        manager
            .create_preset_system_xyz(&entry.template_name, position.x, position.y, position.z)
            .ok()?
    };
    // Capture while still borrowing the exact manager which returned the ID.
    Some(ClientParticleBinding {
        id,
        owner: manager.runtime_owner(),
    })
}

pub(super) fn spawn_client_binding(
    entry: &CombatParticleSystemEntry,
    bindings: &HashMap<u32, ClientParticleBinding>,
) -> Option<ClientParticleBinding> {
    #[cfg(feature = "game_client")]
    {
        with_client_particle_manager(|manager| {
            let owner = manager.runtime_owner();
            if bindings
                .values()
                .any(|binding| binding.owner.is_live() && binding.owner != owner)
            {
                return None;
            }
            create_binding(manager, entry)
        })
        .flatten()
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = (entry, bindings);
        None
    }
}

pub(super) fn retire_client_binding(binding: &ClientParticleBinding) -> bool {
    #[cfg(feature = "game_client")]
    {
        if !binding.owner.is_live() {
            return true;
        }
        // Never initialize another namespace just to retire an old receipt.
        if let Ok(mut guard) = game_client::effects::get_particle_system_manager_mut() {
            if let Some(manager) = guard.as_mut() {
                if binding.owner == manager.runtime_owner() {
                    manager.destroy_particle_system(binding.id);
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = binding;
        true
    }
}

pub(super) fn can_retire_bindings(bindings: &HashMap<u32, ClientParticleBinding>) -> bool {
    #[cfg(feature = "game_client")]
    {
        let owner = game_client::effects::get_particle_system_manager_mut()
            .ok()
            .and_then(|guard| guard.as_ref().map(|manager| manager.runtime_owner()));
        bindings
            .values()
            .all(|binding| !binding.owner.is_live() || owner.as_ref() == Some(&binding.owner))
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = bindings;
        true
    }
}

impl CombatParticleRegistry {
    /// Canonical owner operation, before presentation freeze. Events and
    /// rebuilt snapshots cannot independently create another client system.
    #[cfg(feature = "game_client")]
    pub(crate) fn backfill_client_systems(
        &mut self,
        manager: &mut game_client::effects::ParticleSystemManager,
    ) -> usize {
        let owner = manager.runtime_owner();
        if self
            .client_bindings
            .values()
            .any(|binding| binding.owner.is_live() && binding.owner != owner)
        {
            return 0;
        }
        let mut ids: Vec<_> = self.systems.keys().copied().collect();
        ids.sort_unstable();
        let mut admitted = 0;
        for id in ids {
            let entry = self.systems.get_mut(&id).expect("collected host particle");
            if !entry.active
                || (!entry.fx_list_name.is_empty() && entry.fx_list_name == entry.template_name)
            {
                // spawn_fx_list_marker records an FXList notification, not a
                // ParticleSystem template. Its actual nuggets own emission.
                continue;
            }
            if self
                .client_bindings
                .get(&id)
                .is_some_and(|binding| binding.owner == owner)
            {
                continue;
            }
            if self
                .client_bindings
                .get(&id)
                .is_some_and(|binding| binding.owner.is_live())
            {
                // A live peer manager is not a reset namespace. Keep the
                // original receipt and canonical entry for its actual owner.
                continue;
            }
            self.client_bindings.remove(&id);
            entry.client_system_id = None;
            if let Some(binding) = create_binding(manager, entry) {
                entry.client_system_id = Some(binding.id);
                self.client_bindings.insert(id, binding);
                admitted += 1;
            }
        }
        admitted
    }

    pub(super) fn update_client_position(&mut self, id: u32, position: Vec3) {
        if let Some(entry) = self.systems.get_mut(&id) {
            entry.position = position;
        }
        #[cfg(feature = "game_client")]
        {
            with_client_particle_manager(|manager| {
                let owner = manager.runtime_owner();
                if self
                    .client_bindings
                    .values()
                    .any(|binding| binding.owner.is_live() && binding.owner != owner)
                {
                    return;
                }
                let current = self
                    .client_bindings
                    .get(&id)
                    .filter(|binding| binding.owner == manager.runtime_owner());
                if let Some(binding) = current {
                    if let Some(system) = manager.find_particle_system_mut(binding.id) {
                        let position = host_local_to_cpp(position);
                        system.set_position(Vec3::new(position.x, position.y, position.z));
                    }
                } else if let Some(entry) = self.systems.get_mut(&id) {
                    if self
                        .client_bindings
                        .get(&id)
                        .is_some_and(|binding| binding.owner.is_live())
                    {
                        return;
                    }
                    self.client_bindings.remove(&id);
                    entry.client_system_id = None;
                    if let Some(binding) = create_binding(manager, entry) {
                        entry.client_system_id = Some(binding.id);
                        self.client_bindings.insert(id, binding);
                    }
                }
            });
        }
    }
}

#[cfg(feature = "game_client")]
impl CombatParticleRegistry {
    /// Same retirement operation with the actual receiving manager already
    /// borrowed, used by callers which own that service directly.
    pub(crate) fn deactivate_with_client_manager(
        &mut self,
        id: u32,
        manager: &mut game_client::effects::ParticleSystemManager,
    ) {
        if self.client_bindings.get(&id).is_some_and(|binding| {
            binding.owner.is_live() && binding.owner != manager.runtime_owner()
        }) {
            return;
        }
        let Some(entry) = self.systems.get_mut(&id).filter(|entry| entry.active) else {
            return;
        };
        entry.active = false;
        entry.client_system_id = None;
        if let Some(binding) = self.client_bindings.remove(&id) {
            if binding.owner == manager.runtime_owner() {
                manager.destroy_particle_system(binding.id);
            }
        }
    }

    pub(crate) fn clear_with_client_manager(
        &mut self,
        manager: &mut game_client::effects::ParticleSystemManager,
    ) {
        if self
            .client_bindings
            .values()
            .any(|binding| binding.owner.is_live() && binding.owner != manager.runtime_owner())
        {
            return;
        }
        let ids: Vec<_> = self.systems.keys().copied().collect();
        for id in ids {
            self.deactivate_with_client_manager(id, manager);
        }
        // Inactive entries cannot retain a live ownership receipt.
        self.client_bindings.clear();
        self.systems.clear();
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
        self.muzzle_flash_until.clear();
        self.next_id = 1;
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "combat_particle_bindings_tests.rs"]
mod tests;

#[cfg(feature = "game_client")]
impl CombatParticleRegistry {
    /// The companion loader's exact successful namespace receipt authorizes
    /// adoption of saved IDs. Bare host snapshot restoration never calls this.
    pub(crate) fn adopt_loaded_client_systems(
        &mut self,
        manager: &game_client::effects::ParticleSystemManager,
        receipt: &game_client::effects::particle_manager::ParticleSystemRestoreReceipt,
    ) -> Result<usize, String> {
        if !receipt.belongs_to(manager) {
            return Err("particle companion receipt does not name the receiving manager".into());
        }
        let owner = manager.runtime_owner();
        if self
            .client_bindings
            .values()
            .any(|binding| binding.owner.is_live() && binding.owner != owner)
        {
            return Err("particle registry still belongs to another live manager".into());
        }
        let mut seen = HashSet::new();
        for entry in self.systems.values().filter(|entry| entry.active) {
            if let Some(id) = entry.client_system_id {
                if !seen.insert(id) {
                    return Err("duplicate client particle ID in paired host snapshot".into());
                }
                if receipt.contains_system(id) {
                    if let Some(system) = manager.find_particle_system(id) {
                        let expected =
                            manager.find_template(&entry.template_name).ok_or_else(|| {
                                format!(
                                    "missing receiving particle template {}",
                                    entry.template_name
                                )
                            })?;
                        // Resolve aliases through this manager's admitted definitions.
                        // Restored runtime parameters are independent of the immutable
                        // authored template; this row names its canonical definition.
                        if expected.name() != system.template().name() {
                            return Err(format!(
                                "loaded particle system {id} does not match host template {}",
                                entry.template_name
                            ));
                        }
                    }
                }
            }
        }
        self.client_bindings.clear();
        let mut adopted = 0;
        for entry in self.systems.values_mut().filter(|entry| entry.active) {
            let Some(id) = entry.client_system_id else {
                continue;
            };
            if receipt.contains_system(id) && manager.find_particle_system(id).is_some() {
                self.client_bindings.insert(
                    entry.id,
                    ClientParticleBinding {
                        id,
                        owner: owner.clone(),
                    },
                );
                adopted += 1;
            } else {
                // The actual C++ companion omits destroyed/non-saveable
                // systems. A historical host row must not replay them.
                entry.active = false;
                entry.client_system_id = None;
            }
        }
        Ok(adopted)
    }
}

#[cfg(feature = "game_client")]
impl CombatParticleRegistry {
    /// Receipt from actual per-system attachment resolution, after initial delay
    /// and wind. This stops host emission observations, never deletes/drains the
    /// Client system early; its manager retains the exact source lifetime.
    pub(crate) fn admit_client_object_attachment_lost(
        &mut self,
        manager: &game_client::effects::particle_manager::ParticleSystemOwner,
        client_id: u32,
        object_id: u32,
    ) -> bool {
        let Some(host_id) = self.client_bindings.iter().find_map(|(id, binding)| {
            (binding.id == client_id && &binding.owner == manager).then_some(*id)
        }) else {
            return false;
        };
        let Some(entry) = self.systems.get_mut(&host_id) else {
            return false;
        };
        if !entry
            .attachment
            .as_ref()
            .is_some_and(|value| value.owner.0 == object_id)
        {
            return false;
        }
        entry.attachment = None;
        entry.active = false;
        // Retain the namespace receipt while Client particles drain; clear/reset
        // still owns that real resource and cannot acquire a peer's numeric ID.
        true
    }
}
