//! Actual CPU Drawable lifetimes and their receiving-client identity.
//!
//! Object associations, inverse lookup, and validated runtime binding reads
//! operate on this owner. GameClient still coordinates creation, updates,
//! destruction callbacks, and Xfer in their original order.
//! Construction allocates inert containers and never publishes client state.

use super::*;

/// Presentation lifetime: candidate construction owns both Drawable identity and
/// its particle namespace. Committing a map moves them together.
pub struct DrawableWorld {
    pub(super) state: DrawableState,
    pub(super) particles: crate::effects::particle_manager::ParticleSystemManager,
}
impl DrawableWorld {
    pub fn new() -> Self {
        Self {
            state: DrawableState::new(),
            particles: crate::effects::particle_manager::ParticleSystemManager::new(),
        }
    }
    pub fn particle_systems(&self) -> &crate::effects::particle_manager::ParticleSystemManager {
        &self.particles
    }
    pub fn particle_systems_mut(
        &mut self,
    ) -> &mut crate::effects::particle_manager::ParticleSystemManager {
        &mut self.particles
    }
}
impl std::ops::Deref for DrawableWorld {
    type Target = DrawableState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl std::ops::DerefMut for DrawableWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}
/// Drawable-only loan permits particle callbacks to borrow the same world's
/// manager without taking either owner out of its lifetime.
pub struct DrawableState {
    pub(super) light_scene: crate::fx_list::DisplayLightScene,
    pub(super) next_drawable_id: DrawableId,
    pub(super) ray_effects: crate::effects::ray_effect_registry::RayEffectSystem,
    /// C++ GameClient.cpp480 prepends each newly constructed Drawable.
    pub(super) native_drawable_order: Vec<DrawableId>,
    pub(super) owned_laser_order: Vec<DrawableId>,
    pub(super) drawable_map: HashMap<DrawableId, Box<dyn Drawable>>,
    pub(super) drawable_object_map: HashMap<ObjectID, DrawableId>,
    /// Runtime-only direct binding metadata. C++ reconstructs associations
    /// around loaded Drawables; this table does not enter Xfer.
    pub(super) presentation_direct_drawable_bindings:
        HashMap<DrawableId, PresentationDirectDrawableBinding>,
    /// Survives world invalidation so reused IDs do not inherit old bindings.
    pub(super) next_presentation_direct_binding_generation: u64,
    pub(super) text_bearing_drawables: Vec<DrawableId>,
    // Last field: Drawable destructors run before their resource manager clears.
    pub(super) projected_decals: super::native_projected_resources::NativeProjectedDecals,
}

impl DrawableState {
    pub fn new() -> Self {
        Self {
            light_scene: Default::default(),
            next_drawable_id: DrawableId(1),
            ray_effects: Default::default(),
            native_drawable_order: Vec::new(),
            owned_laser_order: Vec::new(),
            drawable_map: HashMap::with_capacity(super::super::DRAWABLE_HASH_SIZE),
            drawable_object_map: HashMap::new(),
            presentation_direct_drawable_bindings: HashMap::new(),
            next_presentation_direct_binding_generation: 1,
            text_bearing_drawables: Vec::new(),
            projected_decals: Default::default(),
        }
    }

    pub(super) fn admit_native_drawable(&mut self, id: DrawableId, drawable: Box<dyn Drawable>) {
        if !self.native_drawable_order.contains(&id) {
            self.native_drawable_order.insert(0, id);
        }
        self.drawable_map.insert(id, drawable);
    }

    /// A decoding receiver is absent from both live lookup and visitation until
    /// decoding succeeds. An early error drops the detached node, never a live ID.
    pub(super) fn take_native_drawable_for_load(
        &mut self,
        id: DrawableId,
    ) -> Option<(Box<dyn Drawable>, usize)> {
        let drawable = self.drawable_map.remove(&id)?;
        let position = self
            .native_drawable_order
            .iter()
            .position(|known| *known == id)
            .expect("registered native decoding receiver has its canonical roster node");
        self.native_drawable_order.remove(position);
        self.owned_laser_order.retain(|known| *known != id);
        Some((drawable, position))
    }

    pub(super) fn restore_native_drawable_after_load(
        &mut self,
        id: DrawableId,
        drawable: Box<dyn Drawable>,
        position: usize,
    ) {
        self.native_drawable_order.insert(position, id);
        self.drawable_map.insert(id, drawable);
    }

    pub(super) fn alloc_drawable_id(&mut self) -> DrawableId {
        // GameClient.cpp461-467: unsigned allocation deliberately wraps.
        let id = self.next_drawable_id;
        self.next_drawable_id = DrawableId(id.0.wrapping_add(1));
        id
    }

    pub(super) fn alloc_binding_generation(&mut self) -> u64 {
        let generation = self.next_presentation_direct_binding_generation.max(1);
        self.next_presentation_direct_binding_generation = generation.wrapping_add(1).max(1);
        generation
    }

