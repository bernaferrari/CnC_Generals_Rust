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
            body_auto_particle_base: self.body_auto_particle_base.clone(),
            body_auto_particle_effective: self.body_auto_particle_effective.clone(),
        }
    }
}

/// A short, explicit loan of the exact receiving presentation lifetime.
/// An absent manager is an actual headless capability, never an active slot.
pub trait ParticleBindingAccess {
    #[cfg(feature = "game_client")]
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager>;
}

/// Crate-private receiving pose capability. Keep NativeSlaveSource out of the
/// public particle binding API; only admitted Main presentation paths need it.
pub(crate) trait ReceivingParticlePoseAccess: ParticleBindingAccess {
    /// Receiving live bones for one already admitted Draw source. None means
    /// this adapter has no native installed-pose capability.
    fn current_bone_transforms(
        &mut self,
        _source: &crate::game_logic::NativeSlaveSource,
        _bone: &str,
        _start: i32,
        _limit: usize,
    ) -> Result<Option<Vec<glam::Mat4>>, String> {
        Ok(None)
    }
    /// Immutable receiving asset pose for a bone when no installed pose is
    /// available (headless mode or a genuine current-pose bone miss).
    fn pristine_bone_transforms(
        &mut self,
        _source: &crate::game_logic::NativeSlaveSource,
        _bone: &str,
        _start: i32,
        _limit: usize,
    ) -> Result<Option<Vec<glam::Mat4>>, String> {
        Ok(None)
    }
    fn receiving_local_player_id(&self) -> Option<u32> {
        None
    }
}
#[cfg(feature = "game_client")]
impl ParticleBindingAccess for game_client::effects::ParticleSystemManager {
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager> {
        Some(self)
    }
}
impl ParticleBindingAccess for () {
    #[cfg(feature = "game_client")]
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager> {
        None
    }
}
impl<T: ParticleBindingAccess + ?Sized> ParticleBindingAccess for Option<&mut T> {
    #[cfg(feature = "game_client")]
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager> {
        self.as_deref_mut()?.particle_systems_mut()
    }
}
impl ParticleBindingAccess for crate::game_logic::ScriptPresentationEffects<'_> {
    #[cfg(feature = "game_client")]
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager> {
        self.view.as_deref_mut()?.particle_systems_mut()
    }
}
impl ReceivingParticlePoseAccess for () {}
impl ReceivingParticlePoseAccess for crate::game_logic::ScriptPresentationEffects<'_> {
    fn current_bone_transforms(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        bone: &str,
        start: i32,
        limit: usize,
    ) -> Result<Option<Vec<glam::Mat4>>, String> {
        #[cfg(feature = "game_client")]
        {
            self.view
                .as_deref_mut()
                .map(|view| view.current_client_bone_transforms(source, bone, start, limit))
                .transpose()
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (source, bone, start, limit);
            Ok(None)
        }
    }
    fn pristine_bone_transforms(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        bone: &str,
        start: i32,
        limit: usize,
    ) -> Result<Option<Vec<glam::Mat4>>, String> {
        self.pristine_bones
            .map(|assets| assets.pristine_bone_transforms(source, bone, start, limit))
            .transpose()
    }
    fn receiving_local_player_id(&self) -> Option<u32> {
        self.view.as_ref().map(|view| view.local_player_id())
    }
}
#[cfg(feature = "game_client")]
impl ParticleBindingAccess for crate::game_logic::host_fx_list_dispatch::ObjectFxServices<'_, '_> {
    fn particle_systems_mut(&mut self) -> Option<&mut game_client::effects::ParticleSystemManager> {
        self.particle_systems_mut()
    }
}

#[cfg(feature = "game_client")]
fn create_binding(
    manager: &mut game_client::effects::ParticleSystemManager,
    entry: &CombatParticleSystemEntry,
) -> Option<ClientParticleBinding> {
    manager.admit_effective_definitions().ok()?;
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
        let template = manager.find_template(&entry.template_name)?;
        let id = manager.create_particle_system(&template, true).ok()?;
        manager
            .find_particle_system_mut(id)?
            .set_position(Vec3::new(position.x, position.y, position.z));
        id
    };
    // Capture while still borrowing the exact manager which returned the ID.
    Some(ClientParticleBinding {
        id,
        owner: manager.runtime_owner(),
    })
}

