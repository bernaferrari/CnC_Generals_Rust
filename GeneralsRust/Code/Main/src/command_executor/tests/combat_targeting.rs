#[test]
fn attack_move_sets_is_attack_path_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AM_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.add_kind_of(KindOf::Attackable);
    tpl.set_health(200.0);
    logic.templates.insert("AM_V".to_string(), tpl);
    let id = logic.create_object("AM_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
        });
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[id], Vec3::new(90.0, 0.0, 0.0), -1),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(u.is_attack_path);
    assert_eq!(u.ai_state, AIState::AttackMoving);
}

#[test]
fn follow_waypoint_as_team_preserves_offsets() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("FT_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("FT_V".to_string(), tpl);
    let a = logic
        .create_object("FT_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FT_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    // Stamp formation.
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
    }
    let wps = vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_follow_waypoint_path(&[a, b], &wps, true, true),
            CommandResult::Success
        );
    }
    let ga = logic
        .host_object(a)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(a).unwrap().movement.target_position)
        .unwrap();
    let gb = logic
        .host_object(b)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(b).unwrap().movement.target_position)
        .unwrap();
    // Offsets should keep ~40 world units separation on X (formation).
    let sep = (ga.x - gb.x).abs();
    assert!(
        sep > 20.0,
        "as-team should preserve formation separation, ga={ga:?} gb={gb:?} sep={sep}"
    );
    // Formation id preserved.
    assert_eq!(
        logic.host_object(a).unwrap().formation_id,
        logic.host_object(b).unwrap().formation_id
    );
    assert_ne!(logic.host_object(a).unwrap().formation_id, 0);
}

#[test]
fn do_command_button_using_waypoints_attack_moves() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("BW_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("BW_V".to_string(), tpl);
    let id = logic.create_object("BW_V", Team::USA, Vec3::ZERO).unwrap();
    let wps = vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(80.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button_using_waypoints(&[id], "Command_AttackMove", &wps),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        !u.movement.path.is_empty() || u.movement.target_position.is_some(),
        "should path along waypoints"
    );
}

#[test]
fn do_command_button_dispatches_stop() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DC_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DC_V".to_string(), tpl);
    let id = logic.create_object("DC_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.set_ai_state(AIState::Moving);
        u.set_target(Some(id));
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button(&[id], "Command_Stop", None, None),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        matches!(u.ai_state, AIState::Idle) || u.target.is_none() || !u.status.moving,
        "stop should clear action state={:?} target={:?}",
        u.ai_state,
        u.target
    );
}

#[test]
fn do_command_button_at_position_moves() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DC_M");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DC_M".to_string(), tpl);
    let id = logic.create_object("DC_M", Team::USA, Vec3::ZERO).unwrap();
    let dest = Vec3::new(55.0, 0.0, 10.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button(&[id], "Command_AttackMove", Some(dest), None),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        !u.movement.path.is_empty() || u.movement.target_position.is_some(),
        "attack-move should path"
    );
}

#[test]
fn surrender_stops_and_flags_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SR_I");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("SR_I".to_string(), tpl);
    let id = logic.create_object("SR_I", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_surrender(&[id], true), CommandResult::Success);
    }
    let o = logic.host_object(id).unwrap();
    assert!(o.is_surrendered);
    assert!(o.target.is_none());
}

#[test]
fn attack_team_engages_member_of_team() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, _t) in [("AT_U", Team::USA), ("AT_E", Team::GLA)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("AT_U", Team::USA, Vec3::ZERO).unwrap();
    let e = logic
        .create_object("AT_E", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        use crate::game_logic::Weapon;
        let uo = logic.host_object_mut(u).unwrap();
        uo.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        // team_code 0 = GLA
        assert_eq!(
            exec.execute_attack_team(&[u], 0, -1),
            CommandResult::Success
        );
    }
    let unit = logic.host_object(u).unwrap();
    assert!(
        unit.target == Some(e)
            || matches!(
                unit.ai_state,
                AIState::Attacking | AIState::AttackMoving | AIState::Moving
            ),
        "target={:?} state={:?}",
        unit.target,
        unit.ai_state
    );
    assert_eq!(unit.max_shots_to_fire, -1);
    assert_eq!(
        unit.attack_priority_set.as_deref(),
        Some("AIGroup.AttackTeam.GLA")
    );
}

#[test]
fn weapon_lock_forces_slot_and_release() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WL_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("WL_V".to_string(), tpl);
    let id = logic.create_object("WL_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
        u.secondary_weapon = Some(Weapon {
            damage: 5.0,
            range: 80.0,
            ..Weapon::default()
        });
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_weapon_lock(&[id], 1, 2),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.weapon_lock_type, WeaponLockType::LockedPermanently);
    assert_eq!(u.weapon_lock_slot, 1);
    assert_eq!(u.active_weapon_slot, 1);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_release_weapon_lock(&[id], 2),
            CommandResult::Success
        );
    }
    assert_eq!(
        logic.host_object(id).unwrap().weapon_lock_type,
        WeaponLockType::NotLocked
    );
}

#[test]
fn do_weapon_locks_the_requested_secondary_slot_before_targeting() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{
        AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DW_SECONDARY");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DW_SECONDARY".to_string(), tpl);
    let unit_id = logic
        .create_object("DW_SECONDARY", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
        unit.secondary_weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            ..Weapon::default()
        });
    }

    let target = Vec3::new(75.0, 0.0, 25.0);
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Secondary,
        1,
        &WeaponTarget::Location(target),
    );
    assert_eq!(result, CommandResult::Success);

    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 1);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(unit.weapon_lock_slot, 1);
    assert_eq!(unit.target_location, Some(target));
    assert_eq!(unit.ai_state, AIState::AttackingGround);
    assert_eq!(unit.max_shots_to_fire, 1);
}

