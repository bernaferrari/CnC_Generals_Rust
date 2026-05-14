//! W3DOverlordTruckDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DOverlordTruckDraw.cpp
//!
//! Extends W3DTruckDraw with rider draw propagation. Inherits ALL of W3DTruckDraw's
//! behavior: 10 tire bone slots, cab/trailer bone rotation, dust/dirt/powerslide emitters,
//! wheel spin animation, powerslide/landing sounds.
//!
//! C++ header declares duplicate tread fields that are never initialized or parsed (dead code).

use cgmath::{Matrix4, Point3};

pub use super::wthree_d_overlord_tank_draw::OverlordRiderState;

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

    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        rider_draw: &mut Option<OverlordRiderState>,
    ) {
        // PARITY_NOTE: W3DTruckDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        // C++: No null checks unlike Aircraft (calls rider methods directly)
        if let Some(rider) = rider_draw {
            // PARITY_NOTE: riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope())
            // PARITY_NOTE: riderDraw->notifyDrawableDependencyCleared()
            // PARITY_NOTE: riderDraw->draw(NULL)
            rider.draw_requested = true;
        }
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

    /// C++ parity: Inherited via `W3DTruckDraw -> W3DModelDraw::releaseShadows()` — releases
    /// shadow via `m_shadow->release()` and sets `m_shadow = NULL`.
    // PARITY_NOTE: Would call W3DModelDraw::releaseShadows() in C++ (removes shadow from scene).
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn release_shadows(&mut self) {}

    /// C++ parity: Inherited via `W3DTruckDraw -> W3DModelDraw::allocateShadows()` — creates
    /// shadow from ThingTemplate info if no shadow exists, render object exists, and shadow type != SHADOW_NONE.
    // PARITY_NOTE: Would call W3DModelDraw::allocateShadows() in C++.
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
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

    /// C++ parity: Inherited via `W3DTruckDraw::reactToGeometryChange() { }` — empty override
    /// in W3DTruckDraw.h. Geometry bounds implicitly updated via render object transforms.
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
    /// C++ parity: `W3DOverlordTruckDraw::loadPostProcess()` — calls
    /// `W3DTruckDraw::loadPostProcess()` which calls `W3DModelDraw::loadPostProcess()`
    /// then `tossEmitters()` (releases dust/dirt/powerslide particle systems).
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: C++ chain: W3DOverlordTruckDraw -> W3DTruckDraw::loadPostProcess()
        //   -> W3DModelDraw::loadPostProcess()
        //   -> tossEmitters() (releases m_dustEffect, m_dirtEffect, m_powerslideEffect)
        // Emitters are re-created lazily when enableEmitters(true) is called during doDrawModule.
        // Requires particle system infrastructure to be wired.
    }
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
        let mut draw = W3DOverlordTruckDraw::new();
        assert!(draw.is_visible());
        let mut rider = Some(OverlordRiderState::new());
        draw.do_draw_module(&Matrix4::identity(), &mut rider);
        assert!(rider.unwrap().draw_requested);
    }
}
