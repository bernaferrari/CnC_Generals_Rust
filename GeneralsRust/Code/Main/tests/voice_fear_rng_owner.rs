use game_engine::common::random_value::SessionRandom;
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const OBJECT: &str = "ReceivingVoiceFearUnit";

/// Seed whose first `next_int(0, 99)` is below VoiceFear's 25 chance. Fixed
/// from RandomValue's own known sequence (no search over seeds).
const SUCCESSFUL_FEAR_ROLL_SEED: u32 = 4;
/// Seed whose first two `next_int(0, 99)` draws are a success (< 25) then a
/// failure (>= 25), used as the shared global stream's distractor state.
const SUCCESS_THEN_FAILURE_SEED: u32 = 5;

fn seed_words_for(seed: u32) -> [u32; 6] {
    let random = SessionRandom::default();
    random.seed_random(seed);
    random.seed_words()
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
    assert_eq!(game.apply_host_damage(id, 20.).unwrap(), (false, false));
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
    let seed_a = SUCCESSFUL_FEAR_ROLL_SEED;
    let global_words = seed_words_for(SUCCESS_THEN_FAILURE_SEED);
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
