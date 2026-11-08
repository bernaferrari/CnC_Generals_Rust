use super::*;
use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
use gamelogic::common::ObjectShroudStatus;

fn garrison_template(name: &str) -> ThingTemplate {
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        is_enclosing_container: true,
        ..ContainModuleMetadata::default()
    };
    t
}

fn infantry_template(name: &str) -> ThingTemplate {
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    t.transport_slot_count = Some(1);
    t
}

fn set_target_shroud(
    logic: &GameLogic,
    player_id: u32,
    object_id: ObjectId,
    status: ObjectShroudStatus,
) {
    let shroud_manager = logic.world_services.shroud();
    let mut mgr = shroud_manager.lock().expect("shroud");
    mgr.set_host_object_shroud_status(player_id, object_id.0, status);
}

/// C++ `isObjectShroudedForAction`: human + not FromScript + shroud >= Fogged.
#[test]
fn human_enter_rejects_fogged_or_shrouded_container() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "HqSb42vHuman", true));
    logic.add_player(Player::new(2, Team::China, "HqSb42vAi", false));
    logic
        .templates
        .insert("HqSb42vBunker".into(), garrison_template("HqSb42vBunker"));
    logic
        .templates
        .insert("HqSb42vRanger".into(), infantry_template("HqSb42vRanger"));

    let bunker = logic
        .create_object_for_player("HqSb42vBunker", 1, glam::Vec3::ZERO)
        .expect("bunker");
    let ranger = logic
        .create_object_for_player("HqSb42vRanger", 1, glam::Vec3::new(4.0, 0.0, 0.0))
        .expect("ranger");
    let ai_bunker = logic
        .create_object_for_player("HqSb42vBunker", 2, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("ai bunker");
    let ai_ranger = logic
        .create_object_for_player("HqSb42vRanger", 2, glam::Vec3::new(44.0, 0.0, 0.0))
        .expect("ai ranger");

    assert!(
        logic.can_unit_enter_normal_target(ranger, bunker),
        "uninitialized shroud must fail-open like missing PartitionData"
    );

    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::Clear);
    assert!(
        logic.can_unit_enter_normal_target(ranger, bunker),
        "CLEAR container stays enterable"
    );
    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::PartialClear);
    assert!(
        logic.can_unit_enter_normal_target(ranger, bunker),
        "PARTIAL_CLEAR is below Fogged"
    );

    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::Fogged);
    assert!(
        !logic.can_unit_enter_normal_target(ranger, bunker),
        "human Enter must reject FOGGED garrison"
    );
    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::Shrouded);
    assert!(
        !logic.can_unit_enter_normal_target(ranger, bunker),
        "human Enter must reject SHROUDED garrison"
    );

    set_target_shroud(&logic, 2, ai_bunker, ObjectShroudStatus::Fogged);
    assert!(
        logic.can_unit_enter_normal_target(ai_ranger, ai_bunker),
        "computer player skips the human fog gate"
    );

    if let Some(b) = logic.host_object_mut(bunker) {
        b.template_mut().always_visible = true;
    }
    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::Fogged);
    assert!(
        logic.can_unit_enter_normal_target(ranger, bunker),
        "AlwaysVisible getShroudedStatus is CLEAR"
    );

    set_target_shroud(&logic, 1, bunker, ObjectShroudStatus::Clear);
    set_target_shroud(&logic, 2, ai_bunker, ObjectShroudStatus::Clear);
}

#[test]
fn receiving_pick_view_distinguishes_local_fog_enemy_fog_black_and_containment() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "PickLocal", true));
    world.add_player(Player::new(2, Team::China, "PickEnemy", false));
    assert!(world.select_local_player(Some(1)));
    world.templates.insert("PickRanger".into(), infantry_template("PickRanger"));
    let local = world.create_object_for_player("PickRanger", 1, glam::Vec3::ZERO).unwrap();
    let enemy = world.create_object_for_player("PickRanger", 2, glam::Vec3::ZERO).unwrap();

    for (status, local_hidden, enemy_hidden) in [
        (ObjectShroudStatus::Clear, false, false),
        (ObjectShroudStatus::PartialClear, false, false),
        (ObjectShroudStatus::Fogged, false, true),
        (ObjectShroudStatus::Shrouded, true, true),
    ] {
        set_target_shroud(&world, 1, local, status);
        set_target_shroud(&world, 1, enemy, status);
        let view = world.translator_pick_object_view();
        assert_eq!(view.get(local.0).unwrap().hidden_for_local_player, local_hidden);
        assert_eq!(view.get(enemy.0).unwrap().hidden_for_local_player, enemy_hidden);
    }
    world.host_object_mut(local).unwrap().contained_by = Some(enemy);
    assert!(!world.translator_pick_object_view().get(local.0).unwrap().selectable);
}
