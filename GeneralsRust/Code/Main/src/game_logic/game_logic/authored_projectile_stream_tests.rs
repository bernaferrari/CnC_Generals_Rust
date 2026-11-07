//! C++ Weapon.cpp:203,288,2388-2418: streams belong to the exact firing
//! WeaponTemplate. Empty is authoritative; no other WeaponSet slot is queried.
//! Actual parsed Weapon.ini -> admitted Object -> accepted shot -> materialize
//! -> ordinary simulation tick, without shot/stream-registry injection.
use super::*;
use crate::game_logic::object::WeaponLockType;

const PRIMARY: &str = "AuthoredStreamPrimaryWeapon";
const SECONDARY: &str = "AuthoredStreamSecondaryWeapon";
const TERTIARY: &str = "AuthoredStreamTertiaryWeapon";
const EMPTY_DRAGON: &str = "AuthoredDragonFlameExplicitEmptyStream";
const OMITTED_DRAGON: &str = "AuthoredDragonFlameDefaultEmptyStream";
const EMPTY_SECONDARY: &str = "AuthoredEmptySecondaryStreamWeapon";
const EMPTY_TERTIARY: &str = "AuthoredEmptyTertiaryStreamWeapon";
const PRIMARY_STREAM: &str = "AuthoredPrimaryProjectileStream";
const SECONDARY_STREAM: &str = "AuthoredSecondaryProjectileStream";
const TERTIARY_STREAM: &str = "AuthoredTertiaryProjectileStream";

fn stream_world(missing_projectile: bool) -> GameLogic {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    let mut rows = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut rows,
            "Weapon AuthoredDragonFlameExplicitEmptyStream\n ProjectileStreamName = PreviouslyAuthoredProjectileStream\nEnd\n",
            WeaponDefinitionLoadType::Normal,
        ),
        1
    );
    assert_eq!(
        rows[&EMPTY_DRAGON.to_ascii_lowercase()].projectile_stream_name,
        "PreviouslyAuthoredProjectileStream"
    );
    let rules = [
        (PRIMARY, Some(PRIMARY_STREAM)),
        (SECONDARY, Some(SECONDARY_STREAM)),
        (TERTIARY, Some(TERTIARY_STREAM)),
        (EMPTY_DRAGON, Some("")),
        (OMITTED_DRAGON, None),
        (EMPTY_SECONDARY, Some("")),
        (EMPTY_TERTIARY, Some("")),
    ];
    let mut ini = String::new();
    for (name, stream) in rules {
        ini.push_str(&format!(
            "Weapon {name}\n  PrimaryDamage = 7\n  PrimaryDamageRadius = 0\n  AttackRange = 200\n  DamageType = SMALL_ARMS\n  ProjectileObject = AuthoredStreamLiveProjectile\n  WeaponSpeed = 30\n  DelayBetweenShots = 1000\n  ClipSize = 8\n  ClipReloadTime = 1000\n  PreAttackDelay = 0\n  AntiGround = Yes\n"
        ));
        if let Some(stream) = stream {
            ini.push_str(&format!("  ProjectileStreamName = \"{stream}\"\n"));
        }
        ini.push_str("End\n");
    }
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut rows,
            &ini,
            WeaponDefinitionLoadType::CreateOverrides,
        ),
        rules.len()
    );
    for (name, stream) in rules {
        let definition = &rows[&name.to_ascii_lowercase()];
        assert_eq!(definition.projectile_stream_name, stream.unwrap_or(""));
        assert_eq!(definition.projectile_name, "AuthoredStreamLiveProjectile");
        assert_eq!(
            definition.weapon_speed.to_bits(),
            (30.0f32 * (1.0f32 / 30.0f32)).to_bits()
        );
        assert_eq!(definition.clip_size, 8);
        assert_eq!(definition.attack_range, 200.0);
    }
    if missing_projectile {
        rows.get_mut(&PRIMARY.to_ascii_lowercase())
            .unwrap()
            .projectile_name = "UnresolvedHostDragonFlameStreamContract".into();
    }
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(
        "Object AuthoredStreamLiveProjectile\n Behavior = MissileAIUpdate ModuleTag_Flight\n IgnitionDelay = 0\n TryToFollowTarget = Yes\n End\nEnd",
        "authored-stream-content.ini",
    ).unwrap();
    let objects: Vec<_> = parser
        .get_all_definitions()
        .iter()
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect();
    let mut world = GameLogic::new();
    world
        .admit_weapon_content_snapshot(
            rows.into_values()
                .map(|row| (row.name.clone(), row))
                .collect(),
            &objects,
        )
        .unwrap();
    world
}

