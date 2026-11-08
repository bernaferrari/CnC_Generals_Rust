//! Actual ordinary receiving combat with same-name opposing native definitions.
use super::*;
fn world(damage: f32) -> (GameLogic, ObjectId, ObjectId) {
    world_with_type(damage, "UNRESISTABLE")
}
fn world_with_type(damage: f32, damage_type: &str) -> (GameLogic, ObjectId, ObjectId) {
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
                "Weapon RangerMachineGun\n PrimaryDamage = {damage}\n DamageType = {damage_type}\n DeathType = NORMAL\n AttackRange = 150\n ClipSize = 4\n DelayBetweenShots = 0\n ClipReloadTime = 0\n WeaponSpeed = 999999\n ProjectileObject = NONE\n FireFX = None\n FireSound = None\n AntiGround = Yes\nEnd\n"
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
