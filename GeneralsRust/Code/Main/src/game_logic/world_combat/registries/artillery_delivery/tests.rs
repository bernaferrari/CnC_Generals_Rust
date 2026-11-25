use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::special_power_strikes::HostStrikePhase;
use crate::save_load::snapshot::{Snapshot, WorldSnapshot};
use crate::save_load::{SnapshotBuilder, Xfer, XferLoad, XferSave};

fn world() -> GameLogic {
    world_with_delays(0, 0)
}

fn world_with_delays(door_delay_ms: u32, drop_delay_ms: u32) -> GameLogic {
    world_with_carrier(door_delay_ms, drop_delay_ms, None)
}

fn world_with_carrier(
    door_delay_ms: u32,
    drop_delay_ms: u32,
    start_at_max_speed: Option<bool>,
) -> GameLogic {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    let (physics, altitude_settings, preferred_height) = if start_at_max_speed.is_some() {
        (
            "Behavior = PhysicsBehavior CarrierPhysics
 Mass = 2
 End",
            "MinSpeed = 150
 MinTurnSpeed = 70
 PreferredHeight = 500
 Lift = 120
 AllowAirborneMotiveForce = Yes
 ZAxisBehavior = SURFACE_RELATIVE_HEIGHT",
            "Yes",
        )
    } else {
        ("", "", "No")
    };
    let inherit_velocity = if start_at_max_speed.is_some() {
        "Yes"
    } else {
        "No"
    };
    let max_speed = if start_at_max_speed == Some(true) {
        "Yes"
    } else {
        "No"
    };
    let mut world = GameLogic::new();
    world.override_world_size(1000.0, 1000.0);
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors
        .apply_source(&format!(
            "Locomotor TestPayloadFlight
 Surfaces = AIR
 Speed = 450
 Acceleration = 600
 TurnRate = 360
 Appearance = THRUST
End
Locomotor TestArtilleryCarrierFlight
 Surfaces = AIR
 Speed = 150
 Acceleration = 80
 TurnRate = 200
 Appearance = WINGS
 {altitude_settings}
End"
        ))
        .unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_object_ini_sources([(
            "artillery-delivery.ini".into(),
            format!(
                "
Object TestArtilleryCaster
 KindOf = STRUCTURE
 Behavior = OCLSpecialPower Artillery
  SpecialPowerTemplate = SuperweaponArtilleryBarrage
  OCL = TestArtilleryDelivery
  CreateLocation = CREATE_AT_EDGE_FARTHEST_FROM_TARGET
 End
End
Object TestArtilleryCarrier
 KindOf = AIRCRAFT VEHICLE
 Locomotor = SET_NORMAL TestArtilleryCarrierFlight
 {physics}
 Behavior = DeliverPayloadAIUpdate Delivery
  DoorDelay = {door_delay_ms}
 End
 Draw = W3DModelDraw CarrierDraw
  DefaultConditionState
   Model = TestCarrier
  End
 End
 Body = ActiveBody CarrierBody
  MaxHealth = 600
  InitialHealth = 600
 End
End
Object TestArtilleryShell
 KindOf = PROJECTILE
 Body = ActiveBody ShellBody
  MaxHealth = 50
  InitialHealth = 50
 End
 Behavior = PhysicsBehavior ShellPhysics
  Mass = 1
 End
 Behavior = MissileAIUpdate ShellFlight
  FuelLifetime = 10000
  DetonateOnNoFuel = Yes
  InitialVelocity = 60
  DistanceToTargetBeforeDiving = 500
  DistanceToTargetForLock = 25
 End
 Locomotor = SET_NORMAL TestPayloadFlight
End
Object TestArtilleryVictim
 KindOf = VEHICLE
 Body = ActiveBody VictimBody
  MaxHealth = 500
  InitialHealth = 500
 End
End
"
            ),
        )])
        .unwrap();
    let mut weapons = Default::default();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut weapons,
            "
Weapon TestArtilleryWarhead
 PrimaryDamage = 105
 PrimaryDamageRadius = 50
 RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
 DamageType = EXPLOSION
 WeaponSpeed = 99999
End
",
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    world
        .admit_receiving_weapon_snapshot(weapons.into_iter().collect())
        .unwrap();
    world
        .admit_object_creation_list_sources([format!(
            "
ObjectCreationList TestArtilleryDelivery
 DeliverPayload
  Transport = TestArtilleryCarrier
  FormationSize = 1
  StartAtPreferredHeight = {preferred_height}
  StartAtMaxSpeed = {max_speed}
  DeliveryDistance = 100
  DropDelay = {drop_delay_ms}
  VisiblePayloadTemplateName = TestArtilleryShell
  VisiblePayloadWeaponTemplate = TestArtilleryWarhead
  VisibleSubObjectBaseName = Bomb
  VisibleNumBones = 1
  VisibleItemsDroppedPerInterval = 1
  InheritTransportVelocity = {inherit_velocity}
  SelfDestructObject = Yes
 End
End
"
        )])
        .unwrap();
    world.add_player(Player::new(1, Team::China, "Owner", true));
    world.add_player(Player::new(2, Team::GLA, "Target", false));
    world
}

