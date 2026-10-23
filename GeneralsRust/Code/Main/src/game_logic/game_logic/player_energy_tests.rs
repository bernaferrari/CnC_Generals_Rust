//! Actual authored power and snapshot admission; C++ Energy masks observations,
//! never the canonical counter needed by objectLeavingInfluence.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};

fn admitted_world() -> GameLogic {
    use game_engine::common::{dict::Dict, well_known_keys::key_player_name};
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "owned-energy.ini".into(),
        "Object OwnedEnergyProducer\nKindOf = VEHICLE\nEnergyProduction = 20\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject OwnedEnergyConsumer\nKindOf = VEHICLE\nEnergyProduction = -10\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    let mut player = Player::new(1, Team::USA, "OwnedEnergy", true);
    let mut side = Dict::new();
    side.set_ascii_string(key_player_name(), "OwnedEnergySlot");
    player.apply_map_side_dict(&side, false, &mut Default::default());
    world.add_player(player);
    world
}
fn authored_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = admitted_world();
    let producer = world
        .create_object_for_player("OwnedEnergyProducer", 1, glam::Vec3::ZERO)
        .unwrap();
    let consumer = world
        .create_object_for_player("OwnedEnergyConsumer", 1, glam::Vec3::ONE)
        .unwrap();
    assert!(world.objects[&producer].is_constructed());
    assert!(world.objects[&consumer].is_constructed());
    world.frame = 37;
    world
        .players
        .get_mut(&1)
        .unwrap()
        .power_sabotaged_till_frame = 50;
    world.update_player_resources(0.0);
    (world, producer, consumer)
}
#[test]
fn authored_sabotage_masks_observers_not_canonical_production_or_same_id_peer() {
    let (mut world, producer, consumer) = authored_world();
    let (peer, peer_producer, peer_consumer) = authored_world();
    assert_eq!((producer, consumer), (peer_producer, peer_consumer));
    let player = &world.players[&1];
    assert_eq!((player.power_produced, player.power_consumed), (20, 10));
    assert_eq!(player.energy_production_at(37), 0);
    assert_eq!(player.energy_supply_ratio_at(37), 0.0);
    assert!(!player.has_sufficient_energy_at(37));
    assert_eq!(world.player_economy(1).unwrap().1, -10);
    assert_eq!(world.player_economy(1).unwrap().2, 0);
    // The actual receiving Energy operation returns the immediate callback
    // decision. Team removal/its callback dispatcher are a later prerequisite.
    assert_eq!(
        world
            .players
            .get_mut(&1)
            .unwrap()
            .remove_template_energy(20, 37),
        Some(true)
    );
    assert_eq!(
        (
            world.players[&1].power_produced,
            world.players[&1].power_consumed
        ),
        (0, 10)
    );
    assert_eq!(world.players[&1].power_available, -10);
    assert_eq!(
        (
            peer.players[&1].power_produced,
            peer.players[&1].power_consumed
        ),
        (20, 10)
    );
    assert_eq!(peer.players[&1].power_sabotaged_till_frame, 50);
}
#[test]
fn sabotage_deadline_and_repeated_energy_signals_preserve_cpp_zero_consumption_semantics() {
    let (mut world, _, _) = authored_world();
    let player = world.players.get_mut(&1).unwrap();
    assert_eq!(player.remove_template_energy(-10, 49), Some(true));
    assert_eq!((player.power_produced, player.power_consumed), (20, 0));
    assert_eq!(player.energy_available_at(49), 0);
    assert!(
        !player.has_sufficient_energy_at(49),
        "sabotage is brownout even when consumption is zero"
    );
    assert_eq!(player.energy_supply_ratio_at(49), 0.0);
    assert_eq!(
        player.remove_template_energy(0, 49),
        None,
        "zero Energy delta emits no callback"
    );
    assert_eq!(player.energy_production_at(50), 20);
    assert_eq!(player.energy_available_at(50), 20);
    assert!(player.has_sufficient_energy_at(50));
    assert_eq!(
        player.energy_supply_ratio_at(50),
        20.0,
        "CPP zero-consumption ratio is production, not one"
    );
    assert_eq!(
        player.power_sabotaged_till_frame, 50,
        "observation never mutates the deadline"
    );
    // Same brownout state still returns a signal for every nonzero adjustment.
    assert_eq!(player.remove_template_energy(1, 49), Some(true));
    assert_eq!(player.remove_template_energy(1, 49), Some(true));
    assert_eq!(player.power_produced, 18);
}
#[test]
fn sabotage_snapshot_restores_raw_grid_and_future_deadline_without_replaying_admission() {
    let (mut source, producer, consumer) = authored_world();
    let (peer, peer_producer, _) = authored_world();
    assert_eq!(producer, peer_producer);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    assert_eq!(snapshot.player_energy.len(), 1);
    assert_eq!(snapshot.player_energy[0].power_sabotaged_till_frame, 50);
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = admitted_world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.frame, 37);
    assert_eq!(
        (
            restored.players[&1].power_produced,
            restored.players[&1].power_consumed
        ),
        (20, 10)
    );
    assert_eq!(restored.player_economy(1), source.player_economy(1));
    for frame in [49, 50, 51] {
        source.frame = frame;
        restored.frame = frame;
        source.update_player_resources(0.0);
        restored.update_player_resources(0.0);
        assert_eq!(source.player_economy(1), restored.player_economy(1));
        assert_eq!(
            (
                restored.players[&1].power_produced,
                restored.players[&1].power_consumed
            ),
            (20, 10)
        );
        assert_eq!(
            restored.players[&1].energy_production_at(frame),
            if frame < 50 { 0 } else { 20 }
        );
        assert_eq!(
            restored.players[&1].has_sufficient_energy_at(frame),
            frame >= 50
        );
        assert!(restored.objects.contains_key(&consumer));
    }
    assert_eq!(peer.frame, 37);
    assert_eq!(peer.players[&1].power_sabotaged_till_frame, 50);
    assert_eq!(peer.players[&1].power_produced, 20);
}
#[cfg(feature = "game_client")]
#[test]
fn actual_presentation_masks_and_recovers_at_deadline_without_an_economy_tick() {
    let (mut world, _, _) = authored_world();
    let frozen = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(
        (
            frozen.local_power_produced,
            frozen.local_power_consumed,
            frozen.local_power
        ),
        (0, 10, -10)
    );
    assert!(
        !frozen
            .eva_context
            .local_player
            .as_ref()
            .unwrap()
            .sufficient_power
    );
    assert_eq!(world.players[&1].power_produced, 20);
    world.frame = 50;
    // Cached power_available still reflects frame37. Energy observers must
    // derive their current result from the single raw owner and current frame.
    assert_eq!(world.players[&1].power_available, -10);
    let recovered = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(
        (
            recovered.local_power_produced,
            recovered.local_power_consumed,
            recovered.local_power
        ),
        (20, 10, 10)
    );
    assert!(
        recovered
            .eva_context
            .local_player
            .as_ref()
            .unwrap()
            .sufficient_power
    );
    assert_eq!(world.player_economy(1).unwrap().1, 10);
    assert_eq!(world.players[&1].power_produced, 20);
}

