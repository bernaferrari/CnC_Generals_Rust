//! LaserUpdate.cpp223–366 supplied owner observations and presentation effects.
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct LaserObjectObservation {
    pub position: Coord3D,
    pub drawable: Option<DrawableId>,
    pub visible_to_local_player: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct LaserDrawableObservation {
    pub position: Coord3D,
    pub effectively_dead: bool,
    pub bone_position: Option<Coord3D>,
}

/// The receiving client supplies actual drawable/particle operations. No
/// callback chooses an active client or substitutes a synthetic effect ID.
pub trait LaserExecution {
    fn frame(&self) -> u32;
    fn drawable(
        &self,
        id: DrawableId,
        bone: &str,
    ) -> Result<Option<LaserDrawableObservation>, String>;
    fn create_particle(&mut self, name: &str) -> Result<Option<ParticleSystemId>, String>;
    fn position_particle(&mut self, id: ParticleSystemId, position: &Coord3D)
    -> Result<(), String>;
    fn destroy_particle(&mut self, id: ParticleSystemId) -> Result<(), String>;
    fn position_owner(&mut self, id: ThingId, position: &Coord3D) -> Result<(), String>;
    fn destroy_owner(&mut self, id: ThingId) -> Result<(), String>;
}

impl LaserUpdate {
    /// Native creation is inert; the driving owner performs particle teardown.
    pub fn new_owned(thing: ThingId, data: LaserUpdateModuleData) -> Self {
        let mut update = Self::new(thing, data);
        update.external_particle_lifetime = true;
        update
    }

    pub fn init_laser_with_services(
        &mut self,
        parent: Option<LaserObjectObservation>,
        target: Option<LaserObjectObservation>,
        start: Option<&Coord3D>,
        end: Option<&Coord3D>,
        bone: String,
        size_delta_frames: i32,
        services: &mut dyn LaserExecution,
    ) -> Result<(), String> {
        self.set_size_delta_at(size_delta_frames, services.frame());
        self.parent_bone_name = bone;
        if let Some(parent) = parent {
            if let Some(drawable) = parent.drawable {
                self.parent_id = Some(drawable);
            }
            self.update_start_with_services(services)?;
        } else if let Some(start) = start {
            self.start_pos = *start;
        } else {
            services.destroy_owner(self.thing)?;
            return Ok(());
        }
        if let Some(target) = target.filter(|_| end.is_none()) {
            if let Some(drawable) = target.drawable {
                self.target_id = Some(drawable);
            }
            self.end_pos = target.position;
        } else if let Some(end) = end {
            self.end_pos = *end;
        } else {
            services.destroy_owner(self.thing)?;
            return Ok(());
        }
        if self.particle_system_id.is_none()
            && parent.is_none_or(|parent| parent.visible_to_local_player)
        {
            if !self.module_data.particle_system_name.is_empty() {
                self.particle_system_id =
                    services.create_particle(&self.module_data.particle_system_name)?;
            }
            if !self.module_data.target_particle_system_name.is_empty() {
                self.target_particle_system_id =
                    services.create_particle(&self.module_data.target_particle_system_name)?;
            }
        }
        if let Some(id) = self.particle_system_id {
            services.position_particle(id, &self.start_pos)?;
        }
        if let Some(id) = self.target_particle_system_id {
            services.position_particle(id, &self.end_pos)?;
        }
        let position = parent
            .map(|parent| parent.position)
            .unwrap_or_else(|| (self.start_pos + self.end_pos) * 0.5);
        services.position_owner(self.thing, &position)?;
        self.dirty = true;
        Ok(())
    }

    pub fn client_update_with_services(
        &mut self,
        services: &mut dyn LaserExecution,
    ) -> Result<(), String> {
        self.update_start_with_services(services)?;
        self.update_end_with_services(services)?;
        self.update_width_at(services.frame());
        Ok(())
    }
    pub fn retire_particles_with_services(
        &mut self,
        services: &mut dyn LaserExecution,
    ) -> Result<(), String> {
        if let Some(id) = self.particle_system_id {
            services.destroy_particle(id)?;
            self.particle_system_id = None;
        }
        if let Some(id) = self.target_particle_system_id {
            services.destroy_particle(id)?;
            self.target_particle_system_id = None;
        }
        Ok(())
    }
    pub fn set_decay_frames_at(&mut self, frames: u32, now: u32) {
        if frames > 0 {
            self.decaying = true;
            self.decay_start_frame = now;
            self.decay_finish_frame = now.wrapping_add(frames);
            self.current_width_scalar = 1.0;
        }
    }
    pub(super) fn set_size_delta_at(&mut self, delta: i32, now: u32) {
        if delta > 0 {
            self.widening = true;
            self.widen_start_frame = now;
            self.widen_finish_frame = now.wrapping_add(delta as u32);
            self.current_width_scalar = 0.0;
        } else if delta < 0 {
            self.decaying = true;
            self.decay_start_frame = now;
            self.decay_finish_frame = now.wrapping_add(delta.unsigned_abs());
            self.current_width_scalar = 1.0;
        }
    }
    pub(super) fn update_width_at(&mut self, now: u32) {
        if self.decaying {
            self.current_width_scalar = 1.0
                - now.wrapping_sub(self.decay_start_frame) as f32
                    / self.decay_finish_frame.wrapping_sub(self.decay_start_frame) as f32;
            self.dirty = true;
            if self.current_width_scalar <= 0.0 {
                self.current_width_scalar = 0.0;
            }
        } else if self.widening {
            self.current_width_scalar = now.wrapping_sub(self.widen_start_frame) as f32
                / self.widen_finish_frame.wrapping_sub(self.widen_start_frame) as f32;
            self.dirty = true;
            if self.current_width_scalar >= 1.0 {
                self.current_width_scalar = 1.0;
                self.widening = false;
            }
        }
    }
    pub(super) fn update_start_with_services(
        &mut self,
        services: &dyn LaserExecution,
    ) -> Result<(), String> {
        let Some(id) = self.parent_id else {
            return Ok(());
        };
        let Some(parent) = services.drawable(id, &self.parent_bone_name)? else {
            return Ok(());
        };
        let previous = self.start_pos;
        if self.parent_bone_name.is_empty() {
            self.start_pos = parent.position;
        } else if let Some(bone) = parent.bone_position {
            self.start_pos = bone;
        } else {
            self.start_pos = parent.position;
            return Ok(());
        }
        if self.start_pos != previous {
            self.dirty = true;
        }
        Ok(())
    }
    pub(super) fn update_end_with_services(
        &mut self,
        services: &dyn LaserExecution,
    ) -> Result<(), String> {
        let Some(id) = self.target_id else {
            return Ok(());
        };
        let previous = self.end_pos;
        match services.drawable(id, "")? {
            Some(target) if !target.effectively_dead => self.end_pos = target.position,
            _ => {
                if self.module_data.punch_through_scalar > 0.0 {
                    self.end_pos = self.start_pos
                        + (self.end_pos - self.start_pos) * self.module_data.punch_through_scalar;
                }
                self.target_id = None;
            }
        }
        if self.end_pos != previous {
            self.dirty = true;
        }
        Ok(())
    }
}

/// Compatibility ingress only; the canonical algorithm above also serves the
/// explicitly supplied client backend. No native operation constructs this.
pub(super) struct AmbientLaserExecution;
impl LaserExecution for AmbientLaserExecution {
    fn frame(&self) -> u32 {
        TheGameLogic::get_frame()
    }
    fn drawable(
        &self,
        id: DrawableId,
        bone: &str,
    ) -> Result<Option<LaserDrawableObservation>, String> {
        let Some(drawable) = TheGameClient::get().and_then(|client| client.get_drawable_arc(id))
        else {
            return Ok(None);
        };
        let drawable = drawable.read().map_err(|_| "laser drawable poisoned")?;
        let effectively_dead = drawable
            .get_object()
            .and_then(|object| {
                object
                    .read()
                    .ok()
                    .map(|object| object.is_effectively_dead())
            })
            .unwrap_or(false);
        let bone_position = if bone.is_empty() {
            None
        } else {
            drawable
                .get_current_worldspace_client_bone_positions(bone)
                .map(|matrix| Coord3D::new(matrix.w_axis.x, matrix.w_axis.y, matrix.w_axis.z))
        };
        Ok(Some(LaserDrawableObservation {
            position: drawable.get_position(),
            effectively_dead,
            bone_position,
        }))
    }
    fn create_particle(&mut self, name: &str) -> Result<Option<ParticleSystemId>, String> {
        Ok(TheParticleSystemManager::get()
            .and_then(|manager| manager.create_particle_system(Some(name))))
    }
    fn position_particle(
        &mut self,
        id: ParticleSystemId,
        position: &Coord3D,
    ) -> Result<(), String> {
        if let Some(manager) = TheParticleSystemManager::get() {
            manager.set_particle_system_position(id, position);
        }
        Ok(())
    }
    fn destroy_particle(&mut self, id: ParticleSystemId) -> Result<(), String> {
        if let Some(manager) = TheParticleSystemManager::get() {
            manager.destroy_particle_system(id);
        }
        Ok(())
    }
    fn position_owner(&mut self, id: ThingId, position: &Coord3D) -> Result<(), String> {
        if let Some(client) = TheGameClient::get() {
            if id != 0 {
                client.set_drawable_position(id, position);
            }
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(id, |object| {
                if let Some(drawable) = object.get_drawable() {
                    client.set_drawable_position(drawable.get_id(), position);
                }
            });
        }
        Ok(())
    }
    fn destroy_owner(&mut self, id: ThingId) -> Result<(), String> {
        if id != 0 {
            if let Some(client) = TheGameClient::get() {
                client.destroy_drawable(id);
            }
        }
        Ok(())
    }
}

pub(super) fn object_observation(object: &Object) -> LaserObjectObservation {
    let local = ThePlayerList()
        .read()
        .ok()
        .map(|players| players.get_local_player_index())
        .unwrap_or(-1);
    LaserObjectObservation {
        position: *object.get_position(),
        drawable: object.get_drawable().map(|drawable| drawable.get_id()),
        visible_to_local_player: (object.get_shrouded_status(local) as u8)
            <= ObjectShroudStatus::PartialClear as u8,
    }
}
