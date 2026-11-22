//! Actual ordinary receiving combat with same-name opposing native definitions.
use super::*;
fn world(damage: f32) -> (GameLogic, ObjectId, ObjectId) {
    world_with_type(damage, "UNRESISTABLE")
}
fn world_with_type(damage: f32, damage_type: &str) -> (GameLogic, ObjectId, ObjectId) {
    world_with_prefire(damage, damage_type, 0, "PER_SHOT")
}
fn world_with_prefire(damage: f32, damage_type: &str, delay: u32, prefire: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    for (id, team, name) in [(1, Team::USA, "Owner"), (2, Team::GLA, "Victim")] {
        let mut player = Player::new(id, team, name, id == 1);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let mut definitions = HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            &format!(
                "Weapon RangerMachineGun\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 5\n DamageType = {damage_type}\n DeathType = NORMAL\n AttackRange = 150\n ClipSize = 4\n DelayBetweenShots = 0\n ClipReloadTime = 0\n PreAttackDelay = {delay}\n PreAttackType = {prefire}\n WeaponBonus = PLAYER_UPGRADE PRE_ATTACK 50%\n WeaponSpeed = 999999\n ProjectileObject = NONE\n FireFX = None\n FireSound = None\n AntiGround = Yes\nEnd\n"
            ),
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal
        ),
        1
    );
    // The shooter is placed in AI_ATTACK states, so it owns an authored
    // AIUpdateInterface like retail infantry (AIStates.cpp attack machine).
    world
        .admit_object_ini_sources([(
            "receiving-ordinary-shooter.ini".to_string(),
            "Object ReceivingOrdinaryShooter\nKindOf = INFANTRY\nBody = ActiveBody ModuleTag_Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface ModuleTag_AI\nEnd\nEnd\nObject ReceivingOrdinaryVictim\nBody = ActiveBody ModuleTag_Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nEnd\n"
                .to_string(),
        )])
        .expect("authored shooter admits");
    let mut source = world.templates["ReceivingOrdinaryShooter"].clone();
    source.add_kind_of(KindOf::Attackable); // host-only targeting flag
    source.set_primary_weapon_name("RangerMachineGun");
    let mut target = world.templates["ReceivingOrdinaryVictim"].clone();
    target.set_health(1000.0).add_kind_of(KindOf::Attackable);
    world.templates.insert(source.name.clone(), source);
    world.templates.insert(target.name.clone(), target);
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    assert!(world.weapon_definitions.is_some());
    world.frame = 10;
    let source = world
        .create_object_for_player("ReceivingOrdinaryShooter", 1, Vec3::ZERO)
        .unwrap();
    let target = world
        .create_object_for_player("ReceivingOrdinaryVictim", 2, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let object = world.host_object_mut(source).unwrap();
    object.restore_ai_state(AIState::Attacking);
    object.target = Some(target);
    (world, source, target)
}

#[test]
fn authored_ground_prefire_waits_before_each_shot_and_is_owner_local() {
    for prefire in ["PER_SHOT", "PER_ATTACK", "PER_CLIP"] {
        let (mut game, source, target) = world_with_prefire(20.0, "UNRESISTABLE", 1000, prefire);
        let (mut peer, peer_source, peer_target) = world_with_prefire(40.0, "UNRESISTABLE", 0, prefire);
        for (world, actor) in [(&mut game, source), (&mut peer, peer_source)] {
            let object = world.host_object_mut(actor).unwrap();
            object.target = None;
            object.target_location = Some(Vec3::new(40.0, 0.0, 0.0));
            object.restore_ai_state(AIState::AttackingGround);
        }
        game.frame = 100;
        game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
        assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
        assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
        peer.frame = 100;
        peer.update_combat(&[peer_source], LOGIC_FRAME_TIMESTEP).unwrap();
        assert_eq!(peer.host_object(peer_target).unwrap().health.current, 960.0);
        game.frame = 129;
        game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
        assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
        assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
        game.frame = 130;
        game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
        assert_eq!(game.host_object(target).unwrap().health.current, 980.0);
        assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(3));
        game.frame = 131;
        game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
        let expected = if prefire == "PER_CLIP" { 960.0 } else { 980.0 };
        assert_eq!(game.host_object(target).unwrap().health.current, expected);
        if prefire != "PER_CLIP" {
            game.frame = 161;
            game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
            assert_eq!(game.host_object(target).unwrap().health.current, 960.0);
        }
    }
}

