use super::*;

#[test]
fn actual_definition_parser_retains_sparse_slots_without_asset_registry() {
    let module = crate::assets::ini_parser::BehaviorModuleDefinition {
        class_name: "TransitionDamageFX".into(),
        module_tag: None,
        attributes: [
            (
                "DamagedFXList3".into(),
                "Loc: X:3 Y:4 Z:5 FXList:FX_Third".into(),
            ),
            ("DamagedOCL1".into(), "OCL:OCL_First".into()),
            ("DamagedParticleSystem12".into(), "PSys:Smoke_Last".into()),
        ]
        .into_iter()
        .collect(),
    };
    let mut data = HostTransitionDamageFxData::vehicle_residual();
    overlay_authored_transition_module(&mut data, &module);
    assert_eq!(data.fx_lists_for_state[1], vec!["", "", "FX_Third"]);
    assert_eq!(data.fx_locs_for_state[1][2].loc, [3., 4., 5.]);
    assert_eq!(data.ocl_for_state[1], vec!["OCL_First"]);
    assert_eq!(data.particles_for_state[1].len(), 12);
    assert!(data.particles_for_state[1][0].name.is_empty());
    assert_eq!(data.particles_for_state[1][11].name, "Smoke_Last");
    let event = on_body_damage_state_change(
        &data,
        HostBodyDamageType::Pristine,
        HostBodyDamageType::Damaged,
    )
    .unwrap();
    assert!(event.fx_name.is_none());
    assert_eq!(event.extra_fx_names, vec!["", "FX_Third"]);
}

#[test]
fn failed_retirement_retains_receipt_in_its_authored_slot() {
    let mut ids = [10, 0, 20];
    let mut attempts = Vec::new();
    retire_old_transition_ids(&mut ids, |id| {
        attempts.push(id);
        id != 20
    });
    assert_eq!(attempts, vec![10, 20]);
    assert_eq!(ids, [0, 0, 20]);
    retire_old_transition_ids(&mut ids, |_| true);
    assert_eq!(ids, [0, 0, 0]);
}

#[derive(serde::Deserialize)]
struct CppCase {
    name: String,
    arguments: Vec<u32>,
    trace: Vec<String>,
}

#[test]
fn original_cpp_callback_matches_slot_selection_cleanup_and_effect_order() {
    // Expected data is produced only by cpp_transition_slots_oracle.py's verbatim
    // original callback. This checks selection/order and successful cleanup, not
    // native allocation, bone sampling or full-game equivalence.
    let cases: Vec<CppCase> =
        serde_json::from_str(include_str!("cpp_transition_slots.json")).unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let a = &case.arguments;
        let mut data = HostTransitionDamageFxData::vehicle_residual();
        let state = a[1] as usize;
        for (bits, names, prefix) in [
            (a[2], &mut data.fx_lists_for_state[state], "FX"),
            (a[3], &mut data.ocl_for_state[state], "OCL"),
        ] {
            names.resize(12, String::new());
            for slot in 0..12 {
                if bits & (1 << slot) != 0 {
                    names[slot] = format!("{prefix}_{slot}");
                }
            }
        }
        data.particles_for_state[state].resize(12, HostTransitionParticle::default());
        for slot in 0..12 {
            if a[4] & (1 << slot) != 0 {
                data.particles_for_state[state][slot].name = format!("PS_{slot}");
            }
        }
        data.damage_fx_types = if a[6] != 0 { u64::MAX } else { 0 };
        data.damage_ocl_types = if a[7] != 0 { u64::MAX } else { 0 };
        data.damage_particle_types = if a[8] != 0 { u64::MAX } else { 0 };
        let old = HostBodyDamageType::from_ordinal(a[0] as u8);
        let new = HostBodyDamageType::from_ordinal(a[1] as u8);
        let event = on_body_damage_state_change_for_damage(
            &data,
            old,
            new,
            (a[9] != 0).then_some(gamelogic::damage::DamageType::Flame),
        )
        .unwrap();
        let mut ids = [0; 12];
        for slot in 0..12 {
            if a[5] & (1 << slot) != 0 {
                ids[slot] = 100 + slot as u32;
            }
        }
        let mut actual = Vec::new();
        assert_eq!(event.clear_old_state, Some(old.ordinal()));
        retire_old_transition_ids(&mut ids, |id| {
            actual.push(format!("destroy:{id}"));
            true
        });
        for_each_transition_slot(&event, |effect| {
            actual.push(match effect {
                TransitionSlotEffect::Fx { slot, .. } => format!("fx:{slot}"),
                TransitionSlotEffect::Ocl { slot, .. } => format!("ocl:{slot}"),
                TransitionSlotEffect::Particle { slot, .. } => format!("create:{slot}"),
            })
        });
        actual.push(format!(
            "old_ids:{}",
            ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        ));
        let expected: Vec<_> = case
            .trace
            .into_iter()
            .filter(|line| {
                ["destroy:", "fx:", "ocl:", "create:", "old_ids:"]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            })
            .collect();
        assert_eq!(actual, expected, "original C++ case {}", case.name);
    }
}

