//! Reconstruct selected immutable LocomotorTemplate facts without entering a state.
//! CPP LocomotorSet::xfer creates the named template before Locomotor::xfer
//! overlays mutable motion/pose. No phase seed, events or physics writes here.
use super::HostLocomotorBinding;

pub(crate) fn apply_host_locomotor_template_facts(
    object: &mut crate::game_logic::object::Object,
    binding: &HostLocomotorBinding,
) {
    object.min_speed = binding.min_speed;
    object.min_turn_speed = binding.min_turn_speed;
    object.loco_behavior_z = binding.behavior_z;
    object.loco_appearance = binding.appearance;
    object.loco_extra_2d_friction = binding.extra_2d_friction;
    object.loco_apply_2d_friction_airborne = binding.apply_2d_friction_when_airborne;
    object.allow_motive_force_while_airborne = binding.allow_motive_force_while_airborne;
    object.locomotor_works_when_dead = binding.locomotor_works_when_dead;
    object.airborne_targeting_height = binding.airborne_targeting_height;
    object.can_move_backward = binding.can_move_backward;
    object.downhill_only = binding.downhill_only;
    object.max_lift_damaged = binding.max_lift_damaged;
    object.speed_limit_z = binding.speed_limit_z;
    object.circling_radius = binding.circling_radius;
    object.max_thrust_angle = binding.max_thrust_angle;
    object.turn_pivot_offset = binding.turn_pivot_offset;
    object.stick_to_ground = binding.stick_to_ground;
    object.locomotor_surfaces = binding.locomotor_surfaces;
    object.wander_width_factor = binding.wander_width_factor;
    object.ultra_accurate_slide_factor = binding.ultra_accurate_slide_factor;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};

    #[test]
    fn locomotor_template_facts_restore_is_complete_and_inert() {
        let binding = crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(
            "BasicHumanLocomotor",
        )
        .expect("authored locomotor");
        let mut template = ThingTemplate::new("FactReceiver");
        template.add_kind_of(KindOf::Infantry);
        let mut object = Object::new(template, ObjectId(1), Team::USA);
        object.wander_angle_offset = 0.37;
        object.wander_offset_increment = 0.08;
        object.wander_offset_increasing = true;
        object.braking_factor = 0.7;
        object.is_braking = true;
        object.physics_accel = glam::Vec3::new(1.0, 2.0, 3.0);
        object.movement.velocity = glam::Vec3::new(4.0, 5.0, 6.0);
        object.movement.max_speed = 123.0;
        object.requested_destination = Some(glam::Vec3::new(100.0, 0.0, 200.0));
        object.path_goal_position = Some(glam::Vec3::new(80.0, 0.0, 190.0));
        object.path_timestamp = 21;
        object.set_movement_goal_object(Some(ObjectId(2)));
        apply_host_locomotor_template_facts(&mut object, &binding);
        macro_rules! fact {
            ($field:ident,$source:ident) => {
                assert_eq!(object.$field, binding.$source);
            };
        }
        fact!(min_speed, min_speed);
        fact!(min_turn_speed, min_turn_speed);
        fact!(loco_behavior_z, behavior_z);
        fact!(loco_appearance, appearance);
        fact!(loco_extra_2d_friction, extra_2d_friction);
        fact!(
            loco_apply_2d_friction_airborne,
            apply_2d_friction_when_airborne
        );
        fact!(
            allow_motive_force_while_airborne,
            allow_motive_force_while_airborne
        );
        fact!(locomotor_works_when_dead, locomotor_works_when_dead);
        fact!(airborne_targeting_height, airborne_targeting_height);
        fact!(can_move_backward, can_move_backward);
        fact!(downhill_only, downhill_only);
        fact!(max_lift_damaged, max_lift_damaged);
        fact!(speed_limit_z, speed_limit_z);
        fact!(circling_radius, circling_radius);
        fact!(max_thrust_angle, max_thrust_angle);
        fact!(turn_pivot_offset, turn_pivot_offset);
        fact!(stick_to_ground, stick_to_ground);
        fact!(locomotor_surfaces, locomotor_surfaces);
        fact!(wander_width_factor, wander_width_factor);
        fact!(ultra_accurate_slide_factor, ultra_accurate_slide_factor);
        assert_eq!(object.wander_angle_offset, 0.37);
        assert_eq!(object.wander_offset_increment, 0.08);
        assert!(object.wander_offset_increasing);
        assert_eq!(object.braking_factor, 0.7);
        assert!(object.is_braking);
        assert_eq!(object.physics_accel, glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(object.movement.velocity, glam::Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(object.movement.max_speed, 123.0);
        assert_eq!(
            object.requested_destination,
            Some(glam::Vec3::new(100.0, 0.0, 200.0))
        );
        assert_eq!(
            object.path_goal_position,
            Some(glam::Vec3::new(80.0, 0.0, 190.0))
        );
        assert_eq!(object.path_timestamp, 21);
        assert_eq!(object.movement_goal_object(), Some(ObjectId(2)));
    }
}
