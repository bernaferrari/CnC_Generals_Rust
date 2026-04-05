//! W3DDependencyModelDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDependencyModelDraw.cpp
//!
//! A draw module identical to W3DModelDraw, except it cannot draw unless another
//! object explicitly clears the dependency. Primary use: passengers inside a transport
//! container whose draw order depends on the container drawing first.

use cgmath::{Matrix4, Point3};

/// W3DDependencyModelDrawModuleData - INI-parsed configuration
#[derive(Debug, Clone, Default)]
pub struct W3DDependencyModelDrawModuleData {
    /// Bone name in the container to attach to (INI: "AttachToBoneInContainer")
    pub attach_to_drawable_bone_in_container: String,
}

/// W3DDependencyModelDraw implementation
///
/// Extends W3DModelDraw with a dependency gate. `doDrawModule()` only draws
/// when `dependency_cleared` is true, then re-latches it to false.
/// `adjust_transform_mtx()` positions this object at the container's bone when attached.
#[derive(Debug)]
pub struct W3DDependencyModelDraw {
    module_data: W3DDependencyModelDrawModuleData,
    /// Set by external caller; re-latched to false after draw
    dependency_cleared: bool,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DDependencyModelDraw {
    pub fn new(module_data: W3DDependencyModelDrawModuleData) -> Self {
        Self {
            module_data,
            dependency_cleared: false,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    pub fn new_default() -> Self {
        Self::new(W3DDependencyModelDrawModuleData::default())
    }

    /// Only draws when dependency_cleared is true, then re-latches.
    /// After drawing, syncs stealth appearance with container.
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix4<f32>) {
        if !self.dependency_cleared {
            return;
        }

        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        self.dependency_cleared = false;

        // PARITY_NOTE: Sync stealth with container:
        // Object *me = getDrawable()->getObject();
        // if (me->getContain() && !me->getContain()->isEnclosingContainerForSomethingElse()) {
        //     myDrawable->imitateStealthLook(*containerDrawable);
        // }
    }

    /// Sets dependency_cleared = true. Called by container after it finishes drawing.
    pub fn notify_draw_module_dependency_cleared(&mut self) {
        self.dependency_cleared = true;
    }

    /// If attach_to_drawable_bone_in_container is set and object is contained,
    /// overrides transform with container's bone transform (or container transform as fallback).
    pub fn adjust_transform_mtx(&self, _mtx: &mut Matrix4<f32>) {
        // PARITY_NOTE: Full implementation:
        // 1. W3DModelDraw::adjustTransformMtx(mtx)
        // 2. If attach_to_drawable_bone_in_container non-empty and contained:
        //    mtx = containerDrawable->getCurrentWorldspaceClientBonePositions(boneName)
        //    or fallback: mtx = containerDrawable->getTransformMatrix()
    }

    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
    }
    pub fn react_to_geometry_change(&mut self) {}

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    pub fn get_module_data(&self) -> &W3DDependencyModelDrawModuleData {
        &self.module_data
    }

    pub fn is_dependency_cleared(&self) -> bool {
        self.dependency_cleared
    }

    pub fn crc(&self) -> u32 {
        0
    }

    /// Save: returns (version, dependency_cleared)
    pub fn xfer_save(&self) -> (u32, bool) {
        (1, self.dependency_cleared)
    }

    /// Load: restores dependency_cleared from xfer data
    pub fn xfer_load(&mut self, _version: u32, dependency_cleared: bool) {
        self.dependency_cleared = dependency_cleared;
    }

    pub fn load_post_process(&mut self) {}
}

impl Default for W3DDependencyModelDraw {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_dependency_model_draw_basic() {
        let mut draw = W3DDependencyModelDraw::new_default();
        assert!(!draw.is_dependency_cleared());

        draw.do_draw_module(&Matrix4::identity());
        assert!(!draw.is_dependency_cleared());

        draw.notify_draw_module_dependency_cleared();
        assert!(draw.is_dependency_cleared());
        draw.do_draw_module(&Matrix4::identity());
        assert!(!draw.is_dependency_cleared());
    }

    #[test]
    fn test_wthree_d_dependency_model_draw_xfer() {
        let mut draw = W3DDependencyModelDraw::new_default();
        draw.notify_draw_module_dependency_cleared();
        let (version, cleared) = draw.xfer_save();
        assert_eq!(version, 1);
        assert!(cleared);

        let mut draw2 = W3DDependencyModelDraw::new_default();
        draw2.xfer_load(version, cleared);
        assert!(draw2.is_dependency_cleared());
    }
}
