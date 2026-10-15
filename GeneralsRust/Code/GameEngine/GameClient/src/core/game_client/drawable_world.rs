//! Actual CPU Drawable lifetimes and their receiving-client identity.
//!
//! This owner groups the existing storage; GameClient still drives every
//! creation, update, destruction, and Xfer operation in its original order.
//! Construction allocates inert containers and never publishes client state.

use super::*;

pub(super) struct DrawableWorld {
    pub(super) next_drawable_id: DrawableId,
    pub(super) ray_effects: crate::effects::ray_effect_registry::RayEffectSystem,
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
}

impl DrawableWorld {
    pub(super) fn new() -> Self {
        Self {
            next_drawable_id: DrawableId(1),
            ray_effects: Default::default(),
            owned_laser_order: Vec::new(),
            drawable_map: HashMap::with_capacity(super::super::DRAWABLE_HASH_SIZE),
            drawable_object_map: HashMap::new(),
            presentation_direct_drawable_bindings: HashMap::new(),
            next_presentation_direct_binding_generation: 1,
            text_bearing_drawables: Vec::new(),
        }
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
}