fn prepare_world(
    slots: [&str; 3],
    selected: u8,
    missing_projectile: bool,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = stream_world(missing_projectile);
    let mut source_definition = ThingTemplate::new("AuthoredStreamShooter");
    source_definition
        .set_health(100.0)
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_name(slots[0])
        .set_secondary_weapon_name(slots[1])
        .set_tertiary_weapon_name(slots[2]);
    world
        .templates
        .insert(source_definition.name.clone(), source_definition);
    let source = world
        .create_object("AuthoredStreamShooter", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut target_definition = ThingTemplate::new("AuthoredStreamTarget");
    target_definition
        .set_health(100.0)
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_none();
    world
        .templates
        .insert(target_definition.name.clone(), target_definition);
    let target = world
        .create_object(
            "AuthoredStreamTarget",
            Team::China,
            Vec3::new(30.0, 0.0, 0.0),
        )
        .unwrap();
    assert!(
        world.objects[&target].weapon.is_none(),
        "actual target admission honors explicit unarmed rules"
    );
    world.set_current_frame(100);
    let target_position = world.objects[&target].get_position();
    {
        let shooter = world.objects.get_mut(&source).unwrap();
        for slot in 0..3u8 {
            assert_eq!(
                shooter.authored_weapon_name_for_slot(slot),
                Some(slots[usize::from(slot)])
            );
            assert_eq!(shooter.weapon_slot(slot).unwrap().ammo, Some(8));
        }
        // A real player-selected slot, not a fabricated queue or registry entry.
        shooter.set_active_weapon_slot(selected);
        assert!(shooter.set_weapon_lock(selected, WeaponLockType::LockedPermanently));
        shooter.set_prev_victim_pos(Some(target_position));
        assert_eq!(shooter.selected_weapon_slot(), Some(selected));
    }
    (world, source, target)
}

fn fire_and_tick(slots: [&str; 3], selected: u8) -> (GameLogic, ObjectId, ObjectId) {
    let (mut world, source, target) = prepare_world(slots, selected, false);
    assert!(
        world.objects[&source].is_within_attack_range_for_slot(selected, &world.objects[&target])
    );
    assert_eq!(
        world.attack_fire_weapon_update(source, target, 10.0),
        AttackFireResult::Success
    );
    assert_eq!(
        world.objects[&source].weapon_discharge_marker().weapon_slot,
        selected
    );
    assert_eq!(world.objects[&source].last_fire_slot, selected);
    for slot in 0..3u8 {
        assert_eq!(
            world.objects[&source].weapon_slot(slot).unwrap().ammo,
            Some(if slot == selected { 7 } else { 8 })
        );
    }
    world.drain_pending_projectiles_into_combat();
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert!(world.projectile_stream_snapshot().is_empty());
    {
        let projectiles = world.combat_system.projectiles_snapshot();
        let shot = projectiles[0];
        assert_eq!(shot.historic_weapon_key, slots[usize::from(selected)]);
        assert_eq!(shot.projectile_object_name, "AuthoredStreamLiveProjectile");
        assert_eq!(shot.shooter_id, source);
        assert_eq!(shot.target_id, Some(target));
        assert_eq!(shot.damage, 7.0);
    }
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_eq!(world.frame, 101, "actual ordinary simulation advanced once");
    assert_eq!(
        world.combat_system.projectile_count(),
        1,
        "slow accepted projectile still flies"
    );
    assert_eq!(
        world.objects[&target].health.current, 100.0,
        "stream observation precedes real impact"
    );
    assert_eq!(world.objects[&source].active_weapon_slot, selected);
    (world, source, target)
}

fn assert_stream_case(slots: [&str; 3], selected: u8, expected: Option<&str>) {
    let (world, source, target) = fire_and_tick(slots, selected);
    let streams = world.projectile_stream_snapshot();
    match expected {
        None => assert!(
            streams.is_empty(),
            "CPP exact empty template must not create stream; got {streams:?}"
        ),
        Some(expected) => {
            assert_eq!(streams.len(), 1);
            let (owner, name, points, victim) = &streams[0];
            assert_eq!(*owner, source);
            assert_eq!(name, expected);
            assert!(!points.is_empty());
            assert_eq!(*victim, Some(target));
        }
    }
}

#[test]
fn authored_explicit_empty_dragon_stream_never_uses_substring_seed() {
    assert_stream_case([EMPTY_DRAGON, SECONDARY, TERTIARY], 0, None);
}

#[test]
fn authored_default_empty_dragon_stream_never_uses_substring_seed() {
    assert_stream_case([OMITTED_DRAGON, SECONDARY, TERTIARY], 0, None);
}

#[test]
fn authored_empty_secondary_stream_never_inherits_primary_or_tertiary() {
    assert_stream_case([PRIMARY, EMPTY_SECONDARY, TERTIARY], 1, None);
}

#[test]
fn authored_empty_tertiary_stream_never_inherits_primary_or_secondary() {
    assert_stream_case([PRIMARY, SECONDARY, EMPTY_TERTIARY], 2, None);
}

#[test]
fn authored_secondary_stream_uses_exact_accepted_weapon_template() {
    assert_stream_case([PRIMARY, SECONDARY, TERTIARY], 1, Some(SECONDARY_STREAM));
}

#[test]
fn authored_tertiary_stream_uses_exact_accepted_weapon_template() {
    assert_stream_case([PRIMARY, SECONDARY, TERTIARY], 2, Some(TERTIARY_STREAM));
}

#[test]
fn missing_receiving_projectile_rejects_before_ammo_effects_or_stream() {
    let (mut world, source, target) = prepare_world([PRIMARY, SECONDARY, TERTIARY], 0, true);
    let marker = world.objects[&source].weapon_discharge_marker().sequence;
    assert_ne!(
        world.attack_fire_weapon_update(source, target, 10.0),
        AttackFireResult::Success
    );
    for slot in 0..3u8 {
        assert_eq!(
            world.objects[&source].weapon_slot(slot).unwrap().ammo,
            Some(8)
        );
    }
    assert_eq!(
        world.objects[&source].weapon_discharge_marker().sequence,
        marker
    );
    assert!(world.take_weapon_discharges_for_presentation().is_empty());
    world.drain_pending_projectiles_into_combat();
    assert_eq!(world.combat_system.projectile_count(), 0);
    let _ = world.update_with_dt(1.0 / 30.0);
    assert!(world.projectile_stream_snapshot().is_empty());
    assert_eq!(world.objects[&target].health.current, 100.0);
}

#[test]
fn receiving_stream_launch_binding_survives_override_and_encoded_save_load() {
    // Launch-time stream binding must outlive later Weapon.ini overrides. Full
    // WorldSnapshot capture now requires admitted ReceivingBaseAI; PJST + live
    // combat projectiles prove the same residual without that gate.
    let (mut world, source, target) = fire_and_tick([PRIMARY, SECONDARY, TERTIARY], 1);
    let binding = world.combat_system.projectiles_snapshot()[0]
        .stream_binding
        .clone()
        .unwrap();
    assert_eq!(binding.slot, 1);
    assert_eq!(binding.name, SECONDARY_STREAM);
    world
        .apply_receiving_weapon_overrides(&[format!(
            "Weapon {SECONDARY}\n ProjectileStreamName = ChangedAfterLaunch\nEnd\n"
        )])
        .unwrap();
    assert_eq!(
        world.combat_system.projectiles_snapshot()[0]
            .stream_binding
            .as_ref(),
        Some(&binding),
        "override must not rewrite the accepted shot binding"
    );
    let mut tail = Vec::new();
    crate::save_load::snapshot::projectile_stream_persist::append_to_lifecycle_tail(
        &mut tail, &world,
    );
    // Clear and restore streams from PJST while the in-flight projectile keeps
    // its launch binding (C++ Weapon::m_projectileStreamID / stream name).
    world.projectile_streams.clear();
    assert!(world.projectile_stream_snapshot().is_empty());
    crate::save_load::snapshot::projectile_stream_persist::apply_from_lifecycle_tail(
        &tail, &mut world,
    )
    .unwrap();
    // Re-seed from live projectiles through the ordinary tick residual.
    let _ = world.update_with_dt(1.0 / 30.0);
    let streams = world.projectile_stream_snapshot();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].0, source);
    assert_eq!(streams[0].1, SECONDARY_STREAM);
    assert_eq!(streams[0].3, Some(target));
    assert_eq!(
        world.combat_system.projectiles_snapshot()[0]
            .stream_binding
            .as_ref(),
        Some(&binding)
    );
    assert_eq!(world.objects[&target].health.current, 100.0);
    assert_eq!(
        world.objects[&source].weapon_slot(1).unwrap().ammo,
        Some(7)
    );
}