#[test]
fn do_weapon_uses_the_real_tertiary_slot_without_secondary_aliasing() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{
        GameLogic, KindOf, ObjectId, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("DW_TERTIARY");
    template.add_kind_of(KindOf::Vehicle);
    template.add_kind_of(KindOf::Selectable);
    template.set_health(200.0);
    logic.templates.insert("DW_TERTIARY".to_string(), template);
    let unit_id = logic
        .create_object("DW_TERTIARY", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
        unit.secondary_weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            ..Weapon::default()
        });
        unit.tertiary_weapon = Some(Weapon {
            damage: 73.0,
            range: 300.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    let target = ObjectId(999);
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Tertiary,
        -1,
        &WeaponTarget::Object(target),
    );
    assert_eq!(result, CommandResult::Success);

    let unit = logic.host_object_mut(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 2);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(unit.weapon_lock_slot, 2);
    assert!(unit.fire_at(target, 1.0));
    assert_eq!(unit.last_fire_slot, 2);
    assert!((unit.last_fire_damage - 73.0).abs() < f32::EPSILON);
    assert_eq!(
        unit.secondary_weapon
            .as_ref()
            .map(|weapon| weapon.last_fire_time),
        Some(0.0),
        "the secondary weapon must remain untouched"
    );
}

#[test]
fn do_weapon_rejects_unrepresented_slots_without_primary_fallback() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DW_PRIMARY_ONLY");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DW_PRIMARY_ONLY".to_string(), tpl);
    let unit_id = logic
        .create_object("DW_PRIMARY_ONLY", Team::USA, Vec3::ZERO)
        .expect("unit");
    logic.host_object_mut(unit_id).expect("unit").weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    });

    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Tertiary,
        -1,
        &WeaponTarget::Location(Vec3::new(75.0, 0.0, 25.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);

    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 0);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::NotLocked);
    assert_eq!(unit.target_location, None);

    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Slot(u32::MAX),
        -1,
        &WeaponTarget::Location(Vec3::new(80.0, 0.0, 30.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);
    assert_eq!(
        logic.host_object(unit_id).expect("unit").target_location,
        None
    );
}

#[test]
fn set_emoticon_stores_name_and_duration() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EM_U");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("EM_U".to_string(), tpl);
    let id = logic.create_object("EM_U", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_emoticon(&[id], "Emoticon_Alert", 60),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.emoticon_name, "Emoticon_Alert");
    assert_eq!(u.emoticon_frames_left, 60);
}

#[test]
fn mine_clearing_detail_toggles_weapon_set_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("MC_D");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(300.0);
    logic.templates.insert("MC_D".to_string(), tpl);
    let d = logic.create_object("MC_D", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[d], true),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .host_object(d)
            .unwrap()
            .weapon_set_mine_clearing_detail
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[d], false),
            CommandResult::Success
        );
    }
    assert!(
        !logic
            .host_object(d)
            .unwrap()
            .weapon_set_mine_clearing_detail
    );
}

#[test]
fn ordinary_do_weapon_uses_authored_mine_detail_and_disarms_only_when_armed() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut clearer = ThingTemplate::new("AuthoredMineDetailClearer");
    clearer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_primary_weapon_none()
        .set_mine_clearing_primary_weapon_name("DozerMineDisarmingWeapon");
    logic
        .templates
        .insert("AuthoredMineDetailClearer".to_string(), clearer);

    let clearer_id = logic
        .create_object("AuthoredMineDetailClearer", Team::USA, Vec3::ZERO)
        .expect("typed clearer");
    let mine_id = logic
        .place_land_mine(Team::GLA, Vec3::new(2.0, 0.0, 0.0), None)
        .expect("enemy mine");
    // The normal creation path starts a weapon's reload clock at frame zero.
    // Start this focused combat assertion with the authored detail weapon
    // ready, just as the other direct `update_combat` tests do.
    logic
        .host_object_mut(clearer_id)
        .and_then(|clearer| clearer.mine_clearing_primary_weapon.as_mut())
        .expect("authored mine detail weapon")
        .last_fire_time = -10.0;

    // The actual retail FIRE_WEAPON does not become usable merely because the
    // object owns a mine row.  The parsed option arms the persistent detail
    // bit first; an untyped/no-option command must fail closed here.
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[clearer_id],
        &WeaponSlot::Primary,
        -1,
        &WeaponTarget::Location(Vec3::new(2.0, 0.0, 0.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);
    assert!(
        logic
            .host_object(mine_id)
            .and_then(|mine| mine.mine_data.as_ref())
            .is_some_and(|data| data.is_active()),
        "an unarmed or untyped primary must not clear the mine"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[clearer_id], true),
            CommandResult::Success
        );
        assert_eq!(
            exec.execute_weapon(
                &[clearer_id],
                &WeaponSlot::Primary,
                -1,
                &WeaponTarget::Location(Vec3::new(2.0, 0.0, 0.0)),
            ),
            CommandResult::Success
        );
    }
    let clearer = logic.host_object(clearer_id).expect("typed clearer");
    assert!(clearer.weapon_set_mine_clearing_detail);
    assert_eq!(
        clearer.weapon_name_for_slot(0),
        Some("DozerMineDisarmingWeapon"),
        "ordinary DoWeapon must select the authored conditional source name"
    );
    assert_eq!(clearer.target_location, Some(Vec3::new(2.0, 0.0, 0.0)));

    // The normal combat loop owns ground impact and DAMAGE_DISARM; no
    // synthetic ClearMines executor path participates in this assertion.
    logic.update_combat(&[clearer_id, mine_id], 1.0 / 30.0);
    assert!(
        logic
            .host_object(mine_id)
            .map(|mine| mine.status.destroyed)
            .unwrap_or(true),
        "the ordinary location weapon path must apply DAMAGE_DISARM to the mine"
    );
}

