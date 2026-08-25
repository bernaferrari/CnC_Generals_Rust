//! Rodio stand-in for Miles `AIL_set_3D_position` + one falloff pass.

use super::audio_event_rts::{
    AudioEventRts, Coord3D, MilesVolumeSliders, miles_event_world_position,
    miles_get_effective_volume, miles_positional_gain, miles_positional_ranges,
};
use super::game_audio::Real;

/// Pre-distance slider volume (event * shift * category slider).
#[must_use]
pub fn miles_slider_volume(event: &AudioEventRts, sliders: &MilesVolumeSliders) -> f32 {
    let mut volume = event.get_volume() * event.get_volume_shift();
    match event
        .get_audio_event_info()
        .as_deref()
        .map(|info| info.sound_type)
    {
        Some(super::audio_event_rts::AudioType::Music) => volume *= sliders.music_volume,
        Some(super::audio_event_rts::AudioType::Streaming) => volume *= sliders.speech_volume,
        _ => {
            if event.is_positional_audio() {
                volume *= sliders.sound_3d_volume;
            } else {
                volume *= sliders.sound_volume;
            }
        }
    }
    volume
}

/// One Miles falloff from listener to the event's current (object-followed) position.
#[must_use]
pub fn miles_distance_gain(
    event: &AudioEventRts,
    listener: &Coord3D,
    sliders: &MilesVolumeSliders,
) -> f32 {
    if !event.is_positional_audio() {
        return 1.0;
    }
    let pos = miles_event_world_position(event);
    let dx = listener.x - pos.x;
    let dy = listener.y - pos.y;
    let dz = listener.z - pos.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let (min_distance, max_distance) = miles_positional_ranges(
        event.get_audio_event_info().as_deref(),
        sliders.global_min_range,
        sliders.global_max_range,
    );
    miles_positional_gain(distance, min_distance, max_distance)
}

/// Full Miles effective volume using the live object/drawable position.
#[must_use]
pub fn miles_effective_volume_now(
    event: &AudioEventRts,
    listener: &Coord3D,
    sliders: &MilesVolumeSliders,
) -> f32 {
    miles_get_effective_volume(event, listener, sliders)
}

/// Equal-power pan in [-1, 1] from listener heading (xy) to the source.
#[must_use]
pub fn stereo_pan(listener: &Coord3D, heading_x: f32, heading_y: f32, source: &Coord3D) -> Real {
    let to_x = source.x - listener.x;
    let to_y = source.y - listener.y;
    let len = (to_x * to_x + to_y * to_y).sqrt();
    if len <= f32::EPSILON {
        return 0.0;
    }
    let hx = heading_x;
    let hy = heading_y;
    let hlen = (hx * hx + hy * hy).sqrt();
    if hlen <= f32::EPSILON {
        return 0.0;
    }
    let fx = hx / hlen;
    let fy = hy / hlen;
    let right_x = fy;
    let right_y = -fx;
    let nx = to_x / len;
    let ny = to_y / len;
    (nx * right_x + ny * right_y).clamp(-1.0, 1.0)
}
