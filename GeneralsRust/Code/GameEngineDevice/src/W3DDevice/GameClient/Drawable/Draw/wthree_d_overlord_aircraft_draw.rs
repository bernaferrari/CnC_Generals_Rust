//! W3DOverlordAircraftDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DOverlordAircraftDraw.cpp
//!
//! Extends W3DModelDraw with rider draw propagation. Inherits from W3DModelDraw directly
//! (NOT W3DTruckDraw), so does NOT get wheel rotation, treads, or particle emitters.

use cgmath::Matrix4;

#[derive(Debug, Clone, Default)]
pub struct W3DOverlordAircraftDrawModuleData {}

/// Extends W3DModelDraw. The only unique logic is rider draw propagation:
/// after the Overlord aircraft draws, the contained rider is also drawn
/// with the parent's color tint.
#[derive(Debug)]
pub struct W3DOverlordAircraftDraw {
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DOverlordAircraftDraw {
    pub fn new() -> Self {
        Self {
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Calls W3DModelDraw::doDrawModule(transformMtx), then draws the rider.
    /// Rider access: getDrawable()->getObject()->getContain()->friend_getRider()->getDrawable()
    /// Copies tint, clears dependency, calls riderDraw->draw(NULL).
    /// Note: C++ has a DEBUG_ASSERTCRASH after the null check (dead assert).
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        // PARITY_NOTE: Rider propagation with null checks on riderDraw and tintEnvelope
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
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
        _old_pos: &cgmath::Point3<f32>,
        _old_angle: f32,
    ) {
    }
    pub fn react_to_geometry_change(&mut self) {}
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }
    pub fn crc(&self) -> u32 {
        0
    }
    pub fn xfer(&self) -> u32 {
        1
    }
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DOverlordAircraftDraw {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_overlord_aircraft_draw_basic() {
        let mut draw = W3DOverlordAircraftDraw::new();
        assert!(draw.is_visible());
        draw.set_hidden(true);
        assert!(!draw.is_visible());
        draw.do_draw_module(&Matrix4::identity());
    }
}
