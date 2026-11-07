//! Unit command tests.

use super::*;

fn three_slot_logic() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "ThreeSlotTest".to_string(),
        ThingTemplate::new("ThreeSlotTest"),
    );
    let id = logic
        .create_object("ThreeSlotTest", Team::USA, glam::Vec3::ZERO)
        .expect("test object");
    let object = logic.host_object_mut(id).expect("test object mut");
    object.weapon = Some(Weapon {
        damage: 1.0,
        range: 100.0,
        ..Weapon::default()
    });
    object.secondary_weapon = Some(Weapon {
        damage: 2.0,
        range: 100.0,
        ..Weapon::default()
    });
    object.tertiary_weapon = Some(Weapon {
        damage: 3.0,
        range: 100.0,
        ..Weapon::default()
    });
    (logic, id)
}

#[test]
fn select_tertiary_slot_is_concrete_and_temporary_locked() {
    let (mut logic, id) = three_slot_logic();
    assert!(logic.unit_command_select_weapon_slot(id, 2));
    let object = logic.host_object(id).expect("selected object");
    assert_eq!(object.active_weapon_slot, 2);
    assert_eq!(object.weapon_lock_slot, 2);
    assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(object.weapon_slot(2).map(|weapon| weapon.damage), Some(3.0));
    assert!(object.weapon_slot(3).is_none(), "unknown slots fail closed");
}

#[test]
fn absent_or_unknown_tertiary_selection_cannot_fall_back_to_primary() {
    let (mut logic, id) = three_slot_logic();
    logic.host_object_mut(id).expect("object").tertiary_weapon = None;
    assert!(!logic.unit_command_select_weapon_slot(id, 2));
    assert!(!logic.unit_command_select_weapon_slot(id, 7));
    let object = logic.host_object(id).expect("object");
    assert_eq!(object.weapon_slot(0).map(|weapon| weapon.damage), Some(1.0));
    assert!(object.weapon_slot(2).is_none());
}

#[test]
fn player_tertiary_fire_order_replaces_an_existing_nested_attack() {
    let (mut logic, id) = three_slot_logic();
    let target_id = logic
        .create_object("ThreeSlotTest", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("target");
    {
        let object = logic.host_object_mut(id).expect("object");
        object.status.is_aiming_weapon = true;
        object.status.is_firing_weapon = true;
        object.set_attack_substate(AttackSubState::FireWeapon);
    }

    assert!(logic.unit_command_select_weapon_slot(id, 2));
    assert!(logic.unit_command_fire_weapon(id, Some(target_id), None, -1));

    let object = logic.host_object(id).expect("object");
    assert_eq!(object.target, Some(target_id));
    assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(object.weapon_lock_slot, 2);
    assert!(!object.status.is_aiming_weapon);
    assert!(!object.status.is_firing_weapon);
    assert_eq!(object.attack_substate(), AttackSubState::AimAtTarget);
}

#[test]
fn switch_weapons_cycles_to_tertiary_when_it_is_next_available_slot() {
    let (mut logic, id) = three_slot_logic();
    logic
        .host_object_mut(id)
        .expect("object")
        .active_weapon_slot = 1;
    assert!(logic.unit_command_switch_weapons(id));
    let object = logic.host_object(id).expect("object");
    assert_eq!(object.active_weapon_slot, 2);
    assert_eq!(object.weapon_lock_slot, 2);
    assert_eq!(object.weapon_lock_type, WeaponLockType::LockedPermanently);
}

/// hq-65aus: path fail must not set_destination (straight-line through obstacles).
#[test]
fn attack_move_path_fail_does_not_set_destination() {
    use crate::game_logic::host_deploy_style::{HostDeployStyleData, HostDeployStyleState};
    let mut logic = GameLogic::new();
    logic
        .templates
        .insert("AtkMvBlock".to_string(), ThingTemplate::new("AtkMvBlock"));
    let id = logic
        .create_object("AtkMvBlock", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            range: 80.0,
            damage: 10.0,
            ..Weapon::default()
        });
        unit.deploy_style = Some(HostDeployStyleData {
            state: HostDeployStyleState::ReadyToAttack,
            ready_frame: 0,
            pack_frames: 30,
            unpack_frames: 30,
            ..Default::default()
        });
    }
    let dest = glam::Vec3::new(250.0, 0.0, 0.0);
    assert!(
        logic
            .unit_command_attack_move_to(id, dest)
            .expect("checked AttackMove command")
    );
    let (state, stored, retry, sleep, target) = {
        let unit = logic.host_object(id).expect("unit");
        (
            unit.ai_state().clone(),
            unit.requested_destination,
            unit.attack_move_retry_count,
            unit.attack_move_sleep_until,
            unit.movement.target_position,
        )
    };
    assert_eq!(state, AIState::AttackMoving);
    assert_eq!(stored, Some(logic.clip_player_goal(id, dest)));
    assert_eq!(retry, 5);
    assert_eq!(sleep, 0);
    assert!(
        target.is_none(),
        "path fail must not walk a raw dest line; got {target:?}"
    );
}

