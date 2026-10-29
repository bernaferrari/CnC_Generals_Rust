use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use glam::Vec3;

const WORKER: &str = "TestWorker";
const MISCLASSIFIED_WORKER: &str = "GLAInfantryWorker";
const WORKER_SHOES: &str = "Upgrade_GLAWorkerShoes";

fn world() -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(1, Team::GLA, "Worker owner", false));
    world.add_player(Player::new(2, Team::USA, "Capturing owner", false));
    world
        .admit_object_ini_sources([(
            "WorkerShoesVoiceOwner.ini".into(),
            format!(
                "Object {WORKER}\n KindOf = INFANTRY DOZER HARVESTER SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject {MISCLASSIFIED_WORKER}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .expect("admit worker fixture");
    let worker = world
        .create_object_for_player(WORKER, 1, Vec3::ZERO)
        .expect("create worker");
    let mislabeled_worker = world
        .create_object_for_player(MISCLASSIFIED_WORKER, 1, Vec3::ZERO)
        .expect("create worker with incomplete KindOf bits");
    (world, worker, mislabeled_worker)
}

#[test]
fn upgraded_move_voice_uses_only_the_receiving_controlling_players_completion() {
    let (mut first, first_worker, first_mislabeled) = world();
    let (mut peer, peer_worker, peer_mislabeled) = world();
    assert_eq!(first_worker, peer_worker, "worlds reuse Object IDs");
    assert_eq!(first_mislabeled, peer_mislabeled, "worlds reuse Object IDs");

    // C++ CommandXlat checks Player::hasUpgradeComplete only. An Object-level
    // completion/effect must not make this Player-only voice route eligible.
    first
        .host_object_mut(first_worker)
        .expect("first worker")
        .apply_upgrade_tag(WORKER_SHOES);
    assert!(!first.try_queue_picked_voice_move_upgraded(&[first_worker]));
    assert!(!peer.try_queue_picked_voice_move_upgraded(&[peer_worker]));
    assert!(first.queued_audio_events.drain_requests().is_empty());
    assert!(peer.queued_audio_events.drain_requests().is_empty());

    // Neither a lookalike name/completion suffix nor an unlocked-science
    // string represents C++ Player::hasUpgradeComplete for this template.
    first
        .get_player_mut(1)
        .expect("first controller")
        .completed_upgrades
        .insert("Upgrade_GLAWorkerShoesSuffix".into());
    peer.get_player_mut(1)
        .expect("peer controller")
        .unlocked_sciences
        .insert(WORKER_SHOES.into());
    assert!(!first.try_queue_picked_voice_move_upgraded(&[first_worker]));
    assert!(!peer.try_queue_picked_voice_move_upgraded(&[peer_worker]));

    // The same Player completion is observed only in its receiving world.
    first
        .get_player_mut(1)
        .expect("first controller")
        .completed_upgrades
        .insert(WORKER_SHOES.into());
    assert!(first.try_queue_picked_voice_move_upgraded(&[first_worker]));
    assert!(!peer.try_queue_picked_voice_move_upgraded(&[peer_worker]));
    assert!(!first.try_queue_picked_voice_move_upgraded(&[first_mislabeled]));
    assert!(!peer.try_queue_picked_voice_move_upgraded(&[peer_mislabeled]));
    assert_eq!(
        first
            .queued_audio_events
            .drain_requests()
            .iter()
            .map(|request| request.event_type.as_str())
            .collect::<Vec<_>>(),
        ["VoiceMoveUpgraded"]
    );
    assert!(peer.queued_audio_events.drain_requests().is_empty());

    // Capture changes the controlling Player. The old Object effect tag stays
    // on the worker, but must not keep the prior Player's voice entitlement.
    assert!(first.transfer_object_to_player(first_worker, 2));
    assert!(
        first
            .host_object(first_worker)
            .expect("captured worker")
            .has_upgrade_tag(WORKER_SHOES)
    );
    assert!(!first.try_queue_picked_voice_move_upgraded(&[first_worker]));
    assert!(first.queued_audio_events.drain_requests().is_empty());
}