#[test]
fn go_prone_sets_prone_timer_and_bit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{
        host_enum_table_residual::model_condition_bit_name_index, GameLogic, KindOf, Team,
        ThingTemplate,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GP_I");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("GP_I".to_string(), tpl);
    let i = logic.create_object("GP_I", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_go_prone(&[i]), CommandResult::Success);
    }
    let u = logic.host_object(i).unwrap();
    assert!(u.prone_timer > 0.0);
    if let Some(bit) = model_condition_bit_name_index("PRONE") {
        assert_ne!(u.model_condition_bits & (1u128 << bit), 0);
    }
}

#[test]
fn attack_area_engages_enemy_inside_radius() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, team) in [("AA_U", Team::USA), ("AA_E", Team::GLA)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
        let _ = team;
    }
    let u = logic.create_object("AA_U", Team::USA, Vec3::ZERO).unwrap();
    let e = logic
        .create_object("AA_E", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_area(&[u], Vec3::new(40.0, 0.0, 0.0), 80.0, None),
            CommandResult::Success
        );
    }
    let unit = logic.host_object(u).unwrap();
    assert!(
        unit.attack_priority_set
            .as_deref()
            .is_some_and(|t| t.starts_with("AIGroup.AttackArea.")),
        "AttackArea must persist AIAttackAreaState tag, got {:?}",
        unit.attack_priority_set
    );
    assert!(
        unit.target == Some(e)
            || matches!(
                unit.ai_state,
                AIState::Attacking | AIState::AttackMoving | AIState::Moving
            ),
        "attack area should engage or path, target={:?} state={:?}",
        unit.target,
        unit.ai_state
    );
}

#[test]
fn move_to_and_evacuate_sets_pending_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EV_T");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    // transport capacity residual
    logic.templates.insert("EV_T".to_string(), tpl);
    let mut pax_tpl = ThingTemplate::new("EV_P");
    pax_tpl.add_kind_of(KindOf::Infantry);
    pax_tpl.add_kind_of(KindOf::Selectable);
    pax_tpl.set_health(100.0);
    logic.templates.insert("EV_P".to_string(), pax_tpl);

    let transport = logic.create_object("EV_T", Team::USA, Vec3::ZERO).unwrap();
    let pax = logic
        .create_object("EV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        // Force containable capacity
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
    }
    assert!(!logic
        .host_object(transport)
        .unwrap()
        .contained_units()
        .is_empty());

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move_to_and_evacuate(&[transport], Vec3::new(80.0, 0.0, 0.0), false),
            CommandResult::Success
        );
    }
    let t = logic.host_object(transport).unwrap();
    assert!(
        t.pending_evacuate_on_stop,
        "should pending evacuate after move command"
    );
    assert!(!t.pending_exit_after_evacuate);
}

#[test]
fn move_to_and_evacuate_unloads_when_path_completes() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, kind) in [("EV2_T", KindOf::Vehicle), ("EV2_P", KindOf::Infantry)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(kind);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(400.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let transport = logic.create_object("EV2_T", Team::USA, Vec3::ZERO).unwrap();
    let pax = logic
        .create_object("EV2_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        assert!(t.add_occupant(pax));
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move_to_and_evacuate(&[transport], Vec3::new(10.0, 0.0, 0.0), false),
            CommandResult::Success
        );
    }
    // Simulate arrival: complete path + movement tick.
    if let Some(t) = logic.host_object_mut(transport) {
        // Snap to end of path and finish
        if let Some(last) = t.movement.path.last().copied() {
            t.set_position(last);
        } else {
            t.set_position(Vec3::new(10.0, 0.0, 0.0));
        }
        t.movement.current_path_index = t.movement.path.len().saturating_sub(0);
        // Force index past end
        t.movement.current_path_index = t.movement.path.len();
        t.pending_evacuate_on_stop = true;
    }
    // Direct evacuate_now residual (arrival hook)
    assert!(logic.evacuate_container_now(transport, false));
    assert!(
        logic
            .host_object(transport)
            .map(|t| t.contained_units().is_empty())
            .unwrap_or(false),
        "passengers should unload"
    );
    let p = logic.host_object(pax).unwrap();
    assert!(p.contained_by.is_none());
    assert_ne!(p.ai_state, crate::game_logic::AIState::Docked);
}

#[test]
fn evacuate_requires_container_not_passenger_only() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EV_INF");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("EV_INF".to_string(), tpl);
    let a = logic
        .create_object("EV_INF", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    // C++ groupEvacuate no-ops on non-containers without AI contain.
    assert_eq!(exec.execute_evacuate(&[a]), CommandResult::InvalidCommand);
}

#[test]
fn attack_move_uses_assign_unit_path() {
    // C++ AIGroup::groupAttackMoveToPosition (AIGroup.cpp:2260-2273): one pos.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    let i = prod.find("fn execute_attack_move").expect("attack_move");
    let rest = &prod[i..];
    let end = rest
        .find("fn execute_force_move")
        .unwrap_or(1500.min(rest.len()));
    let w = &rest[..end];
    assert!(
        w.contains("unit_command_attack_move_to_ex")
            && !w.contains("group_move_destinations")
            && !w.contains("set_destination(goal)"),
        "attack-move must send every member to the identical pos"
    );
    let j = prod.find("fn execute_force_move").expect("force_move");
    let w2 = &prod[j..prod.len().min(j + 1200)];
    assert!(
        w2.contains("group_move_destinations")
            && w2.contains("unit_command_force_move_to")
            && !w2.contains("set_destination(goal)"),
        "force-move must pathfind like Move via unit_command_force_move_to"
    );
}

