//! 3D sound implementation based on the legacy `Sound3DClass`.

use crate::{
    audible_sound::AudibleSound,
    math::{Matrix3D, Vector3},
    mixer::{VoiceSpatialMode, VoiceSpatialParams},
    sound_types::{SoundClassId, SoundState},
};

#[derive(Clone)]
pub struct Sound3D {
    pub base: AudibleSound,
    pub listener_transform: Matrix3D,
    pub max_vol_radius: f32,
    pub dropoff_radius: f32,
    pub is_static: bool,
    pub last_update_ms: u64,
    pub current_velocity: Vector3,
}

impl Sound3D {
    pub fn new(id: crate::SoundObjectId) -> Self {
        Self {
            base: AudibleSound::new(id, SoundClassId::ThreeD),
            listener_transform: Matrix3D::default(),
            max_vol_radius: 0.0,
            dropoff_radius: 1.0,
            is_static: false,
            last_update_ms: 0,
            current_velocity: Vector3::ZERO,
        }
    }

    pub fn as_audible(&self) -> &AudibleSound {
        &self.base
    }

    pub fn as_audible_mut(&mut self) -> &mut AudibleSound {
        &mut self.base
    }

    pub fn make_static(&mut self, is_static: bool) {
        self.is_static = is_static;
        self.base.base.mark_static(is_static);
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }

    pub fn set_listener_transform(&mut self, transform: Matrix3D) {
        self.listener_transform = transform;
    }

    pub fn set_transform(&mut self, transform: Matrix3D) {
        self.base.set_transform(transform);
        self.base.base.update_timestamp(self.last_update_ms);
    }

    pub fn transform(&self) -> Matrix3D {
        self.base.base.transform()
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.base.set_position(position);
    }

    pub fn position(&self) -> Vector3 {
        self.base.position()
    }

    pub fn set_velocity(&mut self, velocity: Vector3) {
        self.base.set_velocity(velocity);
        self.current_velocity = velocity;
        self.base.enable_auto_velocity(false);
    }

    pub fn velocity(&self) -> Vector3 {
        self.base.velocity()
    }

    pub fn auto_calc_velocity(&mut self, enabled: bool) {
        self.base.enable_auto_velocity(enabled);
    }

    pub fn is_auto_velocity_enabled(&self) -> bool {
        self.base.auto_velocity_enabled()
    }

    pub fn set_max_vol_radius(&mut self, radius: f32) {
        self.max_vol_radius = radius.max(0.0);
    }

    pub fn max_vol_radius(&self) -> f32 {
        self.max_vol_radius
    }

    pub fn set_dropoff_radius(&mut self, radius: f32) {
        self.dropoff_radius = radius.max(0.0);
    }

    pub fn dropoff_radius(&self) -> f32 {
        self.dropoff_radius
    }

    pub fn update_miles_transform(&mut self) {
        // Placeholder for integration with hardware specific 3D buffers.
    }

    pub fn on_frame_update(&mut self, delta_ms: u32) {
        self.last_update_ms = self.last_update_ms.saturating_add(delta_ms as u64);
        self.base.update_velocity_from_position(delta_ms as f32);
    }

    pub fn on_loop_end(&mut self) {
        self.base.on_loop_end();
    }

    pub fn play(&mut self, alloc_handle: bool) {
        let _ = alloc_handle;
        let _ = self.base.play(true);
    }

    pub fn get_priority(&self) -> f32 {
        if self.base.base.flags.is_culled {
            0.0
        } else {
            self.base.priority()
        }
    }

    pub fn add_to_scene(&mut self) {
        self.base.base.set_state(SoundState::Playing);
    }

    pub fn remove_from_scene(&mut self) {
        self.base.base.set_state(SoundState::Stopped);
    }

    pub fn set_culled(&mut self, culled: bool) {
        self.base.set_culled(culled);
    }

    pub fn dropoff_radius_value(&self) -> f32 {
        self.dropoff_radius
    }

    pub fn update_spatial_audio(&mut self, listener: &crate::Listener3D) {
        let listener_pos = listener.position();
        let listener_transform = listener.transform();
        let right = listener_transform.right_vector().normalize();

        let relative = self.position() - listener_pos;
        let distance = relative.length();
        let dropoff = self.dropoff_radius.max(0.001);
        let attenuation = if distance >= dropoff {
            0.0
        } else {
            let t = 1.0 - (distance / dropoff);
            (t * t).clamp(0.0, 1.0)
        };
        let pan_scalar = if relative.length_squared() <= f32::EPSILON {
            0.0
        } else {
            relative.normalize().dot(right).clamp(-1.0, 1.0)
        };
        let pan = (pan_scalar * 1000.0) as i32;

        if self.base.base.is_culled() {
            self.base.set_volume(0.0);
        } else {
            self.base.set_volume(attenuation);
        }
        self.base.set_pan(pan);

        if let Some(handle) = self.base.handle.as_ref() {
            let min_distance = self.max_vol_radius.max(0.0001);
            let max_distance = self.dropoff_radius.max(min_distance + 0.0001);
            let spatial = VoiceSpatialParams {
                mode: VoiceSpatialMode::Full3D,
                position: self.position(),
                velocity: self.velocity(),
                listener_position: listener_pos,
                listener_velocity: listener.velocity(),
                listener_transform,
                min_distance,
                max_distance,
            };
            handle.update_spatial_params(spatial);
            let _ = handle.set_sample_pan(pan);
        }
    }

    pub fn max_volume_radius_value(&self) -> f32 {
        self.max_vol_radius
    }
}
