//! First live authored `OCLSpecialPower` Attack delivery (hq-2qedt.282).
//! C++ ObjectCreationList.cpp:163-195 locks the authored slot, orders
//! aiAttackPosition(CMD_FROM_AI), then stamps the authored RadiusDecalUpdate
//! delivery decal. The pad's own weapons discharge through the shared shot
//! boundary; no synthetic ScudStorm missile registry spawns on top.

use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
use crate::game_logic::object::WeaponLockType;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;
use std::collections::HashSet;

const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / 30.0;

const WEAPONS: &str = "Weapon StormDummyGun\n\
  PrimaryDamage = 5\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 20000\n\
  DamageType = ARMOR_PIERCING\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  DelayBetweenShots = 2000\n\
End\n\
Weapon StormOrderWitness\n\
  PrimaryDamage = 0\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 20000\n\
  DamageType = EXPLOSION\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 600\n\
  DelayBetweenShots = 500\n\
  ProjectileObject = StormOrderMissile\n\
End\n\
Weapon StormDeathLauncher\n\
  PrimaryDamage = 0\n\
  PrimaryDamageRadius = 0\n\
  AttackRange = 20000\n\
  DamageType = EXPLOSION\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 600\n\
  ProjectileObject = StormDeathMissile\n\
End\n\
Weapon StormDeathWitness\n\
  PrimaryDamage = 23\n\
  PrimaryDamageRadius = 100\n\
  DamageType = UNRESISTABLE\n\
  DeathType = NORMAL\n\
  WeaponSpeed = 999999\n\
  RadiusDamageAffects = ENEMIES\n\
  FireOCL = StormFieldOnDeath\n\
  ProjectileObject = NONE\n\
End\n";

/// The pad is a fully admitted structure: receiving template + AIUpdate +
/// OCLSpecialPower (SpecialPowerTemplate + OCL token) + RadiusDecalUpdate.
fn pad_definition(ocl_name: &str) -> String {
    pad_definition_with(ocl_name, "StormOrderWitness")
}

fn pad_definition_with(ocl_name: &str, secondary_weapon: &str) -> String {
    format!(
        "Object TestStormPad\n\
         KindOf = STRUCTURE IMMOBILE SELECTABLE CAN_ATTACK\n\
         WeaponSet\n\
         Conditions = None\n\
         Weapon = PRIMARY StormDummyGun\n\
         Weapon = SECONDARY {secondary_weapon}\n\
         End\n\
         Body = ActiveBody BodyTag\n\
         MaxHealth = 500\n\
         InitialHealth = 500\n\
         End\n\
         Behavior = AIUpdateInterface PadAI\n\
         End\n\
         Behavior = OCLSpecialPower PadStormPower\n\
         SpecialPowerTemplate = SuperweaponScudStorm\n\
         OCL = {ocl_name}\n\
         End\n\
         Behavior = RadiusDecalUpdate PadDecal\n\
         End\n\
         End\n"
    )
}

const MISSILE: &str = "Object StormOrderMissile\n\
  KindOf = PROJECTILE SMALL_MISSILE\n\
  Body = ActiveBody BodyTag\n\
  MaxHealth = 100\n\
  InitialHealth = 100\n\
  End\n\
  Locomotor = SET_NORMAL AuthoredMissileFlight\n\
  Behavior = PhysicsBehavior PhysicsTag\n\
  Mass = 1\n\
  End\n\
  Behavior = MissileAIUpdate FlightTag\n\
  TryToFollowTarget = Yes\n\
  FuelLifetime = 3000\n\
  IgnitionDelay = 0\n\
  End\n\
  End\n";

fn world_with(ocl_name: &str, ocls: &[String]) -> GameLogic {
    world_with_objects(&format!("{}{}", pad_definition(ocl_name), MISSILE), ocls)
}