#[test]
fn ground_prefire_continues_for_same_order_but_restarts_after_replacement_or_stop() {
    let (mut game, source, target) = world_with_prefire(20.0, "UNRESISTABLE", 1000, "PER_SHOT");
    let first = Vec3::new(40.0, 0.0, 0.0);
    let replacement = Vec3::new(41.0, 0.0, 0.0);
    let order = |game: &mut GameLogic, position, frame| {
        let actor = game.host_object_mut(source).unwrap();
        actor.set_target_location(Some(position), frame, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        actor.restore_ai_state(AIState::AttackingGround);
    };
    order(&mut game, first, 100);
    game.frame = 100;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    order(&mut game, first, 110);
    game.frame = 110;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert!((game.host_object(source).unwrap().pre_attack_ready_at - 130.0 / 30.0).abs() < 1e-5);
    order(&mut game, replacement, 120);
    game.frame = 120;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    game.frame = 130;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
    game.frame = 150;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 980.0);
    game.frame = 151;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    game.host_object_mut(source).unwrap().stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    order(&mut game, first, 160);
    game.frame = 160;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    game.frame = 181;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 980.0);
    game.frame = 190;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 960.0);
}

#[test]
fn ground_prefire_does_not_transfer_between_weapon_slots() {
    let (mut game, source, target) = world_with_prefire(20.0, "UNRESISTABLE", 1000, "PER_SHOT");
    let actor = game.host_object_mut(source).unwrap();
    actor.target = None;
    actor.target_location = Some(Vec3::new(40.0, 0.0, 0.0));
    actor.restore_ai_state(AIState::AttackingGround);
    game.frame = 100;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    {
        let actor = game.host_object_mut(source).unwrap();
        actor.set_active_weapon_slot(0);
        assert!((actor.pre_attack_ready_at - 130.0 / 30.0).abs() < 1e-5);
        actor.set_active_weapon_slot(1);
        actor.set_active_weapon_slot(0);
    }
    game.frame = 110;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    game.frame = 130;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
    game.frame = 140;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 980.0);
}

#[test]
fn ground_prefire_uses_receiving_weapon_bonus() {
    let (mut game, source, target) = world_with_prefire(20.0, "UNRESISTABLE", 1000, "PER_SHOT");
    let actor = game.host_object_mut(source).unwrap();
    actor.target = None;
    actor.target_location = Some(Vec3::new(40.0, 0.0, 0.0));
    actor.restore_ai_state(AIState::AttackingGround);
    actor.set_weapon_bonus_player_upgrade(true);
    game.frame = 100;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    game.frame = 114;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
    game.frame = 115;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(target).unwrap().health.current, 980.0);
    assert_eq!(game.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(3));
}

