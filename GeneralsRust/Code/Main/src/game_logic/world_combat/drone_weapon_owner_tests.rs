//! Deployed/slaved callers use the ordinary installed weapon lifecycle.
use crate::game_logic::{
    AIState, DeployStyleMetadata, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon,
};
use glam::Vec3;
const SOURCE: ObjectId = ObjectId(1);
const TARGET: ObjectId = ObjectId(2);

fn world(deployed: bool, pre_delay: f32) -> GameLogic {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "source", true));
    let mut target_player = Player::new(1, Team::China, "target", false);
    target_player.alliance_team = 1;
    game.add_player(target_player);
    let mut template = ThingTemplate::new(if deployed {
        "OwnedDroneShot"
    } else {
        "AmericaVehicleHellfireDrone"
    });
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    if !deployed {
        template
            .set_primary_weapon_name(crate::game_logic::host_slave_drones::HELLFIRE_MISSILE_WEAPON);
    }
    if deployed {
        template.deploy_style_metadata = Some(DeployStyleMetadata {
            unpack_time_frames: 3,
            turrets_function_only_when_deployed: true,
            ..Default::default()
        });
    }
    game.templates
        .insert(template.name.clone(), template.clone());
    // Factory admission installs the authored DeployStyle runtime; an inert
    // Object constructor deliberately has no update-module callbacks.
    let source_id = game
        .create_object_for_player(&template.name, 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(source_id, SOURCE);
    let source = game.host_object_mut(source_id).unwrap();
    source.weapon = Some(Weapon {
        damage: 37.0,
        range: 200.0,
        last_fire_time: -10.0,
        ammo: Some(2),
        clip_size: 2,
        projectile_speed: 9.0,
        pre_attack_delay: pre_delay,
        ..Weapon::default()
    });
    assert!(source.set_weapon_barrel_count_for_slot(0, 3));
    let mut template = ThingTemplate::new("OwnedDroneVictim");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game.templates
        .insert(template.name.clone(), template.clone());
    let target_id = game
        .create_object_for_player("OwnedDroneVictim", 1, Vec3::new(90.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(target_id, TARGET);
    game.frame = 30;
    game
}
fn ammo(game: &GameLogic) -> Option<u32> {
    game.host_object(SOURCE)
        .unwrap()
        .weapon
        .as_ref()
        .unwrap()
        .ammo
}
fn health(game: &GameLogic) -> f32 {
    game.host_object(TARGET).unwrap().health.current
}
fn delayed(game: &GameLogic) -> usize {
    crate::game_logic::combat::live_projectileless_delayed_count_for_test(&game.combat_system)
}

#[test]
fn sentry_requires_real_deploy_then_weapon_windup_and_commits_one_delayed_shot() {
    let mut game = world(true, 0.5);
    let peer = world(true, 0.5);
    let before = game
        .host_object(SOURCE)
        .unwrap()
        .weapon_barrel_cursor_for_snapshot(0);
    game.try_sentry_drone_residual_fire(SOURCE);
    assert_eq!(ammo(&game), Some(2));
    assert_eq!(delayed(&game), 0);
    assert!(!game.host_object(SOURCE).unwrap().deploy_style_allows_fire());
    for frame in 31..=33 {
        game.frame = frame;
        game.tick_deploy_style_updates();
    }
    game.try_sentry_drone_residual_fire(SOURCE);
    assert!(game.host_object(SOURCE).unwrap().deploy_style_allows_fire());
    assert_eq!(ammo(&game), Some(2));
    game.frame = 47;
    game.try_sentry_drone_residual_fire(SOURCE);
    assert_eq!(ammo(&game), Some(2));
    game.frame = 48;
    game.try_sentry_drone_residual_fire(SOURCE);
    assert_eq!(ammo(&game), Some(1));
    assert_eq!(game.sentry_drone_residual_auto_fires, 1);
    assert_eq!(delayed(&game), 1);
    assert_eq!(health(&game), 100.0);
    assert_ne!(
        game.host_object(SOURCE)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(0),
        before
    );
    game.frame = 49;
    game.try_sentry_drone_residual_fire(SOURCE);
    assert_eq!(ammo(&game), Some(1));
    assert_eq!(game.sentry_drone_residual_auto_fires, 1);
    game.frame = 57;
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), 100.0);
    game.frame = 58;
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), 63.0);
    assert_eq!(delayed(&game), 0);
    assert_eq!(ammo(&peer), Some(2));
    assert_eq!(health(&peer), 100.0);
}

