use glam::Vec3;

/// ParticleSys.cpp:334-350: acceleration, then damping, then velocity plus
/// drift into position. Drift is a position increment, not a force.
/// The caller owns interpolation history and applies wind at its existing
/// scheduling boundary; this function neither clears forces nor advances time.
#[inline]
pub fn integrate_translation(
    position: &mut Vec3,
    velocity: &mut Vec3,
    acceleration: Vec3,
    damping: f32,
    drift: Vec3,
) {
    *velocity += acceleration;
    velocity.x *= damping;
    velocity.y *= damping;
    velocity.z *= damping;
    position.x += velocity.x + drift.x;
    position.y += velocity.y + drift.y;
    position.z += velocity.z + drift.z;
}

/// ParticleSys.cpp:460-541. The resolved emitter position is an explicit
/// input; object/drawable attachment lookup belongs to the driving instance.
#[inline]
pub fn apply_wind_motion(position: &mut Vec3, angle: f32, emitter_position: Vec3, randomness: f32) {
    const FULL_FORCE_DISTANCE: f32 = 75.0;
    const NO_FORCE_DISTANCE: f32 = 200.0;
    let dx = position.x - emitter_position.x;
    let dy = position.y - emitter_position.y;
    let dz = position.z - emitter_position.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance < NO_FORCE_DISTANCE {
        let mut strength = 2.0 * randomness;
        if distance > FULL_FORCE_DISTANCE {
            strength *= 1.0
                - ((distance - FULL_FORCE_DISTANCE) / (NO_FORCE_DISTANCE - FULL_FORCE_DISTANCE));
        }
        position.x += angle.cos() * strength;
        position.y += angle.sin() * strength;
    }
}
