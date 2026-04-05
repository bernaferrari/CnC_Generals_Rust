//! W3DDefaultDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDefaultDraw.cpp
//!
//! This is a test/debug draw module, only active when `LOAD_TEST_ASSETS` is defined.
//! In production, it does nothing. It is the simplest possible DrawModule --
//! just holds a render object and shadow when test assets are loaded.

use crate::W3DDevice::GameClient::wthree_d_scene::{DrawableInfo, RenderObjectId};
use cgmath::{Matrix4, Point3, Vector3};

/// W3DDefaultDraw implementation
///
/// C++ parity note: In the C++ code, the render object and shadow are only created
/// under `#ifdef LOAD_TEST_ASSETS`. In production builds, this draw module is a no-op.
#[derive(Debug)]
pub struct W3DDefaultDraw {
    /// PARITY_NOTE: In C++, this is `RenderObjClass* m_renderObject` (only under LOAD_TEST_ASSETS)
    render_object_id: Option<RenderObjectId>,
    /// PARITY_NOTE: In C++, this is `Shadow* m_shadow` (only under LOAD_TEST_ASSETS)
    shadow_id: Option<RenderObjectId>,
    /// Whether the module is hidden
    hidden: bool,
    /// Whether fully obscured by shroud
    fully_obscured_by_shroud: bool,
    /// Whether shadows are enabled
    shadow_enabled: bool,
    /// PARITY_NOTE: Whether LOAD_TEST_ASSETS is defined (always false in production)
    #[cfg(feature = "load_test_assets")]
    test_assets_enabled: bool,
}

impl W3DDefaultDraw {
    /// Create new instance
    ///
    /// C++ parity: Constructor only creates render objects under `#ifdef LOAD_TEST_ASSETS`.
    /// It loads the LTA model from ThingTemplate, creates a shadow, adds to scene.
    pub fn new() -> Self {
        Self {
            render_object_id: None,
            shadow_id: None,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
            #[cfg(feature = "load_test_assets")]
            test_assets_enabled: false,
        }
    }

    /// Main per-frame draw method
    ///
    /// C++ parity: Under LOAD_TEST_ASSETS, applies instance scale to transform.
    /// In production, this is a no-op.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS defined, this would:
        // 1. Get instance scale from drawable
        // 2. If scale != 1.0, apply scaled transform to render object
        // 3. Set render object transform via m_renderObject->Set_Transform()
        //
        // In production (no LOAD_TEST_ASSETS), this is a no-op.
    }

    /// React to transform change
    ///
    /// C++ parity: Under LOAD_TEST_ASSETS, calls
    /// `m_renderObject->Set_Transform(*getDrawable()->getTransformMatrix())`
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS:
        // m_renderObject->Set_Transform(*getDrawable()->getTransformMatrix());
    }

    /// Set shadows enabled
    ///
    /// C++ parity: Under LOAD_TEST_ASSETS, calls `m_shadow->enableShadowRender(enable)`
    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS:
        // if (m_shadow) m_shadow->enableShadowRender(enable);
    }

    /// Release shadows (for options screen)
    pub fn release_shadows(&mut self) {
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS, releases shadow resources
    }

    /// Allocate shadows (for options screen)
    pub fn allocate_shadows(&mut self) {
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS, re-creates shadow if needed
    }

    /// Set fully obscured by shroud
    ///
    /// C++ parity: Under LOAD_TEST_ASSETS, calls `m_shadow->enableShadowInvisible(fullyObscured)`
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS:
        // if (m_shadow) m_shadow->enableShadowInvisible(fullyObscured);
    }

    /// React to geometry change (no-op in C++)
    pub fn react_to_geometry_change(&mut self) {}

    /// Set hidden state
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }

    /// CRC computation
    pub fn crc(&self) -> u32 {
        // PARITY_NOTE: In C++, calls DrawModule::crc(xfer)
        0
    }

    /// Save/load (Xfer)
    ///
    /// C++ parity: Version 1, calls DrawModule::xfer(xfer), no extra data
    pub fn xfer(&self) -> u32 {
        1 // version
    }

    /// Post-load processing
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: In C++, calls DrawModule::loadPostProcess()
    }

    /// Cleanup on delete
    pub fn on_delete(&mut self) {
        // PARITY_NOTE: In C++ with LOAD_TEST_ASSETS:
        // - Removes shadow from TheW3DShadowManager
        // - Removes render object from W3DDisplay::m_3DScene
        // - REF_PTR_RELEASE(m_renderObject)
        if let Some(id) = self.render_object_id.take() {
            // PARITY_NOTE: W3DDisplay::global_scene().write().remove_render_object(id);
            let _ = id; // suppress unused warning
        }
        if let Some(id) = self.shadow_id.take() {
            let _ = id;
        }
    }
}

impl Default for W3DDefaultDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DDefaultDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_default_draw_basic() {
        let mut draw = W3DDefaultDraw::new();
        assert!(!draw.is_visible());
        draw.set_hidden(false);
        assert!(draw.is_visible());
        draw.set_fully_obscured_by_shroud(true);
        assert!(!draw.is_visible());
    }
}
