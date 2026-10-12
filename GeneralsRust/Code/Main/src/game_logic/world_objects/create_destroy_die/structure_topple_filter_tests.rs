//! Real authored Main admission and phase callbacks, original Die/FX predicates.
use super::*;

#[test]
fn owned_die_mux_rejection_precedes_topple_selection_ai_and_effect_mutation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "owned_die_mux_rejection_precedes_topple_selection_ai_and_effect_mutation",
        || {
            let effects = install_fx();
            for (filter, hijacked, raw_hijacked) in [
                ("DeathTypes = NONE", false, false),
                ("VeterancyLevels = ALL -REGULAR", false, false),
                ("RequiredStatus = HIJACKED", false, true),
                ("ExemptStatus = HIJACKED", true, false),
            ] {
                let (mut owner, id) = world_with_filters(0, 10.0, None, filter);
                let object = owner.host_object_mut(id).unwrap();
                object.selected = true;
                object.status.selected = true;
                object.ai_state = AIState::Moving;
                object.status.hijacked = hijacked;
                object.object_status_bits = if raw_hijacked {
                    crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names(&[
                        "HIJACKED",
                    ])
                } else {
                    0
                };
                let before = object.get_transform_matrix();
                assert!(!owner.try_begin_structure_topple_instead_of_destroy(id));
                let object = owner.host_object(id).unwrap();
                assert!(object.selected && object.status.selected);
                assert_eq!(object.ai_state, AIState::Moving);
                assert!(object.structure_topple_data.is_none());
                assert_eq!(object.get_transform_matrix(), before);
                assert_eq!(effects.try_iter().count(), 0, "{filter}");
            }
            let (mut admitted, id) = world_with_filters(
                0,
                10.0,
                None,
                "DeathTypes = NONE +NORMAL\nVeterancyLevels = NONE +VETERAN\nRequiredStatus = HIJACKED\nExemptStatus = BOOBY_TRAPPED",
            );
            let object = admitted.host_object_mut(id).unwrap();
            object.status.hijacked = true;
            object.experience.level = VeterancyLevel::Veteran;
            object.selected = true;
            object.status.selected = true;
            object.ai_state = AIState::Moving;
            assert!(admitted.try_begin_structure_topple_instead_of_destroy(id));
            let object = admitted.host_object(id).unwrap();
            assert!(!object.selected && !object.status.selected);
            assert_eq!(object.ai_state, AIState::Idle);
            assert!(object.structure_topple_data.is_some());
            assert_eq!(effects.try_iter().count(), 1);
        },
    );
}

#[test]
fn actual_on_die_reads_death_type_from_its_same_id_owner() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_on_die_reads_death_type_from_its_same_id_owner",
        || {
            let effects = install_fx();
            let (mut owner, id) = world_with_filters(0, 10.0, None, "DeathTypes = NONE +CRUSHED");
            let (mut peer, peer_id) =
                world_with_filters(7, 30.0, None, "DeathTypes = NONE +CRUSHED");
            assert_eq!(id, peer_id);
            // The default kill helper deliberately asserts an admitted
            // toppler. Here the original DieMux must reject NORMAL after
            // the actual body/onDie producer has recorded that death type.
            assert!(
                owner
                    .apply_owned_kill(
                        id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal
                    )
                    .unwrap()
                    .destroyed
            );
            let rejected = owner.host_object(id).unwrap();
            assert_eq!(rejected.health.current, 0.0);
            assert!(rejected.status.on_die_started);
            assert_eq!(
                rejected.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal
            );
            assert!(
                owner
                    .host_object(id)
                    .unwrap()
                    .structure_topple_data
                    .is_none()
            );
            assert_eq!(effects.try_iter().count(), 0);
            assert!(
                peer.apply_owned_kill(
                    peer_id,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Crushed
                )
                .unwrap()
                .destroyed
            );
            assert!(
                peer.host_object(peer_id)
                    .unwrap()
                    .structure_topple_data
                    .is_some()
            );
            let admitted = peer.host_object(peer_id).unwrap();
            assert_eq!(admitted.health.current, 0.0);
            assert!(admitted.status.on_die_started);
            assert_eq!(
                admitted.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed
            );
            assert_eq!(effects.try_iter().count(), 1);
            assert!(
                owner
                    .host_object(id)
                    .unwrap()
                    .structure_topple_data
                    .is_none()
            );
        },
    );
}