fn world_with_objects(objects: &str, ocls: &[String]) -> GameLogic {
    let mut world = GameLogic::new();
    let mut locomotors =
        game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "Locomotor AuthoredMissileFlight\n Surfaces = AIR\n Speed = 300\n\
             MinSpeed = 30\n Acceleration = 675\n TurnRate = 3600\n\
             MaxThrustAngle = 45\n AllowAirborneMotiveForce = Yes\n\
             PreferredHeight = 240\n Appearance = THRUST\nEnd\n",
        )
        .unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-scud-order.ini".into(),
                objects.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    if !ocls.is_empty() {
        world
            .admit_object_creation_list_sources(ocls.iter().cloned().collect::<Vec<_>>())
            .unwrap();
    }
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

const STORM_ORDER: &str = "ObjectCreationList StormOrder\n\
  Attack\n\
  NumberOfShots = 2\n\
  WeaponSlot = SECONDARY\n\
  DeliveryDecalRadius = 250\n\
  DeliveryDecal\n\
  Texture = StormTargetDecal\n\
  OpacityMin = 50\n\
  OpacityMax = 90\n\
  OpacityThrobTime = 500\n\
  OnlyVisibleToOwningPlayer = Yes\n\
  End\n\
  End\n\
  End\n";

/// The authored OCL orders the pad's own locked weapon at the location;
/// the shared shot boundary then discharges exactly NumberOfShots authored
/// projectiles with the pad as producer. The synthetic nine-missile
/// registry and the name-peeled hard-coded decal stay out of the way.
#[test]
fn authored_scud_storm_orders_the_selected_weapon() {
    let mut world = world_with("StormOrder", &[STORM_ORDER.to_string()]);
    world.add_player(Player::new(0, Team::GLA, "GLA", true));
    let pad = world
        .create_object("TestStormPad", Team::GLA, Vec3::ZERO)
        .unwrap();
    let target = Vec3::new(300.0, 0.0, 40.0);

    world.set_current_frame(10);
    let strike =
        world.queue_special_power_strike(&crate::command_system::SpecialPowerType::ScudStorm, pad, target);
    assert!(strike.is_some());

    let pad_obj = world.host_object(pad).unwrap();
    // C++ AttackNugget::create — temp lock on the authored slot, then
    // aiAttackPosition(CMD_FROM_AI) with the authored shot budget.
    assert_eq!(pad_obj.last_command_source, HUNT_CMD_FROM_AI);
    assert_eq!(pad_obj.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(pad_obj.weapon_lock_slot, 1);
    assert_eq!(pad_obj.max_shots_to_fire, 2);
    assert_eq!(pad_obj.target_location, Some(target));
    // No synthetic ScudStorm registry wave, no immediate projectile —
    // the attack order discharges through the combat pass.
    assert!(world.scud_storm_missile_flight_reg.pending.is_empty());
    assert!(world.combat_system.get_projectiles().is_empty());

    // C++ authored RadiusDecalUpdate delivery decal (template fields only;
    // radius from DeliveryDecalRadius, not the retail 200 constant).
    let decal = &pad_obj
        .radius_decal_update
        .as_ref()
        .expect("authored RadiusDecalUpdate receives the delivery decal")
        .delivery_decal;
    assert!(!decal.empty);
    assert_eq!(decal.radius, 250.0);
    assert_eq!(decal.position, target);
    let tmpl = decal.template.as_ref().unwrap();
    assert_eq!(tmpl.texture, "StormTargetDecal");
    assert_eq!(tmpl.opacity_min, 0.5);
    assert_eq!(tmpl.opacity_max, 0.9);
    assert_eq!(tmpl.throb_frames, 15);
    assert!(tmpl.only_visible_to_owner);
    assert!(pad_obj
        .radius_decal_update
        .as_ref()
        .unwrap()
        .kill_when_no_longer_attacking);

    // The order commits through the shared shot boundary: two authored
    // missiles, each carrying the pad as its producer.
    let mut seen: HashSet<ObjectId> = HashSet::new();
    for frame in 11..11 + 90 {
        world.set_current_frame(frame);
        world.update_combat(&[pad], LOGIC_FRAME_TIMESTEP).unwrap();
        let _ = world
            .update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP)
            .unwrap();
        for (id, object) in world.host_objects().registered() {
            if object.template_name == "StormOrderMissile" {
                assert_eq!(object.producer_id, Some(pad));
                seen.insert(*id);
            }
        }
    }
    assert_eq!(seen.len(), 2, "NumberOfShots = 2 authored budget");
    assert_eq!(world.host_object(pad).unwrap().max_shots_to_fire, 0);
    assert!(world.scud_storm_missile_flight_reg.pending.is_empty());
}