#[test]
fn concurrent_primary_and_secondary_streams_stay_distinct() {
    // C++ Weapon.cpp:2388 — each Weapon owns m_projectileStreamID. A dual-slot
    // firer must keep two ribbons instead of collapsing onto the shooter.
    let (mut world, source, target) = prepare_world([PRIMARY, SECONDARY, TERTIARY], 0, false);
    for slot in [0u8, 1u8] {
        {
            let shooter = world.objects.get_mut(&source).unwrap();
            assert!(shooter.set_weapon_lock(slot, WeaponLockType::LockedPermanently));
            shooter.set_active_weapon_slot(slot);
            assert_eq!(shooter.selected_weapon_slot(), Some(slot));
        }
        assert_eq!(
            world.attack_fire_weapon_update(source, target, 10.0),
            AttackFireResult::Success,
            "slot {slot} must fire"
        );
    }
    world.drain_pending_projectiles_into_combat();
    assert_eq!(world.combat_system.projectile_count(), 2);
    let bindings: Vec<_> = world
        .combat_system
        .projectiles_snapshot()
        .into_iter()
        .map(|p| {
            p.stream_binding
                .clone()
                .expect("accepted shot keeps stream binding")
        })
        .collect();
    assert!(
        bindings.iter().any(|b| b.slot == 0 && b.name == PRIMARY_STREAM),
        "missing primary binding: {bindings:?}"
    );
    assert!(
        bindings.iter().any(|b| b.slot == 1 && b.name == SECONDARY_STREAM),
        "missing secondary binding: {bindings:?}"
    );
    let _ = world.update_with_dt(1.0 / 30.0);
    let mut streams = world.projectile_stream_snapshot();
    streams.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(
        streams.len(),
        2,
        "per-Weapon streams must not collapse: {streams:?}"
    );
    let names: Vec<&str> = streams.iter().map(|s| s.1.as_str()).collect();
    assert!(names.contains(&PRIMARY_STREAM), "{names:?}");
    assert!(names.contains(&SECONDARY_STREAM), "{names:?}");
    assert!(streams.iter().all(|s| s.0 == source));
    assert!(streams.iter().all(|s| s.3 == Some(target)));
}

