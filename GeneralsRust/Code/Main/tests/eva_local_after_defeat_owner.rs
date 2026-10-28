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

#[test]
fn defeated_local_owner_still_gets_base_under_attack_eva() {
    use glam::Vec3;

    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("EVA.ini".into(), "Object AmericaCommandCenter\n KindOf = STRUCTURE SELECTABLE COMMANDCENTER MP_COUNT_FOR_VICTORY\n Body = ActiveBody Body\n MaxHealth = 5000\n InitialHealth = 5000\n End\nEnd\n".into())]).unwrap();
        world.start_new_game(generals_main::game_logic::GameMode::SinglePlayer);
        let mut local = Player::new(7, Team::USA, "Local", true);
        local.is_local = true;
        world.add_player(local);
        world
    }

    let mut defeated = world();
    let mut living_peer = world();
    let defeated_cc = defeated
        .create_object_for_player("AmericaCommandCenter", 7, Vec3::new(10.0, 0.0, 20.0))
        .expect("defeated local command center");
    let peer_cc = living_peer
        .create_object_for_player("AmericaCommandCenter", 7, Vec3::new(10.0, 0.0, 20.0))
        .expect("peer command center");
    // Construction redirects inactive players to Neutral in C++; defeat the
    // selected owner after the object has already been admitted instead.
    defeated.get_player_mut(7).unwrap().is_alive = false;

    assert!(defeated.try_under_attack_event(defeated_cc));
    assert!(living_peer.try_under_attack_event(peer_cc));
    assert!(defeated.honesty_eva_base_under_attack_ok());
    assert!(living_peer.honesty_eva_base_under_attack_ok());
}

#[test]
fn defeated_local_owner_gets_overcharge_exhaustion_only_in_its_world() {
    use glam::Vec3;

    fn world(local_is_owner: bool) -> (GameLogic, generals_main::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("Overcharge.ini".into(), "Object OwnerOvercharger\n KindOf = STRUCTURE SELECTABLE\n EnergyBonus = 5\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = OverchargeBehavior Overcharge\n HealthPercentToDrainPerSecond = 50%\n NotAllowedWhenHealthBelowPercent = 90%\n End\nEnd\n".into())]).unwrap();
        world.start_new_game(generals_main::game_logic::GameMode::SinglePlayer);
        let mut owner = Player::new(7, Team::China, "Owner", local_is_owner);
        owner.is_local = local_is_owner;
        world.add_player(owner);
        if !local_is_owner {
            let mut local = Player::new(8, Team::USA, "Local", true);
            local.is_local = true;
            world.add_player(local);
        }

        let id = world
            .create_object_for_player("OwnerOvercharger", 7, Vec3::new(30.0, 0.0, 40.0))
            .expect("admitted overcharger");
        world.get_player_mut(7).unwrap().is_alive = false;
        (world, id)
    }

    let (mut local_owner_world, local_owner_plant) = world(true);
    let (mut peer_world, peer_owner_plant) = world(false);
    assert!(local_owner_world.toggle_overcharge_object(local_owner_plant));
    assert!(peer_world.toggle_overcharge_object(peer_owner_plant));

    local_owner_world.update_overcharge_drain(1.0);
    peer_world.update_overcharge_drain(1.0);

    assert_eq!(
        local_owner_world.last_radar_event_position(),
        Some(Vec3::new(30.0, 0.0, 40.0)),
        "the retained local Player identity receives the exhaustion marker after defeat"
    );
    assert_eq!(
        peer_world.last_radar_event_position(),
        None,
        "the same defeated owner is not local in its independent peer world"
    );
}