fn activate(world: &mut GameLogic, aim: Vec3) -> (u32, ObjectId, ObjectId) {
    let caster = world
        .create_object_for_player("TestArtilleryCaster", 1, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object_for_player("TestArtilleryVictim", 2, aim)
        .unwrap();
    let strike = world
        .queue_special_power_strike(&SpecialPowerType::Artillery, caster, aim)
        .unwrap();
    let carrier = world
        .objects
        .registered()
        .find(|(_, object)| object.artillery_barrage_transport.is_some())
        .map(|(&id, _)| id)
        .expect("one carrier from the receiving OCL");
    assert_eq!(
        world.objects[&carrier].template_name,
        "TestArtilleryCarrier"
    );
    (strike, carrier, victim)
}

#[test]
fn authored_artillery_release_follows_carrier_through_save_load_and_destruction() {
    let aim = Vec3::new(80.0, 0.0, 50.0);
    let mut world = world();
    let (strike, carrier, victim) = activate(&mut world, aim);
    let approach = world.objects[&carrier].get_position();
    assert!(approach.y >= 300.0);
    world.frame = 30;
    world.update_artillery_barrage_flights().unwrap();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        snapshot.xfer(&mut writer).unwrap();
        writer.xfer_u32(&mut 0x4137_11e0).unwrap();
    }
    let mut decoded = WorldSnapshot::default();
    let mut reader = XferLoad::new(std::io::Cursor::new(bytes.into_inner()));
    decoded.xfer(&mut reader).unwrap();
    let mut sentinel = 0;
    reader.xfer_u32(&mut sentinel).unwrap();
    assert_eq!(sentinel, 0x4137_11e0);
    let mut restored = self::world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();

    world.mark_object_for_destruction(carrier, None);
    world.process_destroy_list();
    for frame in 31..420 {
        world.frame = frame;
        world.update_special_power_strikes().unwrap();
        world.update_artillery_barrage_flights().unwrap();
    }
    assert!(world.combat_system.get_projectiles().is_empty());
    assert_eq!(world.objects[&victim].health.current, 500.0);
    assert_eq!(
        world.special_power_strikes.get(strike).unwrap().phase,
        HostStrikePhase::Cancelled
    );

    let mut shell = None;
    for frame in 31..420 {
        restored.frame = frame;
        restored.update_special_power_strikes().unwrap();
        assert_eq!(restored.objects[&victim].health.current, 500.0);
        restored.update_artillery_barrage_flights().unwrap();
        if let Some((&id, projectile)) = restored.combat_system.get_projectiles().iter().next() {
            assert_eq!(projectile.target_position, aim);
            assert_eq!(projectile.historic_weapon_key, "TestArtilleryWarhead");
            assert!(projectile.position.y >= 300.0);
            assert!(projectile.missile_motion.is_some());
            assert_eq!(restored.objects[&id].producer_id, Some(carrier));
            assert_eq!(restored.objects[&id].owner_player_id, Some(1));
            assert!(
                restored.objects[&carrier]
                    .sub_object_visibility
                    .is_hidden("Bomb01")
            );
            shell = Some(id);
            break;
        }
    }
    let shell = shell.expect("restored carrier releases one authored missile");
    restored.frame += 1;
    restored.update_artillery_barrage_flights().unwrap();
    restored.process_destroy_list();
    assert!(restored.objects.get(&carrier).is_none());
    assert!(
        restored
            .combat_system
            .get_projectiles()
            .contains_key(&shell)
    );
    for _ in 0..400 {
        restored.frame += 1;
        restored
            .update_owned_projectile_impacts(1.0 / 30.0)
            .unwrap();
        restored.update_special_power_strikes().unwrap();
        restored.update_artillery_barrage_flights().unwrap();
    }
    let result = restored.special_power_strikes.get(strike).unwrap();
    assert_eq!(result.phase, HostStrikePhase::Completed);
    assert_eq!(result.multi_strike_applied, 1);
    assert_eq!(restored.artillery_barrage_flight_reg.shells_dropped, 1);
    assert!(result.total_damage_applied <= 105.0);
}

