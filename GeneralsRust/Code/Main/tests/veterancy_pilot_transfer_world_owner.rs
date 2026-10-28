//! Pilot recrew promotes the receiving vehicle with its world's GameData
//! health factors; same-ID sessions and save continuation must stay separate.
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    GameLogic, GameMode, HostHealthEvents, ObjectAudio, ObjectId, Player, Team, VeterancyLevel,
};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

const PLAYER: u32 = 42;
const PILOT: &str = "ReceivingPilot";
const VEHICLE: &str = "ReceivingVehicle";

fn world(veteran_bonus: u32, elite_bonus: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n HealthBonus_Veteran = {veteran_bonus}%\n HealthBonus_Elite = {elite_bonus}%\nEnd\n"
        )])
        .expect("receiving GameData accepted");
    world
        .admit_object_ini_sources([(
            "Veterancy.ini".into(),
            format!(
                "Object {PILOT}\n IsTrainable = Yes\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody PilotBody\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface PilotAI\n End\n Behavior = VeterancyGainCreate PilotRank\n StartingLevel = VETERAN\n End\n Behavior = VeterancyCrateCollide PilotCollide\n IsPilot = Yes\n RequiredKindOf = VEHICLE\n ForbiddenKindOf = DOZER\n EffectRange = 0\n AddsOwnerVeterancy = Yes\n End\nEnd\n\
                 Object {VEHICLE}\n KindOf = VEHICLE SELECTABLE SCORE\n IsTrainable = Yes\n ExperienceRequired = 0 60 150 300\n TransportSlotCount = 3\n Body = ActiveBody VehicleBody\n MaxHealth = 250\n InitialHealth = 250\n End\n Behavior = AIUpdateInterface VehicleAI\n End\nEnd\n"
            ),
        )])
        .expect("receiving pilot and vehicle definitions accepted");
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(PLAYER, Team::USA, "PilotOwner", true));
    world
}

fn spawn_pair(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let pilot = world
        .create_object_for_player(PILOT, PLAYER, Vec3::ZERO)
        .expect("authored veteran pilot");
    let vehicle = world
        .create_object_for_player(VEHICLE, PLAYER, Vec3::ZERO)
        .expect("trainable receiving vehicle");
    assert_eq!(
        world.host_object(pilot).unwrap().experience.level,
        VeterancyLevel::Veteran,
        "VeterancyGainCreate must seed the pilot's rank from admitted Object INI"
    );
    // C++ KillPilot establishes the unmanned state before an Enter order. Keep
    // the setup on the public Object method, then drive the normal command and
    // GameLogic update path that performs pilot veterancy transfer.
    let mut health_events = HostHealthEvents::default();
    let GameLogic {
        objects,
        queued_audio_events,
        ..
    } = world;
    objects
        .get_mut(&vehicle)
        .expect("vehicle")
        .apply_kill_pilot_unmanned(
            &mut health_events,
            &mut ObjectAudio::queue(queued_audio_events),
        );
    assert!(world.can_execute_pilot_recrew(pilot, vehicle));
    (pilot, vehicle)
}

fn enter_vehicle(world: &mut GameLogic, pilot: ObjectId, vehicle: ObjectId) {
    world.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: vehicle },
        player_id: PLAYER,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![pilot],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    for _ in 0..4 {
        world.update_with_dt(1.0 / 30.0);
        if world
            .host_object(vehicle)
            .is_some_and(|object| object.experience.level == VeterancyLevel::Veteran)
        {
            break;
        }
    }
}

#[test]
fn pilot_recrew_uses_receiving_health_bonus_across_same_id_worlds_and_restore() {
    let mut first = world(130, 160);
    let mut peer = world(175, 220);
    let (first_pilot, first_vehicle) = spawn_pair(&mut first);
    let (peer_pilot, peer_vehicle) = spawn_pair(&mut peer);
    assert_eq!((first_pilot, first_vehicle), (peer_pilot, peer_vehicle));

    enter_vehicle(&mut first, first_pilot, first_vehicle);
    enter_vehicle(&mut peer, peer_pilot, peer_vehicle);
    for (owner, vehicle, expected) in [(&first, first_vehicle, 325.0), (&peer, peer_vehicle, 437.5)]
    {
        let object = owner.host_object(vehicle).expect("recrewed vehicle");
        assert_eq!(object.experience.level, VeterancyLevel::Veteran);
        assert!((object.health.maximum - expected).abs() < 0.001);
        assert!((object.health.current - expected).abs() < 0.001);
        assert!(!object.is_unmanned());
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&first)
        .expect("receiving owner snapshot");
    let mut restored = world(130, 160);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore receiving world");

    first.award_experience(first_vehicle, 90.0);
    restored.award_experience(first_vehicle, 90.0);
    peer.award_experience(peer_vehicle, 90.0);
    for (owner, vehicle, expected) in [
        (&first, first_vehicle, 400.0),
        (&restored, first_vehicle, 400.0),
        (&peer, peer_vehicle, 550.0),
    ] {
        let object = owner.host_object(vehicle).expect("continued vehicle");
        assert_eq!(object.experience.level, VeterancyLevel::Elite);
        assert!((object.health.maximum - expected).abs() < 0.001);
        assert!((object.health.current - expected).abs() < 0.001);
    }
}
