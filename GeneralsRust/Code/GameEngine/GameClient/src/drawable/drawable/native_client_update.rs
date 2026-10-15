//! Receiving Drawable client-module phase and original subsequent visual update.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;
use gamelogic::common::{Coord3D, Matrix3D};
use gamelogic::object::draw::draw_module::ClientParticleBoneRuntime;
use gamelogic::object::update::AnimatedParticleSysBoneClientUpdateModule;

struct ReceivingBonePose<'a> {
    world: glam::Mat4,
    assets: Option<&'a ww3d_assets::AssetManager>,
}

impl ReceivingBonePose<'_> {
    fn bone_transform(&self, model: &str, index: i32) -> Option<Matrix3D> {
        // C++ reads this selected render object's stored bone index directly.
        // The admitted native source currently supplies static bind poses;
        // live animation remains under Main's existing playback owner.
        let local = crate::drawable::logic_visual_hooks::lookup_w3d_client_bone_index_in_assets(
            self.assets?,
            model,
            index,
        )?;
        Some(self.world * local)
    }
}

struct ReceivingParticleBones<'a, 'b> {
    particles: Option<&'a mut ParticleSystemManager>,
    pose: &'a ReceivingBonePose<'b>,
}

impl ClientParticleBoneRuntime for ReceivingParticleBones<'_, '_> {
    fn particle_system_exists(&mut self, id: u32) -> bool {
        self.particles
            .as_deref()
            .is_some_and(|manager| manager.find_particle_system(id).is_some())
    }

    fn bone_transform(&mut self, model: &str, index: i32, _name: &str) -> Option<Matrix3D> {
        self.pose.bone_transform(model, index)
    }

    fn set_particle_bone_transform(
        &mut self,
        id: u32,
        position: &Coord3D,
        z_rotation: f32,
        transform: &Matrix3D,
    ) {
        let Some(system) = self
            .particles
            .as_deref_mut()
            .and_then(|manager| manager.find_particle_system_mut(id))
        else {
            return;
        };
        // W3DModelDraw.cpp:2687-2693, including the original setter order.
        system.set_position(glam::Vec3::new(position.x, position.y, position.z));
        system.rotate_local_transform_z(z_rotation);
        system.set_local_transform(glam::Mat3::from_mat4(*transform));
        system.set_skip_parent_xfrm(true);
    }
}

impl BasicDrawable {
    /// C++ Drawable::getTransformMatrix returns the underlying Thing/Object
    /// transform, separate from instance scale and render-only recoil.
    pub(crate) fn native_particle_parent_transform(&self) -> glam::Mat4 {
        Matrix4::translation(self.position)
            .mul(&self.instance_transform)
            .to_glam()
    }

    /// Installed native ClientUpdate phase, before fade/decal/tint. The manager
    /// is borrowed by the driving Client; callbacks never rediscover its owner.
    pub(crate) fn update_native_client_modules(
        &mut self,
        mut particles: Option<&mut ParticleSystemManager>,
        host_projection: bool,
        assets: Option<&ww3d_assets::AssetManager>,
    ) -> Result<(), String> {
        if let Some(manager) = particles.as_deref_mut() {
            self.synchronize_native_particle_visibility(manager);
        } else if self.native_state_particle_count() != 0 {
            return Err(
                "receiving particle manager is absent for installed native emitters".into(),
            );
        }
        let mut world = self.get_transform().to_glam();
        if host_projection {
            // Main direct visuals are Y-up; authored particles/bones are C++ Z-up.
            let swizzle =
                glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
            world = swizzle * world * swizzle;
        }
        let pose = ReceivingBonePose { world, assets };
        for index in 0..self.draw_modules.len() {
            let animated = self.draw_modules[index].drawable_module_type_index() == 1
                && self.draw_modules[index]
                    .logic_module_mut()
                    .and_then(|module| {
                        module
                            .as_any_mut()
                            .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                    })
                    .is_some();
            if !animated {
                continue;
            }
            // Life belongs to this admitted module. It is deliberately absent
            // from native Xfer, matching the original version-only payload.
            self.draw_modules[index]
                .logic_module_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                .unwrap()
                .client_update_with(|| {});
            for draw_index in 0..self.draw_modules.len() {
                if self.draw_modules[draw_index].drawable_module_type_index() != 0 {
                    continue;
                }
                let Some(module) = self.draw_modules[draw_index].logic_module_mut() else {
                    continue;
                };
                let mut runtime = ReceivingParticleBones {
                    particles: particles.as_deref_mut(),
                    pose: &pose,
                };
                if gamelogic::object::drawable::update_client_particle_bones(module, &mut runtime) {
                    break;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn update_after_client_modules(&mut self, logic_frame: u32) {
        self.update_fade();
        self.flush_dirty_model_condition();

        if self.terrain_decal_type != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                if let Some(dm) = self.draw_modules.first_mut() {
                    dm.set_terrain_decal_opacity(self.decal_opacity);
                }
                if let Some(handle) = &self.terrain_decal_handle {
                    handle.set_opacity((self.decal_opacity.clamp(0.0, 1.0) * 255.0) as i32);
                }
                self.decal_opacity += self.decal_opacity_fade_rate;
                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.set_terrain_decal(TerrainDecalType::None);
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                    if let Some(dm) = self.draw_modules.first_mut() {
                        dm.set_terrain_decal_opacity(self.decal_opacity);
                    }
                }
            }
            if let Some(handle) = &self.terrain_decal_handle {
                handle.set_position(self.position.x, self.position.y, self.position.z);
            }
        } else {
            self.decal_opacity = 0.0;
        }

        // C++ Drawable.cpp:1209-1213 — `m_expirationDate != 0 && now >= m_expirationDate`
        // uses Logic time and returns before flash and tint. The caller owns
        // retirement; the receiving Client clock below still drives flashing.
        if self.is_expired(logic_frame) {
            return;
        }

        if !self.test_tint_status(TintStatus::FRENZY) {
            let effectively_dead = self.object_id.is_some_and(|obj_id| {
                OBJECT_REGISTRY
                    .get_object(obj_id)
                    .and_then(|obj_arc| {
                        obj_arc.read().ok().map(|guard| guard.is_effectively_dead())
                    })
                    .unwrap_or(false)
            });
            if effectively_dead {
                self.second_material_pass_opacity = 0.0;
            } else if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }
        self.overlay_data.second_material_pass_opacity = self.second_material_pass_opacity;

        if self.flash_count > 0 && self.current_frame.is_multiple_of(DRAWABLE_FRAMES_PER_FLASH) {
            self.color_flash_envelope(Some(self.flash_color), DEF_DECAY_FRAMES, 0, 0);
            self.flash_count = self.flash_count.saturating_sub(1);
        }

        self.update_tint_status();

        // C++ `updateDrawable` ticks both envelopes every frame so EMP/status
        // tints and selection flash actually fade instead of sticking.
        if let Some(envelope) = self.tint_envelope.as_mut() {
            envelope.update();
        }
        if let Some(envelope) = self.selection_flash_envelope.as_mut() {
            envelope.update();
        }

        if let Some(icon_info) = self.icon_info.as_mut() {
            icon_info.update(self.current_frame);
        }

        self.publish_wheel_info_to_logic();
        self.apply_pending_time_of_day();
        self.restart_ambient_if_dropped();
    }
}
