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

fn register_stream_rules() {
    // A genuine partial Weapon.ini override clears a previous stream, rather
    // than an empty host field being mistaken for missing authored rules.
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            "Weapon AuthoredDragonFlameExplicitEmptyStream\n  ProjectileStreamName = PreviouslyAuthoredProjectileStream\nEnd\n"
        ),
        1
    );
    gamelogic::weapon::with_weapon_store(|store| {
        assert_eq!(
            store
                .find_weapon_template(EMPTY_DRAGON)
                .unwrap()
                .projectile_stream_name,
            "PreviouslyAuthoredProjectileStream"
        );
    })
    .unwrap();
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
        crate::assets::ini_template_loader::register_weapons_from_ini_text(&ini),
        rules.len()
    );
    gamelogic::weapon::with_weapon_store(|store| {
        for (name, stream) in rules {
            let definition = store.find_weapon_template(name).unwrap();
            assert_eq!(definition.projectile_stream_name, stream.unwrap_or(""));
            assert_eq!(definition.projectile_name, "AuthoredStreamLiveProjectile");
            assert_eq!(definition.weapon_speed, 1.0); // CPP distance/logic frame
            assert_eq!(definition.clip_size, 8);
            assert_eq!(definition.attack_range, 200.0);
        }
    })
    .unwrap();
}

fn fire_and_tick(slots: [&str; 3], selected: u8) -> (GameLogic, ObjectId, ObjectId) {
    register_stream_rules();
    let mut world = GameLogic::new();
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
        shooter.prev_victim_pos = Some(target_position);
        assert_eq!(shooter.selected_weapon_slot(), Some(selected));
    }
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
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
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
fn missing_host_template_stream_fallback_is_distinct_from_authored_empty() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    const MISSING: &str = "UnresolvedHostDragonFlameStreamContract";
    gamelogic::weapon::with_weapon_store(|store| {
        assert!(store.find_weapon_template(MISSING).is_none())
    })
    .unwrap();
    assert_eq!(
        crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name(MISSING),
        "DragonTankFlameStream"
    );
}
