//! Source cache kernel plus actual receiving update/save/event/stage witnesses.
use super::*;
#[cfg(test)]
mod receiving_tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
    pub(super) fn world(name: &str) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "NamedCacheOwner", true));
        let mut template = ThingTemplate::new("NamedCacheProbe");
        template.set_health(100.0);
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object_for_player("NamedCacheProbe", 1, glam::Vec3::ZERO)
            .unwrap();
        world.set_host_object_name(id, name);
        (world, id)
    }
    pub(super) fn first_update(world: &mut GameLogic) {
        world.scripts_loaded = true;
        world.evaluate_and_execute_scripts(1.0 / 30.0);
    }
    #[test]
    fn named_map_rejects_stale_same_id_first_receipt_and_hides_later_live_duplicate() {
        let (mut world, id) = world("Stale");
        let live_birth = Arc::clone(&world.host_object(id).unwrap().visual_draw_identity_token);
        let stale_birth = Arc::new(());
        world.named_script_cache.seed([
            ("Stale".into(), id, Arc::clone(&stale_birth)),
            ("Stale".into(), id, Arc::clone(&live_birth)),
            ("stale".into(), id, Arc::clone(&live_birth)),
        ]);

        let expected = std::collections::HashMap::from([("stale".to_owned(), id.0)]);
        assert_eq!(world.host_object_id_by_script_name("Stale"), None);
        assert_eq!(world.host_named_unit_id_map(), expected);
        assert_eq!(
            world.named_script_cache.wire(),
            [("Stale".into(), id.0), ("Stale".into(), id.0), ("stale".into(), id.0)]
        );

        world.named_script_cache.remove(id, &stale_birth);
        assert!(world.named_script_cache.did_exist("Stale"));
        assert_eq!(world.host_named_unit_id_map(), expected);
    }

    #[test]
    fn snapshot_rejects_a_cached_name_whose_object_id_was_reused() {
        use crate::game_logic::Object;
        use crate::save_load::{SaveLoadError, SnapshotBuilder};

        let (mut owner, donor) = world("Donor");
        let target = owner
            .create_object_for_player("NamedCacheProbe", 1, glam::Vec3::X)
            .unwrap();
        assert!(owner.set_host_object_name(target, "Original"));
        first_update(&mut owner);
        owner.host_object_mut(target).unwrap().name.clear();
        assert!(owner.transfer_script_object_name(donor, target));
        assert_eq!(owner.host_object_id_by_script_name("Donor"), Some(target));
        assert_eq!(owner.host_object_id_by_script_name("Original"), Some(target));
        owner.destroy_object(target).unwrap();
        owner.process_destroy_list();

        let template = owner.templates["NamedCacheProbe"].clone();
        assert_eq!(
            owner
                .try_add_object(Object::new(template, target, Team::USA))
                .unwrap(),
            target
        );
        assert_eq!(owner.host_object_id_by_script_name("Donor"), None);
        assert_eq!(owner.host_object_id_by_script_name("Original"), None);
        let builder = SnapshotBuilder::new();
        let Err(SaveLoadError::Serialization(error)) = builder.create_world_snapshot(&owner) else {
            panic!("snapshot must refuse a stale named binding");
        };
        assert!(
            error.contains(&format!("stale named cache object {target}")),
            "{error}"
        );

        let (mut peer, _) = world("PeerDonor");
        let peer_target = peer
            .create_object_for_player("NamedCacheProbe", 1, glam::Vec3::X)
            .unwrap();
        assert_eq!(peer_target, target);
        assert!(peer.set_host_object_name(peer_target, "Original"));
        first_update(&mut peer);
        let saved = builder.create_world_snapshot(&peer).unwrap();
        let (mut receiver, _) = world("Other");
        builder.restore_from_snapshot(&saved, &mut receiver).unwrap();
        assert_eq!(receiver.host_object_id_by_script_name("Original"), Some(target));
    }

    #[test]
    fn actual_first_script_update_seeds_once_and_plain_set_name_does_not_rewrite_cached_pointer() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_first_script_update_seeds_once_and_plain_set_name_does_not_rewrite_cached_pointer",
            || {
                let (mut owner, id) = world("Original");
                let (mut peer, peer_id) = world("Peer");
                assert_eq!(id, peer_id);
                assert_eq!(
                    owner.host_object_id_by_script_name("Original"),
                    None,
                    "original first-update phase has not run"
                );
                first_update(&mut owner);
                first_update(&mut peer);
                assert_eq!(owner.host_object_id_by_script_name("Original"), Some(id));
                owner.set_host_object_name(id, "Renamed");
                first_update(&mut owner);
                assert_eq!(
                    owner.host_object_id_by_script_name("Original"),
                    Some(id),
                    "Object::setName does not edit ScriptEngine cache"
                );
                assert_eq!(owner.host_object_id_by_script_name("Renamed"), None);
                owner.add_receiving_named_script_object(id);
                assert_eq!(owner.host_object_id_by_script_name("Original"), None);
                assert_eq!(owner.host_object_id_by_script_name("Renamed"), Some(id));
                assert_eq!(peer.host_object_id_by_script_name("Peer"), Some(peer_id));
                assert_eq!(peer.host_object_id_by_script_name("Renamed"), None);
            },
        );
    }
    #[test]
    fn actual_transfer_keeps_donor_name_color_and_first_tombstone_through_encoded_future() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_transfer_keeps_donor_name_color_and_first_tombstone_through_encoded_future",
            || {
                let (mut owner, donor) = world("Donor");
                let target = owner
                    .create_object_for_player("NamedCacheProbe", 1, glam::Vec3::new(20.0, 0.0, 0.0))
                    .unwrap();
                owner.set_host_object_name(target, "TargetOld");
                first_update(&mut owner);
                owner
                    .host_object_mut(donor)
                    .unwrap()
                    .set_custom_indicator_color_raw(0xff123456);
                assert!(owner.transfer_script_object_name(donor, target));
                assert_eq!(owner.host_object(donor).unwrap().name, "Donor");
                assert_eq!(
                    owner.host_object(target).unwrap().custom_indicator_color,
                    Some(0xff123456)
                );
                assert_eq!(owner.host_object_id_by_script_name("Donor"), Some(target));
                assert_eq!(owner.host_object_id_by_script_name("TargetOld"), None);
                assert!(owner.named_script_cache.did_exist("TargetOld"));
                let tracker = gamelogic::scripting::engine::get_named_object_tracker();
                tracker
                    .register_named_object("ForeignOnly".into(), 999)
                    .unwrap();
                let builder = crate::save_load::SnapshotBuilder::new();
                let snapshot = builder.create_world_snapshot(&owner).unwrap();
                let wire = bincode_legacy::serialize(&snapshot).unwrap();
                let decoded =
                    crate::save_load::snapshot::decode_bincode_world_snapshot(&wire).unwrap();
                let (mut restored, _) = world("Unrelated");
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
                assert_eq!(
                    restored.named_script_cache.wire(),
                    owner.named_script_cache.wire()
                );
                assert_eq!(
                    tracker.get_object_id("ForeignOnly").unwrap(),
                    Some(999),
                    "restore never clears foreign tracker"
                );
                owner.destroy_object(target).unwrap();
                restored.destroy_object(target).unwrap();
                owner.process_destroy_list();
                restored.process_destroy_list();
                assert!(owner.named_script_cache.did_exist("Donor"));
                assert!(restored.named_script_cache.did_exist("Donor"));
                assert_eq!(
                    restored.host_object_id_by_script_name("Donor"),
                    None,
                    "a live donor Object.name cannot bypass first tombstone"
                );
                assert_eq!(restored.host_object(donor).unwrap().name, "Donor");
            },
        );
    }
    #[test]
    fn actual_restore_rejects_valid_then_foreign_named_row_before_running_world_mutation() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_restore_rejects_valid_then_foreign_named_row_before_running_world_mutation",
            || {
                let (mut source, id) = world("Source");
                first_update(&mut source);
                let (mut live, live_id) = world("Live");
                first_update(&mut live);
                let before = live.named_script_cache.wire();
                let builder = crate::save_load::SnapshotBuilder::new();
                let mut snapshot = builder.create_world_snapshot(&source).unwrap();
                snapshot
                    .persist_v18
                    .script_engine_tail
                    .as_mut()
                    .unwrap()
                    .named_objects = vec![("Valid".into(), id.0), ("Foreign".into(), 999_999)];
                assert!(builder.restore_from_snapshot(&snapshot, &mut live).is_err());
                assert_eq!(live.named_script_cache.wire(), before);
                assert_eq!(live.host_object_id_by_script_name("Live"), Some(live_id));
                assert_eq!(source.host_object_id_by_script_name("Source"), Some(id));
            },
        );
    }
}

