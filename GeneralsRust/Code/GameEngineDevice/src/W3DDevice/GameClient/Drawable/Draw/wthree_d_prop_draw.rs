//! W3DPropDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DPropDraw.cpp
//!
//! W3DPropDraw does NOT manage its own RenderObjClass. It hands off the prop to
//! the terrain render system via `TheTerrainRenderObject->addProp()`. The terrain
//! system handles all rendering. This draw module is essentially a one-shot
//! registration mechanism.

use cgmath::{Matrix4, Point3};

/// W3DPropDrawModuleData fields (referenced from INI)
#[derive(Debug, Clone, Default)]
pub struct W3DPropDrawModuleData {
    /// Name of the W3D model to render as a prop (INI key: "ModelName")
    pub model_name: String,
}

/// W3DPropDraw implementation
///
/// C++ parity: The prop is rendered by the terrain system, not by this draw module.
/// `doDrawModule()` is a no-op. The key logic is in `reactToTransformChange()`
/// which performs one-shot registration with the terrain system.
#[derive(Debug)]
pub struct W3DPropDraw {
    /// One-shot flag: whether the prop has been registered with the terrain system
    prop_added: bool,
    /// Module data containing the model name
    module_data: W3DPropDrawModuleData,
    /// Whether hidden
    hidden: bool,
    /// Whether fully obscured by shroud
    fully_obscured_by_shroud: bool,
}

impl W3DPropDraw {
    /// Create new instance
    ///
    /// C++ parity: Constructor sets `m_propAdded = false`.
    pub fn new(module_data: W3DPropDrawModuleData) -> Self {
        Self {
            prop_added: false,
            module_data,
            hidden: false,
            fully_obscured_by_shroud: false,
        }
    }

    /// Create with default module data
    pub fn new_default() -> Self {
        Self::new(W3DPropDrawModuleData::default())
    }

    /// Main per-frame draw method
    ///
    /// C++ parity: **No-op**. Returns immediately. The prop is rendered by the
    /// terrain system, not by this draw module.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // C++ parity: void W3DPropDraw::doDrawModule(const Matrix3D*) { return; }
    }

    /// React to transform change
    ///
    /// C++ parity: On first call after position is set (not at 0,0), calls:
    /// ```cpp
    /// TheTerrainRenderObject->addProp(drawID, position, orientation, scale, modelName)
    /// ```
    /// Sets `m_propAdded = true` to prevent re-adding.
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
        if self.prop_added {
            return;
        }
        // PARITY_NOTE: In C++, the actual registration happens here:
        // if (position is not zero) {
        //     TheTerrainRenderObject->addProp(
        //         getDrawable()->getID(),
        //         *getDrawable()->getPosition(),
        //         getDrawable()->getOrientation(),
        //         getDrawable()->getScale(),
        //         getW3DPropDrawModuleData()->m_modelName
        //     );
        //     m_propAdded = true;
        // }
        //
        // For now, we just mark as added to prevent repeated calls.
        self.prop_added = true;
    }

    /// Set shadows enabled (no-op in C++)
    pub fn set_shadows_enabled(&mut self, _enable: bool) {
        // C++ parity: inline no-op
    }

    /// Release shadows (no-op in C++)
    pub fn release_shadows(&mut self) {
        // C++ parity: inline no-op
    }

    /// Allocate shadows (no-op in C++)
    pub fn allocate_shadows(&mut self) {
        // C++ parity: inline no-op
    }

    /// Set fully obscured by shroud (no-op in C++)
    pub fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {
        // C++ parity: inline no-op
    }

    /// React to geometry change (no-op in C++)
    pub fn react_to_geometry_change(&mut self) {
        // C++ parity: inline no-op
    }

    /// Set hidden state
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    /// Get module data reference
    pub fn get_module_data(&self) -> &W3DPropDrawModuleData {
        &self.module_data
    }

    /// CRC computation
    pub fn crc(&self) -> u32 {
        0
    }

    /// Save/load (Xfer)
    ///
    /// C++ parity: Version 1, calls DrawModule::xfer(xfer), no extra data
    pub fn xfer(&self) -> u32 {
        1
    }

    /// Post-load processing
    pub fn load_post_process(&mut self) {
        // C++ parity: calls DrawModule::loadPostProcess()
    }
}

impl Default for W3DPropDraw {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_prop_draw_basic() {
        let mut draw = W3DPropDraw::new_default();
        assert!(!draw.is_visible());
        draw.set_hidden(false);
        assert!(draw.is_visible());
        assert_eq!(draw.xfer(), 1);
    }
}
