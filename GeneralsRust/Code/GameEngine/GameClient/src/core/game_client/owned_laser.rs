// Receiving GameClient LaserUpdate operations. The update remains installed
// throughout callbacks; only final owner pose/destruction follows its loan.
use game_engine::common::random_value::RandomState;
use gamelogic::object::draw::w3d_laser_draw::LaserDrawInput;
use gamelogic::object::update::laser_update::{
    LaserDrawableObservation, LaserExecution, LaserObjectObservation, LaserUpdateModule,
    LaserUpdateModuleData,
};

pub struct ClientLaserServices<'a> {
    pub frame: u32,
    pub particles: &'a mut crate::effects::particle_manager::ParticleSystemManager,
    pub random: &'a mut RandomState,
    /// Supplied simulation owner resolves death, including missing target.
    pub effectively_dead: &'a dyn Fn(DrawableId) -> Result<bool, String>,
}

struct ClientLaserPhase<'a, 'b> {
    services: &'a mut ClientLaserServices<'b>,
    observations: HashMap<(u32, String), LaserDrawableObservation>,
    owner: u32,
    final_position: Option<gamelogic::common::Coord3D>,
    destroy: bool,
}
impl LaserExecution for ClientLaserPhase<'_, '_> {
    fn frame(&self) -> u32 {
        self.services.frame
    }
    fn drawable(&self, id: u32, _bone: &str) -> Result<Option<LaserDrawableObservation>, String> {
        Ok(self.observations.get(&(id, _bone.to_string())).copied())
    }
    fn create_particle(&mut self, name: &str) -> Result<Option<u32>, String> {
        let template = self
            .services
            .particles
            .find_template(name)
            .ok_or_else(|| format!("LaserUpdate particle template unavailable: {name}"))?;
        let random = &mut self.services.random;
        self.services
            .particles
            .create_particle_system_with_random(&template, true, &mut |lo, hi| {
                random.next_real(lo, hi)
            })
            .map(Some)
            .map_err(|e| e.to_string())
    }
    fn position_particle(
        &mut self,
        id: u32,
        pos: &gamelogic::common::Coord3D,
    ) -> Result<(), String> {
        let system = self
            .services
            .particles
            .find_particle_system_mut(id)
            .ok_or_else(|| format!("LaserUpdate particle {id} unavailable"))?;
        system.set_position(glam::Vec3::new(pos.x, pos.y, pos.z));
        Ok(())
    }
    fn destroy_particle(&mut self, id: u32) -> Result<(), String> {
        if self.services.particles.find_particle_system(id).is_none() {
            return Err(format!("LaserUpdate particle {id} unavailable"));
        }
        self.services.particles.destroy_particle_system(id);
        Ok(())
    }
    fn position_owner(&mut self, id: u32, pos: &gamelogic::common::Coord3D) -> Result<(), String> {
        if id != self.owner {
            return Err("LaserUpdate owner identity mismatch".into());
        }
        self.final_position = Some(*pos);
        Ok(())
    }
    fn destroy_owner(&mut self, id: u32) -> Result<(), String> {
        if id != self.owner {
            return Err("LaserUpdate owner identity mismatch".into());
        }
        self.destroy = true;
        Ok(())
    }
}

impl GameClient {
    /// Actual authored template factory, no synthetic Drawable on failure.
    pub fn create_owned_ray(
        &mut self,
        template: &ThingTemplate,
        start: gamelogic::common::Coord3D,
        end: gamelogic::common::Coord3D,
    ) -> GameClientResult<DrawableId> {
        let updates: Vec<_> = template
            .get_client_update_module_info()
            .iter()
            .filter(|entry| entry.name.as_str() == "LaserUpdate")
            .collect();
        if updates.len() != 1 {
            return Err(GameClientError::InvalidOperation(
                "ray template requires exactly one authored LaserUpdate".into(),
            ));
        }
        let entry = updates[0];
        let data = if let Some(data) = entry.data.as_any().downcast_ref::<LaserUpdateModuleData>() {
            data.clone()
        } else if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<gamelogic::object::behavior::laser_update::LaserUpdateModuleData>(
        ) {
            LaserUpdateModuleData {
                module_tag_name_key: entry.data.get_module_tag_name_key(),
                particle_system_name: data.particle_system_name.clone(),
                target_particle_system_name: data.target_particle_system_name.clone(),
                punch_through_scalar: data.punch_through_scalar,
            }
        } else {
            return Err(GameClientError::InvalidOperation(
                "ray LaserUpdate data type mismatch".into(),
            ));
        };
        let identifier = if entry.module_tag.is_empty() {
            entry.name.to_string()
        } else {
            entry.module_tag.to_string()
        };
        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        for module in Self::create_snapshot_modules_from_template(template) {
            drawable.add_draw_module(module);
        }
        drawable.add_draw_module(Box::new(
            LogicDrawModuleSnapshotAdapter::client_update_module(
                identifier,
                Box::new(LaserUpdateModule::new_owned(
                    NameKeyGenerator::name_to_key("LaserUpdate"),
                    Arc::new(data),
                    0,
                )),
            ),
        ));
        drawable.set_position(glam::Vec3::new(
            (start.x + end.x) * 0.5,
            (start.y + end.y) * 0.5,
            (start.z + end.z) * 0.5,
        ));
        let id = self.register_drawable_with_template(
            Box::new(drawable),
            Some(template.get_name().to_string()),
        )?;
        self.with_owned_laser(id, |module| {
            module.bind_owned_drawable(id.0);
            Ok(())
        })?;
        // C++ addRayEffect does not destroy a successfully created Drawable on capacity exhaustion.
        self.ray_effects.add(id, start, end);
        // GameClient.cpp480 prepends to the owning Drawable visitation list.
        self.owned_laser_order.insert(0, id);
        Ok(id)
    }