pub(super) fn spawn_client_binding<P: ParticleBindingAccess + ?Sized>(
    entry: &CombatParticleSystemEntry,
    bindings: &HashMap<u32, ClientParticleBinding>,
    particle_owner: &mut P,
) -> Option<ClientParticleBinding> {
    #[cfg(feature = "game_client")]
    {
        let manager = particle_owner.particle_systems_mut()?;
        let owner = manager.runtime_owner();
        if bindings
            .values()
            .any(|binding| binding.owner.is_live() && binding.owner != owner)
        {
            log::error!("particle source does not own registry namespace");
            return None;
        }
        create_binding(manager, entry)
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = (entry, bindings, particle_owner);
        None
    }
}
pub(super) fn retire_client_binding<P: ParticleBindingAccess + ?Sized>(
    binding: &ClientParticleBinding,
    particle_owner: &mut P,
) -> bool {
    #[cfg(feature = "game_client")]
    {
        if !binding.owner.is_live() {
            return true;
        }
        let Some(manager) = particle_owner.particle_systems_mut() else {
            return false;
        };
        if binding.owner != manager.runtime_owner() {
            return false;
        }
        manager.destroy_particle_system(binding.id);
        true
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = (binding, particle_owner);
        true
    }
}
pub(super) fn can_retire_bindings(bindings: &HashMap<u32, ClientParticleBinding>) -> bool {
    #[cfg(feature = "game_client")]
    {
        bindings.values().all(|binding| !binding.owner.is_live())
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
        for (id, binding) in &self.client_bindings {
            if binding.owner.is_live() && binding.owner != owner {
                log::error!("particle frame has a peer namespace receipt");
                return 0;
            }
            let _ = id;
        }
        // Creation occurs only at the immediate authored source point.
        0
    }

    pub(super) fn update_client_position(
        &mut self,
        id: u32,
        position: Vec3,
        particle_owner: &mut dyn ParticleBindingAccess,
    ) {
        if let Some(entry) = self.systems.get_mut(&id) {
            entry.position = position;
        }
        #[cfg(feature = "game_client")]
        {
            let Some(manager) = particle_owner.particle_systems_mut() else {
                return;
            };
            if let Some(binding) = self
                .client_bindings
                .get(&id)
                .filter(|binding| binding.owner == manager.runtime_owner())
            {
                if let Some(system) = manager.find_particle_system_mut(binding.id) {
                    let position = host_local_to_cpp(position);
                    system.set_position(Vec3::new(position.x, position.y, position.z));
                }
            }
        }
        #[cfg(not(feature = "game_client"))]
        let _ = particle_owner;
    }
}

#[cfg(feature = "game_client")]
impl CombatParticleRegistry {
    /// Whether every still-live client receipt belongs to this exact receiving
    /// manager. Historical/dead receipts need no native teardown authority.
    #[cfg(feature = "game_client")]
    pub(crate) fn can_clear_with_client_manager(
        &self,
        manager: &game_client::effects::particle_manager::ParticleSystemManager,
    ) -> bool {
        let owner = manager.runtime_owner();
        self.client_bindings
            .values()
            .all(|binding| !binding.owner.is_live() || binding.owner == owner)
    }

    /// Checked version for reset boundaries: reject a peer manager before any
    /// registry or native particle mutation.
    #[cfg(feature = "game_client")]
    pub(crate) fn clear_with_client_manager_checked(
        &mut self,
        manager: &mut game_client::effects::particle_manager::ParticleSystemManager,
    ) -> Result<(), String> {
        if !self.can_clear_with_client_manager(manager) {
            return Err("combat particle reset requires its exact live receiving manager".into());
        }
        self.clear_with_client_manager(manager);
        Ok(())
    }

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

impl CombatParticleRegistry {
    pub(super) fn bind_source_entry(
        &mut self,
        id: u32,
        particle_owner: &mut dyn ParticleBindingAccess,
    ) {
        let Some(entry) = self.systems.get(&id) else {
            return;
        };
        if let Some(binding) = spawn_client_binding(entry, &self.client_bindings, particle_owner) {
            self.systems
                .get_mut(&id)
                .expect("source entry")
                .client_system_id = Some(binding.id);
            self.client_bindings.insert(id, binding);
        }
    }
}