#[test]
fn ground_prefire_deadline_survives_world_snapshot() {
    let (mut game, source, target) = world_with_prefire(20.0, "UNRESISTABLE", 1000, "PER_SHOT");
    let actor = game.host_object_mut(source).unwrap();
    actor.target = None;
    actor.target_location = Some(Vec3::new(40.0, 0.0, 0.0));
    actor.restore_ai_state(AIState::AttackingGround);
    game.frame = 100;
    game.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&game).unwrap();
    let (mut restored, _, _) = world_with_prefire(20.0, "UNRESISTABLE", 1000, "PER_SHOT");
    builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();
    restored.frame = 129;
    restored.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(restored.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(restored.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(4));
    restored.frame = 130;
    restored.update_combat(&[source], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(restored.host_object(target).unwrap().health.current, 980.0);
    assert_eq!(restored.host_object(source).unwrap().weapon.as_ref().unwrap().ammo, Some(3));
}
#[test]
fn ordinary_update_combat_keeps_same_name_receiving_damage_and_once_ammo() {
    let (mut a, source, target) = world(20.0);
    let (mut b, other_source, other_target) = world(40.0);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    a.frame = 100;
    a.update_combat(&[source, target], LOGIC_FRAME_TIMESTEP);
    assert_eq!(a.host_object(target).unwrap().health.current, 980.0);
    assert!(
        a.combat_particles
            .systems_of_kind(
                crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash
            )
            .is_empty(),
        "native FireFX None cannot fall back to a generic flash"
    );
    assert_eq!(
        a.host_object(source).unwrap().weapon.as_ref().unwrap().ammo,
        Some(3)
    );
    assert_eq!(b.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(
        b.host_object(source).unwrap().weapon.as_ref().unwrap().ammo,
        Some(4)
    );
    b.frame = 100;
    b.update_combat(&[source, target], LOGIC_FRAME_TIMESTEP);
    assert_eq!(b.host_object(target).unwrap().health.current, 960.0);
    assert_eq!(
        b.host_object(source).unwrap().weapon.as_ref().unwrap().ammo,
        Some(3)
    );
    assert_eq!(a.host_object(target).unwrap().health.current, 980.0);
}
#[test]
fn ordinary_native_zero_damage_does_not_adopt_stock_ranger_damage_or_spend_ammo() {
    let (mut game, source, target) = world_with_type(0.0, "SMALL_ARMS");
    assert_eq!(
        game.host_object(source)
            .unwrap()
            .selected_native_weapon_rule(0)
            .unwrap()
            .primary_damage,
        0.0
    );
    game.frame = 100;
    game.update_combat(&[source, target], LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(
        game.host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(4)
    );
}

#[test]
fn ordinary_zero_unresistable_remains_selected_and_spends_one_real_shot() {
    let (mut game, source, target) = world(0.0);
    game.frame = 100;
    game.update_combat(&[source, target], LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(target).unwrap().health.current, 1000.0);
    assert_eq!(
        game.host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(3),
        "CPP WeaponSet858 exempts Unresistable from zero-damage elimination"
    );
    assert_eq!(game.weapon_discharge_log.len(), 1);
    assert!(
        game.combat_particles
            .systems_of_kind(
                crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash
            )
            .is_empty()
    );
}

#[test]
fn idle_reload_deadline_tracks_fired_slot_and_is_local_to_each_owner() {
    let mut game = GameLogic::new();
    for (id, team) in [(1, Team::USA), (2, Team::GLA)] {
        let mut player = Player::new(id, team, "Deadline", id == 1);
        player.alliance_team = id as i32;
        game.add_player(player);
    }
    let mut definitions = HashMap::new();
    let ini = "Weapon IdleDeadlinePrimary\n PrimaryDamage = 1\n DamageType = UNRESISTABLE\n AttackRange = 150\n ClipSize = 0\n DelayBetweenShots = 0\n AutoReloadWhenIdle = 333\n WeaponSpeed = 999999\n ProjectileObject = NONE\n AntiGround = Yes\nEnd\n\
Weapon IdleDeadlineSecondary\n PrimaryDamage = 2\n DamageType = UNRESISTABLE\n AttackRange = 150\n ClipSize = 4\n AutoReloadsClip = No\n DelayBetweenShots = 0\n AutoReloadWhenIdle = 0\n WeaponSpeed = 999999\n ProjectileObject = NONE\n AntiGround = Yes\nEnd\n";
    assert_eq!(crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut definitions, ini,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    ), 2);
    let mut source = ThingTemplate::new("IdleDeadlineShooter");
    source.add_kind_of(KindOf::Infantry).add_kind_of(KindOf::Attackable);
    source.set_primary_weapon_name("IdleDeadlinePrimary");
    source.set_secondary_weapon_name("IdleDeadlineSecondary");
    let mut target = ThingTemplate::new("IdleDeadlineTarget");
    target.set_health(1000.0).add_kind_of(KindOf::Attackable);
    game.templates.insert(source.name.clone(), source);
    game.templates.insert(target.name.clone(), target);
    game.admit_weapon_content_snapshot(
        definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
        &[],
    ).unwrap();
    game.frame = 10;
    let a = game.create_object_for_player("IdleDeadlineShooter", 1, Vec3::ZERO).unwrap();
    let b = game.create_object_for_player("IdleDeadlineShooter", 1, Vec3::new(0.0, 5.0, 0.0)).unwrap();
    let victim = game.create_object_for_player("IdleDeadlineTarget", 2, Vec3::new(40.0, 0.0, 0.0)).unwrap();
    for id in [a, b] {
        let actor = game.host_object_mut(id).unwrap();
        actor.restore_ai_state(AIState::Attacking);
        actor.target = Some(victim);
    }
    game.frame = 100;
    game.update_combat(&[a], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(victim).unwrap().health.current, 999.0);
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 110);
    assert_eq!(game.host_object(b).unwrap().frame_to_force_reload, 0);
    for id in [a, b] {
        game.host_object_mut(id).unwrap().set_active_weapon_slot(1);
    }
    game.frame = 103;
    game.update_combat(&[a, b], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(victim).unwrap().health.current, 995.0);
    for id in [a, b] {
        assert_eq!(game.host_object(id).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
        let actor = game.host_object_mut(id).unwrap();
        actor.restore_ai_state(AIState::Idle);
        actor.target = None;
        // Prevent autonomous reacquisition from adding shots during host ticks.
        actor.status.weapons_jammed = true;
    }
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 110);
    assert_eq!(game.host_object(b).unwrap().frame_to_force_reload, 0);
    game.frame = 109;
    game.update_simulation(LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(a).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
    game.frame = 110;
    game.update_simulation(LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(a).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(4));
    assert_eq!(game.host_object(b).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 0);
    {
        let actor = game.host_object_mut(a).unwrap();
        actor.restore_ai_state(AIState::Attacking);
        actor.target = Some(victim);
        actor.status.weapons_jammed = false;
        actor.set_active_weapon_slot(0);
    }
    game.frame = 120;
    game.update_combat(&[a], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 130);
    game.host_object_mut(a).unwrap().set_active_weapon_slot(1);
    game.frame = 123;
    game.update_combat(&[a], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(a).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 130);
    game.host_object_mut(a).unwrap().set_active_weapon_slot(0);
    game.frame = 125;
    game.update_combat(&[a], LOGIC_FRAME_TIMESTEP).unwrap();
    assert_eq!(game.host_object(a).unwrap().frame_to_force_reload, 135);
    {
        let actor = game.host_object_mut(a).unwrap();
        actor.restore_ai_state(AIState::Idle);
        actor.target = None;
        actor.status.weapons_jammed = true;
    }
    game.frame = 130;
    game.update_simulation(LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(a).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
    game.frame = 135;
    game.update_simulation(LOGIC_FRAME_TIMESTEP);
    assert_eq!(game.host_object(a).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(4));
    assert_eq!(game.host_object(b).unwrap().secondary_weapon.as_ref().unwrap().ammo, Some(3));
}

/// Stock WeaponSet policies from INIZH Object/AmericaVehicle.ini and AmericaAir.ini.
/// Only the fields used by weapon choice are needed to exercise the native decision.
fn stock_weapon_choice_world() -> GameLogic {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };

    let mut world = GameLogic::new();
    let objects = r#"
Object AmericaVehicleHumvee
    KindOf = VEHICLE CAN_ATTACK
    Body = ActiveBody BodyTag
        MaxHealth = 240
        InitialHealth = 240
    End
    WeaponSet
        Conditions = None
        Weapon = PRIMARY HumveeGun
        Weapon = SECONDARY HumveeMissileWeaponAirDummy
    End
    WeaponSet
        Conditions = PLAYER_UPGRADE
        Weapon = PRIMARY HumveeGun
        Weapon = SECONDARY HumveeMissileWeapon
        Weapon = TERTIARY HumveeMissileWeaponAir
        PreferredAgainst = TERTIARY AIRCRAFT
    End
End
Object AmericaVehicleComanche
    KindOf = VEHICLE AIRCRAFT CAN_ATTACK
    Body = ActiveBody BodyTag
        MaxHealth = 220
        InitialHealth = 220
    End
    WeaponSet
        Conditions = None
        Weapon = PRIMARY Comanche20mmCannonWeapon
        PreferredAgainst = PRIMARY INFANTRY
        AutoChooseSources = PRIMARY FROM_PLAYER FROM_SCRIPT FROM_AI
        Weapon = SECONDARY ComancheAntiTankMissileWeapon
        PreferredAgainst = SECONDARY VEHICLE STRUCTURE
        AutoChooseSources = SECONDARY FROM_PLAYER FROM_SCRIPT FROM_AI
        Weapon = TERTIARY NONE
    End
    WeaponSet
        Conditions = PLAYER_UPGRADE
        Weapon = PRIMARY Comanche20mmCannonWeapon
        PreferredAgainst = PRIMARY INFANTRY
        AutoChooseSources = PRIMARY FROM_PLAYER FROM_SCRIPT FROM_AI
        Weapon = SECONDARY ComancheAntiTankMissileWeapon
        PreferredAgainst = SECONDARY VEHICLE STRUCTURE
        AutoChooseSources = SECONDARY FROM_PLAYER FROM_SCRIPT FROM_AI
        Weapon = TERTIARY ComancheRocketPodWeapon
        AutoChooseSources = TERTIARY NONE
    End
End
Object WeaponChoiceAircraft
    KindOf = VEHICLE AIRCRAFT
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
End
Object WeaponChoiceGround
    KindOf = SELECTABLE
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
End
"#;
    world
        .admit_object_ini_sources([("stock-weapon-choice.ini".into(), objects.into())])
        .unwrap();
    let mut weapons_ini = String::new();
    for (name, damage, range, damage_type, clip, projectile, anti_vehicle, anti_ground) in [
        ("HumveeGun", 10.0, 150, "COMANCHE_VULCAN", 0, "NONE", "No", "Yes"),
        ("HumveeMissileWeaponAirDummy", 0.0001, 320, "EXPLOSION", 0, "NONE", "Yes", "No"),
        ("HumveeMissileWeapon", 30.0, 150, "EXPLOSION", 1, "HumveeMissile", "No", "Yes"),
        ("HumveeMissileWeaponAir", 50.0, 320, "EXPLOSION", 1, "PatriotMissile", "Yes", "No"),
        ("Comanche20mmCannonWeapon", 6.0, 200, "COMANCHE_VULCAN", 0, "NONE", "No", "Yes"),
        ("ComancheAntiTankMissileWeapon", 50.0, 200, "JET_MISSILES", 4, "ComancheAntiTankMissile", "No", "Yes"),
        ("ComancheRocketPodWeapon", 30.0, 200, "EXPLOSION", 20, "ComancheRocketPodRocket", "No", "Yes"),
    ] {
        weapons_ini.push_str(&format!(
            "Weapon {name}\nPrimaryDamage = {damage}\nAttackRange = {range}\n\
             DamageType = {damage_type}\nClipSize = {clip}\nProjectileObject = {projectile}\n\
             AntiAirborneVehicle = {anti_vehicle}\nAntiGround = {anti_ground}\nEnd\n"
        ));
    }
    let mut weapons = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut weapons,
            &weapons_ini,
            WeaponDefinitionLoadType::Normal,
        ),
        7
    );
    world
        .admit_weapon_content_snapshot(
            weapons
                .into_values()
                .map(|weapon| (weapon.name.clone(), weapon))
                .collect(),
            &[],
        )
        .unwrap();
    world.frame = 100;
    world
}

