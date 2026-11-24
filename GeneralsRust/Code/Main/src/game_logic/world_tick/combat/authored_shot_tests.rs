//! Direct combat fires the selected WeaponTemplate through the shared shot
//! operation (C++ Weapon::fireWeaponTemplate), whatever the firer is named.
use crate::game_logic::{GameLogic, ObjectId, Team};
const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / 30.0;
use glam::Vec3;

const WEAPONS: &str = "Weapon AuthoredInstantGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
End\n\
Weapon AuthoredSlowGun\n\
  PrimaryDamage = 40\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 150\n\
  DelayBetweenShots = 2000\n\
End\n\
Weapon ScatterInstantGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
  ScatterRadiusVsInfantry = 120\n\
End\n\
Weapon ScatterLaserGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
  ScatterRadiusVsInfantry = 120\n\
  LaserName = ScatterCheckBeam\n\
End\n\
Weapon ScatterDelayGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 9\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 150\n\
  DelayBetweenShots = 2000\n\
  ScatterRadiusVsInfantry = 120\n\
End\n\
Weapon SelfPositionGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 9\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 150\n\
  DelayBetweenShots = 2000\n\
  DamageDealtAtSelfPosition = Yes\n\
End\n\
Weapon ConeDelayGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 150\n\
  DelayBetweenShots = 2000\n\
  RadiusDamageAngle = 45\n\
End\n\
Weapon ScatterTableGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
  ScatterTarget = X:0 Y:50\n\
  ScatterTargetScalar = 1\n\
End\n\
Weapon LaserCoordGun\n\
  PrimaryDamage = 25\n\
  PrimaryDamageRadius = 120\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
  ScatterRadius = 120\n\
  LaserName = ScatterCoordBeam\n\
End\n\
Weapon OrderBeamGun\n\
  PrimaryDamage = 50\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 200\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
  LaserName = OrderCheckBeam\n\
End\n";

fn object(name: &str, weapon: Option<&str>) -> String {
    let weapon_set = weapon
        .map(|weapon| format!("WeaponSet\nConditions = None\nWeapon = PRIMARY {weapon}\nEnd\n"))
        .unwrap_or_default();
    format!(
        "Object {name}\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\n{weapon_set}\
         Body = ActiveBody BodyTag\nMaxHealth = 500\nInitialHealth = 500\nEnd\nEnd\n"
    )
}

/// A world whose shooter template is named like an old per-unit residual but
/// authors only `weapon`; the victim authors nothing special.
fn world(shooter: &str, weapon: &str) -> GameLogic {
    world_with(shooter, weapon, "")
}

fn world_with(shooter: &str, weapon: &str, extra_objects: &str) -> GameLogic {
    world_from(&object(shooter, Some(weapon)), extra_objects)
}

fn world_from(shooter_template: &str, extra_objects: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-authored-shot.ini".into(),
                format!(
                    "{}{}{}",
                    shooter_template,
                    object("PlainVictim", None),
                    extra_objects,
                ),
            )])
            .unwrap(),
        )
        .unwrap();
    let mut weapons = std::collections::HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut weapons,
        WEAPONS,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    world
        .admit_receiving_weapon_snapshot(weapons.into_iter().collect())
        .unwrap();
    world
}

/// Small-sphere infantry so ScatterRadiusVsInfantry engages and the
/// bounding-sphere splash distance stays deterministic.
fn infantry_object(name: &str) -> String {
    object(name, None)
        .replace("VEHICLE SELECTABLE CAN_ATTACK", "INFANTRY SELECTABLE CAN_ATTACK")
        .replace(
            "Body = ActiveBody BodyTag",
            "Geometry = SPHERE\nGeometryMajorRadius = 1\nGeometryMinorRadius = 1\nGeometryHeight = 2\n\
             Body = ActiveBody BodyTag",
        )
}

fn engage(world: &mut GameLogic, shooter: ObjectId, victim: ObjectId) {
    let frame = world.frame;
    let (attacker, events) = world.host_object_and_attack_events_mut(shooter).unwrap();
    attacker.attack_target(
        victim,
        frame,
        events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
}

fn step(world: &mut GameLogic, ids: &[ObjectId]) {
    world.update_combat(ids, LOGIC_FRAME_TIMESTEP).unwrap();
    world.apply_owned_projectileless_delayed_damage().unwrap();
    let _ = world.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP).unwrap();
}

fn hp(world: &GameLogic, id: ObjectId) -> f32 {
    world.host_object(id).unwrap().health.current
}

