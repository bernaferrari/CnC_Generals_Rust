//! Object.cpp985-1012: an actual UC edge completes its mine callbacks inline.
//! Resident mines have passed the original PM phase; new dirty births have not.
use super::*;
use gamelogic::common::Relationship;

const SITE: &str = "UCStatusSite541";
const MINE: &str = "UCStatusMine541";
const UPGRADE: &str = "Upgrade_UCStatusPulse541";

fn world(relation: Relationship) -> (GameLogic, ObjectId) {
    world_with_masks(relation, "UNDER_CONSTRUCTION", "UNDER_CONSTRUCTION")
}

fn world_with_masks(relation: Relationship, set: &str, clear: &str) -> (GameLogic, ObjectId) {
    let source = format!(
        r#"
Object UCStatusSite541
  KindOf = STRUCTURE SELECTABLE
  Geometry = CYLINDER
  GeometryMajorRadius = 20
  GeometryHeight = 10
  Body = ActiveBody BodyTag
    MaxHealth = 200
    InitialHealth = 200
  End
  Behavior = StatusBitsUpgrade PulseTag
    TriggeredBy = Upgrade_UCStatusPulse541
    StatusToSet = {set}
    StatusToClear = {clear}
  End
End
Object UCStatusMine541
  KindOf = MINE IMMOBILE
  Geometry = SPHERE
  GeometryMajorRadius = 2
  GeometryHeight = 2
  Body = ActiveBody BodyTag
    MaxHealth = 25
    InitialHealth = 25
  End
  Behavior = InstantDeathBehavior MineDeathTag
  End
End
"#
    );
    let mut owner = GameLogic::new();
    owner
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "uc-status-owner.ini".to_owned(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    for (id, team, name) in [
        (0, Team::USA, "BuilderOwner"),
        (1, Team::GLA, "MineOwner"),
        (2, Team::USA, "FriendlyMineOwner"),
        (3, Team::China, "NeutralMineOwner"),
    ] {
        owner.add_player(Player::new(id, team, name, id == 0));
    }
    let player = owner.players.get_mut(&0).unwrap();
    player.set_map_relationship(1, relation);
    player.set_map_relationship(2, Relationship::Allies);
    player.set_map_relationship(3, Relationship::Neutral);
    owner.frame = 53;
    let site = owner
        .create_object_for_player(SITE, 0, Vec3::new(100., 0., 100.))
        .unwrap();
    (owner, site)
}

fn mine(owner: &mut GameLogic, player: u32) -> ObjectId {
    let id = owner
        .create_object_for_player(MINE, player, Vec3::new(100., 0., 100.))
        .unwrap();
    assert!(
        owner.host_object(id).unwrap().mine_data.is_none(),
        "KINDOF_MINE and its name cannot synthesize an undeclared MinefieldBehavior"
    );
    id
}

fn wave(owner: &mut GameLogic) -> [ObjectId; 3] {
    [mine(owner, 1), mine(owner, 2), mine(owner, 3)]
}

/// C++ PartitionManager::update, after births and before the later UC edge.
/// This is deliberately not hidden inside factory/mine helpers or the sweep.
fn resident_partition_phase(owner: &mut GameLogic) {
    let frame = owner.frame;
    owner.update_owned_partition_with_effects(&mut Default::default());
    assert_eq!(
        owner.frame, frame,
        "PM phase does not advance the logic clock"
    );
}

fn pending_count(owner: &GameLogic, id: ObjectId) -> usize {
    owner
        .objects_to_destroy
        .iter()
        .filter(|event| event.id == id)
        .count()
}

fn assert_killed(owner: &GameLogic, id: ObjectId) {
    let object = owner.host_object(id).expect("kill retains the queued Body");
    assert_eq!(object.health.current, 0.);
    assert!(object.status.effectively_dead);
    assert!(object.status.on_die_started);
    assert!(
        object.status.destroyed,
        "actual InstantDeath queued removal"
    );
    // Object::setStatus calls default Object::kill, not mine proximity fire.
    assert_eq!(
        object.last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Unresistable)
    );
    assert_eq!(
        object.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal
    );
    assert_eq!(object.last_damage_source, None);
    assert_eq!(object.last_damage_timestamp, Some(53));
    assert_eq!(pending_count(owner, id), 1);
}