#[test]
fn authored_delivery_waits_for_the_door_and_drop_interval_across_save() {
    use crate::game_logic::host_enum_table_residual::{
        door_1_closing_model_bit, door_1_opening_model_bit,
    };

    let aim = Vec3::new(80.0, 0.0, 50.0);
    let mut source = world_with_delays(70, 170);
    let (_, carrier, _) = activate(&mut source, aim);
    source.set_fixture_object_position(carrier, aim + Vec3::new(40.0, 300.0, 0.0));

    let opening = 1u128 << door_1_opening_model_bit();
    let closing = 1u128 << door_1_closing_model_bit();
    source.frame = 60;
    source.update_artillery_barrage_flights().unwrap();
    assert_eq!(
        source.objects[&carrier].model_condition_bits & (opening | closing),
        opening
    );
    assert!(
        source.combat_system.get_projectiles().is_empty(),
        "Approach only enters Delivering"
    );

    source.frame = 61;
    source.update_artillery_barrage_flights().unwrap();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut saved = WorldSnapshot::default();
    saved
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let mut restored = world_with_delays(70, 170);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();

    for frame in 62..=71 {
        for world in [&mut source, &mut restored] {
            world.frame = frame;
            world.update_artillery_barrage_flights().unwrap();
            let object = &world.objects[&carrier];
            assert_eq!(
                world.combat_system.get_projectiles().len(),
                usize::from(frame >= 64),
                "release on frame 64 after three waiting updates"
            );
            assert_eq!(
                object.model_condition_bits & (opening | closing),
                if frame < 71 { opening } else { closing },
                "door on frame {frame}"
            );
            assert_eq!(object.is_alive(), frame < 71, "carrier on frame {frame}");
            assert!(
                !object.status.on_die_started,
                "HeadOffMap destroys the carrier directly"
            );
        }
    }
    restored.process_destroy_list();
    assert!(!restored.objects.contains_key(&carrier));
    assert_eq!(restored.combat_system.get_projectiles().len(), 1);
}

fn flat_recovery_map(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("ArtilleryRecovery.map");
    let mut map = game_engine::common::system::DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 4);
    for field in [41, 41, 2, 1, 34, 36, 1681] {
        map.write_int(field);
    }
    for _ in 0..1681 {
        map.write_byte(32);
    }
    map.close_data_chunk();
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    path
}