#[cfg(feature = "game_client")]
#[test]
fn native_transition_creates_and_retires_sparse_attachments_inline_without_replay() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_transition_creates_and_retires_sparse_attachments_inline_without_replay",
        || {
            use crate::game_logic::combat_particles::{CombatParticleKind, CombatParticleRegistry};
            use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
            use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
            let mut template = ThingTemplate::new("InlineTransitionOwner");
            template.set_health(100.);
            let mut object = Object::new(template, ObjectId(9938), Team::USA);
            object.set_position(glam::Vec3::new(10., 4., 12.));
            let mut registry = CombatParticleRegistry::new();
            let old = registry.spawn(
                CombatParticleKind::DeathSmoke,
                object.get_position(),
                0,
                Some(object.id),
                None,
            );
            let mut data = HostTransitionDamageFxData::vehicle_residual();
            data.attached_ids[0] = vec![old];
            data.fx_lists_for_state[1] = vec![
                "FX_InlineTransition".into(),
                "".into(),
                "FX_InlineTransition".into(),
            ];
            data.fx_locs_for_state[1] = vec![
                HostTransitionLoc::default(),
                HostTransitionLoc::default(),
                HostTransitionLoc {
                    loc: [3., 4., 5.],
                    ..Default::default()
                },
            ];
            data.particles_for_state[1] = vec![
                HostTransitionParticle {
                    name: "InlineSmokeFirst".into(),
                    loc: [1., 2., 3.],
                    ..Default::default()
                },
                HostTransitionParticle::default(),
                HostTransitionParticle {
                    name: "InlineSmokeThird".into(),
                    ..Default::default()
                },
            ];
            data.audio_for_state[1] = Some("InlineTransitionAudio".into());
            let event = on_body_damage_state_change(
                &data,
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .unwrap();
            object.transition_damage_fx = Some(data);
            let mut definition = FXList::new("FX_InlineTransition".into());
            definition.add_nugget(FXNugget::ViewShake {
                shake_type: CameraShakeType::Normal,
            });
            let definition =
                std::sync::Arc::new(game_client::fx_list::FXList::from_definition(&definition));
            let mut view_owner = crate::presentation_frame::SessionView::default();
            view_owner.target = glam::Vec3::new(10., 0., 12.);
            let mut rules = game_engine::common::global_data::GlobalData::default();
            rules.shake_normal_intensity = 2.;
            rules.max_shake_range = 1000.;
            rules.max_shake_intensity = 100.;
            let mut positions = Vec::new();
            {
                let mut shake =
                    |p: &gamelogic::common::Coord3D,
                     kind: game_client::display::view::CameraShakeType| {
                        positions.push([p.x, p.y, p.z]);
                        view_owner.enqueue(
                            glam::Vec3::new(p.x, p.z, p.y),
                            kind.into(),
                            &rules,
                            &mut |_, _| 0.,
                        );
                    };
                let mut find = |name: &str| {
                    (name == "FX_InlineTransition").then(|| std::sync::Arc::clone(&definition))
                };
                let mut bones = |_, _: &str, _, _| Vec::new();
                let mut visible = |_: Option<&gamelogic::common::Coord3D>| true;
                let mut view = game_client::fx_list::FxViewServices {
                    shake: &mut shake,
                    find_list: &mut find,
                    bone_transforms: &mut bones,
                    position_visible: &mut visible,
                };
                GameLogic::execute_transition_damage_fx_with_view(
                    &mut object,
                    event,
                    None,
                    &mut registry,
                    9,
                    &mut view,
                );
                assert!(
                    !registry.get(old).unwrap().active,
                    "old system retires in callback, without an AI tick"
                );
                assert_eq!(registry.active_count(), 2);
                let ids = &object.transition_damage_fx.as_ref().unwrap().attached_ids[1];
                assert_eq!(ids.len(), 3);
                assert_ne!(ids[0], 0);
                assert_eq!(ids[1], 0);
                assert_ne!(ids[2], 0);
                let first = registry.get(ids[0]).unwrap();
                assert_eq!(first.attach_offset, glam::Vec3::new(1., 3., 2.));
                assert_eq!(first.source_object, Some(object.id));
                assert_eq!(first.spawned_frame, 9);
                assert_eq!(first.template_name, "InlineSmokeFirst");
                let remainder = &object.pending_transition_damage_fx;
                assert_eq!(remainder.len(), 1);
                assert_eq!(
                    remainder[0].audio_name.as_deref(),
                    Some("InlineTransitionAudio")
                );
                assert!(
                    remainder[0].fx_name.is_none()
                        && remainder[0].extra_fx_names.is_empty()
                        && remainder[0].ocl_names.is_empty()
                        && remainder[0].particles.is_empty()
                        && remainder[0].clear_old_state.is_none()
                );
                let healing = on_body_damage_state_change(
                    object.transition_damage_fx.as_ref().unwrap(),
                    HostBodyDamageType::Damaged,
                    HostBodyDamageType::Pristine,
                )
                .unwrap();
                GameLogic::execute_transition_damage_fx_with_view(
                    &mut object,
                    healing,
                    None,
                    &mut registry,
                    10,
                    &mut view,
                );
                assert_eq!(
                    registry.active_count(),
                    0,
                    "improving state only retires old attachments"
                );
                assert!(
                    object.transition_damage_fx.as_ref().unwrap().attached_ids[1]
                        .iter()
                        .all(|id| *id == 0)
                );
            }
            assert_eq!(positions, vec![[10., 12., 4.], [13., 16., 9.]]);
            assert!(view_owner.impulse.intensity > 2.);
            assert_eq!(
                view_owner.impulse.offset,
                glam::Vec3::ZERO,
                "enqueue never advances the view"
            );
        },
    );
}