#[test]
fn residual_named_shooter_deals_exactly_its_authored_damage() {
    let mut world = world("ChinaInfantryRedguard", "AuthoredInstantGun");
    let shooter = world
        .create_object("ChinaInfantryRedguard", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("PlainVictim", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    engage(&mut world, shooter, victim);
    for frame in 100..110 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim]);
    }
    // One shot inside the 2000 ms reload: exactly the authored 25 damage,
    // no residual bayonet/horde/splash invented from the template name.
    assert_eq!(hp(&world, victim), 475.0);
}

/// A live launcher shot leaves only its authored ProjectileDetonationOCL
/// (Weapon.cpp ProjectileDetonationOCL at the missile's impact): any shooter
/// name fires it when authored, and the retail SCUD name alone births no
/// field and no synthetic poison zone when it is not.
#[test]
fn authored_scud_impact_uses_only_projectile_detonation_ocl() {
    const FIELD: &str = "FixtureGammaField";
    // MissileAIUpdate owns the flight; PhysicsBehavior + a THRUST locomotor
    // are what MissileMotion::launch binds (same shape as Scr2ScudMissile).
    let extras = concat!(
        "Object AuthoredTestMissile\nKindOf = PROJECTILE SMALL_MISSILE\n\
         Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
         Locomotor = SET_NORMAL AuthoredMissileFlight\n\
         Behavior = PhysicsBehavior PhysicsTag\nMass = 1\nEnd\n\
         Behavior = MissileAIUpdate FlightTag\nTryToFollowTarget = Yes\nFuelLifetime = 3000\nIgnitionDelay = 0\nEnd\nEnd\n",
        "Object FixtureGammaField\nKindOf = IMMOBILE\n\
         Body = ActiveBody BodyTag\nMaxHealth = 10\nInitialHealth = 10\nEnd\n\
         Behavior = LifetimeUpdate FieldLife\nMinLifetime = 6000\nMaxLifetime = 6000\nEnd\nEnd\n",
    );
    for (shooter_name, detonation_ocl, expected_fields) in [
        ("SampleLauncher", "OCL_PoisonFieldGammaMedium", 1usize),
        ("Chem_GLAVehicleScudLauncher", "None", 0usize),
    ] {
        // `world_with` order, plus the missile's locomotor first:
        // locomotors must be admitted before Object content.
        let mut world = GameLogic::new();
        let mut locomotors =
            game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
        locomotors
            .apply_source(
                "Locomotor AuthoredMissileFlight\n Surfaces = AIR\n Speed = 300\n\
                 MinSpeed = 30\n Acceleration = 600\n TurnRate = 3600\n\
                 Appearance = THRUST\nEnd\n",
            )
            .unwrap();
        world.admit_locomotor_definitions(locomotors).unwrap();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-scud-shot.ini".into(),
                    format!(
                        "{}{}{}",
                        object(shooter_name, Some("Chem_SCUDLauncherGunAnthraxGamma")),
                        object("PlainVictim", None),
                        extras,
                    ),
                )])
                .unwrap(),
            )
            .unwrap();
        world
            .admit_object_creation_list_sources([
                "ObjectCreationList OCL_PoisonFieldGammaMedium\nCreateObject\n\
                 ObjectNames = FixtureGammaField\nCount = 1\nEnd\nEnd\n"
                    .to_string(),
            ])
            .unwrap();
        let mut weapons = std::collections::HashMap::new();
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            &format!(
                "Weapon Chem_SCUDLauncherGunAnthraxGamma\nPrimaryDamage = 25\n\
                 PrimaryDamageRadius = 10\nAttackRange = 200\nDamageType = EXPLOSION\n\
                 WeaponSpeed = 600\nDelayBetweenShots = 4000\n\
                 ProjectileObject = AuthoredTestMissile\n\
                 ProjectileDetonationOCL = {detonation_ocl}\nEnd\n"
            ),
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        );
        world
            .admit_receiving_weapon_snapshot(weapons.into_iter().collect())
            .unwrap();

        let shooter = world
            .create_object(shooter_name, Team::GLA, Vec3::ZERO)
            .unwrap();
        let victim = world
            .create_object("PlainVictim", Team::USA, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        let field_count = |world: &GameLogic| {
            world
                .host_objects()
                .registered()
                .filter(|(_, object)| object.template_name == FIELD)
                .count()
        };

        world.set_current_frame(100);
        engage(&mut world, shooter, victim);
        step(&mut world, &[shooter, victim]);
        assert!(
            !world.combat_system.get_projectiles().is_empty(),
            "{shooter_name}: the authored missile must be in flight after the shot"
        );
        assert_eq!(
            field_count(&world),
            0,
            "{shooter_name}: no field exists before impact"
        );

        // DelayBetweenShots 4000 ms = 120 frames: the missile must land
        // before the next shot so the victim takes exactly one 25 hit.
        let mut impacted = false;
        for frame in 101..200 {
            world.set_current_frame(frame);
            step(&mut world, &[shooter, victim]);
            if hp(&world, victim) < 500.0 || field_count(&world) > 0 {
                impacted = true;
                break;
            }
        }
        assert!(
            impacted,
            "{shooter_name}: the authored missile must impact before the next shot"
        );
        assert_eq!(
            hp(&world, victim),
            475.0,
            "{shooter_name}: only the authored 25 damage lands"
        );
        assert_eq!(
            field_count(&world),
            expected_fields,
            "{shooter_name}: only the authored detonation OCL births a field"
        );
        let deadline = world
            .host_objects()
            .registered()
            .find_map(|(_, object)| {
                (object.template_name == FIELD).then(|| {
                    object
                        .native_lifecycle
                        .as_ref()
                        .and_then(|lifecycle| lifecycle.first_lifetime_deadline())
                })
            })
            .flatten();
        assert_eq!(
            deadline.is_some(),
            expected_fields == 1,
            "{shooter_name}: the field object carries its authored LifetimeUpdate deadline"
        );
        assert_eq!(
            world.scud_poison_zones().active_count(),
            0,
            "{shooter_name}: no synthetic SCUD poison zone"
        );
    }
}

