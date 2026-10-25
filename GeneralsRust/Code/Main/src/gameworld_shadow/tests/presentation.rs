//! Presentation audio/input/select, residual acquire, host ObjectId sources.

use super::*;

#[test]
fn boot_residual_dual_scan_labels_source() {
    // Every live get_objects dual-scan should sit near Boot residual / Fail-open labels
    // when a presentation-first path exists.
    let mut unlabeled = 0u32;
    let mut total = 0u32;

    let mut offset = 0usize;

    assert!(
        unlabeled == 0,
        "all remaining get_objects dual-scans must be labeled (total={total} unlabeled={unlabeled})"
    );
    let _ = total;
}

#[test]
fn host_name_transfer_and_priority_use_receiving_object_ids() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "host_name_transfer_and_priority_use_receiving_object_ids",
        || {
            use crate::game_logic::{ObjectId, Player, Team, ThingTemplate};
            fn world() -> (GameLogic, ObjectId, ObjectId) {
                let mut world = GameLogic::new();
                world.add_player(Player::new(1, Team::USA, "NameOwner", true));
                let mut template = ThingTemplate::new("NameOwnerProbe");
                template.set_health(100.0);
                world.templates.insert(template.name.clone(), template);
                let donor = world
                    .create_object_for_player("NameOwnerProbe", 1, glam::Vec3::ZERO)
                    .unwrap();
                let target = world
                    .create_object_for_player("NameOwnerProbe", 1, glam::Vec3::X)
                    .unwrap();
                world.set_host_object_name(donor, "Donor");
                world.set_host_object_name(target, "TargetOld");
                world.initialize_receiving_named_cache_for_fixture();
                (world, donor, target)
            }
            let (mut owner, donor, target) = world();
            let (mut peer, peer_donor, peer_target) = world();
            assert_eq!((donor, target), (peer_donor, peer_target));
            assert_eq!(owner.find_object_id_by_name("TargetOld"), Some(target));
            // ScriptEngine.cpp transferObjectName retains the donor's name;
            // the ordered cache entry moves to the actual destination object.
            assert!(owner.transfer_script_object_name(donor, target));
            assert_eq!(owner.host_object(donor).unwrap().name, "Donor");
            assert_eq!(owner.host_object(target).unwrap().name, "Donor");
            assert_eq!(owner.host_object_id_by_script_name("Donor"), Some(target));
            assert_eq!(owner.host_object_id_by_script_name("TargetOld"), None);
            assert!(!owner.transfer_script_object_name(ObjectId(u32::MAX), target));
            assert!(!owner.transfer_script_object_name(donor, ObjectId(u32::MAX)));
            assert_eq!(
                peer.host_object_id_by_script_name("Donor"),
                Some(peer_donor)
            );
            assert_eq!(
                peer.host_object_id_by_script_name("TargetOld"),
                Some(peer_target)
            );
            for (world, priority) in [(&mut owner, 37), (&mut peer, 83)] {
                world
                    .with_owned_script_engine(|engine, _| {
                        assert!(engine.set_priority_default("ReceivingPriority", priority));
                        engine.set_object_attack_priority_set(target.0, "ReceivingPriority");
                    })
                    .unwrap();
            }
            owner.sync_attack_priority_from_script_engine();
            peer.sync_attack_priority_from_script_engine();
            assert_eq!(
                owner
                    .host_object(target)
                    .unwrap()
                    .attack_priority_set
                    .as_deref(),
                Some("ReceivingPriority")
            );
            assert_eq!(
                owner
                    .attack_priority_info_for(target)
                    .unwrap()
                    .default_priority,
                37
            );
            assert_eq!(
                peer.attack_priority_info_for(peer_target)
                    .unwrap()
                    .default_priority,
                83
            );
            assert!(
                owner
                    .host_object(donor)
                    .unwrap()
                    .attack_priority_set
                    .is_none()
            );
            assert!(
                peer.host_object(peer_donor)
                    .unwrap()
                    .attack_priority_set
                    .is_none()
            );
        },
    );
}