#[test]
fn authored_artillery_recovers_at_the_active_edge_after_loading_off_map() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_artillery_recovers_at_the_active_edge_after_loading_off_map",
        || {
            use crate::save_load::SaveFileManager;
            let temp = tempfile::tempdir().unwrap();
            let path = flat_recovery_map(temp.path());

            let prepare = |path: &str| {
                let mut world = world();
                assert!(world.load_map(path));
                world.add_player(Player::new(1, Team::China, "Owner", true));
                world.add_player(Player::new(2, Team::GLA, "Target", false));
                world
            };
            let mut source = prepare(path.to_str().unwrap());
            let aim = Vec3::new(80.0, 0.0, 82.0);
            let (_, carrier, _) = activate(&mut source, aim);
            let mut peer = prepare(path.to_str().unwrap());
            let (_, peer_carrier, _) = activate(&mut peer, aim);
            assert_eq!(peer_carrier, carrier);
            let peer_position = peer.objects[&carrier].get_position();

            // Active edge x=340, original full CKMP border edge x=390.
            // The first failed drop is still inside the full border.
            source.frame = 60;
            source.set_fixture_object_position(carrier, aim + Vec3::new(40.0, 300.0, 0.0));
            source.update_artillery_barrage_flights().unwrap();
            source.frame = 61;
            source.set_fixture_object_position(carrier, Vec3::new(370.0, 300.0, 82.0));
            source.update_artillery_barrage_flights().unwrap();
            assert!(!source.objects[&carrier].drawable_hidden);
            assert!(source.partition_manager.is_registered(carrier.0));
            assert!(source.combat_system.get_projectiles().is_empty());

            // Region3D::isInRegionNoZ excludes points on the full border.
            source.frame = 62;
            let outside = Vec3::new(390.0, 300.0, 82.0);
            source.set_fixture_object_position(carrier, outside);
            source.apply_player_drawable_selection(1, &[carrier], true);
            source.objects.get_mut(&carrier).unwrap().physics_accel = Vec3::Y;
            source.update_artillery_barrage_flights().unwrap();
            let carrier_object = &source.objects[&carrier];
            assert!(
                carrier_object.drawable_hidden,
                "missed delivery withdraws off-map carrier"
            );
            assert!(!carrier_object.selected);
            assert!(!source.players[&1].selected_objects.contains(&carrier));
            assert!(!source.partition_manager.is_registered(carrier.0));
            assert_eq!(carrier_object.physics_accel, Vec3::ZERO);
            assert_eq!(carrier_object.movement.velocity, Vec3::ZERO);
            assert_eq!(carrier_object.get_position(), outside);
            // ObjectCreationList.cpp pushes the launch origin back by
            // 1.5 times DeliveryDistance along the ray from aim to corner.
            let corner = glam::Vec2::new(340.0, 360.0);
            let launch = corner + (corner - glam::Vec2::new(aim.x, aim.z)).normalize() * 150.0;
            let actual = glam::Vec2::new(peer_position.x, peer_position.z);
            assert!(
                actual.abs_diff_eq(launch, 1e-3),
                "authored launch uses the active far corner: {actual:?}"
            );

            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves.quick_save(&source).unwrap();
            let decoded = saves.load_game_snapshot("quicksave").unwrap();
            let mut restored = prepare(&decoded.save_info.map_name);
            saves
                .restore_game_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();
            assert!(restored.objects[&carrier].drawable_hidden);
            assert!(!restored.partition_manager.is_registered(carrier.0));

            let foreign = gamelogic::system::engine_stores::new_for_world();
            let _foreign_ai = foreign.ai().write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                // At 200 degrees/sec, ceil(1 / radians-per-logic-frame) = 9.
                for frame in 63..=71 {
                    for world in [&mut source, &mut restored] {
                        world.frame = frame;
                        world.update_artillery_barrage_flights().unwrap();
                        let object = &world.objects[&carrier];
                        assert_eq!(object.drawable_hidden, frame < 71, "frame {frame}");
                        assert_eq!(
                            world.partition_manager.is_registered(carrier.0),
                            frame == 71
                        );
                        assert_eq!(object.movement.velocity, Vec3::ZERO);
                        assert_eq!(
                            object.get_position(),
                            if frame < 71 {
                                outside
                            } else {
                                Vec3::new(340.0, 300.0, 82.0)
                            }
                        );
                        assert!(world.combat_system.get_projectiles().is_empty());
                    }
                }
            });
            for world in [&mut source, &mut restored] {
                assert!(
                    (world.objects[&carrier].get_orientation().abs() - std::f32::consts::PI).abs()
                        < 1e-5
                );
                world
                    .update_owned_partition_with_effects(&mut Default::default())
                    .unwrap();
                assert!(
                    world
                        .partition_manager
                        .ids_in_radius(340.0, 82.0, 1.0)
                        .contains(&carrier.0)
                );
                for frame in 72..180 {
                    world.frame = frame;
                    world.update_artillery_barrage_flights().unwrap();
                    if !world.combat_system.get_projectiles().is_empty() {
                        break;
                    }
                }
                assert_eq!(world.combat_system.get_projectiles().len(), 1);
            }
            assert_eq!(source.frame, restored.frame);
            assert_eq!(peer.objects[&carrier].get_position(), peer_position);
            assert!(!peer.objects[&carrier].drawable_hidden);
            assert!(peer.partition_manager.is_registered(carrier.0));
        },
    );
}