#[test]
fn rally_point_set_message_substitutes_building_display_name() {
    assert_eq!(
        format_rally_point_set_message("Rally point set for %s.", "War Factory"),
        "Rally point set for War Factory."
    );
    assert_eq!(
        format_rally_point_set_message("GUI:RallyPointSet", "Barracks"),
        "Rally point set for Barracks"
    );
    assert_eq!(
        format_rally_point_set_message("MISSING: 'GUI:RallyPointSet'", "Command Center"),
        "Rally point set for Command Center"
    );
}

fn rally_factory_logic() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("RallyFactory");
    tmpl.display_name = "War Factory".to_string();
    tmpl.add_kind_of(KindOf::AutoRallypoint);
    logic.templates.insert("RallyFactory".to_string(), tmpl);
    let id = logic
        .create_object("RallyFactory", Team::USA, glam::Vec3::ZERO)
        .expect("rally factory");
    let obj = logic.host_object_mut(id).expect("factory mut");
    obj.building_data = Some(BuildingData::new(BuildingType::WarFactory));
    obj.template_mut().production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    (logic, id)
}

#[test]
fn set_rally_point_stores_location_on_auto_rallypoint_building() {
    let (mut logic, id) = rally_factory_logic();
    let dest = glam::Vec3::new(40.0, 0.0, 12.0);
    assert!(logic.unit_command_set_rally_point(id, dest));
    let obj = logic.host_object(id).expect("factory");
    assert_eq!(obj.get_display_name(), "War Factory");
    assert_eq!(
        obj.building_data.as_ref().and_then(|b| b.rally_point),
        Some(dest)
    );
}

#[test]
fn set_rally_point_rejects_non_auto_rallypoint() {
    let mut logic = GameLogic::new();
    logic
        .templates
        .insert("PowerPlant".to_string(), ThingTemplate::new("PowerPlant"));
    let id = logic
        .create_object("PowerPlant", Team::USA, glam::Vec3::ZERO)
        .expect("plant");
    let obj = logic.host_object_mut(id).expect("plant mut");
    obj.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
    assert!(!logic.unit_command_set_rally_point(id, glam::Vec3::new(10.0, 0.0, 10.0)));
}

#[test]
fn set_rally_point_uses_open_contain_owner_without_building_data() {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("RallyTransport");
    template.add_kind_of(KindOf::AutoRallypoint);
    template.contain_module.kind = ContainModuleKind::Transport;
    template.contain_module.slots = Some(4);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("RallyTransport", Team::USA, glam::Vec3::ZERO)
        .expect("transport");
    let dest = glam::Vec3::new(40.0, 0.0, 12.0);

    assert!(logic.unit_command_set_rally_point(id, dest));
    let object = logic.host_object(id).expect("transport remains admitted");
    assert_eq!(object.open_contain_rally_point, Some(dest));
    assert!(object.building_data.is_none());
}

#[test]
fn production_exit_interface_keeps_priority_over_open_contain() {
    let (mut logic, id) = rally_factory_logic();
    let object = logic.host_object_mut(id).expect("factory");
    object.template_mut().contain_module.kind = ContainModuleKind::Transport;
    object.template_mut().contain_module.slots = Some(4);
    let dest = glam::Vec3::new(40.0, 0.0, 12.0);

    assert!(logic.unit_command_set_rally_point(id, dest));
    let object = logic.host_object(id).expect("factory");
    assert_eq!(
        object.building_data.as_ref().and_then(|b| b.rally_point),
        Some(dest)
    );
    assert_eq!(object.open_contain_rally_point, None);
}