#[test]
fn path_to_goal_with_state_used_by_guard_scatter_gather() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    assert!(prod.contains("fn path_to_goal_with_state"));
    for name in [
        "fn execute_guard",
        "fn execute_scatter",
        "fn execute_gather",
        "fn execute_build",
    ] {
        let i = prod.find(name).unwrap_or_else(|| panic!("missing {name}"));
        let w = &prod[i..prod.len().min(i + 6000)];
        assert!(
            w.contains("path_to_goal_with_state") || w.contains("assign_unit_path"),
            "{name} must pathfind, not bare set_destination"
        );
        // Guard/scatter/gather should not use bare set_destination(goal)
        if name != "fn execute_build" {
            assert!(
                !w.contains("set_destination(*pos)")
                    && !w.contains("set_destination(pos)")
                    && !w.contains("set_destination(dest)")
                    && !w.contains("set_destination(target_pos)"),
                "{name} still has bare set_destination"
            );
        }
    }
}

/// C++ BuildAssistant.cpp:333-334 clearRemovableForConstruction before create.
#[test]
fn execute_build_source_clears_trees_and_props() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src.find("fn execute_build").expect("execute_build");
    let w = &src[i..src.len().min(i + 6000)];
    assert!(
        w.contains("clear_removable_for_construction")
            && src.contains("remove_trees_and_props_for_construction"),
        "hq-wtzcx: execute_build must clear removable objects and map trees/props"
    );
}

#[test]
fn interaction_commands_pathfind_surface() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    // Production locomotion commands should prefer path_to_goal_with_state.
    assert!(prod.matches("path_to_goal_with_state").count() >= 10);
    // Bare set_destination should not remain in execute_* interaction paths.
    let exec = prod;
    let bare = exec.matches("unit.set_destination(").count()
        + exec.matches("unit_mut.set_destination(").count();
    assert_eq!(
        bare, 0,
        "production execute paths still call unit.set_destination ({bare})"
    );
}

#[test]
fn deploy_style_toggle_residual() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let start = src.find("fn execute_deploy").expect("execute_deploy");
    let body = &src[start..start + 2500];
    assert!(
        body.contains("deploy_style_metadata") && body.contains("unit_command_toggle_deploy_style"),
        "DeployStyle authorization must use exact authored module metadata"
    );
    assert!(
        !body.contains("looks_deployable")
            && !body.contains("tomahawk")
            && !body.contains("nukecannon"),
        "DeployStyle authorization must not fall back to vehicle basenames"
    );
}

#[test]
fn deploy_command_uses_authored_metadata_and_pack_unpack_timing() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{DeployStyleMetadata, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;
    use std::time::UNIX_EPOCH;

    fn deploy_command(id: u32, selected_units: Vec<crate::game_logic::ObjectId>) -> GameCommand {
        GameCommand {
            command_type: CommandType::Deploy,
            player_id: 0,
            command_id: id,
            timestamp: UNIX_EPOCH,
            selected_units,
            modifier_keys: ModifierKeys::default(),
        }
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "source-backed deploy", true));

    let mut authored = ThingTemplate::new("ArbitraryDeployStyleVehicle");
    authored
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    authored.deploy_style_metadata = Some(DeployStyleMetadata {
        pack_time_frames: 3,
        unpack_time_frames: 3,
        ..Default::default()
    });
    logic
        .templates
        .insert("ArbitraryDeployStyleVehicle".to_string(), authored);

    let mut ordinary = ThingTemplate::new("PlainVehicleWithoutBehavior");
    ordinary
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0);
    logic
        .templates
        .insert("PlainVehicleWithoutBehavior".to_string(), ordinary);

    let deployable = logic
        .create_object_for_player("ArbitraryDeployStyleVehicle", 0, Vec3::ZERO)
        .expect("source-backed DeployStyle unit");
    let no_behavior = logic
        .create_object_for_player("PlainVehicleWithoutBehavior", 0, Vec3::new(50.0, 0.0, 0.0))
        .expect("ordinary vehicle");

    // Full player command -> executor -> authoritative GameLogic transition.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor
                .execute_command(deploy_command(1, vec![deployable]))
                .expect("deploy command result"),
            CommandResult::Success
        );
    }
    let first = logic.host_object(deployable).expect("deploying unit");
    assert!(
        first
            .deploy_style
            .as_ref()
            .is_some_and(|style| style.is_busy()),
        "the command starts an authored unpack timer rather than immediately setting deployed"
    );
    assert!(!first.is_deployed());

    logic.frame = 2;
    logic.tick_deploy_style_updates();
    assert!(
        !logic.host_object(deployable).unwrap().is_deployed(),
        "unpack must still be pending before its authored frame boundary"
    );
    logic.frame = 3;
    logic.tick_deploy_style_updates();
    let unpacked = logic.host_object(deployable).expect("unpacked unit");
    assert!(unpacked.is_deployed());
    assert!(unpacked
        .deploy_style
        .as_ref()
        .is_some_and(|style| style.is_ready_to_attack()));

    // The next explicit Deploy reverses direction: deployed status clears at
    // pack start and movement becomes available only on the authored boundary.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor
                .execute_command(deploy_command(2, vec![deployable]))
                .expect("undeploy command result"),
            CommandResult::Success
        );
    }
    assert!(!logic.host_object(deployable).unwrap().is_deployed());
    logic.frame = 5;
    logic.tick_deploy_style_updates();
    assert!(
        !logic
            .host_object(deployable)
            .unwrap()
            .deploy_style_allows_move(),
        "pack remains active through frame 5"
    );
    logic.frame = 6;
    logic.tick_deploy_style_updates();
    assert!(logic
        .host_object(deployable)
        .unwrap()
        .deploy_style_allows_move());

    // A plain vehicle must not inherit DeployStyle behavior because a name or
    // VEHICLE KindOf happened to resemble an older residual list.
    let mut executor = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        executor
            .execute_command(deploy_command(3, vec![no_behavior]))
            .expect("ordinary deploy command result"),
        CommandResult::InvalidCommand
    );
    drop(executor);
    assert!(logic
        .host_object(no_behavior)
        .unwrap()
        .deploy_style
        .is_none());
}

