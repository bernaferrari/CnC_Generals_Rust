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
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-authored-shot.ini".into(),
                format!("{}{}", object(shooter, Some(weapon)), object("PlainVictim", None)),
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
    world.apply_owned_projectileless_delayed_damage();
    let _ = world.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP);
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
    second.tick_native_lifecycle(other_source);
    second.apply_owned_projectileless_delayed_damage();
    assert!(second.objects[&foreign_missile].ecm_missile_jammed);
    first.set_current_frame(u64::from(last) + 29);
    first.tick_native_lifecycle(source);
    first.update_ecm_missile_jam();
    assert!(!first.objects[&missile].ecm_missile_jammed);
    first.set_current_frame(u64::from(last) + 30);
    first.tick_native_lifecycle(source);
    first.apply_owned_projectileless_delayed_damage();
    assert!(first.objects[&missile].ecm_missile_jammed);
    assert_eq!(first.objects[&missile].health.current, 100.0);
}