#[test]
fn direct_shot_admission_uses_the_horde_callback_integer_deadline() {
    let mut world = world("DeadlineProbe", "AuthoredInstantGun");
    world.admit_combat_bonus_ini_sources([
        "GameData\nWeaponBonus = HORDE RATE_OF_FIRE 300%\nEnd\n",
    ]).unwrap();
    let shooter = world
        .create_object("DeadlineProbe", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("PlainVictim", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    world.set_current_frame(15);
    engage(&mut world, shooter, victim);
    step(&mut world, &[shooter, victim]);
    assert_eq!(hp(&world, victim), 475.0);
    let weapon = world.host_object(shooter).unwrap().weapon.as_ref().unwrap();
    assert!(weapon.frame_clock_active);
    assert_eq!(weapon.last_fire_frame, 15);
    assert_eq!(weapon.next_fire_frame, 75);

    world.set_current_frame(35);
    world.apply_owned_horde_morale(shooter, true, Default::default());
    assert_eq!(
        world.host_object(shooter).unwrap().weapon.as_ref().unwrap().next_fire_frame,
        55,
    );
    world.set_current_frame(54);
    assert!(world.prepare_combat_attack(shooter).is_none());
    step(&mut world, &[shooter, victim]);
    assert_eq!(hp(&world, victim), 475.0);

    world.set_current_frame(55);
    assert!(world.prepare_combat_attack(shooter).is_some());
    step(&mut world, &[shooter, victim]);
    assert_eq!(hp(&world, victim), 450.0);
    let weapon = world.host_object(shooter).unwrap().weapon.as_ref().unwrap();
    assert_eq!(weapon.last_fire_frame, 55);
    assert_eq!(weapon.next_fire_frame, 75);
}

#[test]
fn authored_base_defense_auto_fire_commits_the_same_integer_clock() {
    let mut game = GameLogic::new();
    let source = object("DeadlineDefense", Some("AuthoredInstantGun"))
        .replace("VEHICLE SELECTABLE CAN_ATTACK", "STRUCTURE FS_BASE_DEFENSE SELECTABLE CAN_ATTACK");
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "deadline-defense.ini".into(), format!("{source}{}", object("PlainVictim", None)),
        )]).unwrap(),
    ).unwrap();
    let mut weapons = std::collections::HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut weapons, WEAPONS, crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    game.admit_receiving_weapon_snapshot(weapons.into_iter().collect()).unwrap();
    let shooter = game.create_object("DeadlineDefense", Team::China, Vec3::ZERO).unwrap();
    let victim = game.create_object("PlainVictim", Team::USA, Vec3::new(50.0, 0.0, 0.0)).unwrap();
    for (frame, health) in [(15, 475.0), (74, 475.0), (75, 450.0)] {
        game.set_current_frame(frame);
        step(&mut game, &[shooter, victim]);
        assert_eq!(hp(&game, victim), health);
    }
    assert_eq!(game.host_object(shooter).unwrap().weapon.as_ref().unwrap().next_fire_frame, 135);
}