/// An authored pad whose module points at a missing/unsupported list fails
/// closed: no weapon lock, no order, no decal, no synthetic fallback wave.
#[test]
fn authored_scud_storm_with_missing_list_does_nothing() {
    let mut world = world_with("MissingStormOrder", &[]);
    world.add_player(Player::new(0, Team::GLA, "GLA", true));
    let pad = world
        .create_object("TestStormPad", Team::GLA, Vec3::ZERO)
        .unwrap();
    let target = Vec3::new(300.0, 0.0, 40.0);

    world.set_current_frame(10);
    let strike =
        world.queue_special_power_strike(&crate::command_system::SpecialPowerType::ScudStorm, pad, target);
    assert!(strike.is_some());

    let pad_obj = world.host_object(pad).unwrap();
    assert_eq!(pad_obj.weapon_lock_type, WeaponLockType::NotLocked);
    assert!(pad_obj.target_location.is_none());
    let decal_empty = pad_obj
        .radius_decal_update
        .as_ref()
        .map(|rd| rd.delivery_decal.empty)
        .unwrap_or(true);
    assert!(decal_empty);
    assert!(world.scud_storm_missile_flight_reg.pending.is_empty());

    for frame in 11..41 {
        world.set_current_frame(frame);
        world.update_combat(&[pad], LOGIC_FRAME_TIMESTEP).unwrap();
    }
    assert!(world.combat_system.get_projectiles().is_empty());
    assert!(!world
        .host_objects()
        .registered()
        .any(|(_, object)| object.template_name == "StormOrderMissile"));
}

/// A second authored Attack nugget with no DeliveryDecal still runs
/// `RadiusDecalUpdate::createRadiusDecal` (ObjectCreationList.cpp:163-195):
/// the template create returns empty, which clears the previous marker and
/// wakes nothing, while `killWhenNoLongerAttacking` is set regardless.
#[test]
fn authored_scud_storm_later_empty_decal_clears_previous_marker() {
    let list = STORM_ORDER
        .trim_end()
        .strip_suffix("End")
        .unwrap()
        .to_owned()
        + "  Attack
  NumberOfShots = 1
  WeaponSlot = PRIMARY
  End
End
";
    let mut world = world_with("StormOrder", &[list]);
    world.add_player(Player::new(0, Team::GLA, "GLA", true));
    let pad = world
        .create_object("TestStormPad", Team::GLA, Vec3::ZERO)
        .unwrap();
    world.set_current_frame(10);
    assert!(
        world
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::ScudStorm,
                pad,
                Vec3::new(300.0, 0.0, 40.0),
            )
            .is_some()
    );

    let pad = world.host_object(pad).unwrap();
    assert_eq!(pad.weapon_lock_slot, 0);
    assert_eq!(pad.max_shots_to_fire, 1);
    let decal = pad.radius_decal_update.as_ref().unwrap();
    assert!(decal.delivery_decal.is_empty());
    assert!(!decal.awake);
    assert!(decal.kill_when_no_longer_attacking);
}

/// One-shot variant of the authored attack order for the death-weapon
/// flight test; no delivery decal is needed to watch the missile die.
const STORM_ORDER_ONCE: &str = "ObjectCreationList StormOrderOnce\n\
  Attack\n\
  NumberOfShots = 1\n\
  WeaponSlot = SECONDARY\n\
  End\n\
  End\n";

/// Authored FireOCL creation list discharged by the death witness weapon.
const STORM_FIELD_OCL: &str = "ObjectCreationList StormFieldOnDeath\n\
  CreateObject\n\
  ObjectNames = StormDeathWitnessField\n\
  Count = 1\n\
  End\n\
  End\n";

