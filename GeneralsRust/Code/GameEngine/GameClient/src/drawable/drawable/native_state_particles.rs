//! State-emitter portion of the installed native Draw phase; never submits a mesh.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;
use gamelogic::common::{Coord3D, Matrix3D};
use gamelogic::object::draw::draw_module::{
    ClientParticleStateRuntime, InstalledStateParticleRow, NativeStateParticleContext,
    NativeStateParticlePreparation,
};
use gamelogic::object::draw::W3DModelDraw;

struct ReceivingStateParticles<'a> {
    particles: &'a mut ParticleSystemManager,
    assets: Option<&'a ww3d_assets::AssetManager>,
    public_bones: &'a [String],
    scale: f32,
    keys: Option<&'a mut game_engine::common::name_key_generator::NameKeyNamespace>,
}

impl ClientParticleStateRuntime for ReceivingStateParticles<'_> {
    fn name_to_key(&mut self, name: &str) -> u32 {
        match self.keys.as_deref_mut() {
            Some(keys) => keys.name_to_key(name),
            None => game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name),
        }
    }
    fn standard_public_bones(&self) -> Vec<String> {
        self.public_bones.to_vec()
    }
    fn pristine_bone(
        &mut self,
        model: &str,
        scale: f32,
        frame: i32,
        name: &str,
    ) -> Option<(i32, Matrix3D)> {
        crate::drawable::logic_visual_hooks::lookup_w3d_client_bone_in_assets(
            self.assets?,
            model,
            scale,
            frame,
            name,
        )
    }
    fn sub_object_names(&mut self, model: &str) -> Vec<String> {
        self.assets
            .map(|assets| {
                crate::drawable::logic_visual_hooks::lookup_sub_object_names_in_assets(
                    assets, model,
                )
            })
            .unwrap_or_default()
    }
    fn destroy_particle_system(&mut self, id: u32) {
        self.particles.destroy_particle_system(id);
    }
    fn set_particle_system_running(&mut self, id: u32, running: bool) {
        if let Some(system) = self.particles.find_particle_system_mut(id) {
            if running {
                system.start();
            } else {
                system.stop();
            }
        }
    }
    fn create_particle_system(&mut self, name: &str, frame: u32) -> Result<Option<u32>, String> {
        let Some(template) = self.particles.find_template(name) else {
            return Ok(None);
        };
        self.particles
            .create_particle_system_for_client_frame(&template, true, frame)
            .map(Some)
            .map_err(|error| error.to_string())
    }
    fn model_space_bone(&mut self, model: &str, name: &str) -> Option<(i32, Matrix3D)> {
        self.pristine_bone(model, self.scale, 0, name)
    }
    fn configure_particle_system(
        &mut self,
        id: u32,
        drawable_id: u32,
        bone_index: i32,
        local: &Matrix3D,
        hidden: bool,
    ) {
        let system = self
            .particles
            .find_particle_system_mut(id)
            .expect("newly admitted native state emitter");
        if bone_index != 0 {
            system.set_position(local.w_axis.truncate());
            system.rotate_local_transform_z(local.x_axis.y.atan2(local.x_axis.x));
        } else {
            system.set_position(glam::Vec3::ZERO);
        }
        system.attach_to_drawable(DrawableId(drawable_id));
        system.set_saveable(false);
        if hidden {
            system.stop();
        }
    }
    fn particle_system_exists(&self, id: u32) -> bool {
        self.particles.find_particle_system(id).is_some()
    }
    fn follow_particle_bone(
        &mut self,
        id: u32,
        position: &Coord3D,
        z_rotation: f32,
        world: &Matrix3D,
    ) {
        if let Some(system) = self.particles.find_particle_system_mut(id) {
            super::native_client_update::apply_cpp_particle_bone_transform(
                system, position, z_rotation, world,
            );
        }
    }
}

impl BasicDrawable {
    pub(crate) fn installed_state_particle_module(
        &mut self,
        module_tag: &str,
    ) -> Option<&mut W3DModelDraw> {
        // The assigned source index can differ after Drawable's LOD admission.
        let module = self.draw_modules.iter_mut()
            .find(|module| module.snapshot_module_identifier() == Some(module_tag))?
            .logic_module_mut()?;
        let model = module.as_any_mut().downcast_mut::<W3DModelDraw>()?;
        model.accepts_installed_state_particles().then_some(model)
    }

    pub(crate) fn receive_installed_state_particle_rows(
        &mut self,
        module_tag: &str,
        state_index: u32,
        rows: Option<&[InstalledStateParticleRow]>,
        creation_frame: u32,
        particles: &mut ParticleSystemManager,
    ) -> Result<bool, String> {
        let context = NativeStateParticleContext {
            conditions: gamelogic::common::ModelConditionFlags::empty(),
            instance_scale: self.instance_scale,
            state_particles: !self.status.has(DrawableStatus::NO_STATE_PARTICLES),
            drawable_id: self.id.0,
            hidden: self.hidden
                || self.hidden_by_stealth
                || matches!(self.stealth_look, StealthLook::Invisible),
            fully_obscured: self.drawable_fully_obscured_by_shroud,
            creation_frame,
        };
        let mut receiver = ReceivingStateParticles {
            particles,
            assets: None,
            public_bones: &[],
            scale: context.instance_scale,
            keys: None,
        };
        match self.installed_state_particle_module(module_tag) {
            Some(module) => module.receive_installed_state_particles(state_index, rows, &context, &mut receiver),
            None => Ok(false),
        }
    }