#[test]
fn authored_weapon_speed_delays_damage_by_travel_time() {
    let mut world = world("PlainArtillery", "AuthoredSlowGun");
    let shooter = world
        .create_object("PlainArtillery", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("PlainVictim", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);
    // WeaponSpeed 150/s = 5 per frame: 100 units is 20 frames of travel
    // (Weapon.cpp delayInFrames); the victim is untouched on the shot frame.
    assert_eq!(hp(&world, victim), 500.0);
    for frame in 101..130 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim]);
    }
    assert_eq!(hp(&world, victim), 460.0);
}

fn ecm_world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let objects = r#"
Object ChinaTankECM
  KindOf = VEHICLE SELECTABLE CAN_ATTACK
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ECMTankVehicleDisabler
  End
  Body = ActiveBody BodyTag
    MaxHealth = 300
    InitialHealth = 300
    SubdualDamageCap = 600
  End
  Behavior = FireWeaponUpdate Jammer
    Weapon = ECMTankMissileJammer
    ExclusiveWeaponDelay = 1000
  End
End
Object EcmTestVehicle
  KindOf = VEHICLE SELECTABLE
  Body = ActiveBody BodyTag
    MaxHealth = 400
    InitialHealth = 400
    SubdualDamageCap = 600
  End
End
Object TestGuidedMissile
  KindOf = PROJECTILE SMALL_MISSILE
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
    SubdualDamageCap = 200
  End
  Behavior = MissileAIUpdate FlightTag
    TryToFollowTarget = Yes
    FuelLifetime = 3000
  End
End
"#;
    let weapons = r#"
Weapon ECMTankVehicleDisabler
  PrimaryDamage = 24
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SUBDUAL_VEHICLE
  DeathType = NORMAL
  WeaponSpeed = 999999
  DelayBetweenShots = 100
  LaserName = ECMDisableStream
  LaserBoneName = WEAPONA01
  RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
  ClipSize = 0
  ClipReloadTime = 0
  AntiAirborneVehicle = No
  AntiAirborneInfantry = No
  FireSound = FrequencyJammerWeaponLoop
  FireSoundLoopTime = 120
End
Weapon ECMTankMissileJammer
  PrimaryDamage = 100
  PrimaryDamageRadius = 150
  AttackRange = 15
  MinimumAttackRange = 10
  DamageType = SUBDUAL_MISSILE
  DeathType = NORMAL
  WeaponSpeed = 600
  DelayBetweenShots = 650
  RadiusDamageAffects = ENEMIES NEUTRALS
  FireFX = FX_ECMTankMissileJammerPulse
End
"#;
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([
            ("ecm-objects.ini".into(), objects.to_owned()),
        ]).unwrap(),
    ).unwrap();
    let mut definitions = std::collections::HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut definitions, weapons,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    world.admit_receiving_weapon_snapshot(definitions.into_iter().collect()).unwrap();
    let source = world.create_object("ChinaTankECM", Team::China, Vec3::ZERO).unwrap();
    let first = world.create_object("EcmTestVehicle", Team::USA, Vec3::new(50.0, 0.0, 0.0)).unwrap();
    let second = world.create_object("EcmTestVehicle", Team::USA, Vec3::new(60.0, 0.0, 0.0)).unwrap();
    (world, source, first, second)
}

#[test]
fn authored_ecm_primary_only_pulses_its_target_and_stops_with_fire() {
    let (mut world, source, first, second) = ecm_world();
    for frame in 100..104 {
        world.set_current_frame(frame);
        step(&mut world, &[source, first, second]);
        world.update_ecm_missile_jam();
    }
    assert_eq!(world.objects[&first].subdual_damage, 0.0);
    assert_eq!(world.objects[&second].subdual_damage, 0.0);
    assert_eq!(world.objects[&source].weapon.as_ref().unwrap().last_fire_frame, 0);
    assert!(world.weapon_lasers.is_empty());

    engage(&mut world, source, first);
    for frame in 104..114 {
        world.set_current_frame(frame);
        step(&mut world, &[source, first, second]);
    }
    let shooter = &world.objects[&source];
    let shots = shooter.fire_intent_count;
    assert!(shots >= 2 && shots <= 4, "100ms cadence: {shots} shots in ten frames");
    assert_eq!(world.objects[&first].subdual_damage, shots as f32 * 24.0);
    assert_eq!(world.objects[&first].health.current, 400.0);
    assert_eq!(world.objects[&second].subdual_damage, 0.0);
    assert!(world.weapon_lasers.iter().any(|beam|
        beam.laser_name == "ECMDisableStream" && beam.to_id == Some(first)));
    assert!(!world.weapon_lasers.iter().any(|beam| beam.to_id == Some(second)));
    // Weapon.cpp:2418-2449 raises grounded attached targets 10 units.
    let first_pos_y = world.objects[&first].get_position().y;
    assert!(world
        .weapon_lasers
        .iter()
        .filter(|beam| beam.laser_name == "ECMDisableStream" && beam.to_id == Some(first))
        .all(|beam| (beam.to_pos().1 - (first_pos_y + 10.0)).abs() < 0.001));

    let source_obj = world.objects.get_mut(&source).unwrap();
    source_obj.target = None;
    source_obj.set_status_attacking(false);
    for frame in 114..124 {
        world.set_current_frame(frame);
        step(&mut world, &[source, first, second]);
    }
    assert_eq!(world.objects[&source].fire_intent_count, shots);
    assert_eq!(world.objects[&first].subdual_damage, shots as f32 * 24.0);
}

