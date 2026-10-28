use generals_main::game_logic::{GameLogic, ObjectId, Player, Team, VeterancyLevel};
use glam::Vec3;

const UNIT: &str = "ReceivingXpUpgradeUnit";
const VICTIM: &str = "ReceivingXpUpgradeVictim";

fn world(marker: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "VeterancyXpUpgradeOwner.ini".into(),
        format!(
            "Object {UNIT}\n IsTrainable = Yes\n KindOf = INFANTRY SELECTABLE\n ExperienceRequired = 0 100 250 450\n Body = ActiveBody XpBody\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StatusBitsUpgrade VeteranRankMarker\n TriggeredBy = Upgrade_Veterancy_VETERAN\n StatusToSet = {marker}\n End\nEnd\nObject {VICTIM}\n IsTrainable = Yes\n KindOf = STRUCTURE SELECTABLE SCORE\n ExperienceValue = 125 125 125 125\n Body = ActiveBody VictimBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ),
    )])
    .expect("receiving Object INI admitted");
    let mut owner = Player::new(1, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    game.add_player(owner);
    game.add_player(enemy);
    let id = game
        .create_object_for_player(UNIT, 1, Vec3::new(100.0, 0.0, 100.0))
        .expect("trainable receiving unit");
    let victim = game
        .create_object_for_player(VICTIM, 2, Vec3::new(104.0, 0.0, 100.0))
        .expect("receiving kill target");
    (game, id, victim)
}

#[test]
fn kill_reward_dispatches_authored_status_upgrade_in_each_receiving_world() {
    let (mut first, first_id, first_victim) = world("BOOBY_TRAPPED");
    let (mut peer, peer_id, peer_victim) = world("CAN_STEALTH");
    assert_eq!(
        first_id, peer_id,
        "separate worlds reuse the same object ID"
    );

    // Take the public receiving-world damage path through scoreTheKill. The
    // victim awards 125 XP, crossing the Veteran threshold at 100.
    assert_eq!(
        first.apply_host_hive_damage_from(
            first_victim,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(first_id),
        ),
        (true, false)
    );
    let promoted = first.host_object(first_id).expect("first world unit");
    assert_eq!(promoted.experience.level, VeterancyLevel::Veteran);
    assert_eq!(promoted.experience.current, 125.0);
    assert!(promoted.has_object_status_bit("BOOBY_TRAPPED"));
    assert!(!promoted.has_object_status_bit("CAN_STEALTH"));

    let untouched = peer.host_object(peer_id).expect("peer unit");
    assert_eq!(untouched.experience.level, VeterancyLevel::Rookie);
    assert_eq!(untouched.experience.current, 0.0);
    assert!(!untouched.has_object_status_bit("BOOBY_TRAPPED"));
    assert!(!untouched.has_object_status_bit("CAN_STEALTH"));

    assert_eq!(
        peer.apply_host_hive_damage_from(
            peer_victim,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(peer_id),
        ),
        (true, false)
    );
    let peer_promoted = peer.host_object(peer_id).expect("peer promoted unit");
    assert_eq!(peer_promoted.experience.level, VeterancyLevel::Veteran);
    assert_eq!(peer_promoted.experience.current, 125.0);
    assert!(peer_promoted.has_object_status_bit("CAN_STEALTH"));
    assert!(!peer_promoted.has_object_status_bit("BOOBY_TRAPPED"));
}
