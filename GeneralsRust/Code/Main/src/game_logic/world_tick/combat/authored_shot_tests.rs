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
    let (attacker, events) = world.host_object_and_attack_events_mut(shooter).unwrap();
    attacker.attack_target(
        victim,
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
