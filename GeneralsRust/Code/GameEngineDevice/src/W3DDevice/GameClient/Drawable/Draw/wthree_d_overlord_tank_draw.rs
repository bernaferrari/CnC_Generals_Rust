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

use cgmath::Matrix4;

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
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DTankDraw::doDrawModule(transformMtx)

        // PARITY_NOTE: Rider draw propagation (no null checks, unlike Aircraft):
        // Object *me = getDrawable()->getObject();
        // if (me && me->getContain()) {
        //     Object *rider = me->getContain()->friend_getRider();
        //     if (rider && rider->getDrawable()) {
        //         Drawable *riderDraw = rider->getDrawable();
        //         riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope());
        //         riderDraw->notifyDrawableDependencyCleared();
        //         riderDraw->draw(NULL);
        //     }
        // }
    }

    /// Calls W3DTankDraw::setHidden(h), then propagates to rider.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        // PARITY_NOTE: riderDraw->setDrawableHidden(h)
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
        draw.set_hidden(true);
        assert!(!draw.is_visible());
        draw.do_draw_module(&Matrix4::identity());
    }
}