#[test]
fn authored_ecm_missile_jammer_uses_slot_shot_and_world_owned_delay() {
    let (mut first, source, target, other) = ecm_world();
    let (mut second, other_source, _, _) = ecm_world();
    engage(&mut first, source, target);
    for frame in 100..105 {
        first.set_current_frame(frame);
        step(&mut first, &[source, target, other]);
    }
    let last = first.objects[&source].weapon.as_ref().unwrap().last_fire_frame;
    assert!(last >= 100);

    for world in [&mut first, &mut second] {
        let id = world.create_object("TestGuidedMissile", Team::USA, Vec3::new(10.0, 0.0, 0.0)).unwrap();
        let object = world.objects.get_mut(&id).unwrap();
        object.tomahawk_missile_projectile = true;
        object.tomahawk_missile_aim = Some([200.0, 0.0, 0.0]);
    }
    let missile = first.host_objects().registered().find_map(|(id, object)|
        (object.template_name == "TestGuidedMissile").then_some(*id)).unwrap();
    let foreign_missile = second.host_objects().registered().find_map(|(id, object)|
        (object.template_name == "TestGuidedMissile").then_some(*id)).unwrap();
    second.set_current_frame(u64::from(last) + 30);
    second.tick_native_lifecycle(other_source).unwrap();
    second.apply_owned_projectileless_delayed_damage().unwrap();
    assert!(second.objects[&foreign_missile].ecm_missile_jammed);
    first.set_current_frame(u64::from(last) + 29);
    first.tick_native_lifecycle(source).unwrap();
    first.update_ecm_missile_jam();
    assert!(!first.objects[&missile].ecm_missile_jammed);
    first.set_current_frame(u64::from(last) + 30);
    first.tick_native_lifecycle(source).unwrap();
    first.apply_owned_projectileless_delayed_damage().unwrap();
    assert!(first.objects[&missile].ecm_missile_jammed);
    assert_eq!(first.objects[&missile].health.current, 100.0);
}

/// Stock projectileless scatter: `ScatterRadiusVsInfantry` only re-aims the
/// instant weapon; the engaged infantry victim keeps its identity and takes
/// the primary hit (Weapon.cpp fireWeaponTemplate radius-scatter branch).
#[test]
fn instant_scatter_vs_infantry_still_damages_the_engaged_victim() {
    let mut world = world_with(
        "AuthoredScatterShooter",
        "ScatterInstantGun",
        &infantry_object("ScatterInfantry"),
    );
    let shooter = world.create_object("AuthoredScatterShooter", Team::China, Vec3::ZERO).unwrap();
    let victim = world.create_object("ScatterInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();

    engage(&mut world, shooter, victim);
    let probe = crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(
        120.0,
        &game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot()),
    );
    assert!(probe.0 > 0.0, "deterministic fixture must roll a miss radius");

    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);

    assert_eq!(hp(&world, victim), 500.0 - 25.0);
    assert_eq!(
        world.objects.registered().count(),
        2,
        "projectileless shot spawns no missile object"
    );
}

