#![cfg(test)]

use super::super::*;
use super::weapon_visual_capture::source_is_locally_controlled;
use crate::presentation_frame::{
    FrozenW3dWeaponVisualBarrel, FrozenWeaponVisualDischarge, FrozenWeaponVisualDrawState,
    FrozenWeaponVisualFxRoute, FrozenWeaponVisualModuleProbe,
    build_frozen_weapon_visual_dispatch_plan,
};

fn test_object(id: u32) -> Object {
    let mut object = Object::new(
        ThingTemplate::new("DispatchSource"),
        ObjectId(id),
        Team::USA,
    );
    object.weapon = Some(Weapon {
        damage: 1.0,
        range: 100.0,
        ammo: Some(4),
        clip_size: 4,
        ..Weapon::default()
    });
    object
}

fn w3d_probe(
    module_index: u32,
    fire_fx_will_handle: bool,
    starts_recoil_or_muzzle: bool,
) -> FrozenWeaponVisualModuleProbe {
    FrozenWeaponVisualModuleProbe::W3dModelDraw {
        state: FrozenWeaponVisualDrawState {
            draw_module_index: module_index,
            source_template_name: "DispatchSource".into(),
            model_key: format!("Mesh_{module_index}"),
            selected_condition_state_index: module_index,
            draw_state_revision: 1,
        },
        barrel: FrozenW3dWeaponVisualBarrel {
            fire_fx_will_handle,
            starts_recoil_or_muzzle,
        },
    }
}

#[test]
fn capture_reads_ammo_and_stealth_before_mutation() {
    let mut object = test_object(41);
    object.status.stealthed = true;
    object.weapon.as_mut().expect("weapon").ammo = Some(3);
    assert!(object.capture_pending_weapon_visual_dispatch(0, 10, Some(ObjectId(42)), None));
    let capture = object
        .pending_weapon_visual_capture
        .as_ref()
        .expect("pending capture");
    assert_eq!(capture.ammo_at_capture, Some(3));
    assert!(capture.stealthed_at_capture);
    assert!(capture.source_is_stealthed);

    Object::consume_ammo_on_fire_named(
        object.weapon.as_mut().expect("weapon"),
        1.0,
        Some("DispatchSource"),
    );
    object.break_stealth();
    let capture = object
        .pending_weapon_visual_capture
        .as_ref()
        .expect("pending capture survives mutation");
    assert_eq!(capture.ammo_at_capture, Some(3));
    assert!(capture.stealthed_at_capture);
    assert_ne!(object.weapon.as_ref().expect("weapon").ammo, Some(3));
    assert!(!object.status.stealthed);
}

#[test]
fn stealth_gate_uses_local_controller_not_team() {
    assert!(source_is_locally_controlled(Some(1), Some(1)));
    assert!(
        !source_is_locally_controlled(Some(2), Some(1)),
        "same-faction remote owner is not locally controlled"
    );
    assert!(!source_is_locally_controlled(None, Some(1)));

    let mut remote = test_object(50);
    remote.owner_player_id = Some(2);
    remote.status.stealthed = true;
    assert!(remote.capture_pending_weapon_visual_dispatch(0, 8, None, None));
    let capture = remote
        .pending_weapon_visual_capture
        .as_ref()
        .expect("capture");
    assert_eq!(
        capture.source_gate(false).fx_route(),
        Some(crate::presentation_frame::FrozenWeaponVisualFxRoute::DrawableSuppressed)
    );
    let local_route = capture.source_gate(true).fx_route();
    assert_ne!(
        local_route,
        Some(crate::presentation_frame::FrozenWeaponVisualFxRoute::DrawableSuppressed)
    );
}

#[test]
fn suspend_fx_nulls_fx_but_still_broadcasts_recoil() {
    let mut object = test_object(51);
    object
        .weapon
        .as_mut()
        .expect("weapon")
        .set_suspend_fx_frame(20);
    assert!(object.capture_pending_weapon_visual_dispatch(0, 10, None, None));
    let capture = object
        .pending_weapon_visual_capture
        .as_ref()
        .expect("capture");
    assert_eq!(
        capture.source_gate(true).fx_route(),
        Some(FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx)
    );
}

#[test]
fn null_fx_starts_recoil_on_every_eligible_module() {
    let plan = build_frozen_weapon_visual_dispatch_plan(
        FrozenWeaponVisualDischarge {
            world_epoch: 1,
            source: ObjectId(41),
            source_object_generation: 1,
            weapon_slot: 0,
            fired_barrel: 0,
            sequence: 1,
            logic_frame: 10,
        },
        FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx,
        [w3d_probe(0, true, true), w3d_probe(1, true, true)],
    )
    .expect("null FX plan");
    assert_eq!(plan.targets.len(), 2);
    assert!(
        plan.targets
            .iter()
            .all(|target| target.starts_recoil_or_muzzle && !target.stops_after_fire_fx)
    );
}

#[test]
fn barrel_read_at_capture_does_not_advance_cursor() {
    let mut logic = GameLogic::new();
    logic.frame = 12;
    let source = ObjectId(80);
    let mut object = test_object(80);
    assert!(object.set_weapon_barrel_count_for_slot(0, 3));
    object.weapon_barrel_states[0].current_barrel = 1;
    object.weapon_barrel_states[0].shots_left_on_barrel = 2;
    assert!(object.capture_pending_weapon_visual_dispatch(0, 12, None, None));
    assert_eq!(
        object
            .pending_weapon_visual_capture
            .as_ref()
            .expect("capture")
            .fired_barrel,
        1
    );
    assert_eq!(
        object
            .weapon_barrel_state_for_slot(0)
            .expect("cursor")
            .current_barrel,
        1,
        "capture must not advance the barrel"
    );
    logic.objects.insert(source, object);
    let event = logic
        .record_accepted_weapon_discharge(source, 0)
        .expect("accepted");
    assert_eq!(event.fired_barrel, 1);
    assert_eq!(
        logic
            .host_object(source)
            .expect("source")
            .weapon_barrel_state_for_slot(0)
            .expect("cursor")
            .shots_left_on_barrel,
        1,
        "advance happens exactly once in record_accepted_weapon_discharge"
    );
}

#[test]
fn projectile_detonation_selects_detonate_fx_not_fire_fx() {
    assert_eq!(
        super::weapon_visual_capture::select_weapon_template_fx(true, "FireOnly", "DetonateOnly"),
        "DetonateOnly"
    );
    let mut object = test_object(90);
    assert!(object.capture_pending_weapon_visual_dispatch_ex(0, 3, None, None, true));
    let capture = object
        .pending_weapon_visual_capture
        .as_ref()
        .expect("capture");
    assert!(capture.is_projectile_detonation);
}
