use generals_main::ai::AIDifficulty;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

const OWNER: u32 = 77;
const UNIT: &str = "ReceivingSoloHealthUnit";

fn world(
    target_difficulty: AIDifficulty,
    other_difficulty: AIDifficulty,
    easy_factor: u32,
    hard_factor: u32,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world
        .admit_combat_bonus_ini_sources([format!(
            "GameData\nAISoloPlayerHealthBonus_Easy = {easy_factor}%\nAISoloPlayerHealthBonus_Hard = {hard_factor}%\nEnd\n"
        )])
        .expect("receiving GameData should admit AI factors");
    world
        .admit_object_ini_sources([(
            "SoloHealthOwner.ini".into(),
            format!(
                "Object {UNIT}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
            ),
        )])
        .expect("receiving object content should admit the health probe");
    world.add_player(Player::new(OWNER, Team::China, "Target AI", false));
    world.add_ai_opponent(OWNER, Team::China, target_difficulty);
    world.add_ai_opponent(78, Team::GLA, other_difficulty);
    let id = world
        .create_object_for_player(UNIT, OWNER, Vec3::ZERO)
        .expect("probe should be created through the receiving factory");
    (world, id)
}

#[test]
fn ai_factor_uses_receiving_owner_difficulty_across_same_id_worlds() {
    // Both worlds have a Hard dominant/session difficulty, but the first
    // world's receiving actor is Easy. This distinguishes per-Player AI
    // difficulty from the old session-wide lookup.
    let (easy_owner, easy_id) = world(AIDifficulty::Easy, AIDifficulty::Hard, 125, 75);
    let (hard_owner, hard_id) = world(AIDifficulty::Hard, AIDifficulty::Easy, 60, 150);
    assert_eq!(easy_id, hard_id);
    assert_eq!(easy_owner.get_difficulty(), AIDifficulty::Hard);
    assert_eq!(hard_owner.get_difficulty(), AIDifficulty::Hard);
    assert_eq!(
        easy_owner.host_ai_difficulty(OWNER),
        Some(AIDifficulty::Easy)
    );
    assert_eq!(
        hard_owner.host_ai_difficulty(OWNER),
        Some(AIDifficulty::Hard)
    );

    assert_eq!(
        easy_owner.host_object(easy_id).unwrap().health.maximum,
        125.0
    );
    assert_eq!(
        hard_owner.host_object(hard_id).unwrap().health.maximum,
        150.0
    );
    assert_eq!(
        easy_owner.host_object(easy_id).unwrap().health.maximum,
        125.0
    );
}