/// Retail GLAScudStormMissile shape: MissileAI loft, HeightDie kill on the
/// dive, then DestroyDie before FireWeaponWhenDeadBehavior (retail order).
const STORM_DEATH_MISSILE: &str = "Object StormDeathMissile\n\
  KindOf = PROJECTILE SMALL_MISSILE\n\
  Body = ActiveBody BodyTag\n\
  MaxHealth = 100\n\
  InitialHealth = 100\n\
  End\n\
  Locomotor = SET_NORMAL AuthoredMissileFlight\n\
  Behavior = PhysicsBehavior PhysicsTag\n\
  Mass = 1\n\
  End\n\
  Behavior = MissileAIUpdate FlightTag\n\
  TryToFollowTarget = No\n\
  FuelLifetime = 0\n\
  InitialVelocity = 0\n\
  IgnitionDelay = 0\n\
  DistanceToTravelBeforeTurning = 500\n\
  DistanceToTargetBeforeDiving = 200\n\
  End\n\
  Behavior = HeightDieUpdate HeightTag\n\
  TargetHeight = 15\n\
  TargetHeightIncludesStructures = Yes\n\
  OnlyWhenMovingDown = Yes\n\
  SnapToGroundOnDeath = Yes\n\
  InitialDelay = 1000\n\
  End\n\
  Behavior = DestroyDie DestroyTag\n\
  End\n\
  Behavior = SpecialPowerCompletionDie CompletionTag\n\
  SpecialPowerTemplate = SuperweaponScudStorm\n\
  End\n\
  Behavior = FireWeaponWhenDeadBehavior FireWhenDeadTag\n\
  StartsActive = Yes\n\
  DeathWeapon = StormDeathWitness\n\
  DeathTypes = NONE +NORMAL\n\
  End\n\
  End\n";

const STORM_DEATH_FIELD: &str = "Object StormDeathWitnessField\n\
  KindOf = IMMOBILE NO_COLLIDE\n\
  Body = ActiveBody FieldBody\n\
  MaxHealth = 10\n\
  InitialHealth = 10\n\
  End\n\
  End\n";

const STORM_VICTIM: &str = "Object StormVictim\n\
  KindOf = VEHICLE SELECTABLE\n\
  Body = ActiveBody BodyTag\n\
  MaxHealth = 500\n\
  InitialHealth = 500\n\
  End\n\
  End\n";

fn count_objects(world: &GameLogic, template: &str) -> usize {
    world
        .host_objects()
        .registered()
        .filter(|(_, object)| object.template_name == template)
        .count()
}

fn health(world: &GameLogic, id: ObjectId) -> f32 {
    world.host_object(id).unwrap().health.current
}

const STORM_COMPLETION_POWER: &str = "SuperweaponScudStorm";
const PAD_COMPLETION_CREATOR: u32 = 77;

/// The real missile's HeightDieUpdate fires its authored death weapon and
/// reports completion with the pad's ID. The strike registry does neither.
#[test]
fn authored_scud_storm_height_death_fires_its_own_weapon() {
    assert_authored_scud_storm_delivery(false);
}

/// Weapon.cpp:1078-1117 — a shooter with an installed
/// SpecialPowerCompletionDie is notified at launch from its existing
/// creator receipt; the projectile is stamped INVALID so its own die
/// stays silent. Without one, the shooter's ID is stamped on the child
/// and the child's die reports at death.
#[test]
fn authored_scud_storm_completing_shooter_notifies_at_launch() {
    assert_authored_scud_storm_delivery(true);
}

const STORM_DEATH_TARGET: Vec3 = Vec3::new(1000.0, 0.0, 200.0);

fn storm_death_world(shooter_completes: bool) -> GameLogic {
    let mut pad_ini = pad_definition_with("StormOrderOnce", "StormDeathLauncher");
    if shooter_completes {
        let end = pad_ini.trim_end().len() - "End".len();
        pad_ini.insert_str(
            end,
            "Behavior = SpecialPowerCompletionDie PadCompletion\n\
             SpecialPowerTemplate = SuperweaponScudStorm\n\
             End\n",
        );
    }
    let mut world = world_with_objects(
        &format!(
            "{}{}{}{}",
            pad_ini,
            STORM_DEATH_MISSILE,
            STORM_DEATH_FIELD,
            STORM_VICTIM,
        ),
        &[
            STORM_ORDER_ONCE.to_string(),
            STORM_FIELD_OCL.to_string(),
        ],
    );
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    for (id, team, alliance) in [(0, Team::GLA, 0), (1, Team::USA, 1)] {
        let mut player = Player::new(id, team, "storm fixture", true);
        player.alliance_team = alliance;
        world.add_player(player);
    }
    world.install_owned_script_engine(
        gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap(),
    );
    world
}

