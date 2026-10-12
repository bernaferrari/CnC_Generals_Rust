//! Player.cpp:1028-1037 and Object.cpp:3770-3784 energy phases. These exercise
//! the real prepared objects and Player accounting callbacks, without claiming
//! ownership of the separately tracked legacy brownout-object delivery phase.
use super::*;
use crate::common::{Coord3D, DefaultThingTemplate, DisabledType, KindOf, ObjectStatusMaskType};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::Player;

fn prepared(power: i32) -> (ObjectFactory, Arc<RwLock<Object>>) {
    let mut factory = ObjectFactory::new();
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let mut template = DefaultThingTemplate::new(format!("PowerPhase{power}"));
    template.add_kind_of(KindOf::Structure);
    template.set_energy_production(power);
    let id = factory
        .prepare_object(
            &mut identity_owner,
            Arc::new(template),
            Coord3D::ZERO,
            None,
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let object = factory.get_object(id).unwrap().get_base_object().unwrap();
    (factory, object)
}
fn energy(player: &Player) -> (i32, i32) {
    (
        player.get_energy().production(),
        player.get_energy().consumption(),
    )
}

#[test]
fn construction_membership_has_no_power_until_actual_completion_phase() {
    for power in [10, -7] {
        let (_factory, object) = prepared(power);
        let mut old = Player::new(0);
        let mut new = Player::new(1);
        let mut object = object.write().unwrap();
        // Initial/save-state construction bit; this energy-only witness does
        // not invoke unrelated partition-collision status callbacks.
        object.status |= ObjectStatusMaskType::UNDER_CONSTRUCTION;
        assert_eq!(old.apply_membership_power_influence(&object, true), None);
        assert_eq!(old.apply_membership_power_influence(&object, false), None);
        assert_eq!(new.apply_membership_power_influence(&object, true), None);
        assert_eq!(energy(&old), (0, 0));
        assert_eq!(energy(&new), (0, 0));
        object.status &= !ObjectStatusMaskType::UNDER_CONSTRUCTION;
        let expected = if power > 0 { (power, 0) } else { (0, -power) };
        let completion = new.apply_construction_power_influence(
            object.get_template().get_energy_production(),
            object.is_disabled(),
        );
        assert_eq!(completion, Some((1, power < 0)));
        assert_eq!(energy(&new), expected);
        new.apply_membership_power_influence(&object, false);
        old.apply_membership_power_influence(&object, true);
        assert_eq!(energy(&new), (0, 0));
        assert_eq!(energy(&old), expected);
        old.apply_membership_power_influence(&object, false);
        assert_eq!(energy(&old), (0, 0));
    }
}

#[test]
fn completed_membership_energy_moves_exactly_once_between_supplied_players() {
    for power in [10, -7] {
        let (_factory, object) = prepared(power);
        let object = object.read().unwrap();
        let mut old = Player::new(0);
        let mut new = Player::new(1);
        let expected = if power > 0 { (power, 0) } else { (0, -power) };
        old.apply_membership_power_influence(&object, true);
        assert_eq!(energy(&old), expected);
        // These are the actual pre-switch departure and post-switch arrival
        // phases used by setOrRestoreTeam, with no second influence path.
        old.apply_membership_power_influence(&object, false);
        assert_eq!(energy(&old), (0, 0));
        assert_eq!(energy(&new), (0, 0));
        new.apply_membership_power_influence(&object, true);
        assert_eq!(energy(&new), expected);
        new.apply_membership_power_influence(&object, false);
        assert_eq!(energy(&new), (0, 0));
    }
}

#[test]
fn disabled_producers_are_excluded_but_consumers_still_use_energy() {
    for power in [10, -7] {
        let (_factory, object) = prepared(power);
        let mut object = object.write().unwrap();
        object.disabled_mask.set_disabled(DisabledType::DisabledEmp);
        let mut player = Player::new(0);
        let effect = player.apply_membership_power_influence(&object, true);
        if power > 0 {
            assert_eq!(effect, None);
            assert_eq!(energy(&player), (0, 0));
            assert_eq!(
                player.apply_construction_power_influence(power, object.is_disabled()),
                None
            );
        } else {
            assert_eq!(effect, Some(true));
            assert_eq!(energy(&player), (0, -power));
            player.apply_membership_power_influence(&object, false);
            assert_eq!(energy(&player), (0, 0));
        }
    }
}

#[test]
fn same_id_factory_objects_do_not_redirect_supplied_energy_accounting() {
    let (_first_factory, first) = prepared(10);
    let (_peer_factory, peer) = prepared(-7);
    assert_eq!(
        first.read().unwrap().get_id(),
        peer.read().unwrap().get_id()
    );
    let _peer_write = peer.write().unwrap();
    let mut player = Player::new(0);
    let first = first.read().unwrap();
    player.apply_membership_power_influence(&first, true);
    assert_eq!(energy(&player), (10, 0));
    player.apply_membership_power_influence(&first, false);
    assert_eq!(energy(&player), (0, 0));
}

#[test]
fn same_actual_team_assignment_keeps_completed_energy_without_reentering_membership() {
    let mut factory = ObjectFactory::new();
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let mut definition = DefaultThingTemplate::new("SameActualTeamPower".into());
    definition.set_energy_production(10);
    let team = Arc::new(RwLock::new(crate::team::Team::new(
        "SameActualTeam".into(),
        1,
    )));
    team.write().unwrap().set_controlling_player_id(Some(0));
    let player = Arc::new(RwLock::new(Player::new(0)));
    let id = factory
        .prepare_object(
            &mut identity_owner,
            Arc::new(definition),
            Coord3D::ZERO,
            Some(team.clone()),
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let object = factory.get_object(id).unwrap().get_base_object().unwrap();
    let mut object = object.write().unwrap();
    player
        .write()
        .unwrap()
        .apply_membership_power_influence(&object, true);
    // The real setOrRestoreTeam pointer-identity gate precedes all capture,
    // power delivery, registry, and module callbacks.
    object
        .set_or_restore_team(Some(team.clone()), false)
        .unwrap();
    assert_eq!(energy(&player.read().unwrap()), (10, 0));
    assert!(Arc::ptr_eq(&object.get_team().unwrap(), &team));
}