    pub(crate) fn native_state_particle_count(&mut self) -> usize {
        self.draw_modules
            .iter_mut()
            .filter_map(|slot| slot.logic_module_mut())
            .map(gamelogic::object::drawable::native_state_particle_count)
            .sum()
    }

    pub(crate) fn synchronize_native_particle_visibility(
        &mut self,
        particles: &mut ParticleSystemManager,
    ) {
        let hidden = self.hidden
            || self.hidden_by_stealth
            || matches!(self.stealth_look, StealthLook::Invisible);
        let shrouded = self.drawable_fully_obscured_by_shroud;
        let mut receiver = ReceivingStateParticles {
            particles,
            assets: None,
            public_bones: &[],
            scale: self.instance_scale,
            keys: None,
        };
        for slot in &mut self.draw_modules {
            if slot.drawable_module_type_index() != 0 {
                continue;
            }
            if let Some(module) = slot.logic_module_mut() {
                gamelogic::object::drawable::set_native_particle_visibility(
                    module,
                    hidden,
                    shrouded,
                    &mut receiver,
                );
            }
        }
    }

    pub(crate) fn retire_native_state_particles(&mut self, particles: &mut ParticleSystemManager) {
        let mut receiver = ReceivingStateParticles {
            particles,
            assets: None,
            public_bones: &[],
            scale: self.instance_scale,
            keys: None,
        };
        for slot in &mut self.draw_modules {
            if slot.drawable_module_type_index() != 0 {
                continue;
            }
            if let Some(module) = slot.logic_module_mut() {
                gamelogic::object::drawable::retire_native_state_particles(module, &mut receiver);
            }
        }
    }

    pub(crate) fn prepare_native_state_particles(
        &mut self,
        particles: &mut ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        creation_frame: u32,
    ) -> Result<Vec<(usize, NativeStateParticlePreparation)>, String> {
        self.prepare_native_state_particles_with_keys(
            particles,
            assets,
            public_bones,
            creation_frame,
            None,
        )
    }

    pub(crate) fn prepare_native_state_particles_with_keys(
        &mut self,
        particles: &mut ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        creation_frame: u32,
        keys: Option<&mut game_engine::common::name_key_generator::NameKeyNamespace>,
    ) -> Result<Vec<(usize, NativeStateParticlePreparation)>, String> {
        self.prepare_native_state_particles_for_draw(
            particles, assets, public_bones, creation_frame, keys, None,
        )
    }

    pub(crate) fn prepare_native_state_particles_for_draw(
        &mut self,
        particles: &mut ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        creation_frame: u32,
        keys: Option<&mut game_engine::common::name_key_generator::NameKeyNamespace>,
        completed_physics: Option<glam::Mat4>,
    ) -> Result<Vec<(usize, NativeStateParticlePreparation)>, String> {
        // C++ Drawable::draw requests Draw modules after visibility/admission.
        // Ordinary CU updates must not substitute for this Draw-entry phase.
        self.flush_dirty_model_condition();
        let mut outcomes = Vec::new();
        let context = NativeStateParticleContext {
            conditions: gamelogic::common::ModelConditionFlags::from_bits_retain(
                self.model_condition_flags.low_bits(),
            ),
            instance_scale: self.instance_scale,
            state_particles: !self.status.has(DrawableStatus::NO_STATE_PARTICLES),
            drawable_id: self.id.0,
            hidden: self.hidden
                || self.hidden_by_stealth
                || matches!(self.stealth_look, StealthLook::Invisible),
            fully_obscured: self.drawable_fully_obscured_by_shroud,
            creation_frame,
        };
        let (position, base, instance, scale) =
            (self.position, self.base_transform, self.instance_transform, self.instance_scale);
        let draw_world = |local| {
            // Drawable.cpp:2639–2653 applies instance then physics, and
            // W3DModelDraw.cpp:2022 scales before following its bone attachments.
            let world = Matrix4::translation(position).mul(&base).mul(&instance).to_glam()
                * local * glam::Mat4::from_scale(glam::Vec3::splat(scale));
            let swizzle = glam::Mat4::from_cols(
                glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W,
            );
            swizzle * world * swizzle
        };
        let mut world = None;
        let mut receiver = ReceivingStateParticles {
            particles,
            assets,
            public_bones,
            scale: self.instance_scale,
            keys,
        };
        for (module_index, slot) in self.draw_modules.iter_mut().enumerate() {
            if slot.drawable_module_type_index() != 0 {
                continue;
            }
            let Some(module) = slot.logic_module_mut() else {
                continue;
            };
            if let Some(result) = gamelogic::object::drawable::prepare_native_state_particles(
                module,
                &context,
                &mut receiver,
            ) {
                let outcome = result?;
                if let Some(local) = completed_physics
                    && matches!(outcome, NativeStateParticlePreparation::Prepared(true))
                    && let Some(model) = module.as_any().downcast_ref::<W3DModelDraw>()
                    && model.follows_draw_particle_bones()
                {
                    let world = *world.get_or_insert_with(|| draw_world(local));
                    super::native_client_update::update_draw_particle_bones(
                        model, world, assets, receiver.particles,
                    );
                }
                outcomes.push((module_index, outcome));
            }
        }
        Ok(outcomes)
    }
}
