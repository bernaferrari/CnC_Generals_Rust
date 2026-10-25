//! No catalog/registry: exact receiving definitions across same-name worlds.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team};
use glam::Vec3;
fn admitted(carbomb: bool, boat: bool) -> (GameLogic, ObjectId) {
    admitted_with_cliff(carbomb, boat, false)
}
fn admitted_with_cliff(carbomb: bool, boat: bool, cliff: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let weapon = if carbomb {
        "WeaponSet\nConditions = CARBOMB VETERAN\nEnd\nWeaponSet\nConditions = CARBOMB\nEnd\n"
    } else {
        ""
    };
    let kinds = if boat {
        "VEHICLE BOAT SELECTABLE"
    } else {
        "VEHICLE SELECTABLE FORCEATTACKABLE ALWAYS_SELECTABLE"
    };
    let kinds = format!("{kinds}{}", if cliff { " CLIFF_JUMPER" } else { "" });
    let source = format!(
        "Object SameNameCar\nKindOf = {kinds}\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nWeaponSet\nConditions = NONE\nEnd\n{weapon}End\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([("owned-car.ini".into(), source)])
                .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "owner", true));
    let id = world
        .create_object_for_player_with_effects_checked(
            "SameNameCar",
            1,
            Vec3::ZERO,
            None,
            &mut Default::default(),
        )
        .unwrap()
        .unwrap();
    (world, id)
}
#[test]
fn same_name_same_id_car_capability_and_kind_are_owned_and_frozen_without_catalog() {
    let (mut owner, id) = admitted_with_cliff(true, false, true);
    let (mut peer, pid) = admitted(false, true);
    assert_eq!(id, pid);
    assert!(owner.host_object(id).unwrap().authors_carbomb_weapon_set());
    assert!(!peer.host_object(pid).unwrap().authors_carbomb_weapon_set());
    assert!(!carbomb_target_rejected(owner.host_object(id).unwrap()));
    assert!(carbomb_target_rejected(peer.host_object(pid).unwrap()));
    assert!(!hijack_target_rejected(owner.host_object(id).unwrap()));
    assert!(hijack_target_rejected(peer.host_object(pid).unwrap()));
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut owner, 1);
    let other = crate::presentation_frame::PresentationFrame::build_from_logic(&mut peer, 1);
    let a = frame.objects.iter().find(|object| object.id == id).unwrap();
    let b = other
        .objects
        .iter()
        .find(|object| object.id == pid)
        .unwrap();
    assert!(a.authors_carbomb_weapon_set);
    assert!(!b.authors_carbomb_weapon_set);
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(a, KindOf::ForceAttackable)
    );
    assert!(
        !crate::presentation_frame::PresentationFrame::object_has_kind(b, KindOf::ForceAttackable)
    );
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(a, KindOf::AlwaysSelectable)
    );
    assert!(
        !crate::presentation_frame::PresentationFrame::object_has_kind(b, KindOf::AlwaysSelectable)
    );
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .is_kind_of(KindOf::CliffJumper)
    );
    assert!(crate::presentation_frame::PresentationFrame::object_has_kind(a, KindOf::CliffJumper));
    assert!(!crate::presentation_frame::PresentationFrame::object_has_kind(b, KindOf::CliffJumper));
    // Rebuilding from a sparse observer must refresh the complete actual host facts.
    let mut overlaid = frame.clone();
    let stale = overlaid
        .objects
        .iter_mut()
        .find(|object| object.id == id)
        .unwrap();
    stale.kind_of.clear();
    stale.is_force_attackable = false;
    stale.always_selectable = false;
    assert!(overlaid.overlay_host_fx_residual(&owner) > 0);
    let refreshed = overlaid
        .objects
        .iter()
        .find(|object| object.id == id)
        .unwrap();
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            refreshed,
            KindOf::ForceAttackable
        )
    );
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            refreshed,
            KindOf::AlwaysSelectable
        )
    );
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            refreshed,
            KindOf::CliffJumper
        )
    );
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(32);
    shadow.sync_from_host(&owner);
    let engine_frame = crate::presentation_frame::PresentationFrame::build_for_engine(
        &mut owner,
        1,
        Some(&shadow),
    );
    let engine_object = engine_frame
        .objects
        .iter()
        .find(|object| object.id == id)
        .unwrap();
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            engine_object,
            KindOf::ForceAttackable
        )
    );
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            engine_object,
            KindOf::AlwaysSelectable
        )
    );
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            engine_object,
            KindOf::CliffJumper
        )
    );
    owner
        .clear_all_objects_with_effects_checked(&mut Default::default())
        .unwrap();
    let replacement = owner
        .create_object_for_player_with_effects_checked(
            "SameNameCar",
            1,
            Vec3::ZERO,
            None,
            &mut Default::default(),
        )
        .unwrap()
        .unwrap();
    assert!(
        owner
            .host_object(replacement)
            .unwrap()
            .authors_carbomb_weapon_set()
    );
    assert!(!peer.host_object(pid).unwrap().authors_carbomb_weapon_set());
    assert!(
        a.authors_carbomb_weapon_set,
        "completed frame owns capability after reset"
    );
}
#[test]
fn native_restore_rebinds_actual_receiving_carbomb_definition_and_keeps_peer() {
    let (source, id) = admitted(true, false);
    let (mut peer, pid) = admitted(false, false);
    assert_eq!(id, pid);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let (mut receiving, rid) = admitted(true, false);
    assert_eq!(rid, id);
    builder
        .restore_from_snapshot(&saved, &mut receiving)
        .unwrap();
    assert!(
        receiving
            .host_object(id)
            .unwrap()
            .authors_carbomb_weapon_set()
    );
    assert!(!peer.host_object(pid).unwrap().authors_carbomb_weapon_set());
    let peer_saved = builder.create_world_snapshot(&peer).unwrap();
    peer.clear_all_objects_with_effects_checked(&mut Default::default())
        .unwrap();
    builder
        .restore_from_snapshot(&peer_saved, &mut peer)
        .unwrap();
    assert!(!peer.host_object(pid).unwrap().authors_carbomb_weapon_set());
}
#[test]
fn fixed_carbomb_query_uses_source_sparse_match_and_default_set_rejects() {
    let (world, id) = admitted(true, false);
    let definition = world
        .host_object(id)
        .unwrap()
        .get_template()
        .receiving_object_definition
        .as_ref()
        .unwrap();
    assert!(definition_has_carbomb_weapon_set(definition));
    let mut without = definition.as_ref().clone();
    without.weapon_sets.retain(|set| {
        !set.conditions
            .iter()
            .any(|flag| flag.eq_ignore_ascii_case("CARBOMB"))
    });
    assert!(!definition_has_carbomb_weapon_set(&without));
    let mut combined = definition.as_ref().clone();
    combined.weapon_sets.retain(|set| {
        set.conditions
            .iter()
            .any(|flag| flag.eq_ignore_ascii_case("VETERAN"))
    });
    assert!(
        definition_has_carbomb_weapon_set(&combined),
        "intersection wins before extraneous-bit tie break"
    );
}