fn assert_silently_destroyed(owner: &GameLogic, id: ObjectId) {
    let object = owner
        .host_object(id)
        .expect("direct destruction remains queued");
    assert_eq!(object.health.current, 25.);
    assert!(object.status.destroyed);
    assert!(!object.status.on_die_started);
    assert_eq!(object.last_damage_info_type, None);
    assert_eq!(object.last_damage_timestamp, None);
    assert_eq!(pending_count(owner, id), 1);
}

fn assert_live(owner: &GameLogic, ids: impl IntoIterator<Item = ObjectId>) {
    for id in ids {
        let object = owner.host_object(id).unwrap();
        assert_eq!(object.health.current, 25.);
        assert!(!object.status.destroyed);
        assert!(!object.status.on_die_started);
        assert_eq!(pending_count(owner, id), 0);
    }
}

#[test]
fn actual_uc_edges_kill_enemy_bodies_and_destroy_other_mines_inline_only_in_the_driving_world() {
    let (mut owner, site) = world(Relationship::Enemies);
    let (mut peer, peer_site) = world(Relationship::Allies);
    let first = wave(&mut owner);
    let peer_first = wave(&mut peer);
    resident_partition_phase(&mut owner);
    resident_partition_phase(&mut peer);
    assert_eq!((site, first), (peer_site, peer_first));
    assert!(owner.set_object_under_construction_with_effects(site, true, &mut Default::default()));
    assert_killed(&owner, first[0]);
    assert_silently_destroyed(&owner, first[1]);
    assert_silently_destroyed(&owner, first[2]);
    assert_live(&peer, peer_first);
    // A foreign ObjectId must not be delivered by the next world's mine phase.
    peer.update_mines_and_demo_traps();
    assert_live(&peer, peer_first);
    let second = wave(&mut owner);
    let peer_second = wave(&mut peer);
    resident_partition_phase(&mut owner);
    resident_partition_phase(&mut peer);
    assert_eq!(second, peer_second);
    assert!(peer.set_object_under_construction_with_effects(
        peer_site,
        true,
        &mut Default::default()
    ));
    for id in peer_first.into_iter().chain(peer_second) {
        assert_silently_destroyed(&peer, id);
    }
    assert_live(&owner, second);
    owner.update_mines_and_demo_traps();
    assert_live(&owner, second);
    // Clearing the bit is also a real source edge, not merely construction entry.
    assert!(owner.set_object_under_construction_with_effects(site, false, &mut Default::default()));
    assert_killed(&owner, second[0]);
    assert_silently_destroyed(&owner, second[1]);
    assert_silently_destroyed(&owner, second[2]);
    let third = wave(&mut owner);
    let peer_third = wave(&mut peer);
    resident_partition_phase(&mut owner);
    resident_partition_phase(&mut peer);
    assert_eq!(third, peer_third);
    assert!(!owner.set_object_under_construction_with_effects(
        site,
        false,
        &mut Default::default()
    ));
    assert!(!peer.set_object_under_construction_with_effects(
        peer_site,
        true,
        &mut Default::default()
    ));
    owner.update_mines_and_demo_traps();
    peer.update_mines_and_demo_traps();
    assert_live(&owner, third);
    assert_live(&peer, peer_third);
    for id in first.into_iter().chain(second) {
        assert!(
            owner.objects.ordered_ids().contains(&id),
            "status callbacks do not physically delete"
        );
        assert_eq!(pending_count(&owner, id), 1);
    }
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    for id in first.into_iter().chain(second) {
        assert!(owner.host_object(id).is_none());
    }
    assert_live(&owner, third);
    assert_live(&peer, peer_third);
}