// Retail WeaponObjects.ini puts the uranium WeaponBonusUpgrade on the shell,
// not on the Overlord. DumbProjectileBehavior.cpp:318-321 retains launcher
// flags; :517 detonates with those flags plus the projectile's current flags.
fn uranium_projectile_world() -> (GameLogic, ObjectId, ObjectId) {
    const SHELL: &str = "Object OverlordTankShell\nKindOf = PROJECTILE\n\
        Body = ActiveBody ShellBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
        Behavior = DumbProjectileBehavior Flight\nFirstHeight = 10\nSecondHeight = 10\n\
        FirstPercentIndent = 50%\nSecondPercentIndent = 90%\nEnd\n\
        Behavior = PhysicsBehavior Physics\nMass = 0.01\nEnd\n\
        Behavior = WeaponBonusUpgrade Uranium\nTriggeredBy = Upgrade_ChinaUraniumShells\nEnd\nEnd\n";
    const CARRIERS: &str = "\nObject CustomOverlordGunCarrier\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\n\
        Body = ActiveBody BodyTag\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
        WeaponSet\nConditions = None\nWeapon = PRIMARY OverlordTankGun\nEnd\n\
        Behavior = AIUpdateInterface CarrierAI\nEnd\nEnd\n\
        Object ChinaTankOverlord\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\n\
        Body = ActiveBody BodyTag\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
        WeaponSet\nConditions = None\nWeapon = PRIMARY AlternateUraniumGun\nEnd\n\
        Behavior = AIUpdateInterface CarrierAI\nEnd\nEnd\n\
        Object UraniumProjectileTarget\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\n\
        Body = ActiveBody BodyTag\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
        Behavior = AIUpdateInterface TargetAI\nEnd\n\
        Geometry = Sphere\nGeometryMajorRadius = 0\nEnd\n";
    let object_content = format!("{SHELL}{CARRIERS}");
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(&object_content, "uranium-projectile.ini").unwrap();
    let objects: Vec<_> = parser.get_all_definitions().iter()
        .map(|(name, row)| (name.clone(), row.clone())).collect();
    let mut weapons = HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut weapons,
        "Weapon OverlordTankGun\nPrimaryDamage = 80\nPrimaryDamageRadius = 5\n\
         SecondaryDamage = 20\nSecondaryDamageRadius = 10\nAttackRange = 175\n\
         DamageType = ARMOR_PIERCING\nProjectileObject = OverlordTankShell\n\
         WeaponSpeed = 30\nRadiusDamageAffects = ALLIES ENEMIES NEUTRALS\nDelayBetweenShots = 300\nClipSize = 2\n\
         ClipReloadTime = 2000\nAntiGround = Yes\nWeaponBonus = PLAYER_UPGRADE DAMAGE 125%\nEnd\n\
         Weapon AlternateUraniumGun\nPrimaryDamage = 40\nPrimaryDamageRadius = 5\n\
         AttackRange = 175\nDamageType = ARMOR_PIERCING\nProjectileObject = OverlordTankShell\n\
         WeaponSpeed = 30\nClipSize = 2\nAntiGround = Yes\nWeaponBonus = PLAYER_UPGRADE DAMAGE 150%\nEnd\n",
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    let mut world = GameLogic::new();
    for (id, team) in [(0, Team::China), (1, Team::China), (2, Team::GLA)] {
        let mut player = Player::new(id, team, "uranium test", id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, "Upgrade_ChinaUraniumShells", "PLAYER", 0, 1,
    );
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([
            ("uranium-projectile.ini".into(), object_content),
        ]).unwrap(),
    ).unwrap();
    world.admit_weapon_content_snapshot(
        weapons.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
        &objects,
    ).unwrap();
    let source = world.create_object_for_player("CustomOverlordGunCarrier", 0, Vec3::ZERO).unwrap();
    let target = world.create_object_for_player("UraniumProjectileTarget", 2, Vec3::new(30.0, 0.0, 0.0)).unwrap();
    world.frame = 100;
    let target_pos = world.objects[&target].get_position();
    world.objects.get_mut(&source).unwrap().set_prev_victim_pos(Some(target_pos));
    (world, source, target)
}