/// Stock projectileless laser: when the rolled scatter miss exceeds both
/// authored radii, damage lands at the scattered aim point instead of the
/// victim center, so the primary target takes nothing.
#[test]
fn instant_laser_scatter_beyond_both_radii_misses_the_victim() {
    let mut world = world_with(
        "AuthoredLaserShooter",
        "ScatterLaserGun",
        &infantry_object("ScatterInfantry"),
    );
    let shooter = world.create_object("AuthoredLaserShooter", Team::China, Vec3::ZERO).unwrap();
    let victim = world.create_object("ScatterInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();

    engage(&mut world, shooter, victim);
    let probe = crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(
        120.0,
        &game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot()),
    );
    assert!(probe.0 > 0.0, "deterministic fixture must roll a miss radius");

    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);

    assert_eq!(hp(&world, victim), 500.0);
    let beams: Vec<_> = world
        .weapon_lasers
        .iter()
        .filter(|beam| beam.laser_name == "ScatterCheckBeam")
        .collect();
    assert_eq!(beams.len(), 1, "exactly one authored beam");
    let beam = beams[0];
    assert_eq!(beam.to_id, None, "miss past both radii keeps no target");
    assert!(
        Vec3::from(beam.to_pos()).distance(Vec3::new(40.0, 0.0, 0.0) + probe.1) < 0.001,
        "beam aims at the scattered aim point"
    );
}

/// Delayed projectileless scatter: the delayed impact recenters on the
/// victim's live position instead of the stale fire-time aim, and applies
/// exactly once when the travel delay elapses (40 units at 5/frame = 8).
#[test]
fn delayed_scatter_impact_recentered_on_live_victim_position() {
    // The saved shooter carries live BaseAiContinuation control, so its
    // template must admit an AI source for restore — like a real INI unit.
    let shooter_template = object("AuthoredDelayShooter", Some("ScatterDelayGun")).replace(
        "End\nEnd\n",
        "End\nBehavior = AIUpdateInterface AI\nEnd\nEnd\n",
    );
    let extra = format!(
        "{}{}",
        infantry_object("ScatterInfantry"),
        infantry_object("ScatterBystander"),
    );
    let mut world = world_from(&shooter_template, &extra);
    let shooter = world.create_object("AuthoredDelayShooter", Team::China, Vec3::ZERO).unwrap();
    let victim = world.create_object("ScatterInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();
    let near = world.create_object("ScatterBystander", Team::USA, Vec3::new(144.0, 0.0, 0.0)).unwrap();
    let stale = world.create_object("ScatterBystander", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();

    engage(&mut world, shooter, victim);
    let probe = crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(
        120.0,
        &game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot()),
    );
    assert!(probe.0 > 0.0, "deterministic fixture must roll a miss radius");

    world.set_current_frame(100);
    step(&mut world, &[shooter, victim, near, stale]);
    assert_eq!(hp(&world, victim), 500.0, "shot still in flight");

    // Disk round-trip keeps the queued delayed damage live in a fresh
    // matching world before the victim relocates.
    let directory = tempfile::TempDir::new().unwrap();
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let info = crate::save_load::SaveGameInfo {
        filename: "delayed_scatter".into(),
        display_name: "Delayed scatter".into(),
        description: "recentered victim".into(),
        map_name: "ScatterMap".into(),
        pristine_map_name: None,
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    };
    manager.save_game("delayed_scatter", &world, &info).unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: decoded, ..
    } = manager.load_game_snapshot("delayed_scatter").unwrap();
    let mut restored = world_from(&shooter_template, &extra);
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .unwrap();

    // After the scatter commit the victim relocates; the delayed impact must
    // resolve against the live position in both the live and restored world.
    world.set_fixture_object_position(victim, Vec3::new(140.0, 0.0, 0.0));
    restored.set_fixture_object_position(victim, Vec3::new(140.0, 0.0, 0.0));
    for frame in 101u64..=107 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim, near, stale]);
        restored.set_current_frame(frame);
        step(&mut restored, &[shooter, victim, near, stale]);
    }
    assert_eq!(hp(&world, victim), 500.0);
    assert_eq!(hp(&restored, victim), 500.0);
    assert_eq!(hp(&restored, near), 500.0);
    assert_eq!(hp(&restored, stale), 500.0);

    world.set_current_frame(108);
    step(&mut world, &[shooter, victim, near, stale]);
    assert_eq!(hp(&world, victim), 500.0 - 25.0);
    assert_eq!(hp(&world, near), 500.0 - 25.0);
    assert_eq!(hp(&world, stale), 500.0, "stale fire-time position untouched");

    restored.set_current_frame(108);
    step(&mut restored, &[shooter, victim, near, stale]);
    assert_eq!(hp(&restored, victim), 500.0 - 25.0);
    assert_eq!(hp(&restored, near), 500.0 - 25.0);
    assert_eq!(hp(&restored, stale), 500.0);

    for frame in 109u64..=112 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim, near, stale]);
        restored.set_current_frame(frame);
        step(&mut restored, &[shooter, victim, near, stale]);
    }
    assert_eq!(hp(&world, victim), 500.0 - 25.0, "exactly-once delivery");
    assert_eq!(hp(&world, near), 500.0 - 25.0);
    assert_eq!(hp(&world, stale), 500.0);
    assert_eq!(hp(&restored, victim), 500.0 - 25.0);
    assert_eq!(hp(&restored, near), 500.0 - 25.0);
    assert_eq!(hp(&restored, stale), 500.0);
}

