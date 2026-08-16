//! Given/When/Then coverage for the 14 inventoried MAIN_ONLY_TODO groups.

use super::entity_lifecycle_tags::*;
use super::{Object, TurretSubState};
use crate::game_logic::{ObjectId, SupplyTruckState, Team, ThingTemplate, Weapon};

fn test_object() -> Object {
    Object::new(ThingTemplate::new("TodoAudit"), ObjectId(11), Team::USA)
}

#[test]
fn main_only_todo_groups_round_trip_when_present() {
    let mut src = test_object();
    src.radar_extend_done_frame = 40;
    src.radar_extend_complete = true;
    src.radar_active = true;
    src.production_door_phase = 2;
    src.production_door_phase_end_frame = 90;
    src.production_door_hold_open = true;
    src.is_rebuild_hole = true;
    src.rebuild_template_name = Some("USACommandCenter".to_string());
    src.rebuild_ready_frame = 120;
    src.rebuild_spawner_id = Some(ObjectId(3));
    src.rebuild_worker_id = Some(ObjectId(4));
    src.rebuild_reconstructing_id = Some(ObjectId(5));
    src.supply_truck_state = SupplyTruckState::Wanting;
    src.supply_truck_force_pending = true;
    src.supply_truck_next_dock_action_frame = 15;
    src.preferred_dock_id = Some(ObjectId(8));
    src.shock_stun_frames = 12;
    src.shock_yaw_rate = 0.4;
    src.motive_frames_remaining = 7;
    src.locomotor_upgrade = true;
    src.loco_preferred_height = 18.0;
    src.weapon = Some(Weapon {
        damage: 33.0,
        ..Weapon::default()
    });
    src.active_weapon_slot = 1;
    src.turret_enabled = true;
    src.turret_substate = TurretSubState::Aim;
    src.turret_target_id = Some(ObjectId(9));
    src.object_status_bits = 0x10;
    src.model_condition_bits = 0x20;
    src.continuous_fire_consecutive = 6;
    src.continuous_fire_level = 2;
    src.subdual_damage = 25.0;
    src.subdual_heal_amount = 1.5;
    src.has_overlord_gattling_addon = true;
    src.overlord_bunker_capacity = Some(5);

    let envelope = src.entity_lifecycle_envelope();
    let tags: Vec<&str> = envelope
        .module_states
        .iter()
        .map(|m| m.tag.as_str())
        .collect();
    let expected = [
        TAG_RADAR_EXTEND,
        TAG_PRODUCTION_DOOR,
        TAG_REBUILD_HOLE,
        TAG_SUPPLY_TRUCK,
        TAG_SHOCK_STUN,
        TAG_PHYSICS_MOTIVE,
        TAG_LOCOMOTOR,
        TAG_WEAPON_SLOTS,
        TAG_TURRET,
        TAG_STATUS_BITS,
        TAG_MODEL_CONDITION,
        TAG_CONTINUOUS_FIRE,
        TAG_SUBDUAL_DAMAGE,
        TAG_OVERLORD_ADDON,
    ];
    let emitted: Vec<&str> = tags
        .iter()
        .copied()
        .filter(|t| expected.contains(t))
        .collect();
    assert_eq!(emitted, expected);

    let mut dst = test_object();
    dst.entity_apply_lifecycle_envelope(&envelope)
        .expect("apply");
    assert_eq!(dst.radar_extend_done_frame, 40);
    assert!(dst.radar_active);
    assert_eq!(dst.production_door_phase, 2);
    assert_eq!(
        dst.rebuild_template_name.as_deref(),
        Some("USACommandCenter")
    );
    assert_eq!(dst.supply_truck_state, SupplyTruckState::Wanting);
    assert_eq!(dst.shock_stun_frames, 12);
    assert_eq!(dst.motive_frames_remaining, 7);
    assert!(dst.locomotor_upgrade);
    assert_eq!(dst.loco_preferred_height, 18.0);
    assert_eq!(dst.weapon.as_ref().map(|w| w.damage), Some(33.0));
    assert_eq!(dst.active_weapon_slot, 1);
    assert_eq!(dst.turret_substate, TurretSubState::Aim);
    assert_eq!(dst.object_status_bits, 0x10);
    assert_eq!(dst.model_condition_bits, 0x20);
    assert_eq!(dst.continuous_fire_level, 2);
    assert_eq!(dst.subdual_damage, 25.0);
    assert!(dst.has_overlord_gattling_addon);
    assert_eq!(dst.overlord_bunker_capacity, Some(5));
}

#[test]
fn default_object_does_not_emit_todo_inventory_tags() {
    let tags: Vec<String> = test_object()
        .entity_lifecycle_envelope()
        .module_states
        .iter()
        .map(|m| m.tag.clone())
        .collect();
    for forbidden in [
        TAG_RADAR_EXTEND,
        TAG_PRODUCTION_DOOR,
        TAG_REBUILD_HOLE,
        TAG_SUPPLY_TRUCK,
        TAG_SHOCK_STUN,
        TAG_PHYSICS_MOTIVE,
        TAG_LOCOMOTOR,
        TAG_WEAPON_SLOTS,
        TAG_TURRET,
        TAG_STATUS_BITS,
        TAG_MODEL_CONDITION,
        TAG_CONTINUOUS_FIRE,
        TAG_SUBDUAL_DAMAGE,
        TAG_OVERLORD_ADDON,
    ] {
        assert!(
            !tags.iter().any(|t| t == forbidden),
            "default object emitted {forbidden}"
        );
    }
}
