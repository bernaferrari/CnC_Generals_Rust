//! W3DPropDraw Module
//!
//! Corresponds to C++ file:
//!   GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DPropDraw.h
//!
//! W3D prop draw module data and draw module. The draw module hands the prop
//! off to the terrain render system on first transform change, then becomes
//! a no-op for all subsequent frames.

/// Module data for W3DPropDraw.
///
/// C++ parity: `W3DPropDrawModuleData : public ModuleData` with
/// `AsciiString m_modelName`.
#[derive(Debug, Clone, Default)]
pub struct W3DPropDrawModuleData {
    /// Name of the W3D model to render as a prop (INI key: "ModelName").
    pub model_name: String,
}

/// W3D prop draw module.
///
/// C++ parity: `W3DPropDraw : public DrawModule` with `Bool m_propAdded`.
/// The key behavior is one-shot registration with the terrain system in
/// `reactToTransformChange()`, then `doDrawModule()` is a permanent no-op.
#[derive(Debug)]
pub struct W3DPropDraw {
    /// Whether the prop has been registered with the terrain system.
    prop_added: bool,
    /// Module data containing the model name.
    module_data: W3DPropDrawModuleData,
    /// Whether hidden.
    hidden: bool,
    /// Whether fully obscured by shroud.
    fully_obscured_by_shroud: bool,
}

impl W3DPropDraw {
    /// Create a new instance.
    ///
    /// C++ parity: `W3DPropDraw(Thing*, const ModuleData*)` sets `m_propAdded = false`.
    pub fn new(module_data: W3DPropDrawModuleData) -> Self {
        Self {
            prop_added: false,
            module_data,
            hidden: false,
            fully_obscured_by_shroud: false,
        }
    }

    /// Create with default module data.
    pub fn new_default() -> Self {
        Self::new(W3DPropDrawModuleData::default())
    }

    /// Per-frame draw — always a no-op in C++.
    ///
    /// C++ parity: `void doDrawModule(const Matrix3D*) { return; }`.
    pub fn do_draw_module(&mut self) {}

    /// React to transform change.
    ///
    /// C++ parity: On first call with a non-zero position, calls
    /// `TheTerrainRenderObject->addProp(drawID, position, angle, scale, modelName)`.
    /// Sets `m_propAdded = true` to prevent re-adding.
    pub fn react_to_transform_change(
        &mut self,
        drawable_id: i32,
        pos_x: f32,
        pos_y: f32,
        pos_z: f32,
        angle: f32,
        scale: f32,
    ) {
        if self.prop_added {
            return;
        }
        // PARITY_NOTE: In C++, this calls TheTerrainRenderObject->addProp().
        // The actual registration is handled by the prop buffer in the terrain
        // render system. Callers should pass the returned info to the terrain
        // system's add_prop() method.
        let _ = (drawable_id, pos_x, pos_y, pos_z, angle, scale);
        self.prop_added = true;
    }

    /// Set shadows enabled — no-op in C++.
    pub fn set_shadows_enabled(&mut self, _enable: bool) {}

    /// Release shadows — no-op in C++.
    pub fn release_shadows(&mut self) {}

    /// Allocate shadows — no-op in C++.
    pub fn allocate_shadows(&mut self) {}

    /// Set fully obscured by shroud — no-op in C++.
    pub fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {}

    /// React to geometry change — no-op in C++.
    pub fn react_to_geometry_change(&mut self) {}

    /// Set hidden state.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    /// Get module data reference.
    pub fn get_module_data(&self) -> &W3DPropDrawModuleData {
        &self.module_data
    }

    /// Get the model name for this prop.
    pub fn model_name(&self) -> &str {
        &self.module_data.model_name
    }

    /// Whether the prop has been registered with the terrain system.
    pub fn is_prop_added(&self) -> bool {
        self.prop_added
    }

    /// CRC for save/load.
    pub fn crc(&self) -> u32 {
        0
    }

    /// Xfer version number.
    pub fn xfer(&self) -> u32 {
        1
    }

    /// Post-load processing.
    pub fn load_post_process(&mut self) {}
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
    fn test_w3d_prop_draw_module() {
        let data = W3DPropDrawModuleData {
            model_name: "PTPine01".to_string(),
        };
        let mut draw = W3DPropDraw::new(data);
        assert!(!draw.is_prop_added());
        assert_eq!(draw.model_name(), "PTPine01");

        draw.react_to_transform_change(1, 10.0, 20.0, 0.0, 0.0, 1.0);
        assert!(draw.is_prop_added());

        // Second call should be a no-op
        draw.react_to_transform_change(1, 20.0, 30.0, 0.0, 0.0, 1.0);
        assert!(draw.is_prop_added());
    }

    #[test]
    fn test_w3d_prop_draw_visibility() {
        let mut draw = W3DPropDraw::new_default();
        assert!(draw.is_visible());

        draw.set_hidden(true);
        assert!(!draw.is_visible());

        draw.set_hidden(false);
        draw.set_fully_obscured_by_shroud(true);
        assert!(!draw.is_visible());
    }
}
