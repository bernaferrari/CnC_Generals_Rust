//! CPP VictoryConditions::cachePlayerPtrs (284-315) classifies the driving roster.
//! These exercise real Main admission and victory/presentation, not a census fixture.
use super::*;
use crate::game_logic::{GameLogic, ThingTemplate};
use crate::presentation_frame::PresentationFrame;
use glam::Vec3;
use std::sync::{Arc, RwLock};

struct ForeignRoster {
    previous: Option<gamelogic::player::PlayerList>,
}

impl ForeignRoster {
    fn install(observer: bool, civilian: bool) -> Self {
        let mut player = gamelogic::player::Player::new(0);
        if civilian {
            player.init(Arc::new(gamelogic::player::PlayerTemplate::new(
                "FactionCivilian".into(),
            )));
        }
        player.set_display_name("player0");
        player.set_observer(observer);
        let mut foreign = gamelogic::player::PlayerList::new();
        foreign.add_player(Arc::new(RwLock::new(player)));
        let previous = std::mem::replace(
            &mut *gamelogic::player::ThePlayerList().write().unwrap(),
            foreign,
        );
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for ForeignRoster {
    fn drop(&mut self) {
        *gamelogic::player::ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner()) = self.previous.take().unwrap();
    }
}

fn admitted_match() -> (GameLogic, [ObjectId; 2]) {
    let mut world = GameLogic::new();
    world.game_mode = GameMode::Skirmish;
    world.frame = 30;
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, &format!("owner{id}"), id == 0);
        player.alliance_team = id as i32;
        player.resources.supplies = 1234 + id;
        world.add_player(player);
    }
    let mut template = ThingTemplate::new("VictoryRosterOwnedUnit");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let ids = [
        world
            .create_object("VictoryRosterOwnedUnit", Team::USA, Vec3::ZERO)
            .unwrap(),
        world
            .create_object(
                "VictoryRosterOwnedUnit",
                Team::China,
                Vec3::new(20.0, 0.0, 0.0),
            )
            .unwrap(),
    ];
    for (index, id) in ids.iter().enumerate() {
        assert_eq!(
            world.host_object(*id).unwrap().owner_player_id,
            Some(index as u32)
        );
    }
    (world, ids)
}

fn destroy_armies(world: &mut GameLogic, ids: [ObjectId; 2]) {
    for id in ids {
        assert!(
            world
                .host_object_mut(id)
                .unwrap()
                .take_damage_from_immediate(1_000_000.0, None)
        );
        assert!(world.host_object(id).unwrap().status.destroyed);
    }
}

#[test]
fn foreign_observer_cannot_exempt_the_owners_dead_player_from_defeat() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let (mut other, other_ids) = admitted_match();
    assert_eq!(ids, other_ids);
    other.get_player_mut(0).unwrap().is_observer = true;
    let _foreign = ForeignRoster::install(true, false);
    destroy_armies(&mut owner, ids);
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(
        owner.get_player(0).unwrap().resources.supplies,
        0,
        "the receiver's ordinary player must run killPlayer despite a foreign observer at the same index"
    );
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
    assert_eq!(frame.local_supplies, 0);
    assert!(
        owner
            .victory_conditions
            .is_local_allied_defeat(&owner.players)
    );
    assert_eq!(other.get_player(0).unwrap().resources.supplies, 1234);
    assert!(other.host_object(other_ids[0]).unwrap().is_alive());
}

#[test]
fn foreign_civilian_cannot_exempt_the_owners_dead_player_from_defeat() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let _foreign = ForeignRoster::install(false, true);
    destroy_armies(&mut owner, ids);
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 0);
    assert_eq!(frame.local_supplies, 0);
}

#[test]
fn foreign_observer_cannot_remove_a_living_owner_army_from_the_match() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let _foreign = ForeignRoster::install(true, false);
    assert!(owner.evaluate_victory_condition().is_none());
    assert!(owner.host_object(ids[0]).unwrap().is_alive());
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
}

#[test]
fn owned_observer_remains_exempt_with_an_ordinary_foreign_slot() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    owner.get_player_mut(0).unwrap().is_observer = true;
    let _foreign = ForeignRoster::install(false, false);
    destroy_armies(&mut owner, ids);
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
    // CPP isLocalAlliedDefeat returns m_singleAllianceRemaining for observers.
    assert!(
        owner
            .victory_conditions
            .is_local_allied_defeat(&owner.players)
    );
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
}

#[test]
fn unresolved_owned_template_remains_excluded_without_foreign_fallback() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    owner.player_template_bindings.insert(
        0,
        PlayerTemplateIdentity {
            template_name: "VictoryRosterMissingTemplate".into(),
            template_index: None,
        },
    );
    let _foreign = ForeignRoster::install(false, false);
    destroy_armies(&mut owner, ids);
    assert!(owner.evaluate_victory_condition().is_some());
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
}