#[test]
fn execute_stop_clears_guard_residual() {
    // Wave 955: Stop delegates guard clear to GameLogic::unit_command_stop.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let start = src.find("fn execute_stop").expect("execute_stop");
    let body = &src[start..start + 800];
    assert!(
        body.contains("unit_command_stop") && body.contains("apply_player_stealth_mood_delay"),
        "Stop must call unit_command_stop and apply stealth mood delay"
    );
    let gs = gl.find("fn unit_command_stop").expect("unit_command_stop");
    let gbody = &gl[gs..gs + 900];
    assert!(
        gbody.contains("set_guard_position(None)")
            && gbody.contains("end_guard_retaliate")
            && gbody.contains("set_target(None)"),
        "unit_command_stop must clear guard anchors/targets"
    );
    assert!(
        src.contains("fn apply_player_stealth_mood_delay") && src.contains("next_mood_check_time"),
        "shared stealth mood delay helper must schedule next_mood_check_time"
    );
}

#[test]
fn stop_delays_mood_for_unstealthed_stealth_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.set_current_frame(100);
    let mut tpl = ThingTemplate::new("ST_A");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("ST_A".to_string(), tpl);
    let a = logic.create_object("ST_A", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.innate_stealth = true;
        u.stealth_delay_frames = 45;
        u.auto_acquire_when_idle = true;
        u.status.stealthed = false;
        u.status.detected = false;
        u.next_mood_check_time = 0;
        u.set_ai_state(AIState::Moving);
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_stop(&[a]), CommandResult::Success);
    let u = logic.host_object(a).unwrap();
    assert_eq!(u.ai_state, AIState::Idle);
    // now=100 + delay 45 + skew 0
    assert_eq!(
        u.next_mood_check_time, 145,
        "player stop should delay mood until stealth window"
    );
}
#[test]
fn add_waypoint_uses_group_destinations() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["WP_A", "WP_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("WP_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("WP_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        logic.host_object_mut(id).unwrap().selection_radius = 10.0;
    }
    let click = Vec3::new(100.0, 0.0, 50.0);
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a, b], click),
        CommandResult::Success
    );
    // Paths should not be identical stacked goals for multi-select.
    let pa = logic.host_object(a).unwrap().movement.path.clone();
    let pb = logic.host_object(b).unwrap().movement.path.clone();
    assert!(!pa.is_empty() && !pb.is_empty());
    let ga = *pa.last().unwrap();
    let gb = *pb.last().unwrap();
    assert!(
        (ga - gb).length() > 5.0,
        "waypoint goals must spread like group move, ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn add_waypoint_skips_immobile_and_still_succeeds() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut mobile = ThingTemplate::new("WP_MOB");
    mobile
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_MOB".into(), mobile);
    let mut stuck = ThingTemplate::new("WP_STUCK");
    stuck
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Immobile)
        .set_health(100.0);
    logic.templates.insert("WP_STUCK".into(), stuck);
    let a = logic
        .create_object("WP_MOB", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("WP_STUCK", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let click = Vec3::new(80.0, 0.0, 0.0);
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a, b], click),
        CommandResult::Success
    );
    assert!(
        !logic.host_object(a).unwrap().movement.path.is_empty(),
        "mobile member must still receive the waypoint"
    );
    assert!(
        logic.host_object(b).unwrap().movement.path.is_empty(),
        "immobile member is skipped, not a group abort"
    );
}

#[test]
fn add_waypoint_clamps_to_map_extent() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WP_CLAMP");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_CLAMP".into(), tpl);
    let a = logic
        .create_object("WP_CLAMP", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(400.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let goal = *logic
        .host_object(a)
        .unwrap()
        .movement
        .path
        .last()
        .expect("clamped waypoint");
    let (_min, max) = logic.world_bounds();
    let margin = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    assert!(
        goal.x <= max.x - margin + 0.1,
        "AddWaypoint must clampWaypointPosition, goal={goal:?} max={max:?}"
    );
}

#[test]
fn add_waypoint_plays_voice_move_when_idle() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("WP_VOICE", UnitVoiceSlot::Move, "TestWaypointVoiceMove");
    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WP_VOICE");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_VOICE".into(), tpl);
    let a = logic
        .create_object("WP_VOICE", Team::USA, Vec3::ZERO)
        .unwrap();
    logic.queued_audio_events.clear();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(40.0, 0.0, 0.0)),
        CommandResult::Success
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestWaypointVoiceMove"),
        "idle AddWaypoint must play VoiceMove: {:?}",
        logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.clone())
            .collect::<Vec<_>>()
    );
    logic.queued_audio_events.clear();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(80.0, 0.0, 0.0)),
        CommandResult::Success
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "TestWaypointVoiceMove"),
        "already-moving AddWaypoint must not replay VoiceMove"
    );
    clear_test_template_voices();
}