#[test]
fn recovering_artillery_runs_wing_hold_and_restarts_approach_in_the_fixed_tick() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "recovering_artillery_runs_wing_hold_and_restarts_approach_in_the_fixed_tick",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = flat_recovery_map(temp.path());
            let mut world = world_with_carrier(0, 0, Some(true));
            assert!(world.load_map(path.to_str().unwrap()));
            world.add_player(Player::new(1, Team::China, "Owner", true));
            world.add_player(Player::new(2, Team::GLA, "Target", false));
            let aim = Vec3::new(80.0, 0.0, 82.0);
            let (_, carrier, _) = activate(&mut world, aim);
            world.frame = 60;
            world.set_fixture_object_position(carrier, aim + Vec3::new(40.0, 300.0, 0.0));
            world.update_artillery_barrage_flights().unwrap();

            let outside = Vec3::new(390.0, 300.0, 82.0);
            world.set_fixture_object_position(carrier, outside);
            let object = world.objects.get_mut(&carrier).unwrap();
            object.set_orientation_with_partition_owner(0.0, &mut world.partition_manager);
            object.movement.velocity = Vec3::X * 9.0;

            world.frame = 61;
            let result = world.update_simulation(SECONDS_PER_LOGICFRAME_REAL);
            assert_eq!(format!("{result:?}"), "Advanced");
            let object = &world.objects[&carrier];
            assert!(object.drawable_hidden);
            assert!(!world.partition_manager.is_registered(carrier.0));
            assert_eq!(object.maintain_pos, Some(outside));
            let max_turn = 200.0_f32.to_radians() * SECONDS_PER_LOGICFRAME_REAL;
            assert!((object.get_orientation() + max_turn).abs() < 1e-5);
            let velocity = object.movement.velocity;
            let first_x = 80.0 / 900.0;
            assert!((velocity.x - first_x).abs() < 1e-5, "{velocity:?}");
            assert!(velocity.z.abs() < 1e-5);
            assert!(object.physics_previous_accel.y > 0.0);
            assert!((object.get_position().x - outside.x - first_x).abs() < 1e-4);

            let mut saves = crate::save_load::SaveFileManager::with_save_directory(
                temp.path().join("wing-saves"),
            );
            saves.init().unwrap();
            saves.quick_save(&world).unwrap();
            let decoded = saves.load_game_snapshot("quicksave").unwrap();
            let mut restored = world_with_carrier(0, 0, Some(true));
            assert!(restored.load_map(&decoded.save_info.map_name));
            restored.add_player(Player::new(1, Team::China, "Owner", true));
            restored.add_player(Player::new(2, Team::GLA, "Target", false));
            saves
                .restore_game_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();
            let loaded = &restored.objects[&carrier];
            assert_eq!(loaded.get_position(), object.get_position());
            assert_eq!(loaded.movement.velocity, velocity);
            assert_eq!(loaded.maintain_pos, Some(outside));

            // Recovery at 61 uses ceil(1 / turn rate per frame) = 9.
            for frame in 62..=70 {
                for candidate in [&mut world, &mut restored] {
                    candidate.frame = frame;
                    let result = candidate.update_simulation(SECONDS_PER_LOGICFRAME_REAL);
                    assert_eq!(format!("{result:?}"), "Advanced");
                    let object = &candidate.objects[&carrier];
                    assert_eq!(object.drawable_hidden, frame < 70);
                    assert_eq!(
                        candidate.partition_manager.is_registered(carrier.0),
                        frame == 70
                    );
                    assert!(candidate.combat_system.get_projectiles().is_empty());
                }
                let live = &world.objects[&carrier];
                let loaded = &restored.objects[&carrier];
                assert_eq!(loaded.get_position(), live.get_position(), "frame {frame}");
                assert_eq!(
                    loaded.get_orientation(),
                    live.get_orientation(),
                    "frame {frame}"
                );
                assert_eq!(
                    loaded.movement.velocity, live.movement.velocity,
                    "frame {frame}"
                );
            }
            let object = &world.objects[&carrier];
            assert!(!object.maintain_pos_valid);
            assert!(object.get_position().x < 340.0);
            let horizontal_speed = object.movement.velocity.with_y(0.0).length();
            assert!((horizontal_speed - first_x).abs() < 1e-5);
        },
    );
}