    fn with_owned_laser<T>(
        &mut self,
        id: DrawableId,
        operation: impl FnOnce(&mut LaserUpdateModule) -> Result<T, String>,
    ) -> GameClientResult<T> {
        let drawable = self
            .drawable_map
            .get_mut(&id)
            .ok_or(GameClientError::DrawableNotFound(id))?;
        let basic = drawable
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .ok_or_else(|| {
                GameClientError::InvalidOperation("laser requires actual BasicDrawable".into())
            })?;
        for slot in basic.get_draw_modules_mut() {
            if let Some(module) = slot
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<LaserUpdateModule>())
            {
                return operation(module).map_err(GameClientError::InvalidOperation);
            }
        }
        Err(GameClientError::InvalidOperation(
            "actual LaserUpdate module unavailable".into(),
        ))
    }

    fn laser_observations(
        &mut self,
        id: DrawableId,
        services: &ClientLaserServices<'_>,
        incoming: Option<(Option<u32>, Option<u32>, &str)>,
    ) -> GameClientResult<HashMap<(u32, String), LaserDrawableObservation>> {
        let (old_parent, old_target, old_bone) = self.with_owned_laser(id, |module| {
            let (parent, target, bone) = module.tracked_drawables();
            Ok((parent, target, bone.to_string()))
        })?;
        let (parent, target, bone) =
            incoming.unwrap_or((old_parent, old_target, old_bone.as_str()));
        let mut observations = HashMap::new();
        for (tracked, bone) in [(parent, bone), (target, "")] {
            let Some(tracked) = tracked else {
                continue;
            };
            let client_id = DrawableId(tracked);
            let Some(drawable) = self.drawable_map.get(&client_id) else {
                continue;
            };
            let position = drawable.get_position();
            let bone_position = if bone.is_empty() {
                None
            } else {
                let mut matrix = crate::drawable::Matrix4::identity();
                drawable
                    .as_any()
                    .downcast_ref::<BasicDrawable>()
                    .filter(|basic| {
                        basic.get_current_worldspace_client_bone_positions(bone, &mut matrix)
                    })
                    .map(|_| {
                        let pos = matrix.to_glam().w_axis;
                        gamelogic::common::Coord3D::new(pos.x, pos.y, pos.z)
                    })
            };
            observations.insert(
                (tracked, bone.to_string()),
                LaserDrawableObservation {
                    position: gamelogic::common::Coord3D::new(position.x, position.y, position.z),
                    effectively_dead: (services.effectively_dead)(client_id)
                        .map_err(GameClientError::InvalidOperation)?,
                    bone_position,
                },
            );
        }
        Ok(observations)
    }

    fn finish_laser_phase(
        &mut self,
        id: DrawableId,
        phase: ClientLaserPhase<'_, '_>,
    ) -> GameClientResult<()> {
        if let Some(position) = phase.final_position {
            self.drawable_map
                .get_mut(&id)
                .ok_or(GameClientError::DrawableNotFound(id))?
                .set_position(glam::Vec3::new(position.x, position.y, position.z));
        }
        if phase.destroy {
            let mut phase = phase;
            self.with_owned_laser(id, |module| {
                module
                    .update_mut()
                    .retire_particles_with_services(&mut phase)
            })?;
            self.destroy_drawable(id)?;
        }
        Ok(())
    }

    pub fn initialize_owned_laser(
        &mut self,
        id: DrawableId,
        parent: Option<LaserObjectObservation>,
        target: Option<LaserObjectObservation>,
        start: Option<gamelogic::common::Coord3D>,
        end: Option<gamelogic::common::Coord3D>,
        bone: &str,
        size_delta: i32,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        // Initial observations come from the explicit simulation owner, while
        // attachment pose rereads remain tied to this receiving client.
        let observations = self.laser_observations(
            id,
            services,
            Some((
                parent.and_then(|p| p.drawable),
                target.and_then(|t| t.drawable),
                bone,
            )),
        )?;
        let mut phase = ClientLaserPhase {
            services,
            observations,
            owner: id.0,
            final_position: None,
            destroy: false,
        };
        self.with_owned_laser(id, |module| {
            module.update_mut().init_laser_with_services(
                parent,
                target,
                start.as_ref(),
                end.as_ref(),
                bone.to_string(),
                size_delta,
                &mut phase,
            )
        })?;
        self.finish_laser_phase(id, phase)
    }

    pub fn update_owned_laser(
        &mut self,
        id: DrawableId,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        let observations = self.laser_observations(id, services, None)?;
        let mut phase = ClientLaserPhase {
            services,
            observations,
            owner: id.0,
            final_position: None,
            destroy: false,
        };
        self.with_owned_laser(id, |module| {
            module.update_mut().client_update_with_services(&mut phase)
        })?;
        self.finish_laser_phase(id, phase)
    }
}