#[cfg(test)]
mod initial_and_stage_tests {
    use super::receiving_tests::{first_update, world};
    use super::*;
    #[test]
    fn legitimate_initial_cache_rebuilds_from_restored_roster_at_actual_first_update() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "legitimate_initial_cache_rebuilds_from_restored_roster_at_actual_first_update",
            || {
                let (source, id) = world("InitialName");
                assert!(source.named_script_cache.wire().is_empty());
                let builder = crate::save_load::SnapshotBuilder::new();
                let snapshot = builder.create_world_snapshot(&source).unwrap();
                assert!(
                    snapshot
                        .persist_v18
                        .script_engine_tail
                        .as_ref()
                        .unwrap()
                        .first_update
                );
                let (mut restored, _) = world("Other");
                builder
                    .restore_from_snapshot(&snapshot, &mut restored)
                    .unwrap();
                assert_eq!(restored.host_object_id_by_script_name("InitialName"), None);
                first_update(&mut restored);
                assert_eq!(
                    restored.host_object_id_by_script_name("InitialName"),
                    Some(id)
                );
            },
        );
    }
    #[test]
    fn main_stage_does_not_take_or_publish_standalone_named_tracker() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "main_stage_does_not_take_or_publish_standalone_named_tracker",
            || {
                let tracker = gamelogic::scripting::engine::get_named_object_tracker();
                tracker
                    .register_named_object("ForeignNamed".into(), 999)
                    .unwrap();
                assert_eq!(tracker.get_object_id("ForeignNamed").unwrap(), Some(999));
                let (mut candidate, id) = world("CandidateName");
                let stage = gamelogic::runtime_world_transaction::HostRuntimeWorldStage::begin(
                    &candidate.world_services,
                );
                first_update(&mut candidate);
                assert_eq!(
                    candidate.host_object_id_by_script_name("CandidateName"),
                    Some(id)
                );
                let pending = stage.finish();
                drop(pending);
                assert_eq!(tracker.get_object_id("ForeignNamed").unwrap(), Some(999));
                assert_eq!(tracker.get_object_id("CandidateName").unwrap(), None);
            },
        );
    }
}