fn spawn_storm_death_actors(world: &mut GameLogic) -> (ObjectId, ObjectId, ObjectId) {
    let pad = world
        .create_object("TestStormPad", Team::GLA, Vec3::new(200.0, 0.0, 200.0))
        .unwrap();
    let near = world
        .create_object("StormVictim", Team::USA, Vec3::new(1000.0, 0.0, 230.0))
        .unwrap();
    let far = world
        .create_object("StormVictim", Team::USA, Vec3::new(1400.0, 0.0, 200.0))
        .unwrap();
    (pad, near, far)
}

fn assert_authored_scud_storm_delivery(shooter_completes: bool) {
    let mut world = storm_death_world(shooter_completes);
    let (pad, near, far) = spawn_storm_death_actors(&mut world);
    if shooter_completes {
        world
            .host_object_mut(pad)
            .unwrap()
            .set_special_power_completion(STORM_COMPLETION_POWER, PAD_COMPLETION_CREATOR);
    }

    world.set_current_frame(10);
    let strike = world.queue_special_power_strike(
        &crate::command_system::SpecialPowerType::ScudStorm,
        pad,
        STORM_DEATH_TARGET,
    );
    assert!(strike.is_some());
    assert!(world.scud_storm_missile_flight_reg.pending.is_empty());
    assert_eq!(world.host_object(pad).unwrap().max_shots_to_fire, 1);

    let mut missile_id = None;
    let mut death_frame = None;
    for frame in 11..900 {
        world.set_current_frame(frame);
        let ids: Vec<ObjectId> = world
            .host_objects()
            .registered()
            .map(|(id, _)| *id)
            .collect();
        world.update_combat(&ids, LOGIC_FRAME_TIMESTEP).unwrap();
        world.apply_owned_projectileless_delayed_damage().unwrap();
        world.execute_pending_weapon_fire_ocls().unwrap();
        world
            .update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP)
            .unwrap();
        world.update_ai(&ids, LOGIC_FRAME_TIMESTEP).unwrap();
        if missile_id.is_none() {
            missile_id = world
                .host_objects()
                .registered()
                .find(|(_, object)| object.template_name == "StormDeathMissile")
                .map(|(id, _)| *id);
            if missile_id.is_some() {
                assert_eq!(
                    world.special_power_completion_log.notifications,
                    u32::from(shooter_completes),
                    "a completing shooter notifies at launch, never at death",
                );
                let engine = world.owned_script_engine.as_ref().unwrap();
                assert!(
                    !engine
                        .is_special_power_complete(0, STORM_COMPLETION_POWER, false, pad.0),
                    "the pad object itself is never the completion source",
                );
                if shooter_completes {
                    assert!(
                        engine.is_special_power_complete(
                            0,
                            STORM_COMPLETION_POWER,
                            false,
                            PAD_COMPLETION_CREATOR,
                        ),
                        "the pad's creator receipt owns the launch completion",
                    );
                }
            }
        }
        if count_objects(&world, "StormDeathWitnessField") > 0 {
            death_frame = Some(frame);
            break;
        }
    }

    assert!(
        missile_id.is_some(),
        "the authored attack order must launch one real StormDeathMissile",
    );
    let death_frame = death_frame.expect(
        "the diving missile must die to its HeightDieUpdate and fire the authored DeathWeapon",
    );
    assert!(
        world.scud_storm_missile_flight_reg.pending.is_empty(),
        "the registry fallback stays out of the authored delivery",
    );
    assert_eq!(
        count_objects(&world, "StormDeathWitnessField"),
        1,
        "exactly one authored FireOCL field at frame {death_frame}",
    );
    assert_eq!(
        health(&world, near),
        477.0,
        "victim inside the authored radius takes the 23 once",
    );
    assert_eq!(
        health(&world, far),
        500.0,
        "victim outside the authored radius stays untouched",
    );

    // More frames plus a registry update: no synthetic residual wave and
    // no second application of the death weapon.
    for frame in death_frame + 1..=death_frame + 40 {
        world.set_current_frame(frame);
        let ids: Vec<ObjectId> = world
            .host_objects()
            .registered()
            .map(|(id, _)| *id)
            .collect();
        world.update_combat(&ids, LOGIC_FRAME_TIMESTEP).unwrap();
        world.apply_owned_projectileless_delayed_damage().unwrap();
        world.execute_pending_weapon_fire_ocls().unwrap();
        world
            .update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP)
            .unwrap();
        world.update_ai(&ids, LOGIC_FRAME_TIMESTEP).unwrap();
        world.update_special_power_strikes().unwrap();
    }
    assert_eq!(
        count_objects(&world, "StormDeathWitnessField"),
        1,
        "no second field birth from registry or residual paths",
    );
    assert_eq!(
        health(&world, near),
        477.0,
        "no duplicate damage from the registry or the death weapon",
    );
    assert_eq!(
        world.special_power_completion_log.notifications, 1,
        "exactly one completion notification across launch and death",
    );
    let engine = world.owned_script_engine.as_ref().unwrap();
    let expected_creator = if shooter_completes {
        PAD_COMPLETION_CREATOR
    } else {
        pad.0
    };
    assert!(engine.is_special_power_complete(
        0,
        STORM_COMPLETION_POWER,
        true,
        expected_creator,
    ));
    assert!(!engine.is_special_power_complete(0, STORM_COMPLETION_POWER, false, pad.0));
    assert!(!engine.is_special_power_complete(
        0,
        STORM_COMPLETION_POWER,
        false,
        PAD_COMPLETION_CREATOR,
    ));
}