/// Authored `ScatterTarget` picks turn the victim into a coordinate before
/// fireWeaponTemplate: the shot still commits on its fire frame, but no
/// object identity survives for a zero-radius direct hit.
#[test]
fn scatter_target_table_discards_victim_identity_for_the_shot() {
    let mut world = world("AuthoredTableShooter", "ScatterTableGun");
    let shooter = world
        .create_object("AuthoredTableShooter", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("PlainVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();

    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);

    assert_eq!(
        world
            .host_object(shooter)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .last_fire_frame,
        100,
        "shot accepted on the fire frame"
    );
    assert_eq!(hp(&world, victim), 500.0);
}

/// Coordinate-target laser near-scatter: with no victim object the C++
/// projectileDestination stays the scattered aim point (Weapon.cpp:1011-1027
/// resets it to the object's actual position only when victimObj exists).
#[test]
fn coordinate_laser_near_scatter_splashes_at_the_scattered_aim() {
    let extra = infantry_object("LaserBystander");
    let mut world = world_with("AuthoredCoordShooter", "LaserCoordGun", &extra);
    let shooter = world
        .create_object("AuthoredCoordShooter", Team::China, Vec3::ZERO)
        .unwrap();

    let coord = Vec3::new(40.0, 0.0, 0.0);
    world.set_current_frame(100);
    world
        .unit_command_attack_position(shooter, coord, 1)
        .unwrap();
    let (rolled, offset) = crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(
        120.0,
        &game_engine::common::random_value::SessionRandom::from_state(
            world.logic_random.snapshot(),
        ),
    );
    assert!(
        rolled > 0.0 && offset.length() > 0.0,
        "deterministic fixture must roll a miss"
    );
    // Just inside the scattered aim's 120-unit splash but outside a splash
    // centered on the original target point: radius 1 infantry sphere keeps
    // the edge at r + 120 vs r + 121.
    let bystander = world
        .create_object(
            "LaserBystander",
            Team::USA,
            coord + offset.normalize_or_zero() * (offset.length() + 121.0),
        )
        .unwrap();

    step(&mut world, &[shooter, bystander]);

    assert_eq!(hp(&world, bystander), 500.0 - 25.0);
    let beams: Vec<_> = world
        .weapon_lasers
        .iter()
        .filter(|beam| beam.laser_name == "ScatterCoordBeam")
        .collect();
    assert_eq!(beams.len(), 1, "exactly one authored beam");
    let beam = beams[0];
    assert_eq!(beam.to_id, None, "coordinate shot keeps no beam target");
    assert!(
        Vec3::from(beam.to_pos()).distance(coord + offset) < 0.001,
        "beam aims at the scattered aim point"
    );
}

/// Weapon.cpp:1010-1032 creates the LaserName beam before Body damage runs,
/// so a lethal shot's victim CreateObjectDie child must be born after the
/// beam object (allocation order, not queue order).
#[test]
fn lethal_laser_beam_is_born_before_victim_die_child() {
    // The beam object itself must be an admitted receiving template, same as
    // retail beam objects in Object INI.
    let extra = r#"
Object OrderCheckBeam
KindOf = IMMOBILE
Body = ActiveBody BodyTag
MaxHealth = 100
InitialHealth = 100
End
End
Object BeamOrderVictim
KindOf = VEHICLE SELECTABLE
Body = ActiveBody BodyTag
MaxHealth = 10
InitialHealth = 10
End
Behavior = CreateObjectDie CreateTag
CreationList = OCL_BeamOrderChild
End
End
Object BeamOrderChild
KindOf = VEHICLE SELECTABLE
Body = ActiveBody BodyTag
MaxHealth = 10
InitialHealth = 10
End
End
"#;
    let mut world = world_from(
        &object("AuthoredOrderShooter", Some("OrderBeamGun")),
        extra,
    );
    world
        .admit_ocl_application_sources(&[
            "ObjectCreationList OCL_BeamOrderChild\nCreateObject\nObjectNames = BeamOrderChild\nCount = 1\nEnd\nEnd\n"
                .into(),
        ])
        .unwrap();
    let shooter = world
        .create_object("AuthoredOrderShooter", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("BeamOrderVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();

    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);

    let beam = world
        .objects
        .registered_values()
        .find(|object| object.template_name == "OrderCheckBeam")
        .map(|object| object.id)
        .expect("beam object born");
    let child = world
        .objects
        .registered_values()
        .find(|object| object.template_name == "BeamOrderChild")
        .map(|object| object.id)
        .expect("victim die child born");
    assert!(beam < child, "beam {beam} born before child {child}");
}

/// When the tracked victim is gone before the delayed impact resolves, the
/// queued splash falls back to the stored fire-time damage position.
#[test]
fn delayed_scatter_impact_without_victim_uses_stored_position() {
    let extra = format!(
        "{}{}",
        infantry_object("ScatterInfantry"),
        infantry_object("ScatterBystander"),
    );
    let mut world = world_with("AuthoredDelayShooter", "ScatterDelayGun", &extra);
    let shooter = world
        .create_object("AuthoredDelayShooter", Team::China, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("ScatterInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let stale = world
        .create_object("ScatterBystander", Team::USA, Vec3::new(44.0, 0.0, 0.0))
        .unwrap();

    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim, stale]);
    assert_eq!(hp(&world, stale), 500.0, "shot still in flight");

    world.objects.remove(&victim);
    for frame in 101u64..=108 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, stale]);
    }
    assert_eq!(hp(&world, stale), 500.0 - 25.0);
}