#[cfg(test)]
mod prepared_event_tests {
    use super::receiving_tests::{first_update, world};
    use super::*;
    use generals_sim::scripting::program::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
    };
    fn query(world: &mut crate::game_logic::GameLogic, source: &str) -> bool {
        let mut condition = Condition::new(ConditionType::PlayerBuiltUpgradeFromNamed);
        for (kind, value) in [
            (ParameterType::Side, "NamedCacheOwner"),
            (ParameterType::Upgrade, "OwnedUpgrade"),
            (ParameterType::Unit, source),
        ] {
            condition
                .add_parameter(Parameter::with_string(kind, value.into()))
                .unwrap();
        }
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut script = Script::new();
        script.condition = Some(Box::new(branch));
        world
            .with_owned_script_engine(|engine, world| {
                world.evaluate_owned_script_conditions(engine, &mut script, None)
            })
            .unwrap()
    }
    #[test]
    fn prepared_player_adapter_still_uses_actual_receiving_named_cache_before_consuming_edge() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "prepared_player_adapter_still_uses_actual_receiving_named_cache_before_consuming_edge",
            || {
                let (mut owner, id) = world("OwnSource");
                let (mut peer, peer_id) = world("ForeignOnly");
                first_update(&mut owner);
                first_update(&mut peer);
                assert_eq!(id, peer_id);
                owner.game_mode = crate::game_logic::GameMode::Skirmish;
                let mut foreign_player = gamelogic::player::Player::new(1);
                foreign_player.set_player_name_key(
                    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                        "NamedCacheOwner",
                    ),
                );
                let mut list = gamelogic::player::PlayerList::new();
                list.add_player(Arc::new(std::sync::RwLock::new(foreign_player)));
                *gamelogic::player::player_list().write().unwrap() = list;
                gamelogic::scripting::engine::get_named_object_tracker()
                    .register_named_object("ForeignOnly".into(), id.0)
                    .unwrap();
                owner
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .notify_of_completed_upgrade(1, "OwnedUpgrade", id.0);
                assert!(
                    !query(&mut owner, "ForeignOnly"),
                    "missing receiving source cannot consume via foreign tracker"
                );
                assert!(query(&mut owner, "OwnSource"));
                assert!(
                    !query(&mut owner, "OwnSource"),
                    "edge consumed once at actual source"
                );
                assert!(!query(&mut peer, "ForeignOnly"));
            },
        );
    }
}

