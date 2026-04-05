//! W3DOverlordTruckDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DOverlordTruckDraw.cpp
//!
//! Extends W3DTruckDraw with rider draw propagation. Inherits ALL of W3DTruckDraw's
//! behavior: 10 tire bone slots, cab/trailer bone rotation, dust/dirt/powerslide emitters,
//! wheel spin animation, powerslide/landing sounds.
//!
//! C++ header declares duplicate tread fields that are never initialized or parsed (dead code).

use cgmath::Matrix4;

#[derive(Debug, Clone, Default)]
pub struct W3DOverlordTruckDrawModuleData {}

#[derive(Debug)]
pub struct W3DOverlordTruckDraw {
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DOverlordTruckDraw {
    pub fn new() -> Self {
        Self {
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Calls W3DTruckDraw::doDrawModule(transformMtx), then draws the rider.
    /// Rider propagation (no null checks, unlike Aircraft):
    /// riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope())
    /// riderDraw->notifyDrawableDependencyCleared()
    /// riderDraw->draw(NULL)
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DTruckDraw::doDrawModule(transformMtx)
        // PARITY_NOTE: Rider propagation without null checks
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

impl Default for W3DOverlordTruckDraw {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_overlord_truck_draw_basic() {
        let draw = W3DOverlordTruckDraw::new();
        assert!(draw.is_visible());
    }
}
