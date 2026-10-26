//! RepairDockUpdate::action supplies the dock source to receiving Bodies.
use crate::game_logic::combat::DamageType;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_duration(Some("5000"))
}

fn world_with_duration(duration: Option<&str>) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let duration = duration
        .map(|value| format!("TimeForFullHeal = {value}\n"))
        .unwrap_or_default();
    let ini = format!(
        "Object RepairRecipient\n KindOf = DRONE VEHICLE REPAIR_PAD\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 20\n End\n Behavior = RepairDockUpdate RepairTag\n {duration} End\nEnd\n"
    );
    let mut game = GameLogic::new();
    game.add_player(Player::new(1, Team::USA, "RepairOwner", true));
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([("repair-owner.ini".into(), ini)])
            .unwrap(),
    )
    .unwrap();
    let dock = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    let master = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    let drone = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        game.host_object(dock)
            .unwrap()
            .is_kind_of(crate::game_logic::KindOf::RepairPad)
    );
    game.host_object_mut(drone).unwrap().producer_id = Some(master);
    (game, dock, master, drone)
}

#[test]
fn repair_drone_receiving_body_records_dock_source_and_isolates_peer() {
    let (mut game, dock, master, drone) = world();
    let (peer, peer_dock, peer_master, peer_drone) = world();
    assert_eq!((dock, master, drone), (peer_dock, peer_master, peer_drone));
    game.frame = 23;
    game.heal_slave_drone_with_repair_dock(dock, master);
    let recipient = game.host_object(drone).unwrap();
    assert_eq!(recipient.health.current, 100.0);
    assert_eq!(recipient.previous_health, 20.0);
    assert_eq!(recipient.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(recipient.last_damage_source, Some(dock));
    assert_eq!(recipient.last_healing_timestamp, Some(23));
    assert_eq!(peer.host_object(drone).unwrap().health.current, 20.0);
    assert_eq!(game.host_object(master).unwrap().health.current, 20.0);
}

#[test]
fn seeking_repair_master_records_receiving_body_history_before_drone() {
    use crate::game_logic::AIState;
    let (mut game, dock, master, drone) = world();
    game.frame = 29;
    let object = game.host_object_mut(master).unwrap();
    object.target = Some(dock);
    object.set_ai_state(
        AIState::SeekingRepair,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    // This control starts after DockUpdate has admitted the active docker;
    // approach movement is exercised by the separate docking fixtures.
    game.host_object_mut(dock).unwrap().dock_active_docker = Some(master);
    game.update_support_states(&[master], 1.0 / 30.0);
    let object = game.host_object(master).unwrap();
    assert!(object.health.current > 20.0);
    assert_eq!(object.previous_health, 20.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(dock));
    assert_eq!(object.last_healing_timestamp, Some(29));
    let recipient = game.host_object(drone).unwrap();
    assert_eq!(recipient.health.current, 100.0);
    assert_eq!(recipient.last_damage_source, Some(dock));
}

#[test]
fn repair_action_default_is_one_frame_and_completes_on_next_action() {
    let (mut game, dock, master, drone) = world_with_duration(None);
    assert_eq!(
        game.host_object(dock)
            .unwrap()
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(1.0)
    );
    game.frame = 3;
    assert!(game.repair_dock_action_owned(dock, master));
    assert_eq!(game.host_object(master).unwrap().health.current, 100.0);
    assert_eq!(
        game.host_object(dock).unwrap().repair_dock_last_id,
        Some(master)
    );
    assert_eq!(
        game.host_object(dock).unwrap().repair_dock_health_per_frame,
        80.0
    );
    game.host_object_mut(drone)
        .unwrap()
        .internal_change_health(-40.0);
    game.frame = 4;
    assert!(!game.repair_dock_action_owned(dock, master));
    assert_eq!(
        game.host_object(drone).unwrap().health.current,
        60.0,
        "full master exits before healing its drone"
    );
    let dock = game.host_object(dock).unwrap();
    assert_eq!(dock.repair_dock_last_id, None);
    assert_eq!(
        dock.repair_dock_health_per_frame, 80.0,
        "completion clears identity, not cached float"
    );
}

#[test]
fn repair_action_caches_zero_and_does_not_switch_cached_docker() {
    let (mut game, dock, master, drone) = world_with_duration(Some("100"));
    game.host_object_mut(dock)
        .unwrap()
        .restore_repair_dock_state(3.0, Some(master), 0.0);
    assert!(game.repair_dock_action_owned(dock, master));
    assert_eq!(game.host_object(master).unwrap().health.current, 20.0);
    assert_eq!(
        game.host_object(dock).unwrap().repair_dock_health_per_frame,
        0.0
    );
    game.host_object_mut(dock)
        .unwrap()
        .restore_repair_dock_state(3.0, Some(master), 7.0);
    game.host_object_mut(drone)
        .unwrap()
        .internal_change_health(-50.0);
    assert!(game.repair_dock_action_owned(dock, drone));
    assert_eq!(game.host_object(drone).unwrap().health.current, 57.0);
    assert_eq!(
        game.host_object(dock).unwrap().repair_dock_last_id,
        Some(master)
    );
    assert_eq!(
        game.host_object(dock).unwrap().repair_dock_health_per_frame,
        7.0
    );
}

#[test]
fn repair_action_retains_fractional_frames_and_exact_near_max_gate() {
    let (mut game, dock, master, _) = world_with_duration(Some("50"));
    let frames = 50.0_f32 * (30.0_f32 / 1000.0);
    assert_eq!(
        game.host_object(dock)
            .unwrap()
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(frames)
    );
    let current = f32::from_bits(100.0_f32.to_bits() - 1);
    game.host_object_mut(master).unwrap().health.current = current;
    game.frame = 19;
    assert!(
        game.repair_dock_action_owned(dock, master),
        "no 0.01 completion epsilon"
    );
    assert_eq!(
        game.host_object(master).unwrap().last_healing_timestamp,
        Some(19)
    );
    assert_eq!(
        game.host_object(dock)
            .unwrap()
            .repair_dock_health_per_frame
            .to_bits(),
        ((100.0 - current) / frames).to_bits()
    );
}

#[test]
fn seeking_repair_consumes_one_logic_action_independent_of_display_delta() {
    use crate::game_logic::AIState;
    let (mut game, dock, master, _) = world_with_duration(Some("100"));
    let (mut peer, peer_dock, peer_master, _) = world_with_duration(Some("100"));
    for (world, dock, master) in [
        (&mut game, dock, master),
        (&mut peer, peer_dock, peer_master),
    ] {
        let object = world.host_object_mut(master).unwrap();
        object.target = Some(dock);
        object.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        world.host_object_mut(dock).unwrap().dock_active_docker = Some(master);
    }
    game.update_support_states(&[master], 1.0 / 30.0);
    peer.update_support_states(&[peer_master], 0.5);
    assert_eq!(
        game.host_object(master).unwrap().health.current.to_bits(),
        (20.0_f32 + 80.0 / 3.0).to_bits()
    );
    assert_eq!(
        game.host_object(master).unwrap().health.current.to_bits(),
        peer.host_object(peer_master)
            .unwrap()
            .health
            .current
            .to_bits()
    );
    assert_eq!(
        game.host_object(master).unwrap().ai_state(),
        AIState::SeekingRepair
    );
    assert_eq!(
        game.host_object(dock).unwrap().dock_active_docker,
        Some(master)
    );
}
