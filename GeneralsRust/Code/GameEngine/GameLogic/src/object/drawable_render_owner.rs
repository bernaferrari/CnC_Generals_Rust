//! Borrowed owner operations for one synchronous draw-module callback.

use super::*;

/// The driving Drawable and its currently borrowed module entry.
/// Neither reference is retained; construction cannot publish ambient state.
pub struct DrawableRenderOwner<'a> {
    drawable: &'a Drawable,
    current: &'a DrawModuleEntry,
}

impl<'a> DrawableRenderOwner<'a> {
    pub(super) fn new(drawable: &'a Drawable, current: &'a DrawModuleEntry) -> Self {
        Self { drawable, current }
    }

    pub(crate) fn get_should_animate(&self, consider_power: bool) -> bool {
        self.drawable.get_should_animate(consider_power)
    }

    pub(crate) fn get_instance_scale(&self) -> Real {
        self.drawable.get_instance_scale()
    }

    pub(crate) fn test_drawable_status(&self, status: u32) -> bool {
        self.drawable.test_drawable_status(status)
    }

    pub(crate) fn get_drawable_id(&self) -> DrawableID {
        self.drawable.get_drawable_id()
    }

    pub(crate) fn is_drawable_effectively_hidden(&self) -> bool {
        self.drawable.is_drawable_effectively_hidden()
    }

    pub(crate) fn get_bone_local_transform(&self, name: &str) -> Option<Matrix3D> {
        self.drawable.get_bone_local_transform(name)
    }

    pub(crate) fn get_bone_transform(&self, name: &str) -> Option<Matrix3D> {
        self.drawable.get_bone_transform(name)
    }

    /// Preserve ordered current-bone queries while the active entry is borrowed.
    pub(crate) fn get_current_worldspace_client_bone_positions(
        &self,
        current: &dyn ObjectDrawInterface,
        name: &str,
    ) -> Option<Matrix3D> {
        self.drawable
            .get_current_worldspace_client_bone_positions_for_active(
                Some((self.current, current)),
                name,
            )
    }

    /// Query the original module order without locking the active entry again.
    /// The active interface must belong to the current module (including its base).
    pub(crate) fn get_pristine_bone_positions(
        &self,
        current: &dyn ObjectDrawInterface,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<Coord3D> {
        self.drawable.get_pristine_bone_positions_for_active(
            Some((self.current, current)),
            bone_name_prefix,
            start_index,
            max_bones,
        )
    }
}