fn uranium_launch(world: &mut GameLogic, source: ObjectId, target: ObjectId) {
    assert_eq!(world.attack_fire_weapon_update(source, target, 10.0), AttackFireResult::Success);
    world.drain_pending_projectiles_into_combat();
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert_eq!(world.objects[&target].health.current, 1000.0);
}

fn uranium_impact(world: &mut GameLogic, target: ObjectId) -> f32 {
    for _ in 0..300 {
        if world.combat_system.projectile_count() == 0 { break; }
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(world.combat_system.projectile_count(), 0, "shell must actually arrive");
    1000.0 - world.objects[&target].health.current
}

#[test]
fn uranium_shell_upgrade_is_owner_scoped_at_launch_in_flight_and_after_load() {
    const UPGRADE: &str = "Upgrade_ChinaUraniumShells";
    let (mut unupgraded, source, target) = uranium_projectile_world();
    // Collision detonates at the shell position, not the target center
    // (DumbProjectileBehavior.cpp:500-517). A perpendicular offset stays
    // between the 5 and 10 radii even when contact occurs before the center.
    let unupgraded_splash = unupgraded.create_object_for_player(
        "UraniumProjectileTarget", 2, Vec3::new(30.0, 0.0, 7.0),
    ).unwrap();
    unupgraded.add_completed_player_upgrade(1, UPGRADE);
    uranium_launch(&mut unupgraded, source, target);
    assert_eq!(uranium_impact(&mut unupgraded, target), 80.0);
    assert_eq!(unupgraded.objects[&unupgraded_splash].health.current, 980.0);

    let (mut before, source, target) = uranium_projectile_world();
    let upgraded_splash = before.create_object_for_player(
        "UraniumProjectileTarget", 2, Vec3::new(30.0, 0.0, 7.0),
    ).unwrap();
    before.add_completed_player_upgrade(0, UPGRADE);
    assert!(!before.objects[&source].weapon_bonus_player_upgrade, "retail carrier has no upgrade module");
    uranium_launch(&mut before, source, target);
    let (mut independent, independent_source, independent_target) = uranium_projectile_world();
    uranium_launch(&mut independent, independent_source, independent_target);
    assert_eq!(uranium_impact(&mut independent, independent_target), 80.0);
    assert_eq!(uranium_impact(&mut before, target), 100.0);
    assert_eq!(before.objects[&upgraded_splash].health.current, 975.0);

    // The retail chassis name must not select the retail gun's 125% rule.
    // This carrier is born after research and authors a different 150% gun.
    let (mut mismatch, _, target) = uranium_projectile_world();
    mismatch.add_completed_player_upgrade(0, UPGRADE);
    let named = mismatch.create_object_for_player("ChinaTankOverlord", 0, Vec3::ZERO).unwrap();
    let target_pos = mismatch.objects[&target].get_position();
    mismatch.objects.get_mut(&named).unwrap().set_prev_victim_pos(Some(target_pos));
    uranium_launch(&mut mismatch, named, target);
    assert_eq!(uranium_impact(&mut mismatch, target), 60.0);

    // The flight module retains launcher conditions even if the launcher
    // loses them before the shell hits (DumbProjectileBehavior.cpp:319).
    let (mut captured, source, target) = uranium_projectile_world();
    captured.objects.get_mut(&source).unwrap().set_weapon_bonus_player_upgrade(true);
    uranium_launch(&mut captured, source, target);
    captured.objects.get_mut(&source).unwrap().set_weapon_bonus_player_upgrade(false);
    assert_eq!(uranium_impact(&mut captured, target), 100.0);

    let (mut flying, source, target) = uranium_projectile_world();
    uranium_launch(&mut flying, source, target);
    flying.add_completed_player_upgrade(0, UPGRADE);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&flying).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut loaded, _, _) = uranium_projectile_world();
    builder.restore_from_snapshot(&saved, &mut loaded).unwrap();
    assert_eq!(uranium_impact(&mut flying, target), 100.0);
    assert_eq!(uranium_impact(&mut loaded, target), 100.0);

    let (mut before_load, source, target) = uranium_projectile_world();
    uranium_launch(&mut before_load, source, target);
    let saved = builder.create_world_snapshot(&before_load).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut loaded_before_upgrade, _, _) = uranium_projectile_world();
    builder.restore_from_snapshot(&saved, &mut loaded_before_upgrade).unwrap();
    loaded_before_upgrade.add_completed_player_upgrade(0, UPGRADE);
    assert_eq!(uranium_impact(&mut loaded_before_upgrade, target), 100.0);
}