#[test]
fn actual_power_callback_recovers_radar_at_deadline_despite_stale_projection() {
    let (mut world, _, _) = authored_world();
    world.players.get_mut(&1).unwrap().radar_count = 1;
    world.update_power_disabled_state();
    assert!(!world.players[&1].has_radar());
    world.frame = 50;
    assert_eq!(world.players[&1].power_available, -10);
    world.update_power_disabled_state();
    assert!(world.players[&1].has_radar());
    assert_eq!(world.players[&1].power_available, -10);
}

#[test]
fn actual_money_payment_and_refund_do_not_spend_or_refund_grid_energy() {
    let (mut world, _, _) = authored_world();
    let mut events = crate::game_logic::host_economy_log::EconomyEvents::default();
    let player = world.players.get_mut(&1).unwrap();
    player.resources.supplies = 100;
    let before = (
        player.power_produced,
        player.power_consumed,
        player.power_available,
    );
    let cost = crate::game_logic::Resources {
        supplies: 25,
        power: -500,
    };
    assert!(player.spend_resources(&cost, &mut events));
    assert_eq!(player.resources.supplies, 75);
    assert_eq!(
        (
            player.power_produced,
            player.power_consumed,
            player.power_available
        ),
        before
    );
    player.refund_production_cost(&cost, &mut events);
    assert_eq!(player.resources.supplies, 100);
    assert_eq!(
        (
            player.power_produced,
            player.power_consumed,
            player.power_available
        ),
        before
    );
    assert!(!player.has_sufficient_energy_at(49));
    assert!(player.has_sufficient_energy_at(50));
}

#[cfg(feature = "game_client")]
#[test]
fn actual_presentation_preserves_disabled_producer_zero_and_exact_missing_owner() {
    let mut world = admitted_world();
    let mut peer = admitted_world();
    let id = world
        .create_object_for_player("OwnedEnergyProducer", 1, glam::Vec3::ONE)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("OwnedEnergyProducer", 1, glam::Vec3::ONE)
        .unwrap();
    assert_eq!(id, peer_id);
    world
        .host_object_mut(id)
        .unwrap()
        .set_status_disabled_emp(true);
    // Actual Object Energy reconciliation, not a periodic economy tick.
    world.refresh_power_after_disabled_expiry(1);
    assert_eq!(
        (
            world.players[&1].power_produced,
            world.players[&1].power_consumed
        ),
        (0, 0)
    );
    assert_eq!(
        world.objects[&id].power_provided, 20,
        "template output remains positive while disabled"
    );
    let zero = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(
        (
            zero.local_power,
            zero.local_power_produced,
            zero.local_power_consumed
        ),
        (0, 0, 0)
    );
    assert!(zero.eva_context.local_player.unwrap().sufficient_power);
    // Admit the peer grid through the same actual Object Energy fold.
    // Factory admission alone does not yet publish canonical Energy totals.
    peer.refresh_power_after_disabled_expiry(1);
    let other = crate::presentation_frame::PresentationFrame::build_from_logic(&mut peer, 1);
    assert_eq!(other.local_power_produced, 20);
    // Two local players of the same faction never authorize ID99's view.
    let mut other_local = Player::new(2, Team::USA, "OtherLocal", true);
    other_local.power_produced = 35;
    world.add_player(other_local);
    let missing = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 99);
    assert_eq!(
        (
            missing.local_power,
            missing.local_power_produced,
            missing.local_power_consumed
        ),
        (0, 0, 0)
    );
    assert!(missing.eva_context.local_player.is_none());
    assert_eq!(world.players[&2].power_produced, 35);
    assert_eq!(peer.players[&1].power_produced, 20);
}
