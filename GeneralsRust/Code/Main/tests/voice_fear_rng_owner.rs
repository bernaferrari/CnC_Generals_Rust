use game_engine::common::random_value::SessionRandom;
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const OBJECT: &str = "ReceivingVoiceFearUnit";

fn seed_with_successful_roll() -> (u32, [u32; 6]) {
    for seed in 1..=1000 {
        let random = SessionRandom::default();
        random.seed_random(seed);
        if random.next_int(0, 99) < 25 {
            return (seed, random.seed_words());
        }
    }
    unreachable!("a successful fear roll exists in the seed space")
}

fn global_state_with_success_then_failure() -> [u32; 6] {
    for seed in 1..=1000 {
        let random = SessionRandom::default();
        random.seed_random(seed);
        let words = random.seed_words();
        game_engine::common::random_value::set_game_logic_random_seed_state(words);
        let first = game_engine::common::random_value::get_game_logic_random_value(0, 99);
        let second = game_engine::common::random_value::get_game_logic_random_value(0, 99);
        if first < 25 && second >= 25 {
            return words;
        }
    }
    unreachable!("the global stream has a success then failure sequence")
}

fn receiving_world(seed: u32) -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "voice-fear-owner.ini".into(),
        format!(
            "Object {OBJECT}\n KindOf = INFANTRY SELECTABLE\n\
             VoiceFear = ReceivingVoiceFear\n\
             Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ),
    )])
    .unwrap();
    game.add_player(Player::new(1, Team::USA, "Owner", true));
    let id = game
        .create_object_for_player(OBJECT, 1, Vec3::new(120., 4., 80.))
        .unwrap();
    game.host_object_mut(id).unwrap().health.current = 40.;

    // The public snapshot API admits the match seed without exposing an RNG
    // mutator solely for integration tests. Capture the object first, then
    // restore the capsule so any admission-time draws precede the test stream.
    let builder = SnapshotBuilder::new();
    let mut baseline = builder.create_world_snapshot(&game).unwrap();
    baseline.random_seed = u64::from(seed);
    let seeded_random = SessionRandom::default();
    seeded_random.seed_random(seed);
    baseline.logic_rng_seed_words = seeded_random.seed_words();
    builder.restore_from_snapshot(&baseline, &mut game).unwrap();
    (game, id)
}

fn hit(game: &mut GameLogic, id: ObjectId) {
    assert_eq!(game.apply_host_damage(id, 20.), (false, false));
}

fn fear_events(game: &mut GameLogic) -> Vec<String> {
    game.queued_audio_events
        .drain_requests()
        .into_iter()
        .map(|request| request.event_type)
        .collect()
}

#[test]
fn interleaved_world_damage_uses_receiving_voice_fear_rng() {
    let (seed_a, _) = seed_with_successful_roll();
    let global_words = global_state_with_success_then_failure();
    let (mut control_a, control_id) = receiving_world(seed_a);
    let (mut a, a_id) = receiving_world(seed_a);
    let (mut b, b_id) = receiving_world(0xB22D_F002);
    assert_eq!(control_id, a_id);
    assert_eq!(a_id, b_id, "worlds reuse the same ObjectId");

    game_engine::common::random_value::set_game_logic_random_seed_state(global_words);
    hit(&mut control_a, control_id);
    assert_eq!(fear_events(&mut control_a), ["ReceivingVoiceFear"]);

    // Peer-only draws and its own Body crossing must leave A's same-ID impact
    // on the same chance result and stream continuation as the isolated run.
    game_engine::common::random_value::set_game_logic_random_seed_state(global_words);
    hit(&mut b, b_id);
    hit(&mut a, a_id);
    assert_eq!(fear_events(&mut a), ["ReceivingVoiceFear"]);
}
