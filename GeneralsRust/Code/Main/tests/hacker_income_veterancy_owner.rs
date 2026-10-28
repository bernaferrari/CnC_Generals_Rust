//! The scheduled Hacker cash producer must award its authored XP and promotion
//! through the receiving GameLogic when another same-ID world exists.

use generals_main::game_logic::{GameLogic, GameMode, Player, Team, VeterancyLevel};
use glam::Vec3;

const HACKER: &str = "ReceivingWorldHacker";
const PLAYER: u32 = 42;
const DT: f32 = 1.0 / 30.0;

fn world(veteran_health_bonus: u32, cash: u32) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut game = GameLogic::new();
    game.admit_combat_bonus_ini_sources([format!(
        "GameData\n HealthBonus_Veteran = {veteran_health_bonus}%\nEnd\n"
    )])
    .unwrap();
    game.start_new_game(GameMode::SinglePlayer);
    game.add_player(Player::new(PLAYER, Team::China, "Hacker owner", true));

    let marker = if veteran_health_bonus == 130 {
        "BOOBY_TRAPPED"
    } else {
        "CAN_STEALTH"
    };
    game.admit_object_ini_sources([(
        "HackerVeterancyOwner.ini".into(),
        format!(
            "Object {HACKER}\n IsTrainable = Yes\n KindOf = INFANTRY MONEY_HACKER\n ExperienceRequired = 0 10 100 200\n Body = ActiveBody HackerBody\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = HackInternetAIUpdate HackerAI\n UnpackTime = 0\n PackTime = 0\n CashUpdateDelay = 0\n CashUpdateDelayFast = 0\n RegularCashAmount = {cash}\n VeteranCashAmount = {cash}\n EliteCashAmount = {cash}\n HeroicCashAmount = {cash}\n XpPerCashUpdate = 10\n PackUnpackVariationFactor = 0\n End\n Behavior = StatusBitsUpgrade RankMarker\n TriggeredBy = Upgrade_Veterancy_VETERAN\n StatusToSet = {marker}\n End\nEnd\n"
        ),
    )])
    .expect("receiving hacker and upgrade definitions");
    let id = game
        .create_object_for_player(HACKER, PLAYER, Vec3::ZERO)
        .expect("hacker admitted into receiving world");
    (game, id)
}

#[test]
fn hacker_cash_tick_awards_xp_and_veterancy_in_the_receiving_world() {
    let (mut receiver, receiver_id) = world(130, 5);
    let (mut peer, peer_id) = world(180, 9);
    assert_eq!(
        receiver_id, peer_id,
        "the two worlds intentionally share ID"
    );

    let receiver_start_cash = receiver.get_player(PLAYER).unwrap().resources.supplies;
    let peer_start_cash = peer.get_player(PLAYER).unwrap().resources.supplies;

    assert!(
        receiver
            .start_hacker_internet_hack(receiver_id)
            .expect("field hack starts")
    );
    assert!(
        peer.start_hacker_internet_hack(peer_id)
            .expect("peer field hack starts")
    );

    // The authored zero delay fires on the next scheduler update after its
    // decrement frame; run both worlds independently through that boundary.
    for _ in 0..2 {
        let tick = receiver.update_with_dt(DT);
        assert_eq!(tick.steps_run, 1);
    }
    let receiving_hacker = receiver.host_object(receiver_id).expect("receiver hacker");
    assert_eq!(receiving_hacker.experience.current, 10.0);
    assert_eq!(receiving_hacker.experience.level, VeterancyLevel::Veteran);
    assert_eq!(receiving_hacker.health.maximum, 130.0);
    assert!(receiving_hacker.has_object_status_bit("BOOBY_TRAPPED"));
    assert_eq!(
        receiver.get_player(PLAYER).unwrap().resources.supplies,
        receiver_start_cash + 5,
        "C++ HackInternetAIUpdate deposits its authored cash before XP"
    );
    let untouched_peer = peer.host_object(peer_id).expect("peer hacker");
    assert_eq!(untouched_peer.experience.current, 0.0);
    assert_eq!(untouched_peer.health.maximum, 100.0);
    assert!(!untouched_peer.has_object_status_bit("BOOBY_TRAPPED"));
    assert_eq!(
        peer.get_player(PLAYER).unwrap().resources.supplies,
        peer_start_cash
    );

    for _ in 0..2 {
        let tick = peer.update_with_dt(DT);
        assert_eq!(tick.steps_run, 1);
    }
    let peer_hacker = peer.host_object(peer_id).expect("peer hacker");
    assert_eq!(peer_hacker.experience.current, 10.0);
    assert_eq!(peer_hacker.experience.level, VeterancyLevel::Veteran);
    assert_eq!(peer_hacker.health.maximum, 180.0);
    assert!(peer_hacker.has_object_status_bit("CAN_STEALTH"));
    assert_eq!(
        peer.get_player(PLAYER).unwrap().resources.supplies,
        peer_start_cash + 9
    );
}
