use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::special_power_strikes::HostStrikePhase;
use crate::save_load::SnapshotBuilder;

fn world() -> GameLogic {
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
            .into(),
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
        .admit_object_creation_list_sources(["
ObjectCreationList TestArtilleryDelivery
 DeliverPayload
  Transport = TestArtilleryCarrier
  FormationSize = 1
  StartAtPreferredHeight = No
  DeliveryDistance = 100
  VisiblePayloadTemplateName = TestArtilleryShell
  VisiblePayloadWeaponTemplate = TestArtilleryWarhead
  VisibleSubObjectBaseName = Bomb
  VisibleNumBones = 1
  VisibleItemsDroppedPerInterval = 1
  SelfDestructObject = Yes
 End
End
"
        .into()])
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
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
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
}
