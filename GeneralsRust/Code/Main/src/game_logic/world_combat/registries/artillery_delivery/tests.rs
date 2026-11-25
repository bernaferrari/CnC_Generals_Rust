use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::special_power_strikes::HostStrikePhase;
use crate::save_load::snapshot::{Snapshot, WorldSnapshot};
use crate::save_load::{SnapshotBuilder, Xfer, XferLoad, XferSave};

fn world() -> GameLogic {
    world_with_delays(0, 0)
}

fn world_with_delays(door_delay_ms: u32, drop_delay_ms: u32) -> GameLogic {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    let mut world = GameLogic::new();
    world.override_world_size(1000.0, 1000.0);
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors
        .apply_source(
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
End",
        )
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
  StartAtPreferredHeight = No
  DeliveryDistance = 100
  DropDelay = {drop_delay_ms}
  VisiblePayloadTemplateName = TestArtilleryShell
  VisiblePayloadWeaponTemplate = TestArtilleryWarhead
  VisibleSubObjectBaseName = Bomb
  VisibleNumBones = 1
  VisibleItemsDroppedPerInterval = 1
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