#[test]
fn add_waypoint_breaks_off_attack() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["WP_ATK", "WP_VIC"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("WP_ATK", Team::USA, Vec3::ZERO)
        .unwrap();
    let v = logic
        .create_object("WP_VIC", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.set_target(Some(v));
        u.set_guard_target(Some(v));
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(90.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let u = logic.host_object(a).unwrap();
    assert!(
        u.target.is_none(),
        "queued waypoint must drop attack target"
    );
    assert!(u.guard_target.is_none(), "queued waypoint must drop guard");
    assert_eq!(u.ai_state, AIState::Moving);
    assert!(!u.status.attacking);
}

#[test]
fn move_delays_mood_for_unstealthed_stealth_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.set_current_frame(50);
    let mut tpl = ThingTemplate::new("MV_ST");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("MV_ST".to_string(), tpl);
    let a = logic.create_object("MV_ST", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.innate_stealth = true;
        u.stealth_delay_frames = 30;
        u.auto_acquire_when_idle = true;
        u.status.stealthed = false;
        u.status.detected = false;
        u.next_mood_check_time = 0;
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_move(&[a], Vec3::new(50.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let u = logic.host_object(a).unwrap();
    assert_eq!(u.next_mood_check_time, 80); // 50+30+0
}

#[test]
fn dock_uses_controlling_player_for_centers_and_relationship_for_warehouses() {
    use super::CommandExecutor;
    use crate::game_logic::{DockKind, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA slot 0", true));
    logic.add_player(Player::new(1, Team::USA, "USA slot 1", false));

    let mut collector = ThingTemplate::new("DockOwnerCollector");
    collector
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("DockOwnerCollector".to_string(), collector);

    let mut center = ThingTemplate::new("DockOwnerCenter");
    center.add_kind_of(KindOf::Structure).set_health(1_000.0);
    center.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("DockOwnerCenter".to_string(), center);

    let mut warehouse = ThingTemplate::new("DockOwnerWarehouse");
    warehouse.add_kind_of(KindOf::Structure).set_health(1_000.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    logic
        .templates
        .insert("DockOwnerWarehouse".to_string(), warehouse);

    let collector_id = logic
        .create_object("DockOwnerCollector", Team::USA, Vec3::ZERO)
        .expect("collector");
    let center_id = logic
        .create_object("DockOwnerCenter", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("center");
    let warehouse_id = logic
        .create_object("DockOwnerWarehouse", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let collector = logic
            .host_object_mut(collector_id)
            .expect("collector object");
        collector.owner_player_id = Some(0);
        collector.stored_resources.supplies = 1;
    }
    for id in [center_id, warehouse_id] {
        let target = logic.host_object_mut(id).expect("dock target");
        target.owner_player_id = Some(1);
        target.stored_resources.supplies = 1;
    }

    // C++ ActionManager::canTransferSuppliesAt: SupplyCenter checks pointer
    // equality of controlling players, not faction/alliance equality.
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.can_issue_dock(collector_id, center_id), None);
        // The two player slots have no alliance, so the same-faction warehouse
        // is an enemy target and must be refused by its relationship gate.
        assert_eq!(exec.can_issue_dock(collector_id, warehouse_id), None);
    }

    logic.get_player_mut(0).expect("slot 0").alliance_team = 7;
    logic.get_player_mut(1).expect("slot 1").alliance_team = 7;
    let exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.can_issue_dock(collector_id, warehouse_id),
        Some(DockKind::SupplyWarehouse),
        "allied owners may collect from a warehouse"
    );
    assert_eq!(
        exec.can_issue_dock(collector_id, center_id),
        None,
        "allied is still not the same controller for a SupplyCenter"
    );
}

#[test]
fn return_to_base_prefers_exact_owner_producer_and_only_allows_explicitly_allied_fallback() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, ParkingPlaceMetadata, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // Slots 0 and 1 deliberately share a faction but begin as enemies.  A
    // faction/name shortcut would incorrectly send the first unbound jet to
    // the nearer slot-1 airfield.
    logic.add_player(Player::new(0, Team::USA, "USA slot 0", true));
    logic.add_player(Player::new(1, Team::USA, "USA slot 1", false));
    logic.add_player(Player::new(2, Team::China, "China slot 2", false));

    let parking = ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    };
    for name in ["RTBExactOwnerAirfield", "RTBAlternateAirfield"] {
        let mut airfield = ThingTemplate::new(name);
        airfield
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSAirfield)
            .set_health(1_000.0);
        airfield.parking_place = Some(parking.clone());
        logic.templates.insert(name.to_string(), airfield);
    }
    let mut jet = ThingTemplate::new("RTBOwnerJet");
    jet.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("RTBOwnerJet".to_string(), jet);

    let owner_airfield = logic
        .create_object_for_player("RTBExactOwnerAirfield", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("slot-0 airfield");
    let same_faction_other_player_airfield = logic
        .create_object_for_player("RTBAlternateAirfield", 1, Vec3::new(2.0, 0.0, 0.0))
        .expect("slot-1 airfield");
    let enemy_airfield = logic
        .create_object_for_player("RTBAlternateAirfield", 2, Vec3::new(4.0, 0.0, 0.0))
        .expect("enemy airfield");

    // C++ JetAIUpdate first asks getPP(producerID).  Its exact controller is
    // the authority here, even though two invalid alternates are closer.
    let producer_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("producer jet");
    logic
        .host_object_mut(producer_jet)
        .expect("producer jet object")
        .producer_id = Some(owner_airfield);
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[producer_jet]),
            CommandResult::Success
        );
    }
    let producer_jet_object = logic
        .host_object(producer_jet)
        .expect("parked producer jet");
    assert_eq!(producer_jet_object.contained_by, Some(owner_airfield));
    assert_eq!(producer_jet_object.producer_id, Some(owner_airfield));
    assert_eq!(producer_jet_object.airfield_parking_space_index, Some(0));
    assert_ne!(
        producer_jet_object.contained_by,
        Some(same_faction_other_player_airfield),
        "nearer same-faction other-player airfield must not override producer"
    );

    // Remove the exact owner field.  The only remaining candidates are a
    // same-faction other player and a genuine enemy; neither is an ally.
    logic
        .host_object_mut(owner_airfield)
        .expect("owner airfield")
        .status
        .sold = true;
    let rejected_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("rejected jet");
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[rejected_jet]),
            CommandResult::InvalidCommand
        );
    }
    let rejected = logic
        .host_object(rejected_jet)
        .expect("rejected jet object");
    assert_eq!(rejected.contained_by, None);
    assert_eq!(rejected.producer_id, None);
    assert_eq!(rejected.airfield_parking_space_index, None);
    assert_ne!(rejected.contained_by, Some(enemy_airfield));

    // Once the two separate player slots are explicitly allied, C++
    // ALLOW_ALLIES fallback may choose the actual ParkingPlaceBehavior.
    logic.get_player_mut(0).expect("slot 0").alliance_team = 17;
    logic.get_player_mut(1).expect("slot 1").alliance_team = 17;
    let allied_fallback_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("allied fallback jet");
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[allied_fallback_jet]),
            CommandResult::Success
        );
    }
    let allied = logic
        .host_object(allied_fallback_jet)
        .expect("allied fallback jet object");
    assert_eq!(
        allied.contained_by,
        Some(same_faction_other_player_airfield),
        "only an explicit player alliance may unlock alternate-airfield RTB"
    );
    assert_eq!(allied.airfield_parking_space_index, Some(0));

    // The fallback remains relationship-authorized because it owns an actual
    // reserved slot; it must not be reclassified as a same-owner producer or
    // release/reacquire capacity on each RTB update.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[allied_fallback_jet]),
            CommandResult::Success
        );
    }
    let allied_again = logic
        .host_object(allied_fallback_jet)
        .expect("persisted allied fallback reservation");
    assert_eq!(
        allied_again.producer_id,
        Some(same_faction_other_player_airfield)
    );
    assert_eq!(allied_again.airfield_parking_space_index, Some(0));
}