#[test]
fn hellfire_launch_consumes_once_and_deals_damage_only_at_real_impact_phase() {
    let mut game = world(false, 0.0);
    let peer = world(false, 0.0);
    let barrel = game
        .host_object(SOURCE)
        .unwrap()
        .weapon_barrel_cursor_for_snapshot(0);
    assert_eq!(
        game.attack_state_enter(SOURCE, TARGET),
        crate::game_logic::AttackMachineResult::Continue
    );
    // CPP AIM success enters FireWeapon; its body executes at the next real
    // update. Ordinary combat deliberately yields the admitted nested owner.
    game.update_combat(&[SOURCE], 1.0 / 30.0);
    game.tick_nested_attack_machines(&[SOURCE], 1.0, 30);
    assert_eq!(ammo(&game), Some(2));
    assert_eq!(
        game.host_object(SOURCE).unwrap().attack_substate(),
        crate::game_logic::AttackSubState::FireWeapon
    );
    assert_eq!(health(&game), 100.0);
    assert_eq!(delayed(&game), 0);
    game.frame = 31;
    game.update_combat(&[SOURCE], 1.0 / 30.0);
    game.tick_nested_attack_machines(&[SOURCE], 31.0 / 30.0, 31);
    crate::game_logic::combat::drain_pending_projectiles(
        &mut game.combat_system,
        game.objects.read(),
        game.frame,
        &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
    );
    assert_eq!(ammo(&game), Some(1));
    assert_eq!(game.hellfire_drone_residual_auto_fires, 1);
    assert_eq!(
        game.host_object(SOURCE)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(0),
        Some(((barrel.unwrap().0 + 1) % 3, 1))
    );
    assert_eq!(
        game.host_object(SOURCE).unwrap().ai_state(),
        AIState::Attacking
    );
    assert_eq!(health(&game), 100.0);
    assert_eq!(delayed(&game), 1);
    game.frame = 32;
    game.update_combat(&[SOURCE], 1.0 / 30.0);
    game.tick_nested_attack_machines(&[SOURCE], 32.0 / 30.0, 32);
    crate::game_logic::combat::drain_pending_projectiles(
        &mut game.combat_system,
        game.objects.read(),
        game.frame,
        &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
    );
    assert_eq!(ammo(&game), Some(1));
    assert_eq!(game.hellfire_drone_residual_auto_fires, 1);
    game.frame = 40;
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), 100.0);
    game.frame = 41;
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), 63.0);
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), 63.0);
    assert_eq!(ammo(&peer), Some(2));
    assert_eq!(health(&peer), 100.0);
}

#[test]
fn hellfire_scout_sentry_route_on_authored_weapon_not_chassis_name_hq_2qedt_245() {
    use crate::game_logic::host_slave_drones::{
        HELLFIRE_MISSILE_WEAPON, SlaveDroneKind, UPGRADE_AMERICA_DRONE_ARMOR,
        drone_armor_add_max_health,
    };
    for name in [
        "AmericaVehicleHellfireDrone",
        "USA_HellfireDrone",
        "TestHellfireDrone",
        "AirF_AmericaVehicleHellfireDrone",
        "AmericaVehicleScoutDrone",
        "TestScoutDrone",
        "AmericaVehicleSentryDrone",
        "TestSentryDrone",
        "Custom_HellfireDrone",
    ] {
        assert_eq!(
            crate::game_logic::primary_weapon_name_for_unit(name),
            None,
            "{name} weapon comes from authored WeaponSet, not a chassis-name row"
        );
    }

    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "usa", true));
    let mut carrier = ThingTemplate::new("AuthoredHellfireCarrier");
    carrier
        .add_kind_of(KindOf::Vehicle)
        .set_health(100.0)
        .set_primary_weapon_name(HELLFIRE_MISSILE_WEAPON);
    game.templates.insert(carrier.name.clone(), carrier);
    let mut chassis = ThingTemplate::new("AmericaVehicleHellfireDrone");
    chassis
        .add_kind_of(KindOf::Vehicle)
        .set_health(100.0)
        .set_primary_weapon_name("SomeOtherGun");
    game.templates.insert(chassis.name.clone(), chassis);
    let carrier_id = game
        .create_object_for_player("AuthoredHellfireCarrier", 0, Vec3::ZERO)
        .unwrap();
    let chassis_id = game
        .create_object_for_player("AmericaVehicleHellfireDrone", 0, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let carrier_before = game.host_object(carrier_id).unwrap().max_health;
    let chassis_before = game.host_object(chassis_id).unwrap().max_health;
    let n = game.apply_drone_armor_to_team(Team::USA.into(), UPGRADE_AMERICA_DRONE_ARMOR);
    assert_eq!(n, 1, "only the authored Hellfire carrier gets drone armor");
    let carrier_after = game.host_object(carrier_id).unwrap().max_health;
    assert!(
        (carrier_after - carrier_before - drone_armor_add_max_health(SlaveDroneKind::Hellfire))
            .abs()
            < 0.1
    );
    assert_eq!(game.host_object(chassis_id).unwrap().max_health, chassis_before);
}
