use generals_main::game_logic::Object;
use glam::Vec3;

/// Recover the movement-phase velocity before PhysicsUpdate.cpp:627-631
/// applies its 2D lateral or airborne aerodynamic friction. This allows the
/// frozen locomotor force and the separately observed position to be compared
/// without treating m_prevAccel (captured before friction) as the friction.
pub fn marched_velocity(object: &Object, before: Vec3) -> Vec3 {
    let velocity = object.movement.velocity;
    if object.apply_friction_2d_when_airborne
        || !object.height_treats_as_airborne(before.y - object.ground_height)
        || object.is_deck_taxiing()
    {
        let direction = object.unit_direction_vector_2d();
        let lateral = Vec3::new(-direction.y, 0.0, direction.x);
        let friction = object.get_lateral_friction();
        velocity + lateral * (velocity.dot(lateral) * friction / (1.0 - friction))
    } else {
        let scale = 1.0 - object.get_aerodynamic_friction();
        Vec3::new(velocity.x / scale, velocity.y, velocity.z / scale)
    }
}
