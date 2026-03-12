//! Pseudo-3D sound implementation mirroring `SoundPseudo3DClass`.

use crate::{
    math::Vector3,
    mixer::{VoiceSpatialMode, VoiceSpatialParams},
    sound3d::Sound3D,
    sound_types::SoundClassId,
};

#[derive(Clone)]
pub struct SoundPseudo3D {
    pub base: Sound3D,
    #[cfg(test)]
    last_volume_scale: f32,
    #[cfg(test)]
    last_pan_value: i32,
}

impl SoundPseudo3D {
    pub fn new(id: crate::SoundObjectId) -> Self {
        let mut base = Sound3D::new(id);
        base.base.base.class_id = SoundClassId::Pseudo3D;
        Self {
            base,
            #[cfg(test)]
            last_volume_scale: 1.0,
            #[cfg(test)]
            last_pan_value: 0,
        }
    }

    pub fn update_spatial_audio(&mut self, listener: &crate::Listener3D) {
        let listener_position = listener.position();
        let listener_transform = listener.transform();
        self.base.set_listener_transform(listener_transform);

        let right = {
            let basis = listener_transform.right_vector();
            if basis.length_squared() <= f32::EPSILON {
                Vector3::new(1.0, 0.0, 0.0)
            } else {
                basis.normalize()
            }
        };
        let up = {
            let basis = listener_transform.up_vector();
            if basis.length_squared() <= f32::EPSILON {
                Vector3::new(0.0, 1.0, 0.0)
            } else {
                basis.normalize()
            }
        };

        let sound_position = self.base.position();
        let relative = sound_position - listener_position;
        let distance = relative.length();

        let min_distance = self.base.max_volume_radius_value().max(0.0);
        let max_distance = self.base.dropoff_radius_value().max(min_distance);

        let mut volume_scale = 1.0f32;
        if distance > min_distance {
            let delta = max_distance - min_distance;
            if delta <= f32::EPSILON {
                volume_scale = 0.0;
            } else {
                volume_scale = 1.0 - ((distance - min_distance) / delta);
                volume_scale = volume_scale.clamp(0.0, 1.0);
            }
        }

        if max_distance <= min_distance && distance <= min_distance {
            volume_scale = 1.0;
        }
        volume_scale = volume_scale.clamp(0.0, 1.0);

        if self.base.base.base.is_culled() {
            volume_scale = 0.0;
        }

        let rel_x = relative.dot(right);
        let rel_y = relative.dot(up);
        let rel_plane = (rel_x * rel_x + rel_y * rel_y).sqrt();
        let pan_scalar = if rel_plane <= f32::EPSILON {
            0.0
        } else {
            let angle = rel_y.atan2(rel_x);
            ((-angle.sin()) / 2.0 + 0.5).clamp(0.0, 1.0)
        };
        let pan_value = ((pan_scalar * 2.0) - 1.0).clamp(-1.0, 1.0) * 1000.0;
        let pan_value = pan_value as i32;
        self.base.base.set_pan(pan_value);

        let base_gain = self.base.base.volume();
        let final_gain = (base_gain * volume_scale).clamp(0.0, 1.0);

        if let Some(handle) = self.base.base.handle.as_ref() {
            let spatial_max = if max_distance <= min_distance {
                min_distance + 0.0001
            } else {
                max_distance
            };
            let miles_volume = (final_gain * 127.0).round().clamp(0.0, 127.0) as i32;
            let _ = handle.set_sample_volume(miles_volume);

            let spatial = VoiceSpatialParams {
                mode: VoiceSpatialMode::Pseudo3D,
                position: sound_position,
                velocity: self.base.velocity(),
                listener_position,
                listener_velocity: listener.velocity(),
                listener_transform,
                min_distance,
                max_distance: spatial_max,
            };
            handle.update_spatial_params(spatial);
            let _ = handle.set_sample_pan(pan_value);
        }

        #[cfg(test)]
        {
            self.last_volume_scale = final_gain;
            self.last_pan_value = pan_value;
        }
    }

    pub fn set_listener_transform(&mut self, transform: crate::Matrix3D) {
        self.base.set_listener_transform(transform);
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.base.set_position(position);
    }

    pub fn set_culled(&mut self, culled: bool) {
        self.base.set_culled(culled);
    }

    pub fn dropoff_radius_value(&self) -> f32 {
        self.base.dropoff_radius_value()
    }

    pub fn max_volume_radius_value(&self) -> f32 {
        self.base.max_volume_radius_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{listener::Listener3D, math::Matrix3D};

    #[test]
    fn pseudo3d_volume_and_pan_follow_legacy_math() {
        let mut pseudo = SoundPseudo3D::new(1);
        pseudo.base.set_max_vol_radius(2.0);
        pseudo.base.set_dropoff_radius(6.0);
        pseudo.base.set_position(Vector3::new(0.0, 4.0, 0.0));

        let mut listener = Listener3D::new(0);
        listener.set_transform(Matrix3D::IDENTITY);
        listener.set_position(Vector3::ZERO);
        listener.set_velocity(Vector3::ZERO);

        pseudo.update_spatial_audio(&listener);

        assert!((pseudo.last_volume_scale - 0.5).abs() < 1e-4);
        assert_eq!(pseudo.last_pan_value, -1000);
    }
}