#[test]
fn railed_transport_command_fails_closed_without_pathprefix_runtime() {
    use super::CommandExecutor;
    use crate::assets::IniParser;
    use crate::command_system::CommandResult;
    use crate::game_logic::{
        AIState, ContainModuleKind, DockKind, GameLogic, KindOf, Team, ThingTemplate,
    };
    use glam::Vec3;

    // This is deliberately an arbitrary identity rather than a train/ferry
    // name.  Its only authority comes from the same retail behavior modules
    // as AutoFerry: contain, railed AI with PathPrefixName, and railed dock.
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ArbitraryWaterCarrier
  Type = Vehicle
  Model = ArbitraryWaterCarrierModel
  KindOf = SELECTABLE TRANSPORT
  Behavior = RailedTransportContain ModuleTag_03
    Slots = 2
    AllowInsideKindOf = INFANTRY VEHICLE BOAT
  End
  Behavior = RailedTransportAIUpdate ModuleTag_05
    PathPrefixName = Ferry
  End
  Behavior = RailedTransportDockUpdate ModuleTag_06
    NumberApproachPositions = 9
    PullInsideDuration = 4500
    PushOutsideDuration = 4500
    ToleranceDistance = 400.0
  End
End
"#,
            "retail_railed_transport_executor_probe.ini",
        )
        .expect("parse retail-shaped railed transport");
    let definition = parser
        .get_definition("ArbitraryWaterCarrier")
        .expect("railed transport definition");
    assert!(definition.behavior_modules.iter().any(|module| {
        module
            .class_name
            .eq_ignore_ascii_case("RailedTransportAIUpdate")
            && module.attribute("PathPrefixName") == Some("Ferry")
    }));

    let carrier_template =
        GameLogic::build_template_from_object_definition("ArbitraryWaterCarrier", definition, None);
    assert_eq!(carrier_template.dock_kind, DockKind::RailedTransport);
    assert_eq!(carrier_template.railed_path_prefix_name, "Ferry");
    assert_eq!(
        carrier_template.contain_module.kind,
        ContainModuleKind::RailedTransport
    );
    assert_eq!(carrier_template.contain_module.slots, Some(2));

    crate::game_logic::railed_waypoint_overlay_reset();
    let mut logic = GameLogic::new();

    logic
        .templates
        .insert(carrier_template.name.clone(), carrier_template);
    let mut rider_template = ThingTemplate::new("RetailRailedPassenger");
    rider_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert(rider_template.name.clone(), rider_template);

    let carrier = logic
        .create_object("ArbitraryWaterCarrier", Team::USA, Vec3::ZERO)
        .expect("railed transport object");
    let rider = logic
        .create_object("RetailRailedPassenger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("passenger");
    let original_path = vec![Vec3::new(40.0, 0.0, 20.0), Vec3::new(80.0, 0.0, 25.0)];
    let original_target = Some(Vec3::new(80.0, 0.0, 25.0));
    {
        let ferry = logic.host_object_mut(carrier).expect("carrier");
        assert!(ferry.add_occupant(rider));
        ferry.movement.path = original_path.clone();
        ferry.movement.target_position = original_target;
        ferry.movement.current_path_index = 1;
        ferry.set_ai_state(AIState::Idle);
    }
    {
        let passenger = logic.host_object_mut(rider).expect("passenger");
        passenger.set_contained_by(Some(carrier));
        passenger.set_ai_state(AIState::Docked);
    }

    let result = {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        executor.execute_railed_transport(&[carrier])
    };
    assert_eq!(result, CommandResult::InvalidCommand);

    // No StartNN/EndNN pairs in leftover terrain or the host overlay:
    // ExecuteRailedTransport must not masquerade as Evacuate or a generic Move.

    let ferry = logic.host_object(carrier).expect("carrier after rejection");
    assert_eq!(ferry.contained_units(), vec![rider]);
    assert_eq!(ferry.movement.path, original_path);
    assert_eq!(ferry.movement.target_position, original_target);
    assert_eq!(ferry.movement.current_path_index, 1);
    assert_eq!(ferry.ai_state, AIState::Idle);
    let passenger = logic.host_object(rider).expect("passenger after rejection");
    assert_eq!(passenger.contained_by, Some(carrier));
    assert_eq!(passenger.ai_state, AIState::Docked);
}

