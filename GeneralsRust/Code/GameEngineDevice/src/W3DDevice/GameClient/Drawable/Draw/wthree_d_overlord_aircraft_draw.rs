//! W3DOverlordAircraftDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DOverlordAircraftDraw.cpp
//!
//! Extends W3DModelDraw with rider draw propagation. Inherits from W3DModelDraw directly
//! (NOT W3DTruckDraw), so does NOT get wheel rotation, treads, or particle emitters.

use cgmath::{Matrix4, Point3};

pub use super::wthree_d_overlord_tank_draw::OverlordRiderState;

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

    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        rider_draw: &mut Option<OverlordRiderState>,
        has_tint_envelope: bool,
    ) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        // C++ has extra null checks on riderDraw AND tintEnvelope (unlike Tank/Truck)
        // C++ also has a DEBUG_ASSERTCRASH after the null check (dead assert)
        if let Some(rider) = rider_draw {
            if has_tint_envelope {
                // PARITY_NOTE: riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope())
            }
            // PARITY_NOTE: riderDraw->notifyDrawableDependencyCleared()
            // PARITY_NOTE: riderDraw->draw(NULL)
            rider.draw_requested = true;
        }
        // PARITY_NOTE: DEBUG_ASSERTCRASH(riderDraw, ("OverlordAircraftDraw finds no rider's drawable"))
    }

    pub fn set_hidden(&mut self, hidden: bool, rider: Option<&mut OverlordRiderState>) {
        self.hidden = hidden;
        if let Some(r) = rider {
            r.hidden = hidden;
        }
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
        draw.set_hidden(true, None);
        assert!(!draw.is_visible());
        let mut rider = Some(OverlordRiderState::new());
        draw.do_draw_module(&Matrix4::identity(), &mut rider, true);
        assert!(rider.unwrap().draw_requested);
    }
}