#[test]
fn authored_weapon_selection_does_not_rebootstrap_receiving_common() {
    let mut receiving = stock_weapon_choice_world();
    let mut peer = stock_weapon_choice_world();
    let spawn = |world: &mut GameLogic| {
        let humvee = world
            .create_object("AmericaVehicleHumvee", Team::USA, Vec3::ZERO)
            .unwrap();
        let aircraft = world
            .create_object(
                "WeaponChoiceAircraft",
                Team::China,
                Vec3::new(100.0, 10.0, 0.0),
            )
            .unwrap();
        world
            .host_object_mut(aircraft)
            .unwrap()
            .status
            .airborne_target = true;
        (humvee, aircraft)
    };
    let (humvee, aircraft) = spawn(&mut receiving);
    assert_eq!((humvee, aircraft), spawn(&mut peer));
    let frame = receiving.frame;
    let current_time = frame as f32 * LOGIC_FRAME_TIMESTEP;
    receiving
        .host_object_mut(humvee)
        .unwrap()
        .set_weapon_condition_at_frame(3, true, frame);
    assert!(peer.choose_best_weapon_for_target(humvee, Some(aircraft), current_time));
    assert_eq!(
        peer.host_object(humvee).unwrap().selected_weapon_slot(),
        Some(1)
    );

    let upgrade_center = std::sync::Arc::clone(
        &receiving
            .world_services
            .common()
            .system
            .upgrade_center_lifetime,
    );
    peer.with_own_services(|| {
        // An empty center witnesses reentry into Common bootstrapping without
        // holding a guard across the operation or intentionally stalling it.
        let previous = std::mem::take(&mut *upgrade_center.write().unwrap());
        let chose = receiving.choose_best_weapon_for_target(humvee, Some(aircraft), current_time);
        let selected = receiving
            .host_object(humvee)
            .unwrap()
            .selected_weapon_slot();
        let locked = receiving.host_object_mut(humvee).unwrap().set_weapon_lock(
            2,
            crate::game_logic::object::WeaponLockType::LockedPermanently,
        );
        let chose_locked = receiving.choose_best_weapon_for_target(humvee, None, current_time);
        let selected_locked = receiving
            .host_object(humvee)
            .unwrap()
            .selected_weapon_slot();
        receiving
            .host_object_mut(humvee)
            .unwrap()
            .release_weapon_lock(crate::game_logic::object::WeaponLockType::LockedPermanently);
        let chose_ground = receiving.choose_best_weapon_for_target(humvee, None, current_time);
        let selected_ground = receiving
            .host_object(humvee)
            .unwrap()
            .selected_weapon_slot();
        let untouched = {
            let mut center = upgrade_center.write().unwrap();
            let untouched = center.get_all_upgrades().is_empty();
            *center = previous;
            untouched
        };
        assert!(chose);
        assert_eq!(selected, Some(2));
        assert!(locked && chose_locked);
        assert_eq!(selected_locked, Some(2));
        assert!(chose_ground);
        assert_eq!(selected_ground, Some(0));
        assert_eq!(
            peer.host_object(humvee).unwrap().selected_weapon_slot(),
            Some(1)
        );
        assert!(
            untouched,
            "authored selection should not initialize Common upgrades"
        );
    });
}

