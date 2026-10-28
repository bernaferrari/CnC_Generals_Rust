//! EVA keeps using the selected local Player after its defeat flag changes.
use generals_main::game_logic::{GameLogic, Player, Team};

fn world_with_local_player(id: u32, alive: bool) -> GameLogic {
    let mut world = GameLogic::new();
    let mut local = Player::new(id, Team::USA, "Local", true);
    local.is_local = true;
    local.is_alive = alive;
    world.add_player(local);
    world
}

#[test]
fn defeated_local_player_remains_the_receiving_eva_identity() {
    let mut defeated = world_with_local_player(7, false);
    let mut living_peer = world_with_local_player(7, true);

    assert_eq!(defeated.local_player_id(), Some(7));
    assert_eq!(living_peer.local_player_id(), Some(7));

    // GameLogicDispatch.cpp:1632 tests the placer's relationship to
    // PlayerList::getLocalPlayer() before requesting EVA_BeaconDetected.
    // Player::isLocalPlayer compares against that stable selected slot and
    // does not change when the player becomes inactive.
    defeated.try_eva_beacon_detected(7);
    assert!(defeated.honesty_eva_beacon_detected_ok());
    assert!(
        !living_peer.honesty_eva_beacon_detected_ok(),
        "an event in one session must not mutate the same-ID peer"
    );

    living_peer.try_eva_beacon_detected(7);
    assert!(living_peer.honesty_eva_beacon_detected_ok());
    assert!(defeated.honesty_eva_beacon_detected_ok());
}