#[test]
fn authored_reject_unmanned_controls_live_recrew_frozen_facts_and_native_restore() {
    fn receiving(reject: bool) -> (GameLogic, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        let flag = if reject { " REJECT_UNMANNED" } else { "" };
        let source = format!(
            "Object SameCrew\nKindOf = INFANTRY SELECTABLE{flag}\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject SameHusk\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "recrew-owner.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::USA, "owner", true));
        let crew = world
            .create_object_for_player_with_effects_checked(
                "SameCrew",
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let husk = world
            .create_object_for_player_with_effects_checked(
                "SameHusk",
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        world
            .host_object_mut(husk)
            .unwrap()
            .set_status_disabled_unmanned(true);
        (world, crew, husk)
    }
    let (mut owner, crew, husk) = receiving(true);
    let (mut peer, peer_crew, peer_husk) = receiving(false);
    assert_eq!((crew, husk), (peer_crew, peer_husk));
    assert!(
        owner
            .host_object(crew)
            .unwrap()
            .is_kind_of(KindOf::RejectUnmanned)
    );
    assert!(
        !peer
            .host_object(peer_crew)
            .unwrap()
            .is_kind_of(KindOf::RejectUnmanned)
    );
    assert!(!owner.can_execute_infantry_unmanned_recrew(crew, husk));
    assert!(peer.can_execute_infantry_unmanned_recrew(peer_crew, peer_husk));
    // The established compact render bank does not allocate a new bit.
    assert_eq!(
        owner.host_object(crew).unwrap().presentation_kind_of_bits(),
        peer.host_object(peer_crew)
            .unwrap()
            .presentation_kind_of_bits()
    );
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut owner, 1);
    let peer_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut peer, 1);
    let frozen = frame
        .objects
        .iter()
        .find(|object| object.id == crew)
        .unwrap();
    let peer_frozen = peer_frame
        .objects
        .iter()
        .find(|object| object.id == peer_crew)
        .unwrap();
    assert!(frozen.reject_unmanned);
    assert!(!peer_frozen.reject_unmanned);
    assert!(
        crate::presentation_frame::PresentationFrame::object_has_kind(
            frozen,
            KindOf::RejectUnmanned
        )
    );
    assert!(
        !crate::presentation_frame::PresentationFrame::object_has_kind(
            peer_frozen,
            KindOf::RejectUnmanned
        )
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, restored_crew, restored_husk) = receiving(true);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert!(
        restored
            .host_object(restored_crew)
            .unwrap()
            .is_kind_of(KindOf::RejectUnmanned)
    );
    assert!(!restored.can_execute_infantry_unmanned_recrew(restored_crew, restored_husk));
    assert!(peer.can_execute_infantry_unmanned_recrew(peer_crew, peer_husk));
    owner
        .clear_all_objects_with_effects_checked(&mut Default::default())
        .unwrap();
    assert!(
        frozen.reject_unmanned,
        "completed facts survive source reset"
    );
}
