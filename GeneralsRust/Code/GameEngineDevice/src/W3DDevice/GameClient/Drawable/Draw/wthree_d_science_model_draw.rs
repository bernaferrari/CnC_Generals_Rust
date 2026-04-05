//! W3DScienceModelDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DScienceModelDraw.cpp
//!
//! A draw module identical to W3DModelDraw, except it only draws if the local player
//! has the specified Science. Used for science-gated visual elements.

use cgmath::Matrix4;

/// Invalid science sentinel (C++: SCIENCE_INVALID)
const SCIENCE_INVALID: u32 = 0xFFFFFFFF;

/// W3DScienceModelDrawModuleData - INI-parsed configuration
#[derive(Debug, Clone)]
pub struct W3DScienceModelDrawModuleData {
    /// Required science type (INI: "RequiredScience", parsed via INI::parseScience)
    /// SCIENCE_INVALID means not configured.
    pub required_science: u32,
}

impl Default for W3DScienceModelDrawModuleData {
    fn default() -> Self {
        Self {
            required_science: SCIENCE_INVALID,
        }
    }
}

/// W3DScienceModelDraw implementation
///
/// Extends W3DModelDraw with a science gate. The `doDrawModule()` method
/// checks if the local player has the required science before delegating to
/// W3DModelDraw::doDrawModule(). Observers (inactive players) CAN see science-gated objects.
#[derive(Debug)]
pub struct W3DScienceModelDraw {
    module_data: W3DScienceModelDrawModuleData,
    hidden: bool,
    fully_obscured_by_shroud: bool,
}

impl W3DScienceModelDraw {
    pub fn new(module_data: W3DScienceModelDrawModuleData) -> Self {
        Self {
            module_data,
            hidden: false,
            fully_obscured_by_shroud: false,
        }
    }

    pub fn new_default() -> Self {
        Self::new(W3DScienceModelDrawModuleData::default())
    }

    /// If SCIENCE_INVALID: DEBUG_ASSERTCRASH, setHidden(TRUE), return.
    /// If local player does NOT have the science AND player IS active: setHidden(TRUE), return.
    /// Otherwise: delegates to W3DModelDraw::doDrawModule().
    /// Key behavior: Observers (inactive players) CAN see science-gated objects.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        let required_science = self.module_data.required_science;

        if required_science == SCIENCE_INVALID {
            // C++: DEBUG_ASSERTCRASH(0, ("ScienceModelDraw requires a science"));
            self.hidden = true;
            return;
        }

        // PARITY_NOTE: Check ThePlayerList->getLocalPlayer()->hasScience(requiredScience)
        // and ThePlayerList->getLocalPlayer()->isPlayerActive()
        // If player active but doesn't have science: setHidden(TRUE), return
        // Otherwise: W3DModelDraw::doDrawModule(transformMtx)
    }

    pub fn set_shadows_enabled(&mut self, _enable: bool) {}
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

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    pub fn get_module_data(&self) -> &W3DScienceModelDrawModuleData {
        &self.module_data
    }

    pub fn crc(&self) -> u32 {
        0
    }
    pub fn xfer(&self) -> u32 {
        1
    }
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DScienceModelDraw {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_science_model_draw_basic() {
        let draw = W3DScienceModelDraw::new_default();
        assert_eq!(draw.get_module_data().required_science, SCIENCE_INVALID);
        assert!(draw.is_visible());
    }

    #[test]
    fn test_wthree_d_science_model_draw_invalid_science_hides() {
        let mut draw = W3DScienceModelDraw::new_default();
        draw.do_draw_module(&Matrix4::identity());
        assert!(draw.hidden);
        assert!(!draw.is_visible());
    }
}