#[test]
fn damage_fx_none_suppresses_every_phase_but_not_crushing_or_completion() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "damage_fx_none_suppresses_every_phase_but_not_crushing_or_completion",
        || {
            const WEAPON: &str = "OwnedFilteredToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 10.0);
            let (mut owner, id) = world_with_filters(0, 10.0, Some(WEAPON), "DamageFXTypes = NONE");
            kill(&mut owner, id);
            assert_eq!(
                effects.try_iter().count(),
                0,
                "Start FX must be filtered too"
            );
            let before = owner.host_object(id).unwrap().get_transform_matrix();
            let mut template = ThingTemplate::new("FilteredCrushVictim");
            template.add_kind_of(KindOf::Structure).set_health(100.0);
            owner.templates.insert(template.name.clone(), template);
            let position = glam::Vec3::new(20.0, 0.0, 12.0);
            let victim = owner
                .create_object_for_player("FilteredCrushVictim", 0, position)
                .unwrap();
            owner
                .apply_structure_topple_crush_samples(
                    id,
                    vec![crushing_point(position), crushing_point(position)],
                )
                .unwrap();
            assert_eq!(owner.host_object(victim).unwrap().health.current, 80.0);
            assert_eq!(
                effects.try_iter().count(),
                0,
                "only point FX is filtered, not either real weapon hit"
            );
            let mut completed = false;
            let mut rotated = false;
            for frame in 0..600 {
                owner.frame = frame;
                let finished = owner.update_owned_structure_topple(id).unwrap();
                rotated |= owner.host_object(id).unwrap().get_transform_matrix() != before;
                if finished {
                    completed = true;
                    break;
                }
                assert_eq!(effects.try_iter().count(), 0);
            }
            assert!(completed);
            let object = owner.host_object(id).unwrap();
            assert_eq!(object.health.current, 0.0);
            assert_eq!(
                object.structure_topple_data.as_ref().unwrap().state,
                crate::game_logic::host_structure_topple::HostStructureToppleState::Done
            );
            assert!(rotated);
            assert_eq!(effects.try_iter().count(), 0);
        },
    );
}

#[test]
fn current_damage_type_is_observed_again_and_no_body_history_permits_fx() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "current_damage_type_is_observed_again_and_no_body_history_permits_fx",
        || {
            let effects = install_fx();
            let (mut owner, id) = world_with_filters(0, 10.0, None, "DamageFXTypes = NONE +FLAME");
            assert!(
                owner
                    .apply_owned_kill(
                        id,
                        crate::game_logic::combat::DamageType::Flame,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal
                    )
                    .unwrap()
                    .destroyed
            );
            assert_eq!(
                effects
                    .try_iter()
                    .filter(|effect| effect.name == "FX_OwnToppleStart")
                    .count(),
                1
            );
            owner.host_object_mut(id).unwrap().last_damage_info_type =
                Some(crate::game_logic::combat::DamageType::Unresistable);
            owner.update_owned_structure_topple(id).unwrap();
            assert_eq!(
                effects.try_iter().count(),
                0,
                "later Angle/Delay reread body history"
            );
            owner.host_object_mut(id).unwrap().last_damage_info_type = None;
            let mut saw_delay = false;
            let mut saw_done = false;
            for frame in 1..600 {
                owner.frame = frame;
                let complete = owner.update_owned_structure_topple(id).unwrap();
                for effect in effects.try_iter() {
                    saw_delay |= effect.name == "FX_OwnToppleDelay";
                    saw_done |= effect.name == "FX_OwnToppleDone";
                }
                if complete {
                    break;
                }
            }
            assert!(
                saw_delay && saw_done,
                "CPP absent DamageInfo permits FX even for a restricted mask"
            );
        },
    );
}

#[test]
fn filtered_saved_future_restores_actual_body_history_and_own_phase_state() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "filtered_saved_future_restores_actual_body_history_and_own_phase_state",
        || {
            let effects = install_fx();
            let (mut source, id) = world_with_filters(0, 10.0, None, "DamageFXTypes = NONE +FLAME");
            kill(&mut source, id);
            for frame in 0..6 {
                source.frame = frame;
                source.update_owned_structure_topple(id).unwrap();
            }
            assert_eq!(effects.try_iter().count(), 0);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let (mut receiving, other) =
                world_with_filters(0, 10.0, None, "DamageFXTypes = NONE +FLAME");
            assert_eq!(id, other);
            receiving.host_object_mut(id).unwrap().last_damage_info_type =
                Some(crate::game_logic::combat::DamageType::Flame);
            builder
                .restore_from_snapshot(&snapshot, &mut receiving)
                .unwrap();
            assert_eq!(
                receiving.host_object(id).unwrap().last_damage_info_type,
                Some(crate::game_logic::combat::DamageType::Unresistable)
            );
            for frame in 6..600 {
                source.frame = frame;
                receiving.frame = frame;
                let completed = source.update_owned_structure_topple(id).unwrap();
                assert_eq!(
                    completed,
                    receiving.update_owned_structure_topple(id).unwrap()
                );
                assert_eq!(
                    source.host_object(id).unwrap().get_transform_matrix(),
                    receiving.host_object(id).unwrap().get_transform_matrix()
                );
                assert_eq!(effects.try_iter().count(), 0);
                if completed {
                    return;
                }
            }
            panic!("original topple phases must complete");
        },
    );
}