#[test]
fn effective_uc_edges_normalize_bool_and_raw_bits_without_duplicate_callback_or_power_reset() {
    use crate::game_logic::host_status_bits_upgrade::{
        object_status_mask_from_names, status_bits_has,
    };
    let bit = object_status_mask_from_names(&["UNDER_CONSTRUCTION"]);
    for via_masks in [false, true] {
        for (boolean, raw, effective) in [
            (false, false, false),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            let (mut owner, site) = world(Relationship::Enemies);
            let id = mine(&mut owner, 1);
            let object = owner.host_object_mut(site).unwrap();
            object.status.under_construction = boolean;
            object.object_status_bits = if raw { bit } else { 0 };
            object.construction_power_admitted = true;
            resident_partition_phase(&mut owner);
            if via_masks {
                let (set, clear): (&[&str], &[&str]) = if effective {
                    (&["UNDER_CONSTRUCTION"], &[])
                } else {
                    (&[], &["UNDER_CONSTRUCTION"])
                };
                assert!(
                    owner
                        .apply_object_status_bits_upgrade_with_effects(
                            site,
                            set,
                            clear,
                            &mut Default::default()
                        )
                        .is_some()
                );
            } else {
                assert!(!owner.set_object_under_construction_with_effects(
                    site,
                    effective,
                    &mut Default::default()
                ));
            }
            assert_live(&owner, [id]);
            let object = owner.host_object(site).unwrap();
            assert_eq!(object.status.under_construction, effective);
            assert_eq!(
                status_bits_has(object.object_status_bits, "UNDER_CONSTRUCTION"),
                effective
            );
            assert!(
                object.construction_power_admitted,
                "normalization is not a false-to-true edge"
            );
            if via_masks {
                let (set, clear): (&[&str], &[&str]) = if effective {
                    (&[], &["UNDER_CONSTRUCTION"])
                } else {
                    (&["UNDER_CONSTRUCTION"], &[])
                };
                assert!(
                    owner
                        .apply_object_status_bits_upgrade_with_effects(
                            site,
                            set,
                            clear,
                            &mut Default::default()
                        )
                        .is_some()
                );
            } else {
                assert!(owner.set_object_under_construction_with_effects(
                    site,
                    !effective,
                    &mut Default::default()
                ));
            }
            assert_killed(&owner, id);
            let object = owner.host_object(site).unwrap();
            assert_eq!(object.status.under_construction, !effective);
            assert_eq!(
                status_bits_has(object.object_status_bits, "UNDER_CONSTRUCTION"),
                !effective
            );
            assert_eq!(
                object.construction_power_admitted, effective,
                "only an actual entering edge resets power admission"
            );
        }
    }
    // An Object-only value mutation is not an owner callback or a queued request.
    let (mut owner, site) = world(Relationship::Enemies);
    let id = mine(&mut owner, 1);
    resident_partition_phase(&mut owner);
    owner
        .host_object_mut(site)
        .unwrap()
        .set_status_under_construction(true);
    assert_live(&owner, [id]);
    owner.update_mines_and_demo_traps();
    assert_live(&owner, [id]);
    assert!(!owner.set_object_under_construction_with_effects(site, true, &mut Default::default()));
    assert_live(&owner, [id]);
    assert!(owner.set_object_under_construction_with_effects(site, false, &mut Default::default()));
    assert_killed(&owner, id);
}

#[test]
fn authored_status_bits_upgrade_uses_receiving_module_and_inline_set_then_clear() {
    use crate::game_logic::host_status_bits_upgrade::status_bits_has;
    let (mut owner, site) = world(Relationship::Enemies);
    let (mut peer, peer_site) = world_with_masks(Relationship::Enemies, "NO_ATTACK", "CAN_ATTACK");
    let id = mine(&mut owner, 1);
    let peer_id = mine(&mut peer, 1);
    resident_partition_phase(&mut owner);
    resident_partition_phase(&mut peer);
    assert_eq!((site, id), (peer_site, peer_id));
    owner
        .host_object_mut(site)
        .unwrap()
        .construction_power_admitted = true;
    owner
        .apply_upgrade_to_object_with_effects_checked(site, UPGRADE, &mut Default::default())
        .unwrap();
    assert_killed(&owner, id);
    assert_eq!(owner.status_bits_upgrade_reg.applies, 1);
    assert_eq!(
        (
            owner.status_bits_upgrade_reg.bits_set,
            owner.status_bits_upgrade_reg.bits_cleared
        ),
        (1, 1),
        "the authored set and clear operations both execute"
    );
    let object = owner.host_object(site).unwrap();
    assert!(object.has_upgrade_tag(UPGRADE));
    assert!(!object.status.under_construction);
    assert!(!status_bits_has(
        object.object_status_bits,
        "UNDER_CONSTRUCTION"
    ));
    assert!(
        !object.construction_power_admitted,
        "set/clear may not collapse into a final no-edge result"
    );
    assert_live(&peer, [peer_id]);
    let later_mine = mine(&mut owner, 1);
    resident_partition_phase(&mut owner);
    owner
        .apply_upgrade_to_object_with_effects_checked(site, UPGRADE, &mut Default::default())
        .unwrap();
    assert_live(&owner, [later_mine]);
    assert_eq!(
        owner.status_bits_upgrade_reg.applies, 1,
        "an already-given upgrade does not replay its pulse"
    );
    assert_eq!(
        (
            owner.status_bits_upgrade_reg.bits_set,
            owner.status_bits_upgrade_reg.bits_cleared
        ),
        (1, 1)
    );
    assert!(!owner.host_object(site).unwrap().status.under_construction);
    peer.apply_upgrade_to_object_with_effects_checked(peer_site, UPGRADE, &mut Default::default())
        .unwrap();
    assert_live(&peer, [peer_id]);
    let object = peer.host_object(peer_site).unwrap();
    assert!(object.has_upgrade_tag(UPGRADE));
    assert!(status_bits_has(object.object_status_bits, "NO_ATTACK"));
    assert!(!object.status.under_construction);
    assert_killed(&owner, id);
}