/// A live missile must resume on the same frame after loading, including
/// its HeightDie deadline and the launcher ID used for script completion.
#[test]
fn authored_scud_storm_resumes_live_missile_after_encoded_restore() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    let mut source = storm_death_world(false);
    let (pad, near, far) = spawn_storm_death_actors(&mut source);
    assert!(
        source
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::ScudStorm,
                pad,
                STORM_DEATH_TARGET,
            )
            .is_some()
    );

    let step = |world: &mut GameLogic| {
        assert_eq!(world.update_with_dt(LOGIC_FRAME_TIMESTEP).steps_run, 1)
    };
    let flight = |world: &GameLogic, id: ObjectId| {
        let actor = world.host_object(id)?;
        let projectile = world.combat_system.get_projectiles().get(&id)?;
        Some((actor.get_position(), projectile.velocity))
    };

    // This fixture has no model launch bone. Wait for the missile to be
    // moving above ground so the snapshot exercises live flight.
    let mut missile = None;
    for _ in 0..300 {
        step(&mut source);
        missile = source
            .combat_system
            .get_projectiles()
            .iter()
            .find(|(id, projectile)| {
                projectile.owns_receiving_actor()
                    && projectile.missile_motion.is_some()
                    && source.host_object(**id).is_some_and(|actor| {
                        actor.template_name == "StormDeathMissile"
                            && actor.is_alive()
                            && actor.get_position().y > 10.0
                    })
                    && projectile.velocity.length_squared() > 0.0
            })
            .map(|(&id, _)| id);
        if missile.is_some() {
            break;
        }
    }
    let missile = missile.expect(
        "the authored attack must produce a live airborne missile",
    );
    assert_eq!(count_objects(&source, "StormDeathWitnessField"), 0);
    assert_eq!(health(&source, near), 500.0);
    assert_eq!(source.special_power_completion_log.notifications, 0);

    let builder = SnapshotBuilder::new();
    let bytes =
        bincode_legacy::serialize(&builder.create_world_snapshot(&source).unwrap()).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = storm_death_world(false);
    // The shared fixture already seeded the sorted team prototypes (0 then 1).
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.frame, source.frame);
    assert_eq!(flight(&restored, missile), flight(&source, missile));

    let mut impact = None;
    for _ in 0..900 {
        step(&mut source);
        step(&mut restored);
        assert_eq!(restored.frame, source.frame);
        assert_eq!(
            flight(&restored, missile),
            flight(&source, missile),
            "at frame {}",
            source.frame,
        );
        for victim in [near, far] {
            assert_eq!(
                health(&restored, victim),
                health(&source, victim),
                "at frame {}",
                source.frame,
            );
        }
        let fields = count_objects(&source, "StormDeathWitnessField");
        assert_eq!(count_objects(&restored, "StormDeathWitnessField"), fields);
        if fields == 1 {
            impact.get_or_insert(source.frame);
        }
        if impact.is_some_and(|frame| source.frame >= frame + 40) {
            break;
        }
    }
    assert!(
        impact.is_some(),
        "resumed live missile must reach its authored impact",
    );

    for world in [&mut source, &mut restored] {
        assert_eq!(health(world, near), 477.0);
        assert_eq!(health(world, far), 500.0);
        assert_eq!(count_objects(world, "StormDeathWitnessField"), 1);
        assert!(flight(world, missile).is_none());
        assert!(world.scud_storm_missile_flight_reg.pending.is_empty());
        assert_eq!(world.special_power_strikes.toxin_fields_spawned_total(), 0);
        assert_eq!(world.special_power_completion_log.notifications, 1);
        let scripts = world.owned_script_engine.as_ref().unwrap();
        assert!(scripts.is_special_power_complete(
            0,
            STORM_COMPLETION_POWER,
            true,
            pad.0
        ));
        assert!(!scripts.is_special_power_complete(
            0,
            STORM_COMPLETION_POWER,
            false,
            pad.0
        ));
    }
}