#[test]
fn authored_artillery_carrier_has_one_physics_step_with_the_authored_launch_force() {
    let aim = Vec3::new(80.0, 0.0, 50.0);
    let mut boosted = world_with_carrier(0, 0, Some(true));
    let (_, carrier, _) = activate(&mut boosted, aim);
    let mut unboosted = world_with_carrier(0, 0, Some(false));
    let (_, peer, _) = activate(&mut unboosted, aim);
    assert_eq!(carrier, peer);

    let pose = boosted.objects[&carrier].get_position();
    assert_eq!(pose.y, 500.0, "post-creation preferred height");
    assert_eq!(unboosted.objects[&peer].get_position(), pose);
    let direction = (aim - pose).with_y(0.0).normalize();
    let launch_acceleration = boosted.objects[&carrier].physics_accel;
    let peer_acceleration = unboosted.objects[&peer].physics_accel;
    assert_eq!(boosted.objects[&carrier].physics_get_mass(), 2.0);
    assert_eq!(boosted.objects[&carrier].movement.velocity, Vec3::ZERO);
    assert_eq!(peer_acceleration, Vec3::ZERO);
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&boosted).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let mut restored = world_with_carrier(0, 0, Some(true));
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.objects[&carrier].physics_accel,
        launch_acceleration
    );

    for world in [&mut boosted, &mut unboosted, &mut restored] {
        let result = world.update_simulation(SECONDS_PER_LOGICFRAME_REAL);
        assert_eq!(format!("{result:?}"), "Advanced");
        assert!(world.combat_system.get_projectiles().is_empty());
    }

    let boosted_delta = (boosted.objects[&carrier].get_position() - pose).with_y(0.0);
    let unboosted_delta = (unboosted.objects[&peer].get_position() - pose).with_y(0.0);
    assert!(
        launch_acceleration.abs_diff_eq(direction * 5.0, 1e-4),
        "StartAtMaxSpeed is an accumulated motive impulse; got {launch_acceleration:?}, first fixed step travelled {boosted_delta:?}"
    );
    let drive_accel = 80.0 / 900.0;
    assert!(
        boosted_delta.abs_diff_eq(direction * (5.0 + drive_accel), 1e-3),
        "{boosted_delta:?}"
    );
    assert!(
        unboosted_delta.abs_diff_eq(direction * drive_accel, 1e-3),
        "{unboosted_delta:?}"
    );
    assert_eq!(
        restored.objects[&carrier].get_position(),
        boosted.objects[&carrier].get_position()
    );
    assert_eq!(
        restored.objects[&carrier].movement.velocity,
        boosted.objects[&carrier].movement.velocity
    );

    // On a turn, the shared pivot rotates the carrier while this frame's
    // motive force still follows the heading on entry to the locomotor.
    for world in [&mut boosted, &mut restored] {
        let object = world.objects.get_mut(&carrier).unwrap();
        let position = object.get_position();
        let heading = object.get_orientation();
        let velocity = object.movement.velocity;
        let destination = position + Vec3::new(0.0, 0.0, 700.0);
        object.artillery_barrage_transport.as_mut().unwrap().target = destination;
        world.update_artillery_barrage_flights().unwrap();
        let object = &world.objects[&carrier];
        assert_ne!(object.get_orientation(), heading);
        assert_eq!(object.get_position(), position);
        assert_eq!(object.movement.velocity, velocity);
        assert!(object.physics_accel.x * direction.x < 0.0);
        assert!(object.physics_accel.z.abs() < 1e-5);
    }
    assert_eq!(
        restored.objects[&carrier].get_orientation(),
        boosted.objects[&carrier].get_orientation()
    );
    assert_eq!(
        restored.objects[&carrier].physics_accel,
        boosted.objects[&carrier].physics_accel
    );
}

