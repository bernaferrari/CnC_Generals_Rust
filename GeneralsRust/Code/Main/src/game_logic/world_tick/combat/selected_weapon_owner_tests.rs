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
    let mut source = ThingTemplate::new("ReceivingOrdinaryShooter");
    source
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable);
    source.set_primary_weapon_name("RangerMachineGun");
    let mut target = ThingTemplate::new("ReceivingOrdinaryVictim");
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
