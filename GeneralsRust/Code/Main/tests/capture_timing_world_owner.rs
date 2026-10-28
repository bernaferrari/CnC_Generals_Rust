use game_engine::common::random_value::SessionRandom;
use generals_main::game_logic::thing::{CapturePowerKind, ThingTemplate};
use generals_main::game_logic::{
    AIState, GameLogic, GameMode, KindOf, ObjectAudio, ObjectId, Player, SessionAudio, Team,
};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const CAPTOR: &str = "ReceivingTimingRanger";
const TARGET: &str = "ReceivingTimingBuilding";

fn world(seed: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.start_new_game(GameMode::SinglePlayer);
    game.add_player(Player::new(1, Team::USA, "Owner", true));
    game.add_player(Player::new(2, Team::GLA, "Target", false));

    let mut ranger = ThingTemplate::new(CAPTOR);
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    ranger.capture_power = CapturePowerKind::Ranger;
    ranger.capture_start_ability_range = Some(5.0);
    ranger.capture_unpack_time_ms = Some(3_000);
    ranger.capture_preparation_time_ms = Some(20_000);
    ranger.capture_pack_time_ms = Some(2_000);
    ranger.capture_pack_unpack_variation_factor = 0.5;
    game.templates.insert(CAPTOR.into(), ranger);

    let mut building = ThingTemplate::new(TARGET);
    building
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    building.capturable = true;
    game.templates.insert(TARGET.into(), building);

    let captor = game
        .create_object_for_player(CAPTOR, 1, Vec3::new(1.0, 0.0, 1.0))
        .expect("receiving ranger");
    let target = game
        .create_object_for_player(TARGET, 2, Vec3::new(0.0, 0.0, 0.0))
        .expect("receiving building");

    // Admit the match RNG using the same public snapshot path as replay/load.
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&game).unwrap();
    let random = SessionRandom::default();
    random.seed_random(seed);
    snapshot.random_seed = u64::from(seed);
    snapshot.logic_rng_seed_words = random.seed_words();
    builder.restore_from_snapshot(&snapshot, &mut game).unwrap();
    (game, captor, target)
}

fn start_unpacking(game: &mut GameLogic, captor: ObjectId, target: ObjectId) -> f32 {
    let mut audio = SessionAudio::default();
    let object = game.host_object_mut(captor).expect("captor");
    object.target = Some(target);
    object.set_ai_state(AIState::Capturing, &mut ObjectAudio::queue(&mut audio));
    assert_eq!(game.update_with_dt(1.0 / 30.0).steps_run, 1);
    let object = game.host_object(captor).expect("captor survives");
    let channel = object.capture_channel.expect("capture unpack channel");
    assert_eq!(
        channel.phase,
        generals_main::game_logic::CaptureChannelPhase::Unpacking
    );
    channel.remaining_seconds
}

#[test]
fn capture_unpack_timing_isolated_from_a_peer_session() {
    let (mut isolated, isolated_captor, isolated_target) = world(0xA11C_E001);
    let (mut receiving, receiving_captor, receiving_target) = world(0xA11C_E001);
    let (mut peer, peer_captor, peer_target) = world(0xB22D_F002);
    assert_eq!(
        (isolated_captor, isolated_target),
        (receiving_captor, receiving_target)
    );
    assert_eq!(
        (receiving_captor, receiving_target),
        (peer_captor, peer_target)
    );

    let expected = start_unpacking(&mut isolated, isolated_captor, isolated_target);
    start_unpacking(&mut peer, peer_captor, peer_target);
    let actual = start_unpacking(&mut receiving, receiving_captor, receiving_target);

    assert_eq!(
        actual, expected,
        "a peer session cannot perturb the receiving capture operation"
    );
}