#[test]
fn stock_upgraded_humvee_auto_selects_air_tow() {
    let mut world = stock_weapon_choice_world();
    let humvee = world
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::ZERO)
        .unwrap();
    let aircraft = world
        .create_object("WeaponChoiceAircraft", Team::China, Vec3::new(100.0, 10.0, 0.0))
        .unwrap();
    world.host_object_mut(aircraft).unwrap().status.airborne_target = true;
    let frame = world.frame;
    let current_time = frame as f32 * LOGIC_FRAME_TIMESTEP;

    assert!(world.choose_best_weapon_for_target(humvee, Some(aircraft), current_time));
    assert_eq!(world.host_object(humvee).unwrap().selected_weapon_slot(), Some(1));

    let object = world.host_object_mut(humvee).unwrap();
    object.set_weapon_condition_at_frame(3, true, frame); // PLAYER_UPGRADE
    assert!(!object.is_weapon_locked());
    assert_eq!(object.selected_weapon_slot(), Some(0));
    assert_eq!(object.selected_weapon_name(2), Some("HumveeMissileWeaponAir"));
    assert_eq!(object.selected_projectile_name(2), Some("PatriotMissile"));
    assert_eq!(object.weapon_slot(2).unwrap().ammo, Some(1));

    assert!(world.choose_best_weapon_for_target(humvee, Some(aircraft), current_time));
    assert_eq!(world.host_object(humvee).unwrap().selected_weapon_slot(), Some(2));
}