impl GameClient {
    pub fn owned_laser_geometry(
        &mut self,
        id: DrawableId,
        texture_aspect: &dyn Fn(&str) -> Result<f32, String>,
        terrain_height: &mut dyn FnMut(f32, f32) -> Option<f32>,
    ) -> GameClientResult<Vec<game_engine::common::system::SceneLineDesc>> {
        let drawable = self
            .drawable_map
            .get_mut(&id)
            .ok_or(GameClientError::DrawableNotFound(id))?;
        let basic = drawable
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .ok_or_else(|| {
                GameClientError::InvalidOperation("laser requires actual BasicDrawable".into())
            })?;
        let modules = basic.get_draw_modules_mut();
        let update_index = modules
            .iter_mut()
            .position(|slot| {
                slot.logic_module_mut()
                    .is_some_and(|module| module.as_any().is::<LaserUpdateModule>())
            })
            .ok_or_else(|| {
                GameClientError::InvalidOperation("actual LaserUpdate unavailable".into())
            })?;
        let mut output = Vec::new();
        for index in 0..modules.len() {
            let Some(draw) = modules[index]
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<W3DLaserDraw>())
            else {
                continue;
            };
            let self_dirty = draw.is_self_dirty();
            let aspect = if draw.texture_name().is_empty() {
                1.0
            } else {
                texture_aspect(draw.texture_name()).map_err(GameClientError::InvalidOperation)?
            };
            let update = modules[update_index]
                .logic_module_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<LaserUpdateModule>()
                .unwrap();
            let input = LaserDrawInput::consume(update, self_dirty);
            let draw = modules[index]
                .logic_module_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<W3DLaserDraw>()
                .unwrap();
            if let Some(input) = input {
                draw.geometry_with_services(input, aspect, terrain_height);
            }
            output.extend(draw.current_geometry());
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "owned_laser_tests.rs"]
mod owned_laser_tests;

impl GameClient {
    /// Actual LaserUpdate destructor releases receiving particle systems before
    /// its Drawable is removed; no global manager can receive a peer's IDs.
    pub fn destroy_owned_laser(
        &mut self,
        id: DrawableId,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        let mut phase = ClientLaserPhase {
            services,
            observations: HashMap::new(),
            owner: id.0,
            final_position: None,
            destroy: false,
        };
        self.with_owned_laser(id, |module| {
            module
                .update_mut()
                .retire_particles_with_services(&mut phase)
        })?;
        self.destroy_drawable(id)
    }
}

impl GameClient {
    pub fn tick_owned_lasers(
        &mut self,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        // Real owning Drawable order, including those beyond the bookkeeping capacity.
        let ids = self.owned_laser_order.clone();
        for id in ids {
            self.update_owned_laser(id, services)?;
        }
        Ok(())
    }
    pub fn owned_ray_geometry(
        &mut self,
        texture_aspect: &dyn Fn(&str) -> Result<f32, String>,
        terrain_height: &mut dyn FnMut(f32, f32) -> Option<f32>,
    ) -> GameClientResult<Vec<game_engine::common::system::SceneLineDesc>> {
        let ids = self.owned_laser_order.clone();
        let mut output = Vec::new();
        for id in ids {
            output.extend(self.owned_laser_geometry(id, texture_aspect, terrain_height)?);
        }
        Ok(output)
    }
}
