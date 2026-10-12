//! CPP ScriptEngine.cpp5975-5991 literal getUnitNamed and7132-7168 transfer.
use super::*;
use crate::game_logic::NamedSpecialPowerCountdownOp;
use crate::save_load::snapshot::SnapshotBuilder;

fn world(name: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("NamedOwnerProbe");
    template.add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("NamedOwnerProbe", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(world.set_host_object_name(id, name));
    (world, id)
}

#[test]
fn literal_host_name_lookup_ignores_foreign_same_id_cache_and_applies_to_own_name_only() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "literal_host_name_lookup_ignores_foreign_same_id_cache_and_applies_to_own_name_only",
        || {
            let (mut owner, id) = world("OwnerOnly");
            let (peer, peer_id) = world("ForeignOnly");
            assert_eq!(id, peer_id);
            let tracker = gamelogic::scripting::engine::get_named_object_tracker();
            tracker
                .register_named_object("ForeignOnly".into(), peer_id.0)
                .unwrap();
            assert_eq!(
                tracker.get_object_id("ForeignOnly").unwrap(),
                Some(peer_id.0)
            );
            assert_eq!(owner.host_object_id_by_script_name("OwnerOnly"), Some(id));
            assert_eq!(owner.host_object_id_by_script_name("ForeignOnly"), None);
            assert_eq!(owner.host_object_id_by_script_name("owneronly"), None);
            assert_eq!(owner.host_object_id_by_script_name(""), None);
            assert!(!owner.script_named_special_power_countdown(
                "ForeignOnly",
                "SpecialPowerSpySatellite",
                NamedSpecialPowerCountdownOp::Set,
                12
            ));
            assert!(owner.script_named_special_power_countdown(
                "OwnerOnly",
                "SpecialPowerSpySatellite",
                NamedSpecialPowerCountdownOp::Set,
                7
            ));
            assert_eq!(
                owner
                    .host_object(id)
                    .unwrap()
                    .special_power_countdown_seconds(
                        &crate::command_system::SpecialPowerType::SpySatellite
                    ),
                7.0
            );
            assert_eq!(peer.host_object(peer_id).unwrap().name, "ForeignOnly");
            assert!(owner.set_host_object_name(id, "RenamedOwner"));
            assert_eq!(owner.host_object_id_by_script_name("OwnerOnly"), None);
            assert_eq!(
                owner.host_object_id_by_script_name("RenamedOwner"),
                Some(id)
            );
            assert_eq!(
                tracker.get_object_id("ForeignOnly").unwrap(),
                Some(peer_id.0)
            );
            assert_eq!(
                peer.host_object_id_by_script_name("ForeignOnly"),
                Some(peer_id)
            );
        },
    );
}

#[test]
fn actual_name_transfer_and_save_keep_only_current_receiving_names() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_name_transfer_and_save_keep_only_current_receiving_names",
        || {
            let (mut owner, from) = world("TransferredName");
            let to = owner
                .create_object(
                    "NamedOwnerProbe",
                    Team::USA,
                    glam::Vec3::new(20.0, 0.0, 0.0),
                )
                .unwrap();
            assert!(owner.set_host_object_name(to, "FormerTargetName"));
            let tracker = gamelogic::scripting::engine::get_named_object_tracker();
            tracker
                .register_named_object("TransferredName".into(), from.0)
                .unwrap();
            tracker
                .register_named_object("FormerTargetName".into(), to.0)
                .unwrap();
            assert!(owner.transfer_script_object_name(from, to));
            assert!(owner.host_object(from).unwrap().name.is_empty());
            assert_eq!(
                owner.host_object_id_by_script_name("TransferredName"),
                Some(to)
            );
            assert_eq!(
                owner.host_object_id_by_script_name("FormerTargetName"),
                None,
                "a historical cache alias is not a current object's name"
            );
            let snapshotter = SnapshotBuilder::new();
            let saved = snapshotter.create_world_snapshot(&owner).unwrap();
            let (mut receiving, _) = world("UnrelatedReceivingName");
            snapshotter
                .restore_from_snapshot(&saved, &mut receiving)
                .unwrap();
            assert_eq!(
                receiving.host_object_id_by_script_name("TransferredName"),
                Some(to)
            );
            assert_eq!(
                receiving.host_object_id_by_script_name("FormerTargetName"),
                None
            );
            assert_eq!(
                receiving.host_object_id_by_script_name("UnrelatedReceivingName"),
                None
            );
            assert!(receiving.set_host_object_name(to, "AfterLoadName"));
            assert_eq!(
                receiving.host_object_id_by_script_name("TransferredName"),
                None
            );
            assert_eq!(
                receiving.host_object_id_by_script_name("AfterLoadName"),
                Some(to)
            );
            assert_eq!(
                owner.host_object_id_by_script_name("TransferredName"),
                Some(to)
            );
        },
    );
}