#[test]
fn stock_upgraded_comanche_keeps_rocket_pods_button_only() {
    use crate::game_logic::object::{WeaponFireStatus, WeaponLockType};

    let mut world = stock_weapon_choice_world();
    let comanche = world
        .create_object("AmericaVehicleComanche", Team::USA, Vec3::ZERO)
        .unwrap();
    let ground = world
        .create_object("WeaponChoiceGround", Team::China, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let frame = world.frame;
    let current_time = frame as f32 * LOGIC_FRAME_TIMESTEP;
    let object = world.host_object_mut(comanche).unwrap();
    object.set_weapon_condition_at_frame(3, true, frame); // PLAYER_UPGRADE
    assert_eq!(object.selected_weapon_name(2), Some("ComancheRocketPodWeapon"));
    assert_eq!(object.weapon_slot(2).unwrap().frame_status_at(frame), WeaponFireStatus::ReadyToFire);
    // Neither ordinary weapon is ready, so an eligible tertiary would win.
    // The target has no KindOf preferred by the primary or secondary.
    for slot in [0, 1] {
        let weapon = object.weapon_slot_mut(slot).unwrap();
        weapon.next_fire_frame = frame + 10;
        weapon.frame_clock_status = WeaponFireStatus::BetweenFiringShots;
    }

    assert!(world.choose_best_weapon_for_target(comanche, Some(ground), current_time));
    assert_eq!(world.host_object(comanche).unwrap().selected_weapon_slot(), Some(1));

    assert!(world.host_object_mut(comanche).unwrap().set_weapon_lock(
        2,
        WeaponLockType::LockedPermanently,
    ));
    assert!(world.choose_best_weapon_for_target(comanche, Some(ground), current_time));
    assert_eq!(world.host_object(comanche).unwrap().selected_weapon_slot(), Some(2));
}

#[test]
fn stock_bender_weapon_readiness_follows_the_controlling_turret() {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    use crate::game_logic::object::{TurretSubState, WeaponFireStatus};

    let mut world = GameLogic::new();
    // INIZH.big Data/INI/Object/AmericaMiscUnit.ini and Data/INI/Weapon.ini.
    let objects = r#"
Object AmericaVehicleBattleShipUSSBender
    KindOf = PRELOAD CAN_CAST_REFLECTIONS VEHICLE SCORE SELECTABLE
    Geometry = BOX
    GeometryMajorRadius = 200.0
    GeometryMinorRadius = 20.0
    GeometryHeight = 40
    GeometryIsSmall = No
    WeaponSet
        Conditions = None
        Weapon = PRIMARY BattleshipUSSBenderGun
        Weapon = SECONDARY BattleshipUSSBenderGun
        AutoChooseSources = PRIMARY FROM_SCRIPT FROM_PLAYER FROM_AI
        AutoChooseSources = SECONDARY FROM_SCRIPT FROM_PLAYER FROM_AI
    End
    Body = ActiveBody ModuleTag_02
        MaxHealth = 10000.0
        InitialHealth = 10000.0
    End
    Behavior = AIUpdateInterface ModuleTag_03
        Turret
            TurretTurnRate = 80
            TurretPitchRate = 80
            FirePitch = 20
            AllowsPitch = Yes
            RecenterTime = 5000
            ControlledWeaponSlots = PRIMARY
            NaturalTurretAngle = 180
        End
        AltTurret
            TurretTurnRate = 80
            TurretPitchRate = 80
            FirePitch = 20
            AllowsPitch = Yes
            RecenterTime = 5000
            ControlledWeaponSlots = SECONDARY
        End
        TurretsLinked = Yes
    End
End
Object BenderChoiceTarget
    KindOf = VEHICLE SELECTABLE
    Body = ActiveBody BodyTag
        MaxHealth = 1000
        InitialHealth = 1000
    End
End
"#;
    world
        .admit_object_ini_sources([("stock-bender-choice.ini".into(), objects.into())])
        .unwrap();
    let weapon = r#"
Weapon BattleshipUSSBenderGun
    PrimaryDamage = 300
    PrimaryDamageRadius = 15
    AttackRange = 99999
    DamageType = EXPLOSION
    DeathType = EXPLODED
    WeaponSpeed = 400
    ProjectileObject = USSBenderGunShell
    DelayBetweenShots = 750
    ClipSize = 3
    AutoReloadsClip = No
End
"#;
    let mut definitions = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            weapon,
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    world.frame = 100;
    let ship = world
        .create_object("AmericaVehicleBattleShipUSSBender", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object(
            "BenderChoiceTarget",
            Team::China,
            Vec3::new(300.0, 0.0, 0.0),
        )
        .unwrap();
    let current_time = world.frame as f32 * LOGIC_FRAME_TIMESTEP;
    let ship_object = world.host_object(ship).unwrap();
    use gamelogic::common::TurretType;
    assert_eq!(ship_object.which_turret_for_weapon_slot(0), TurretType::Primary);
    assert_eq!(ship_object.which_turret_for_weapon_slot(1), TurretType::Secondary);
    for slot in [0, 1] {
        assert_eq!(
            ship_object.selected_weapon_name(slot),
            Some("BattleshipUSSBenderGun")
        );
        assert_eq!(
            ship_object
                .weapon_slot(slot)
                .unwrap()
                .frame_status_at(world.frame),
            WeaponFireStatus::ReadyToFire
        );
    }
    assert!(world.choose_best_weapon_for_target(ship, Some(victim), current_time));
    assert_eq!(
        world.host_object(ship).unwrap().selected_weapon_slot(),
        Some(0)
    );

    // The linked turrets have the same victim. The forward turret has reached
    // FIRE while the aft turret is still turning from its natural angle.
    world.set_turret_target_object(ship, Some(victim), false);
    world
        .host_object_mut(ship)
        .unwrap()
        .set_active_weapon_slot(1);
    world.set_turret_target_object(ship, Some(victim), false);
    {
        let ship_object = world.host_object_mut(ship).unwrap();
        assert_eq!(ship_object.turret_ai.turret_substate, TurretSubState::Aim);
        let alt = ship_object.alt_turret_ai.as_mut().unwrap();
        assert_eq!(alt.turret_target_id, Some(victim));
        alt.turret_substate = TurretSubState::Fire;
        ship_object.set_active_weapon_slot(0);
    }
    assert!(world.choose_best_weapon_for_target(ship, Some(victim), current_time));
    assert_eq!(
        world.host_object(ship).unwrap().selected_weapon_slot(),
        Some(1)
    );

    let ship_object = world.host_object_mut(ship).unwrap();
    ship_object.turret_ai.turret_substate = TurretSubState::Fire;
    ship_object.alt_turret_ai.as_mut().unwrap().turret_substate = TurretSubState::Aim;
    let ship_object = world.host_object(ship).unwrap();
    let target = world.host_object(victim).unwrap();
    assert!(ship_object.is_weapon_slot_on_turret_and_aiming_at_target(1, target));
    assert!(!ship_object.is_weapon_slot_on_turret_and_aiming_at_target(0, target));
    assert!(world.choose_best_weapon_for_target(ship, Some(victim), current_time));
    assert_eq!(
        world.host_object(ship).unwrap().selected_weapon_slot(),
        Some(0)
    );
}

#[test]
fn authored_damage_choice_and_shot_use_receiving_weapon_bonuses() {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };

    let setup = |global_percent: u32| {
        let mut world = GameLogic::new();
        world
            .admit_combat_bonus_ini_sources([format!(
                "GameData\n WeaponBonus = PLAYER_UPGRADE DAMAGE {global_percent}%\nEnd\n"
            )])
            .unwrap();
        let objects = r#"
Object BonusChoiceShooter
    KindOf = INFANTRY SELECTABLE CAN_ATTACK
    TransportSlotCount = 1
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
    Behavior = AIUpdateInterface Brain
    End
    WeaponSet
        Conditions = NONE
        Weapon = PRIMARY BonusChoicePrimary
        Weapon = SECONDARY BonusChoiceSecondary
    End
End
Object BonusChoiceCarrier
    KindOf = VEHICLE SELECTABLE
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
    Behavior = TransportContain Transport
        ContainMax = 2
        Slots = 2
        AllowInsideKindOf = INFANTRY
        WeaponBonusPassedToPassengers = Yes
        PassengersAllowedToFire = Yes
    End
End
Object BonusChoiceVictim
    KindOf = SELECTABLE
    Body = ActiveBody BodyTag
        MaxHealth = 1000
        InitialHealth = 1000
    End
End
"#;
        world
            .admit_object_ini_sources([("authored-choice-bonus.ini".into(), objects.into())])
            .unwrap();
        let weapons = r#"
Weapon BonusChoicePrimary
    PrimaryDamage = 112.5
    AttackRange = 200
    DamageType = UNRESISTABLE
    WeaponSpeed = 999999
    ProjectileObject = NONE
    DelayBetweenShots = 2000
End
Weapon BonusChoiceSecondary
    PrimaryDamage = 100
    AttackRange = 200
    DamageType = UNRESISTABLE
    WeaponSpeed = 999999
    ProjectileObject = NONE
    DelayBetweenShots = 2000
    WeaponBonus = PLAYER_UPGRADE DAMAGE 125%
End
"#;
        let mut definitions = HashMap::new();
        assert_eq!(
            parse_weapon_definitions_ini_into(
                &mut definitions,
                weapons,
                WeaponDefinitionLoadType::Normal
            ),
            2
        );
        world
            .admit_weapon_content_snapshot(
                definitions
                    .into_values()
                    .map(|rule| (rule.name.clone(), rule))
                    .collect(),
                &[],
            )
            .unwrap();
        for (id, team, name) in [(1, Team::USA, "Owner"), (2, Team::China, "Victim")] {
            let mut player = Player::new(id, team, name, id == 1);
            player.alliance_team = id as i32;
            world.add_player(player);
        }
        world.frame = 100;
        let shooter = world
            .create_object_for_player("BonusChoiceShooter", 1, Vec3::ZERO)
            .unwrap();
        let carrier = world
            .create_object_for_player("BonusChoiceCarrier", 1, Vec3::new(-200.0, 0.0, 0.0))
            .unwrap();
        let victim = world
            .create_object_for_player("BonusChoiceVictim", 2, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        (world, shooter, carrier, victim)
    };
    let (mut driving, shooter, carrier, victim) = setup(100);
    let (mut peer, peer_shooter, peer_carrier, peer_victim) = setup(200);
    assert_eq!(
        (shooter, carrier, victim),
        (peer_shooter, peer_carrier, peer_victim)
    );
    let t = driving.frame as f32 * LOGIC_FRAME_TIMESTEP;

    assert!(driving.choose_best_weapon_for_target(shooter, Some(victim), t));
    assert_eq!(
        driving.host_object(shooter).unwrap().selected_weapon_slot(),
        Some(0)
    );
    for world in [&mut driving, &mut peer] {
        world
            .host_object_mut(shooter)
            .unwrap()
            .set_weapon_bonus_player_upgrade(true);
    }
    // Global 200% plus weapon 125% is additive. Both estimates are 225.
    assert!(peer.choose_best_weapon_for_target(shooter, Some(victim), t));
    assert_eq!(
        peer.host_object(shooter).unwrap().selected_weapon_slot(),
        Some(0)
    );

    peer.with_own_services(|| {
        assert!(driving.choose_best_weapon_for_target(shooter, Some(victim), t));
        assert_eq!(
            driving.host_object(shooter).unwrap().selected_weapon_slot(),
            Some(1)
        );

        let source = driving.host_object_mut(shooter).unwrap();
        source.set_weapon_bonus_player_upgrade(false);
        source.contained_by = Some(carrier);
        assert!(
            driving
                .host_object(carrier)
                .unwrap()
                .passes_weapon_bonus_to_passengers()
        );
        assert!(driving.choose_best_weapon_for_target(shooter, Some(victim), t));
        assert_eq!(
            driving.host_object(shooter).unwrap().selected_weapon_slot(),
            Some(0)
        );
        driving
            .host_object_mut(carrier)
            .unwrap()
            .set_weapon_bonus_player_upgrade(true);
        assert!(driving.choose_best_weapon_for_target(shooter, Some(victim), t));
        assert_eq!(
            driving.host_object(shooter).unwrap().selected_weapon_slot(),
            Some(1)
        );

        // Exercise the separate actual object-fire selection boundary. A
        // physics-only fix would reselect primary here before the shot.
        let source = driving.host_object_mut(shooter).unwrap();
        source.contained_by = None;
        source.set_weapon_bonus_player_upgrade(true);
        source.set_active_weapon_slot(0);
        source.restore_ai_state(AIState::Attacking);
        source.target = Some(victim);
        let result = driving.update_object_target_combat(
            CombatAttackInput {
                attacker_id: shooter,
                current_time: t,
                attacker_team: Team::USA,
                target_id: Some(victim),
                target_location: None,
                overcharge: false,
            },
            victim,
        );
        assert!(matches!(result, CombatShotResult::Delivered));
        assert_eq!(
            driving.host_object(shooter).unwrap().selected_weapon_slot(),
            Some(1)
        );
        assert_eq!(driving.host_object(victim).unwrap().health.current, 875.0);
        assert_eq!(peer.host_object(victim).unwrap().health.current, 1000.0);
        assert_eq!(
            peer.host_object(shooter).unwrap().selected_weapon_slot(),
            Some(0)
        );
    });
}