#[test]
fn construction_completion_observes_uc_mine_death_before_player_and_build_complete_callbacks() {
    use crate::game_logic::host_enum_table_residual::{
        actively_being_constructed_model_bit, awaiting_construction_model_bit,
        partially_constructed_model_bit,
    };
    let (mut owner, site) = world(Relationship::Enemies);
    let (mut peer, peer_site) = world(Relationship::Enemies);
    assert_eq!(site, peer_site);
    assert!(owner.set_object_under_construction_with_effects(site, true, &mut Default::default()));
    let object = owner.host_object_mut(site).unwrap();
    object.construction_percent = 0.75;
    object.internal_change_health(150. - object.health.current);
    object.set_under_construction_model_conditions(true);
    let id = mine(&mut owner, 1);
    let peer_id = mine(&mut peer, 1);
    resident_partition_phase(&mut owner);
    resident_partition_phase(&mut peer);
    assert_eq!(id, peer_id);
    let mut observed = false;
    owner.notify_structure_construction_complete_with_prepared_observer(site, |owner, prepared| {
        observed = true;
        assert_eq!(prepared, site);
        assert_killed(owner, id);
        let object = owner.host_object(site).unwrap();
        assert!(!object.status.under_construction);
        assert_eq!(
            object.health.current, 150.,
            "completion preserves existing scaffold damage"
        );
        assert_eq!(object.construction_percent, 1.);
        assert!(
            !object.construction_power_admitted,
            "Player completion is still later"
        );
        for bit in [
            actively_being_constructed_model_bit(),
            awaiting_construction_model_bit(),
            partially_constructed_model_bit(),
        ] {
            assert_eq!(object.model_condition_bits & (1u128 << bit), 0);
        }
    });
    assert!(observed);
    assert!(owner.host_object(site).unwrap().construction_power_admitted);
    assert_killed(&owner, id);
    assert!(owner.objects.ordered_ids().contains(&id));
    assert_live(&peer, [peer_id]);
    assert!(
        !peer
            .host_object(peer_site)
            .unwrap()
            .status
            .under_construction
    );
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    assert!(owner.host_object(id).is_none());
    assert_live(&peer, [peer_id]);
}

#[test]
fn uc_sweep_observes_original_dirty_birth_boundary_before_resident_cois() {
    let (mut owner, site) = world(Relationship::Enemies);
    let id = mine(&mut owner, 1);
    let in_cells = |world: &GameLogic| {
        world
            .partition_manager
            .ids_in_radius_iterator_order(100.0, 100.0, 22.0, |_| true)
            .contains(&id.0)
    };
    assert!(
        !in_cells(&owner),
        "registerObject attaches an empty dirty COI module"
    );
    assert!(owner.set_object_under_construction_with_effects(site, true, &mut Default::default()));
    assert_live(&owner, [id]);
    assert!(
        !in_cells(&owner),
        "UC potential-collision iteration does not flush dirty births"
    );
    resident_partition_phase(&mut owner);
    assert!(
        in_cells(&owner),
        "the actual PM phase publishes the same birth's COIs"
    );
    assert!(!owner.set_object_under_construction_with_effects(site, true, &mut Default::default()));
    assert_live(&owner, [id]);
    assert!(owner.set_object_under_construction_with_effects(site, false, &mut Default::default()));
    assert_killed(&owner, id);
    assert!(
        owner.objects.ordered_ids().contains(&id),
        "UC kill remains inline and physical deletion stays deferred"
    );
}