/// `DamageDealtAtSelfPosition`: the delayed blast lands on the captured
/// firing pose, so a hostile standing next to the shooter is hit while the
/// intended target 40 units out is untouched.
#[test]
fn delayed_self_position_blast_hits_hostile_next_to_shooter() {
    let extra = infantry_object("SelfBlastBystander");
    let mut world = world_with("AuthoredSelfShooter", "SelfPositionGun", &extra);
    let shooter = world.create_object("AuthoredSelfShooter", Team::China, Vec3::ZERO).unwrap();
    let victim = world.create_object("PlainVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();
    let bystander = world.create_object("SelfBlastBystander", Team::USA, Vec3::new(6.0, 0.0, 0.0)).unwrap();

    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim, bystander]);
    assert_eq!(hp(&world, bystander), 500.0, "blast not due yet");

    for frame in 101u64..=107 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim, bystander]);
    }
    assert_eq!(hp(&world, bystander), 500.0);
    assert_eq!(hp(&world, victim), 500.0);

    world.set_current_frame(108);
    step(&mut world, &[shooter, victim, bystander]);
    assert_eq!(hp(&world, bystander), 500.0 - 25.0);
    assert_eq!(hp(&world, victim), 500.0);
    assert_eq!(hp(&world, shooter), 500.0, "SELF bit not authored");
}

/// Radius-zero authored cone: the delayed Direct hit still checks the
/// attacker's facing at impact time. Rotating the source away mid-flight
/// makes the behind-the-source victim take nothing; facing the victim
/// applies it on the next shot.
#[test]
fn delayed_direct_hit_honors_radius_damage_angle_at_impact() {
    let mut world = world("AuthoredConeShooter", "ConeDelayGun");
    let shooter = world.create_object("AuthoredConeShooter", Team::China, Vec3::ZERO).unwrap();
    let victim = world.create_object("PlainVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0)).unwrap();

    engage(&mut world, shooter, victim);
    world.set_current_frame(100);
    step(&mut world, &[shooter, victim]);
    assert_eq!(hp(&world, victim), 500.0);

    // First shot committed; stop attacking and face the source away from +X
    // before the delayed impact lands.
    {
        let attacker = world.objects.get_mut(&shooter).expect("cone shooter");
        attacker.target = None;
        attacker.set_status_attacking(false);
    }
    world.set_fixture_object_orientation(shooter, std::f32::consts::PI);

    for frame in 101u64..=108 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim]);
    }
    assert_eq!(hp(&world, victim), 500.0, "victim outside the 45-degree cone");

    // Re-engage and face +X: the same cone now admits the delayed direct hit.
    engage(&mut world, shooter, victim);
    world.set_fixture_object_orientation(shooter, 0.0);
    for frame in 109u64..=170 {
        world.set_current_frame(frame);
        step(&mut world, &[shooter, victim]);
    }
    assert_eq!(hp(&world, victim), 500.0 - 25.0);
}