    pub(super) fn bind_drawable_to_object(
        &mut self,
        drawable_id: DrawableId,
        object_id: ObjectID,
    ) -> GameClientResult<()> {
        // C++ binds an actual live Drawable receiver. Reject an unresolved Rust
        // handle before changing another Drawable's association or runtime key.
        if !self.drawable_map.contains_key(&drawable_id) {
            return Err(GameClientError::DrawableNotFound(drawable_id));
        }
        let old_object_id = self
            .drawable_map
            .get(&drawable_id)
            .and_then(|drawable| drawable.get_object_id());

        // C++ `Drawable::friend_bindToObject` does not reset its volatile
        // shroud state when it is rebound to the same Object.  Retain the
        // direct binding only for that fully validated no-op association;
        // any owner change, displacement, or broken inverse map is a new
        // visual lifetime and must receive a fresh key on the next host sync.
        let retains_same_direct_binding = old_object_id == Some(object_id)
            && self.drawable_object_map.get(&object_id).copied() == Some(drawable_id)
            && self
                .presentation_direct_drawable_bindings
                .get(&drawable_id)
                .is_some_and(|binding| {
                    binding.binding_key.object_id == object_id
                        && binding.binding_key.drawable_id == drawable_id
                });
        if !retains_same_direct_binding {
            self.presentation_direct_drawable_bindings
                .remove(&drawable_id);
        }

        if let Some(old_object_id) = old_object_id {
            if old_object_id != object_id
                && self.drawable_object_map.get(&old_object_id).copied() == Some(drawable_id)
            {
                self.drawable_object_map.remove(&old_object_id);
            }
        }

        if let Some(previous_drawable_id) = self.drawable_object_map.get(&object_id).copied() {
            if previous_drawable_id != drawable_id {
                if let Some(previous_drawable) = self.drawable_map.get_mut(&previous_drawable_id) {
                    previous_drawable.set_object_id(None);
                }
                self.presentation_direct_drawable_bindings
                    .remove(&previous_drawable_id);
            }
        }

        if let Some(drawable) = self.drawable_map.get_mut(&drawable_id) {
            drawable.set_object_id(Some(object_id));
            self.drawable_object_map.insert(object_id, drawable_id);
            Ok(())
        } else {
            Err(GameClientError::DrawableNotFound(drawable_id))
        }
    }

    pub(super) fn find_drawable_by_id(&self, id: DrawableId) -> Option<&dyn Drawable> {
        self.drawable_map.get(&id).map(|d| d.as_ref())
    }

    pub(super) fn find_drawable_by_id_mut(
        &mut self,
        id: DrawableId,
    ) -> Option<&mut Box<dyn Drawable>> {
        self.drawable_map.get_mut(&id)
    }

    pub(super) fn get_drawable_for_object(&self, object_id: ObjectID) -> Option<DrawableId> {
        self.drawable_object_map.get(&object_id).copied()
    }

    pub(super) fn presentation_direct_drawable_state(
        &self,
        host_epoch: u64,
        object_id: ObjectID,
    ) -> Option<PresentationDirectDrawableState> {
        // Epoch zero belongs only to the legacy convenience ensure helper.
        // Main's direct host pipeline starts at one and is the only route that
        // may expose a renderable direct binding key.
        if host_epoch == 0 {
            return None;
        }
        let drawable_id = self.drawable_object_map.get(&object_id)?;
        let binding = self
            .presentation_direct_drawable_bindings
            .get(drawable_id)?;
        let binding_key = binding.binding_key;
        if binding_key.host_epoch != host_epoch
            || binding_key.object_id != object_id
            || binding_key.drawable_id != *drawable_id
        {
            return None;
        }
        let drawable = self.drawable_map.get(drawable_id)?;
        if drawable.get_object_id() != Some(object_id) {
            return None;
        }
        let scene_effectively_hidden = drawable.scene_effectively_hidden()?;
        let fully_obscured = drawable.fully_obscured_by_shroud()?;
        Some(PresentationDirectDrawableState {
            binding_key,
            scene_effectively_hidden,
            fully_obscured,
        })
    }

    pub(super) fn presentation_direct_fully_obscured(&self, object_id: ObjectID) -> Option<bool> {
        let drawable_id = self.drawable_object_map.get(&object_id)?;
        let host_epoch = self
            .presentation_direct_drawable_bindings
            .get(drawable_id)?
            .binding_key
            .host_epoch;
        self.presentation_direct_drawable_state(host_epoch, object_id)
            .map(|state| state.fully_obscured)
    }
}

impl Default for DrawableWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl GameClient {
    pub fn particle_systems(&self) -> &crate::effects::particle_manager::ParticleSystemManager {
        &self.drawables.particles
    }
    pub fn particle_systems_mut(
        &mut self,
    ) -> &mut crate::effects::particle_manager::ParticleSystemManager {
        &mut self.drawables.particles
    }
}

impl NativeDrawableOwner<'_> {
    pub fn create_light_pulse(&mut self, pulse: crate::fx_list::DisplayLightPulse) -> bool {
        let admitted = self.drawables.light_scene.create_pulse(pulse);
        // The native scene consumes the light directly; legacy pulse receipts
        // are not another growing writable presentation stream.
        self.drawables.light_scene.drain_pulses();
        admitted
    }
}
impl GameClient {
    pub fn dynamic_lights(&self) -> &[crate::fx_list::DisplayDynamicLight] {
        self.drawables.light_scene.lights()
    }
}
