//! Player cash and power are read from the receiving simulation.
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::game_logic::{GameLogic, Player, Resources, Team};
use crate::presentation_frame::{PresentationEvent, PresentationFrame};
use glam::Vec3;

fn add_player(world: &mut GameLogic, id: u32, supplies: u32) {
    let mut player = Player::new(id, Team::USA, "Economy player", id == 0);
    player.resources.supplies = supplies;
    player.alliance_team = id as i32;
    world.add_player(player);
}

fn published_economy(frame: &PresentationFrame) -> Vec<(u32, u32, i32)> {
    frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::EconomyChanged {
                player_id,
                supplies,
                power_available,
            } => Some((*player_id, *supplies, *power_available)),
            _ => None,
        })
        .collect()
}

#[test]
fn spending_and_credit_publish_in_order_from_the_players_balance() {
    let mut world = GameLogic::new();
    add_player(&mut world, 0, 150);
    let initial = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let cost = Resources {
        supplies: 100,
        power: 0,
    };
    {
        let (player, receipts) = world.player_and_economy_events_mut(0).unwrap();
        assert!(player.spend_resources(&cost, receipts));
        assert_eq!(player.effective_supplies(), 50);
        assert!(!player.spend_resources(&cost, receipts));
        assert_eq!(player.resources.supplies, 50);
        player.add_resources(
            &Resources {
                supplies: 25,
                power: 0,
            },
            receipts,
        );
        assert_eq!(player.effective_supplies(), 75);
        player.credit_supplies(123, receipts);
        assert_eq!(player.effective_supplies(), 198);
    }
    assert_eq!(initial.local_supplies, 150);
    world.complete_presentation_observations();
    world.complete_presentation_observations();
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(frame.local_supplies, 198);
    assert_eq!(
        published_economy(&frame),
        [(0, 50, 0), (0, 75, 0), (0, 198, 0)]
    );
    let again = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(published_economy(&again).is_empty());
    assert_eq!(again.local_supplies, 198);
    assert_eq!(initial.local_supplies, 150);
}

#[test]
fn stolen_cash_debits_and_credits_the_exact_controllers() {
    let mut world = GameLogic::new();
    let mut peer = GameLogic::new();
    for game in [&mut world, &mut peer] {
        add_player(game, 0, 500);
        add_player(game, 1, 100);
        add_player(game, 2, 900);
    }
    assert_eq!(world.steal_cash_between_players(0, 1, 50), 50);
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 450);
    assert_eq!(world.get_player(1).unwrap().effective_supplies(), 150);
    assert_eq!(world.steal_cash_between_players(0, 1, 1000), 450);
    assert_eq!(world.steal_cash_between_players(0, 1, 50), 0);
    assert_eq!(world.steal_cash_between_players(2, 2, 50), 0);
    assert_eq!(world.get_player(2).unwrap().resources.supplies, 900);

    peer.complete_presentation_observations();
    let foreign = PresentationFrame::try_build_from_logic(&mut peer, 1).unwrap();
    assert_eq!(foreign.local_supplies, 100);
    assert!(published_economy(&foreign).is_empty());
    world.complete_presentation_observations();
    let received = PresentationFrame::try_build_from_logic(&mut world, 1).unwrap();
    assert_eq!(received.local_supplies, 600);
    assert_eq!(
        published_economy(&received),
        [(0, 450, 0), (1, 150, 0), (0, 0, 0), (1, 600, 0)]
    );
    assert_eq!(peer.get_player(0).unwrap().resources.supplies, 500);
}

#[test]
fn resource_update_publishes_each_players_authored_energy() {
    let mut world = GameLogic::new();
    add_player(&mut world, 0, 150);
    add_player(&mut world, 1, 250);
    admit_fixture_objects(
        &mut world,
        &[
            FixtureObject::new("PublicationGenerator")
                .kind_of("STRUCTURE")
                .health(100.0)
                .field("EnergyProduction = 40"),
            FixtureObject::new("PublicationConsumer")
                .kind_of("STRUCTURE")
                .health(100.0)
                .field("EnergyProduction = -10"),
        ],
    );
    world
        .create_object_for_player("PublicationGenerator", 0, Vec3::ZERO)
        .unwrap();
    world
        .create_object_for_player("PublicationConsumer", 0, Vec3::X)
        .unwrap();
    world
        .create_object_for_player("PublicationConsumer", 1, Vec3::Z)
        .unwrap();
    world.economy_events.clear();
    // GameLogic's resource phase reads each completed building's influence.
    world.update_player_resources(1.0 / 30.0);
    world.complete_presentation_observations();
    let first = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let other = PresentationFrame::try_build_from_logic(&mut world, 1).unwrap();
    assert_eq!(
        (
            first.local_power_produced,
            first.local_power_consumed,
            first.local_power
        ),
        (40, 10, 30)
    );
    assert_eq!(
        (
            other.local_power_produced,
            other.local_power_consumed,
            other.local_power
        ),
        (0, 10, -10)
    );
    let mut receipts = published_economy(&first);
    receipts.sort_unstable_by_key(|&(id, _, _)| id);
    assert_eq!(receipts, [(0, 150, 30), (1, 250, -10)]);
    assert!(published_economy(&other).is_empty());
}
