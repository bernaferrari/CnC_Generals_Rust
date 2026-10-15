// Receiving GameClient LaserUpdate operations. The update remains installed
// throughout callbacks; only final owner pose/destruction follows its loan.
use game_engine::common::random_value::RandomState;
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
        // LaserUpdate.cpp299/316 explicitly skips an unavailable template.
        let Some(template) = self.services.particles.find_template(name) else {
            return Ok(None);
        };
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
        // The actual particle may already have expired independently.
        let Some(system) = self.services.particles.find_particle_system_mut(id) else {
            return Ok(());
        };
        system.set_position(glam::Vec3::new(pos.x, pos.y, pos.z));
        Ok(())
    }
    fn destroy_particle(&mut self, id: u32) -> Result<(), String> {
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
        if template.get_client_update_module_info().iter().count() != 1 {
            return Err(GameClientError::InvalidOperation(
                "ray has additional unsupplied client-update callbacks".into(),
            ));
        }
        let mut beam_count = 0;
        for entry in template.get_draw_module_info().iter() {
            if entry.name.as_str() != "W3DLaserDraw"
                || !entry.data.as_any().is::<W3DLaserDrawModuleData>()
            {
                return Err(GameClientError::InvalidOperation(
                    "ray has unsupported authored draw callback/data".into(),
                ));
            }
            beam_count += 1;
        }
        if beam_count == 0 {
            return Err(GameClientError::InvalidOperation(
                "ray requires actual W3DLaserDraw".into(),
            ));
        }
        let id = self.create_drawable_from_template(template)?;
        self.drawables
            .drawable_map
            .get_mut(&id)
            .ok_or(GameClientError::DrawableNotFound(id))?
            .set_position(crate::drawable::Vector3::new(
                (start.x + end.x) * 0.5,
                (start.y + end.y) * 0.5,
                (start.z + end.z) * 0.5,
            ));
        // C++ addRayEffect does not destroy a successfully created Drawable on capacity exhaustion.
        self.drawables.ray_effects.add(id, start, end);
        Ok(id)
    }

    fn with_owned_laser<T>(
        &mut self,
        id: DrawableId,
        operation: impl FnOnce(&mut LaserUpdateModule) -> Result<T, String>,
    ) -> GameClientResult<T> {
        let drawable = self
            .drawables
            .drawable_map
            .get_mut(&id)
            .ok_or(GameClientError::DrawableNotFound(id))?;
        let basic = drawable
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .ok_or_else(|| {
                GameClientError::InvalidOperation("laser requires actual BasicDrawable".into())
            })?;
        basic
            .with_native_laser(operation)
            .map_err(GameClientError::InvalidOperation)
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
            let Some(drawable) = self.drawables.drawable_map.get(&client_id) else {
                continue;
            };
            let position = drawable.get_position();
            // Direct host projections are Y-up (camera_drain sends unchanged
            // host XYZ and apply_frozen_direct_presentation_poses rotates Y).
            // Authored Core/client FX drawables remain original C++ Z-up.
            let host_projection = self
                .drawables
                .presentation_direct_drawable_bindings
                .contains_key(&client_id);
            let to_cpp = |x, y, z| {
                if host_projection {
                    gamelogic::common::Coord3D::new(x, z, y)
                } else {
                    gamelogic::common::Coord3D::new(x, y, z)
                }
            };
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
                        to_cpp(pos.x, pos.y, pos.z)
                    })
            };
            observations.insert(
                (tracked, bone.to_string()),
                LaserDrawableObservation {
                    position: to_cpp(position.x, position.y, position.z),
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
            let native_position = if self
                .drawables
                .presentation_direct_drawable_bindings
                .contains_key(&id)
            {
                crate::drawable::Vector3::new(position.x, position.z, position.y)
            } else {
                crate::drawable::Vector3::new(position.x, position.y, position.z)
            };
            self.drawables
                .drawable_map
                .get_mut(&id)
                .ok_or(GameClientError::DrawableNotFound(id))?
                .set_position(native_position);
        }
        if phase.destroy {
            self.destroy_drawable_with_particles(id, Some(phase.services.particles))?;
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
            .drawables
            .drawable_map
            .get_mut(&id)
            .ok_or(GameClientError::DrawableNotFound(id))?;
        let basic = drawable
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .ok_or_else(|| {
                GameClientError::InvalidOperation("laser requires actual BasicDrawable".into())
            })?;
        basic
            .draw_native_laser_geometry(texture_aspect, terrain_height)
            .map_err(GameClientError::InvalidOperation)
    }
}

