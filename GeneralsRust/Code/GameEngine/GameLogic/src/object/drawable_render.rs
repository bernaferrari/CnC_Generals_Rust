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
        // C++ Drawable.cpp:2649 — applyPhysicsXform after instance, before modules.
        // Calc lives in GameClient (crate cycle). Nested Overlord rider draws
        // re-enter this function with the parent-corrected matrix when the
        // caller supplies one; host present path applies the exact calc.
        transform_mtx = drawable_physics_visual::apply_if_gated(transform_mtx);
        let logic_drawable_id = self.drawable_id;
        for (runtime_draw_ordinal, entry) in self
            .modules
            .iter()
            .filter(|entry| entry.mask().0 & ModuleInterfaceType::DRAW.0 != 0)
            .enumerate()
        {
            if let Some(client) = TheGameClient::get() {
                client.begin_active_object_model_draw(
                    self.object_id,
                    ModelDrawSourceIdentity {
                        runtime_draw_ordinal: runtime_draw_ordinal as u32,
                        module_name: entry.name().to_string(),
                        module_tag: entry.tag().to_string(),
                        module_tag_name_key: entry
                            .with_module(|module| module.get_module_tag_name_key()),
                    },
                );
            }
            entry.with_module(|module| {
                with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Laser(laser) => {
                        if let Some(input) = self.laser_draw_input(entry, laser.is_self_dirty()) {
                            laser.draw_from_update(input);
                        }
                    }
                    draw => draw
                        .into_draw_module()
                        .do_draw_module_for_owner(&transform_mtx, Some(self)),
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

impl Drawable {
    fn laser_draw_input(
        &self,
        current_draw: &DrawModuleEntry,
        self_dirty: bool,
    ) -> Option<crate::object::draw::w3d_laser_draw::LaserDrawInput> {
        // CPP W3DLaserDraw.cpp:228-249 queries this Drawable's CLIENT_UPDATE
        // bucket by LaserUpdate name, never the object registry or DRAW bucket.
        let laser_update_key = NameKeyGenerator::name_to_key("LaserUpdate");
        for entry in self
            .modules
            .iter()
            .filter(|entry| entry.mask().0 & ModuleInterfaceType::CLIENT_UPDATE.0 != 0)
        {
            // Never reacquire the canonical DRAW mutex, including malformed
            // modules advertising both interfaces. CPP queries a sibling.
            if std::ptr::eq(entry.as_ref(), current_draw) {
                continue;
            }
            let (matched, input) = entry.with_module(|module| {
                if module.get_module_name_key() != laser_update_key {
                    return (false, None);
                }
                let input = module.get_laser_update_interface().and_then(|update| {
                    crate::object::draw::w3d_laser_draw::LaserDrawInput::consume(update, self_dirty)
                });
                (true, input)
            });
            if matched {
                return input;
            }
        }
        None
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