/// An authored pad whose OCL is missing leaves a registry record only for
/// the activation receipt: no synthetic impact plan ever resolves for it —
/// not before the save, and not after restore hands live-delivery ownership
/// back to the registry. The authored Attack order (when present) and the
/// native projectile are saved through their own state.
#[test]
fn authored_scud_storm_missing_list_stays_inert_after_restore() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    let objects = format!(
        "{}{}{}",
        pad_definition("MissingStormOrder"),
        MISSILE,
        STORM_VICTIM
    );
    let mut source = world_with_objects(&objects, &[]);
    for (id, team, alliance) in [(0, Team::GLA, 0), (1, Team::USA, 1)] {
        let mut player = Player::new(id, team, "storm fixture", true);
        player.alliance_team = alliance;
        source.add_player(player);
    }
    let target = Vec3::new(300.0, 0.0, 40.0);
    let pad = source
        .create_object("TestStormPad", Team::GLA, Vec3::ZERO)
        .unwrap();
    let victim = source
        .create_object("StormVictim", Team::USA, target)
        .unwrap();
    source.set_current_frame(10);
    assert!(
        source
            .queue_special_power_strike(
                &crate::command_system::SpecialPowerType::ScudStorm,
                pad,
                target,
            )
            .is_some()
    );

    let builder = SnapshotBuilder::new();
    let bytes =
        bincode_legacy::serialize(&builder.create_world_snapshot(&source).unwrap()).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    // Seed the receiver's team factory with the same prototypes, then clear:
    // restore owns the runtime roster, not the prototype definitions.
    let mut restored = world_with_objects(&objects, &[]);
    let mut player_ids: Vec<_> = source.players.keys().copied().collect();
    player_ids.sort_unstable();
    for id in player_ids {
        restored.add_player(source.players[&id].clone());
    }
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();

    for world in [&mut restored, &mut source] {
        world.set_current_frame(1000);
        world.update_special_power_strikes().unwrap();
        assert!(
            world.special_power_strikes.completed_this_frame().is_empty(),
            "no synthetic ScudStorm wave resolves for an authored dispatch",
        );
        assert_eq!(health(world, victim), 500.0);
        assert_eq!(world.special_power_strikes.toxin_fields_spawned_total(), 0);
    }
}