#[test]
fn visible_artillery_shell_inherits_a_pending_motive_impulse() {
    let aim = Vec3::new(80.0, 0.0, 50.0);
    let mut world = world_with_carrier(0, 0, Some(true));
    let (_, carrier, _) = activate(&mut world, aim);
    let result = world.update_simulation(SECONDS_PER_LOGICFRAME_REAL);
    assert_eq!(format!("{result:?}"), "Advanced");

    world.set_fixture_object_position(carrier, aim + Vec3::new(40.0, 500.0, 0.0));
    world.frame = 60;
    world.update_artillery_barrage_flights().unwrap();
    assert!(world.combat_system.get_projectiles().is_empty());
    let object = &world.objects[&carrier];
    let inherited = object.movement.velocity;
    assert!(inherited.with_y(0.0).length() > 5.0);
    let launch_position = object.get_position() - inherited;
    let direction = object.get_transform_matrix().x_axis.truncate().normalize();

    world.frame = 61;
    world.update_artillery_barrage_flights().unwrap();
    let projectiles = world.combat_system.get_projectiles();
    assert_eq!(projectiles.len(), 1);
    let (&id, projectile) = projectiles.iter().next().unwrap();
    let shell = &world.objects[&id];
    assert!(projectile.position.abs_diff_eq(launch_position, 1e-4));
    assert!(shell.get_position().abs_diff_eq(launch_position, 1e-4));

    // DeliverPayloadAIUpdate queues the inherited velocity before MissileAI
    // queues InitialVelocity. Physics has run neither force yet.
    assert_eq!(shell.movement.velocity, Vec3::ZERO);
    assert_eq!(projectile.velocity, Vec3::ZERO);
    let expected = inherited / SECONDS_PER_LOGICFRAME_REAL + direction * 60.0;
    assert!(shell.physics_accel.abs_diff_eq(expected, 1e-4));

    let result = world.update_simulation(SECONDS_PER_LOGICFRAME_REAL);
    assert_eq!(format!("{result:?}"), "Advanced");
    let shell = &world.objects[&id];
    let travelled = (shell.get_position() - launch_position).with_y(0.0);
    let committed = shell.physics_previous_accel.with_y(0.0);
    // With no preexisting velocity, the first horizontal displacement is
    // exactly the accumulated motive force, including any first-tick steering.
    assert!(travelled.abs_diff_eq(committed * SECONDS_PER_LOGICFRAME_REAL, 1e-4));
    assert!(travelled.dot(expected.with_y(0.0).normalize()) > 6.0);
}

#[test]
#[ignore = "requires mounted retail Zero Hour assets"]
fn retail_artillery_releases_authored_shells_and_damages_the_target() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let assets = crate::parallel_harness::SharedSkirmishAssets::load_retail().unwrap();
            let mut world = assets.prepare_skirmish(1234).unwrap();
            world.with_own_services_mut(|world| {
                world.update();
                let command_center = |name| {
                    world
                        .objects
                        .registered_values()
                        .find(|object| object.get_template().name == name)
                        .expect("Lone Eagle starting command center")
                };
                let start = command_center("AmericaCommandCenter").get_position();
                let victim = command_center("GLACommandCenter");
                let (victim_id, aim, initial_health) =
                    (victim.id, victim.get_position(), victim.health.current);
                let player = crate::parallel_harness::HumanOrders::PLAYER;
                let caster = world
                    .create_object_for_player(
                        "ChinaCommandCenter",
                        player,
                        start + Vec3::new(100.0, 0.0, 100.0),
                    )
                    .unwrap();
                world
                    .get_player_mut(player)
                    .unwrap()
                    .unlock_science("SCIENCE_ArtilleryBarrage1");
                world
                    .queue_special_power_strike(&SpecialPowerType::Artillery, caster, aim)
                    .unwrap();

                let carriers: std::collections::HashSet<_> = world
                    .objects
                    .registered()
                    .filter(|(_, object)| object.get_template().name == "ChinaArtilleryCannon")
                    .map(|(&id, object)| {
                        assert!(
                            object
                                .artillery_barrage_transport
                                .as_ref()
                                .unwrap()
                                .uses_wing_physics()
                        );
                        let position = object.get_position();
                        let ground = world.raw_terrain_ground_height_at(position);
                        assert!((position.y - ground - 500.0).abs() < 1e-4);
                        assert!((object.physics_accel.with_y(0.0).length() - 5.0).abs() < 1e-4);
                        id
                    })
                    .collect();
                assert_eq!(carriers.len(), 12);

                let mut saw_shell = false;
                let damaged = (0..900).any(|_| {
                    world.update();
                    if !saw_shell
                        && let Some(shell) = world
                            .combat_system
                            .get_projectiles()
                            .values()
                            .find(|shell| carriers.contains(&shell.shooter_id))
                    {
                        assert_eq!(
                            world.objects[&shell.id].get_template().name,
                            "ChinaArtilleryBarrageShell"
                        );
                        saw_shell = true;
                    }
                    world
                        .objects
                        .get(&victim_id)
                        .is_none_or(|victim| victim.health.current < initial_health)
                });
                assert!(
                    saw_shell,
                    "the retail formation did not drop an authored shell"
                );
                assert!(damaged, "the retail shells did not damage the target");
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