/// hq-9fqep: player attack/move replaces AI_HUNT; hunt does not resume.
fn hunting_infantry_pair() -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    let hunter_id = ObjectId(3203);
    let mut hunter_template = ThingTemplate::new("HuntAttackSource");
    hunter_template.add_kind_of(KindOf::Infantry);
    let frame = logic.frame;
    logic.objects.insert(hunter_id, {
        let mut hunter = Object::new(hunter_template, hunter_id, Team::USA);
        hunter.set_position(glam::Vec3::ZERO);
        hunter.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        hunter.secondary_weapon = Some(Weapon {
            range: 80.0,
            damage: 5.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        hunter.set_hunting(true);
        hunter.set_ai_state(
            AIState::Patrolling,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        let _ = hunter.set_weapon_lock(1, WeaponLockType::LockedTemporarily);
        hunter
    });
    let enemy_id = ObjectId(3204);
    let mut enemy_template = ThingTemplate::new("HuntAttackTarget");
    enemy_template.add_kind_of(KindOf::Infantry);
    logic.objects.insert(enemy_id, {
        let mut enemy = Object::new(enemy_template, enemy_id, Team::GLA);
        enemy.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        enemy
    });
    (logic, hunter_id, enemy_id)
}

#[test]
fn player_attack_ends_hunt_and_releases_temp_lock() {
    let (mut logic, hunter_id, enemy_id) = hunting_infantry_pair();
    assert!(
        logic
            .unit_command_attack(hunter_id, enemy_id)
            .expect("valid attack command fixture")
    );
    let hunter = logic.objects.get(&hunter_id).expect("hunter");
    assert!(!hunter.is_hunting(), "player attack must end Hunt");
    assert_eq!(hunter.ai_state(), AIState::Attacking);
    assert_eq!(hunter.target, Some(enemy_id));
    assert_eq!(
        hunter.weapon_lock_type,
        WeaponLockType::NotLocked,
        "AIHuntState::onExit releases LOCKED_TEMPORARILY"
    );
    let frame = logic.frame;
    if let Some(unit) = logic.objects.get_mut(&hunter_id) {
        unit.clear_target(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        unit.set_ai_state(
            AIState::Idle,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    logic
        .update_ai(&[hunter_id, enemy_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    let hunter = logic.objects.get(&hunter_id).expect("hunter after tick");
    assert!(!hunter.is_hunting());
    assert!(
        !matches!(hunter.ai_state(), AIState::Patrolling),
        "Hunt must not resume after the player attack finishes; got {:?}",
        hunter.ai_state()
    );
}

#[test]
fn player_move_ends_hunt_and_does_not_resume() {
    let (mut logic, hunter_id, enemy_id) = hunting_infantry_pair();
    assert!(logic.unit_command_move_to(hunter_id, glam::Vec3::new(40.0, 0.0, 0.0)));
    let hunter = logic.objects.get(&hunter_id).expect("hunter");
    assert!(!hunter.is_hunting(), "player move must end Hunt");
    assert_eq!(hunter.ai_state(), AIState::Moving);
    assert_eq!(
        hunter.weapon_lock_type,
        WeaponLockType::NotLocked,
        "AIHuntState::onExit releases LOCKED_TEMPORARILY"
    );
    let frame = logic.frame;
    if let Some(unit) = logic.objects.get_mut(&hunter_id) {
        unit.set_ai_state(
            AIState::Idle,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.clear_target(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    }
    logic
        .update_ai(&[hunter_id, enemy_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    let hunter = logic.objects.get(&hunter_id).expect("hunter after tick");
    assert!(!hunter.is_hunting());
    assert!(
        !matches!(hunter.ai_state(), AIState::Patrolling),
        "Hunt must not resume after the player move finishes; got {:?}",
        hunter.ai_state()
    );
}

#[test]
fn add_upgrade_to_queue_refuses_command_set_without_button() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("AmericaBarracks");
    tmpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), tmpl);
    let id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    let cost = Resources {
        supplies: 800,
        power: 0,
    };
    assert!(
        !logic.unit_command_building_add_upgrade_to_queue(
            id,
            "Upgrade_AmericaSupplyLines",
            30.0,
            cost,
        ),
        "C++ canProduceUpgrade refuses SupplyLines at Barracks"
    );
}