#[test]
fn attack_team_reacquires_after_victim_dies() {
    // C++ AIGroup::groupAttackTeam → aiAttackTeam (AIGroup.cpp:2179-2193).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["AT2_U", "AT2_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("AT2_U", Team::USA, Vec3::ZERO).unwrap();
    let near = logic
        .create_object("AT2_E", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let far = logic
        .create_object("AT2_E", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    {
        let uo = logic.host_object_mut(u).unwrap();
        uo.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_attack_team(&[u], 0, 7), CommandResult::Success);
    }
    assert_eq!(logic.host_object(u).unwrap().target, Some(near));
    assert_eq!(logic.host_object(u).unwrap().max_shots_to_fire, 7);
    logic.host_object_mut(near).unwrap().health.current = 0.0;
    logic.tick_attack_team_persist(&[u]);
    assert_eq!(
        logic.host_object(u).unwrap().target,
        Some(far),
        "attack-team must re-acquire the next living team member"
    );
}

#[test]
fn attack_team_sleep_ai_picks_no_victim() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    for name in ["ATS_U", "ATS_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("ATS_U", Team::USA, Vec3::ZERO).unwrap();
    let _e = logic
        .create_object("ATS_E", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(u).unwrap();
        o.owner_player_id = Some(1);
        o.set_ai_attitude(HostAiAttitude::Sleep);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    assert_eq!(
        logic.choose_attack_team_victim(u, "teamGLA", false),
        None,
        "leftover choose_victim Sleep returns none"
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 1);
        assert_eq!(
            exec.execute_attack_team(&[u], 0, -1),
            CommandResult::Success
        );
    }
    assert_eq!(
        logic.host_object(u).unwrap().target,
        None,
        "Sleep AI must not acquire an attack-team victim"
    );
}

#[test]
fn attack_team_passive_ai_uses_last_attacker() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    for name in ["ATP_U", "ATP_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("ATP_U", Team::USA, Vec3::ZERO).unwrap();
    let near = logic
        .create_object("ATP_E", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .unwrap();
    let far = logic
        .create_object("ATP_E", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(u).unwrap();
        o.owner_player_id = Some(1);
        o.set_ai_attitude(HostAiAttitude::Passive);
        o.last_damage_source = Some(far);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    assert_eq!(
        logic.choose_attack_team_victim(u, "teamGLA", false),
        Some(far),
        "leftover choose_victim Passive uses last attacker, not nearest"
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 1);
        assert_eq!(
            exec.execute_attack_team(&[u], 0, -1),
            CommandResult::Success
        );
    }
    assert_eq!(logic.host_object(u).unwrap().target, Some(far));
    let _ = near;
}

#[test]
fn attack_team_persist_never_bleeds_into_same_faction_team() {
    // C++ retains the exact Team* in AIAttackSquadState. Killing the current
    // victim must not widen a named squad to every object of its faction.
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["ATN_U", "ATN_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let attacker = logic.create_object("ATN_U", Team::USA, Vec3::ZERO).unwrap();
    let first = logic
        .create_object("ATN_E", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let outsider = logic
        .create_object("ATN_E", Team::GLA, Vec3::new(25.0, 0.0, 0.0))
        .unwrap();
    let replacement = logic
        .create_object("ATN_E", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    logic.host_object_mut(first).unwrap().team_instance_name = "GLA_TargetSquad".into();
    logic
        .host_object_mut(replacement)
        .unwrap()
        .team_instance_name = "GLA_TargetSquad".into();
    logic.host_object_mut(outsider).unwrap().team_instance_name = "GLA_OtherSquad".into();
    {
        let unit = logic.host_object_mut(attacker).unwrap();
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
        unit.target = Some(first);
        unit.set_ai_state(AIState::Attacking);
        unit.attack_priority_set = Some("AIGroup.AttackTeam.GLA_TargetSquad".into());
        unit.auto_acquire_when_idle = true;
    }

    logic.host_object_mut(first).unwrap().health.current = 0.0;
    logic.tick_attack_team_persist(&[attacker]);

    let unit = logic.host_object(attacker).unwrap();
    assert_eq!(unit.target, Some(replacement));
    assert_ne!(unit.target, Some(outsider));
    assert_eq!(
        unit.attack_priority_set.as_deref(),
        Some("AIGroup.AttackTeam.GLA_TargetSquad")
    );
}