#[cfg(test)]
mod load_reset_tests {
    use super::receiving_tests::{first_update, world};

    #[test]
    fn absent_legacy_tail_clears_prior_cache_and_transport_history_then_rebuilds_on_first_update() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "absent_legacy_tail_clears_prior_cache_and_transport_history_then_rebuilds_on_first_update",
            || {
                let (source, _) = world("");
                let mut persist =
                    crate::save_load::snapshot::persist_v18::capture_persist_v18(&source);
                persist.script_engine_tail = None;
                let (mut receiver, id) = world("BeforeLoad");
                let (mut peer, peer_id) = world("Peer");
                first_update(&mut receiver);
                first_update(&mut peer);
                receiver.set_host_object_name(id, "CurrentField");
                assert!(
                    !receiver
                        .owned_script_engine
                        .as_ref()
                        .unwrap()
                        .observe_receiving_transport_empty(id.0, 40, 1)
                );
                crate::save_load::snapshot::persist_v18::restore_persist_v18(
                    &persist,
                    &mut receiver,
                )
                .unwrap();
                assert!(receiver.named_script_cache.wire().is_empty());
                assert_eq!(receiver.host_object_id_by_script_name("BeforeLoad"), None);
                assert!(
                    !receiver
                        .owned_script_engine
                        .as_ref()
                        .unwrap()
                        .observe_receiving_transport_empty(id.0, 41, 0),
                    "load discards previous-frame observation"
                );
                first_update(&mut receiver);
                assert_eq!(
                    receiver.host_object_id_by_script_name("CurrentField"),
                    Some(id)
                );
                assert_eq!(peer.host_object_id_by_script_name("Peer"), Some(peer_id));
            },
        );
    }

    #[test]
    fn exact_tail_retains_saved_rows_and_first_update_flag_but_resets_transport_observations() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "exact_tail_retains_saved_rows_and_first_update_flag_but_resets_transport_observations",
            || {
                let (mut source, id) = world("SavedKey");
                first_update(&mut source);
                let persist = crate::save_load::snapshot::persist_v18::capture_persist_v18(&source);
                assert!(!persist.script_engine_tail.as_ref().unwrap().first_update);
                let (mut receiver, receiving_id) = world("ReceivingField");
                assert_eq!(id, receiving_id);
                first_update(&mut receiver);
                assert!(
                    !receiver
                        .owned_script_engine
                        .as_ref()
                        .unwrap()
                        .observe_receiving_transport_empty(id.0, 40, 1)
                );
                crate::save_load::snapshot::persist_v18::restore_persist_v18(
                    &persist,
                    &mut receiver,
                )
                .unwrap();
                assert_eq!(receiver.host_object_id_by_script_name("SavedKey"), Some(id));
                assert!(
                    !receiver
                        .owned_script_engine
                        .as_ref()
                        .unwrap()
                        .observe_receiving_transport_empty(id.0, 41, 0)
                );
                first_update(&mut receiver);
                assert_eq!(receiver.host_object_id_by_script_name("SavedKey"), Some(id));
                assert_eq!(
                    receiver.host_object_id_by_script_name("ReceivingField"),
                    None
                );
            },
        );
    }
}