impl GameClient {
    /// Use the same UI/unlink/module destruction phase as native expiration.
    /// No global manager can receive a peer's IDs.
    pub fn destroy_owned_laser(
        &mut self,
        id: DrawableId,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        self.with_owned_laser(id, |_| Ok(()))?;
        self.destroy_drawable_with_particles(id, Some(services.particles))
    }
}

impl GameClient {
    pub fn tick_owned_lasers(
        &mut self,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        // Real owning Drawable order, including those beyond the bookkeeping capacity.
        let ids = self.drawables.owned_laser_order.clone();
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
        let ids = self.drawables.owned_laser_order.clone();
        let mut output = Vec::new();
        for id in ids {
            output.extend(self.owned_laser_geometry(id, texture_aspect, terrain_height)?);
        }
        Ok(output)
    }
}

/// FXList's actual receiving Drawable owner and immutable content definitions.
/// Tracer ownership is a distinct callback domain and is rejected before any
/// FX nugget mutations until its actual authored draw module is supplied.
pub struct ClientFxDrawables<'a> {
    pub client: &'a mut GameClient,
    pub definitions: &'a game_engine::common::thing::ThingFactory,
}
impl crate::fx_list::FxDrawableExecution for ClientFxDrawables<'_> {
    fn validate_tracer(&self, _template: &str) -> Result<(), String> {
        Err("receiving tracer draw/update callback domain unavailable".into())
    }
    fn spawn_tracer(
        &mut self,
        _request: &crate::fx_list::FxTracerRequest<'_>,
    ) -> Result<(), String> {
        Err("receiving tracer draw/update callback domain unavailable".into())
    }
    fn validate_ray(&self, name: &str) -> Result<(), String> {
        let template = self
            .definitions
            .find_template(name, false)
            .ok_or_else(|| format!("ray template unavailable: {name}"))?;
        let updates: Vec<_> = template
            .get_client_update_module_info()
            .iter()
            .filter(|entry| entry.name.as_str() == "LaserUpdate")
            .collect();
        if updates.len() != 1 {
            return Err("ray requires exactly one actual LaserUpdate".into());
        }
        if template.get_client_update_module_info().iter().count() != 1 {
            return Err("ray has additional unsupplied client-update callbacks".into());
        }
        for entry in template.get_draw_module_info().iter() {
            if entry.name.as_str() != "W3DLaserDraw"
                || !entry.data.as_any().is::<W3DLaserDrawModuleData>()
            {
                return Err("ray has unsupported authored draw callback/data".into());
            }
        }
        let data = updates[0].data.as_any();
        if !data.is::<LaserUpdateModuleData>()
            && !data.is::<gamelogic::object::behavior::laser_update::LaserUpdateModuleData>()
        {
            return Err("ray LaserUpdate data type mismatch".into());
        }
        if !template
            .get_draw_module_info()
            .iter()
            .any(|entry| entry.name.as_str() == "W3DLaserDraw")
        {
            return Err("ray requires actual W3DLaserDraw".into());
        }
        Ok(())
    }
    fn create_ray(&mut self, request: &crate::fx_list::FxRayRequest<'_>) -> Result<(), String> {
        self.validate_ray(request.template)?;
        let template = self
            .definitions
            .find_template(request.template, false)
            .ok_or_else(|| format!("ray template unavailable: {}", request.template))?;
        self.client
            .create_owned_ray(&template, request.primary, request.secondary)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn bone_transforms(
        &mut self,
        object: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<gamelogic::common::Matrix3D>, String> {
        Ok(self
            .client
            .query_current_client_bone_positions(object, name, start, limit)
            .into_iter()
            .map(|(_, matrix)| matrix)
            .collect())
    }
}

impl GameClient {
    pub fn reset_owned_lasers(
        &mut self,
        services: &mut ClientLaserServices<'_>,
    ) -> GameClientResult<()> {
        let owners = self.drawables.owned_laser_order.clone();
        for owner in owners {
            self.destroy_owned_laser(owner, services)?;
        }
        self.drawables.ray_effects.reset();
        Ok(())
    }
}
