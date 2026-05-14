//! W3DOverlordTankDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DOverlordTankDraw.cpp
//!
//! Extends W3DTankDraw with rider draw propagation. The Overlord tank contains a rider
//! (the turret) via ContainModule. This class ensures the rider draws after the Overlord
//! itself (so the turret renders on top of the tank body).
//!
//! Note: W3DOverlordTankDrawModuleData has duplicate tread fields in the C++ header that
//! are never initialized or parsed via INI. These are NOT ported.

use cgmath::{Matrix4, Point3};

/// State for the Overlord's rider drawable (the turret thing).
/// C++ accesses this via me->getContain()->friend_getRider()->getDrawable().
pub struct OverlordRiderState {
    pub draw_requested: bool,
    pub hidden: bool,
}

impl OverlordRiderState {
    pub fn new() -> Self {
        Self {
            draw_requested: false,
            hidden: false,
        }
    }
}

/// W3DOverlordTankDrawModuleData (extends W3DTankDrawModuleData with no extra INI fields)
#[derive(Debug, Clone, Default)]
pub struct W3DOverlordTankDrawModuleData {}

/// W3DOverlordTankDraw implementation
///
/// Inherits from W3DTankDraw. The only unique logic is rider draw propagation.
/// Constructor and destructor are empty (parent handles everything).
#[derive(Debug)]
pub struct W3DOverlordTankDraw {
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DOverlordTankDraw {
    pub fn new() -> Self {
        Self {
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Calls W3DTankDraw::doDrawModule(transformMtx) (handles all tank drawing + tread animation),
    /// then draws the rider.
    ///
    /// Rider draw logic (same pattern as OverlordAircraftDraw but WITHOUT null checks
    /// on riderDraw and colorTintEnvelope - C++ calls them directly):
    /// 1. riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope())
    /// 2. riderDraw->notifyDrawableDependencyCleared()
    /// 3. riderDraw->draw(NULL)
    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        rider_draw: &mut Option<OverlordRiderState>,
    ) {
        // PARITY_NOTE: W3DTankDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        // C++: get rider via me->getContain()->friend_getRider()->getDrawable()
        // No null checks unlike Aircraft (C++ calls rider methods directly)
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

    /// C++ parity: Inherited via `W3DTankDraw -> W3DModelDraw::releaseShadows()` — releases
    /// shadow via `m_shadow->release()` and sets `m_shadow = NULL`.
    // PARITY_NOTE: Would call W3DModelDraw::releaseShadows() in C++ (removes shadow from scene).
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn release_shadows(&mut self) {}

    /// C++ parity: Inherited via `W3DTankDraw -> W3DModelDraw::allocateShadows()` — creates
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

    /// C++ parity: Inherited from `W3DModelDraw::reactToGeometryChange() { }` — no override
    /// in W3DOverlordTankDraw.h. Geometry bounds implicitly updated via render object transforms.
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
    /// C++ parity: `W3DOverlordTankDraw::loadPostProcess()` — calls
    /// `W3DTankDraw::loadPostProcess()` which calls `W3DModelDraw::loadPostProcess()`
    /// then `tossEmitters()` + `createEmitters()` (re-creates tread debris particle systems).
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: C++ chain: W3DOverlordTankDraw -> W3DTankDraw::loadPostProcess()
        //   -> W3DModelDraw::loadPostProcess()
        //   -> tossEmitters() + createEmitters() (tread debris)
        // Requires particle system infrastructure to be wired.
    }
}

impl Default for W3DOverlordTankDraw {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_overlord_tank_draw_basic() {
        let mut draw = W3DOverlordTankDraw::new();
        assert!(draw.is_visible());
        draw.set_hidden(true, None);
        assert!(!draw.is_visible());
        let mut rider = Some(OverlordRiderState::new());
        draw.do_draw_module(&Matrix4::identity(), &mut rider);
        assert!(rider.unwrap().draw_requested);
    }
}
