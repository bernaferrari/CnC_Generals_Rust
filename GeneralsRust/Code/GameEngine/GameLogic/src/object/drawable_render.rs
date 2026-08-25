//! Rendering bridge for the GameLogic drawable trait and default materials.
//!
//! Actual GPU work remains delegated to draw modules/GameClient; this module
//! preserves the existing draw scheduling and WGPU-facing bridge semantics.

use super::*;

impl crate::drawable::Drawable for Drawable {
    /// Draw the drawable at a specific position
    /// Reference: C++ Drawable.cpp - rendering is delegated to draw modules
    fn draw(&mut self, transform: Option<&Matrix3D>) {
        #[cfg(test)]
        let _draw_depth = super::draw_call_log::enter(self.drawable_id);

        // This happens before the hidden early-out.  The bridge represents the
        // current C++ Drawable::draw() result, not the last visible frame.
        if let Some(client) = TheGameClient::get() {
            client.begin_object_model_draw_frame(self.object_id);
        }

        let object_effectively_dead = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| object.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false);

        // C++ Drawable::draw parity: fade thermal/second pass unless frenzy tint is active.
        if !self.test_tint_status(TintStatus::FRENZY) {
            if object_effectively_dead {
                self.second_material_pass_opacity = 0.0;
            } else if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }

        if self.hidden || self.hidden_by_stealth || self.drawable_fully_obscured_by_shroud {
            return;
        }

        if self.object_ref.is_some() && !object_effectively_dead {
            self.set_shadows_enabled(!matches!(
                self.stealth_look,
                StealthLookType::VisibleDetected
            ));
        }

        let mut transform_mtx = transform.copied().unwrap_or(self.transform);
        if let Some(instance_mtx) = self.instance_matrix {
            transform_mtx = transform_mtx * instance_mtx;
        }
        if (self.instance_scale - 1.0).abs() > f32::EPSILON {
            // C++ Drawable draw: instance scale is applied after the instance matrix.
            transform_mtx =
                transform_mtx * Matrix3D::from_scale(glam::Vec3::splat(self.instance_scale));
        }
        // C++ Drawable.cpp:2649 — applyPhysicsXform after instance, before modules.
        // Calc lives in GameClient (crate cycle). Nested Overlord rider draws
        // re-enter this function with the parent-corrected matrix when the
        // caller supplies one; host present path applies the exact calc.
        transform_mtx = drawable_physics_visual::apply_if_gated(transform_mtx);
        let logic_drawable_id = self.drawable_id;
        for (runtime_draw_ordinal, module_handle) in self
            .get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
            .into_iter()
            .enumerate()
        {
            if let Some(client) = TheGameClient::get() {
                client.begin_active_object_model_draw(
                    self.object_id,
                    ModelDrawSourceIdentity {
                        runtime_draw_ordinal: runtime_draw_ordinal as u32,
                        module_name: module_handle.name().to_string(),
                        module_tag: module_handle.tag().to_string(),
                        module_tag_name_key: module_handle.module_tag_key(),
                    },
                );
            }
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| {
                    draw.do_draw_module(&transform_mtx);
                });
            });
            if let Some(client) = TheGameClient::get() {
                client.commit_active_object_model_draw(self.object_id, logic_drawable_id);
            }
        }
    }

    fn is_visible(&self) -> bool {
        self.is_visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    /// Get current world transform
    fn get_transform(&self) -> Matrix3D {
        self.transform
    }
}

impl Material {
    /// Create a default material
    pub fn default() -> Self {
        Material {
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            diffuse_color: Color::white(),
            specular_color: Color::white(),
            emissive_color: Color::black(),
            shininess: 32.0,
            transparency: 0.0,
            reflectivity: 0.0,
            texture_scale: Coord2D::new(1.0, 1.0),
            texture_offset: Coord2D::ZERO,
            animation_rate: 0.0,
        }
    }
}
