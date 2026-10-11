//! Legacy OXOB reconstructs selected template facts before runtime overlays.
use super::*;
use crate::game_logic::{Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn locomotor_restore_v10_ordinary_facts_and_saved_pose() {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("Ranger".into(), ThingTemplate::new("Ranger"));
    world.add_player(Player::new(0, Team::USA, "USA", false));
    let id = world
        .create_object("Ranger", Team::USA, Vec3::new(21.0, 0.0, 17.0))
        .unwrap();
    let binding = crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(
        "BasicHumanLocomotor",
    )
    .unwrap();
    {
        let mover = world.host_object_mut(id).unwrap();
        mover.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        mover.wander_angle_offset = 0.37;
        mover.wander_offset_increment = -0.04;
        mover.braking = 0.6;
        mover.max_lift = 1.5;
        mover.braking_factor = 0.8;
        mover.locomotor_surfaces = binding.locomotor_surfaces;
    }
    let entry = capture(&world)
        .objects
        .into_iter()
        .find(|entry| entry.object_id == id.0)
        .unwrap();
    let legacy = ObjectXferPersistV10 {
        object_id: entry.object_id,
        disabled_held: entry.disabled_held,
        single_use_command_used: entry.single_use_command_used,
        ai_attitude: entry.ai_attitude,
        custom_indicator_color: entry.custom_indicator_color,
        vision_range: entry.vision_range,
        shroud_clearing_range: entry.shroud_clearing_range,
        shroud_range: entry.shroud_range,
        pre_attack_ready_at: entry.pre_attack_ready_at,
        pre_attack_target: entry.pre_attack_target,
        consecutive_shot_target: entry.consecutive_shot_target,
        max_shots_to_fire: entry.max_shots_to_fire,
        turret_angle_deg: entry.turret_angle_deg,
        turret_pitch_deg: entry.turret_pitch_deg,
        turret_idle_scan_next_frame: entry.turret_idle_scan_next_frame,
        turret_idle_scanning: entry.turret_idle_scanning,
        turret_idle_scan_desired_angle_deg: entry.turret_idle_scan_desired_angle_deg,
        turret_idle_scan_index: entry.turret_idle_scan_index,
        turret_holding: entry.turret_holding,
        turret_hold_until_frame: entry.turret_hold_until_frame,
        turret_idle_recentering: entry.turret_idle_recentering,
        turret_mood_target: entry.turret_mood_target,
        turret_target_id: entry.turret_target_id,
        turret_force_attacking: entry.turret_force_attacking,
        turret_enabled: entry.turret_enabled,
        turret_substate: entry.turret_substate,
        turret_rotating: entry.turret_rotating,
        temporary_stealth_expires_frame: entry.temporary_stealth_expires_frame,
        weapon_bonus_solo: entry.weapon_bonus_solo,
        which_exit_path: entry.which_exit_path,
        cheer_timer: entry.cheer_timer,
        special_cheering: entry.special_cheering,
        weapon_scatter_targets_unused: entry.weapon_scatter_targets_unused,
        weapon_scatter_targets_inited: entry.weapon_scatter_targets_inited,
        weapon_bonus_horde: entry.weapon_bonus_horde,
        weapon_bonus_enthusiastic: entry.weapon_bonus_enthusiastic,
        weapon_bonus_subliminal: entry.weapon_bonus_subliminal,
        safe_occlusion_frame: entry.safe_occlusion_frame,
        health_box_offset: entry.health_box_offset,
        last_fire_frame: entry.last_fire_frame,
        is_recruitable: entry.is_recruitable,
        guard_next_enemy_scan: entry.guard_next_enemy_scan,
        hunt_next_enemy_scan: entry.hunt_next_enemy_scan,
        ignore_collisions_until_frame: entry.ignore_collisions_until_frame,
        do_final_position: entry.do_final_position,
        final_position: entry.final_position,
        ignored_obstacle_id: entry.ignored_obstacle_id,
        can_path_through_units: entry.can_path_through_units,
        last_command_source: entry.last_command_source,
        locomotor_upgrade: entry.locomotor_upgrade,
        cur_locomotor_set: entry.cur_locomotor_set,
        has_locomotor_motion: entry.has_locomotor_motion,
        braking: entry.braking,
        max_lift: entry.max_lift,
        braking_factor: entry.braking_factor,
        has_locomotor_arrival: entry.has_locomotor_arrival,
        close_enough_dist: entry.close_enough_dist,
        close_enough_dist_3d: entry.close_enough_dist_3d,
        donut_timer: entry.donut_timer,
        preferred_height: entry.preferred_height,
        preferred_height_damping: entry.preferred_height_damping,
        has_locomotor_pose: entry.has_locomotor_pose,
        loco_flags: entry.loco_flags,
        maintain_pos: entry.maintain_pos,
        wander_angle_offset: entry.wander_angle_offset,
        wander_offset_increment: entry.wander_offset_increment,
        has_locomotor_template: entry.has_locomotor_template,
        cur_locomotor_name: entry.cur_locomotor_name,
        valid_locomotor_surfaces: entry.valid_locomotor_surfaces,
        downhill_only: entry.downhill_only,
    };
    let payload = bincode_legacy::serialize(&ObjectXferPersistPayloadV10 {
        objects: vec![legacy],
    })
    .unwrap();
    let mut bytes = OXOB_MAGIC.to_vec();
    append_u32(&mut bytes, 10);
    append_u32(&mut bytes, payload.len() as u32);
    bytes.extend(payload);
    {
        let mover = world.host_object_mut(id).unwrap();
        assert_eq!(mover.unit_ai_runtime.guard_phase(), None);
        assert!(mover.panic_runtime.is_none());
        mover.wander_width_factor = 0.0;
        mover.wander_angle_offset = 0.0;
        mover.wander_offset_increment = 0.0;
        mover.braking = 0.0;
    }
    apply_from_lifecycle_tail(&bytes, &mut world).unwrap();
    let mover = world.host_object(id).unwrap();
    assert_eq!(mover.wander_width_factor, binding.wander_width_factor);
    assert_eq!(mover.min_speed, binding.min_speed);
    assert_eq!(mover.min_turn_speed, binding.min_turn_speed);
    assert_eq!(mover.loco_appearance, binding.appearance);
    assert_eq!(mover.wander_angle_offset, 0.37);
    assert_eq!(mover.wander_offset_increment, -0.04);
    assert_eq!(mover.braking, 0.6);
    assert_eq!(mover.max_lift, 1.5);
    assert_eq!(mover.braking_factor, 0.8);
    assert_eq!(mover.get_position(), Vec3::new(21.0, 0.0, 17.0));
}
