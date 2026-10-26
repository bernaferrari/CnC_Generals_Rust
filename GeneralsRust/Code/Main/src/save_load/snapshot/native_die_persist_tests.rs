use super::*;
use crate::assets::ini_parser::BehaviorModuleDefinition;
use crate::game_logic::{Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;

fn owner(tag: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("NativeReceiptOwner");
    template.set_health(100.);
    template.body_kind = Some(crate::game_logic::thing::BodyKind::Active);
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&[BehaviorModuleDefinition {
            class_name: "KeepObjectDie".into(),
            module_tag: Some(tag.into()),
            attributes: HashMap::new(),
        }])
        .into_boxed_slice(),
    );
    template.set_authored_ai_update_interface(Some(false));
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("NativeReceiptOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}
// Rebuild through the actual factory so the replacement has its own
// registered lifetime and partition receipt, rather than overwriting a live slot.
fn recreate_receipt_owner(source: &mut GameLogic, template: ThingTemplate, id: ObjectId) {
    let name = template.name.clone();
    source.reset();
    source.templates.insert(name.clone(), template);
    assert_eq!(
        source.create_object(&name, Team::USA, glam::Vec3::ZERO),
        Some(id)
    );
}
#[test]
fn native_die_capsule_roundtrip_retains_receipts_without_touching_equal_id_peer() {
    let (mut source, id) = owner("KeepA");
    let (mut receiving, receiving_id) = owner("KeepA");
    let (peer, peer_id) = owner("KeepA");
    assert_eq!(id, receiving_id);
    assert_eq!(id, peer_id);
    source
        .apply_owned_damage(
            id,
            100.,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
        )
        .unwrap();
    assert!(source.host_object(id).unwrap().status.on_die_started);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert!(
        receiving
            .host_object(id)
            .unwrap()
            .native_die_runtime
            .completed
            .contains(&0)
    );
    assert!(
        peer.host_object(id)
            .unwrap()
            .native_die_runtime
            .completed
            .is_empty()
    );
}
#[test]
fn native_die_capsule_wrong_receiving_identity_rejects_before_world_mutation() {
    let (source, _) = owner("KeepA");
    let (mut receiving, id) = owner("KeepB");
    receiving.set_current_frame(915);
    receiving.set_fixture_object_position(id, glam::Vec3::new(7., 8., 9.));
    let before = receiving.host_object(id).unwrap().get_position();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let error = SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("receiving Die declarations differ")
    );
    assert_eq!(receiving.get_current_frame(), 915);
    assert_eq!(receiving.host_object(id).unwrap().get_position(), before);
}
#[test]
fn native_die_legacy_zero_health_live_is_allowed_but_saved_started_is_rejected() {
    // Actual InactiveBody construction admits zero HP without entering onDie.
    let (mut source, id) = owner("KeepA");
    let mut template = source.templates["NativeReceiptOwner"].clone();
    template.body_kind = Some(crate::game_logic::thing::BodyKind::Inactive);
    template.set_health(0.);
    source
        .templates
        .insert(template.name.clone(), template.clone());
    recreate_receipt_owner(&mut source, template, id);
    assert!(source.host_object(id).unwrap().is_inactive_body());
    assert_eq!(source.host_object(id).unwrap().health.current, 0.);
    assert!(!source.host_object(id).unwrap().status.on_die_started);
    assert!(
        source.host_object(id).unwrap().status.effectively_dead,
        "InactiveBody construction sets this flag before any death callback"
    );
    // Initial inactive state must not authenticate a forged once-only receipt.
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .completed
        .insert(0);
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap_err()
            .to_string()
            .contains("live unstarted Body")
    );
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .completed
        .clear();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let absent = Decoded {
        records: None,
        version: None,
        removals: Vec::new(),
        legacy_tail: Cow::Borrowed(&[]),
    };
    assert!(
        validate(&absent, &snapshot, &source, &HashMap::new()).is_ok(),
        "inactive zero HP is not proof of death"
    );
    source
        .apply_owned_damage(
            id,
            1.,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
        )
        .unwrap();
    assert!(source.host_object(id).unwrap().status.on_die_started);
    let started = builder.create_world_snapshot(&source).unwrap();
    let error = validate(&absent, &started, &source, &HashMap::new()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("full-score receipt is ambiguous")
    );
}

#[test]
fn native_die_capsule_rejects_pending_create_and_invalid_completed_slot() {
    let (mut source, id) = owner("KeepA");
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .pending_create = Some(0);
    let error = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("incomplete CreateObjectDie dispatch")
    );
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .pending_create = None;
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .completed
        .insert(33);
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap_err()
            .to_string()
            .contains("invalid runtime")
    );
}
#[test]
fn native_die_capsule_rejects_unknown_version_and_truncated_payload() {
    let (source, _) = owner("KeepA");
    let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(
        &super::super::lifecycle_tail::capture_lifecycle_tail(&source).unwrap(),
    );
    let boundary = bytes.len();
    append(&mut bytes, &source).unwrap();
    let mut wrong = bytes.clone();
    wrong[boundary + 4..boundary + 8].copy_from_slice(&9u32.to_le_bytes());
    assert!(decode(&wrong).is_err());
    bytes.pop();
    assert!(decode(&bytes).is_err());
}

#[test]
fn native_die_capsule_forged_completed_or_ai_receipt_rejects_detached_candidate() {
    let (receiving, id) = owner("KeepA");
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&receiving)
        .unwrap();
    for ai_dead in [false, true] {
        let mut runtime = NativeDieRuntime::default();
        if ai_dead {
            runtime.ai_dead = true;
        } else {
            runtime.completed.insert(8);
        }
        let decoded = Decoded {
            version: Some(VERSION),
            removals: Vec::new(),
            records: Some(vec![Record {
                id: id.0,
                declarations: identity(receiving.templates.get("NativeReceiptOwner").unwrap()),
                runtime,
                score_completed: false,
            }]),
            legacy_tail: Cow::Borrowed(&[]),
        };
        assert!(
            validate(&decoded, &snapshot, &receiving, &HashMap::new())
                .unwrap_err()
                .to_string()
                .contains("invalid completed slot or AI receipt")
        );
        let live = receiving.host_object(id).unwrap();
        assert!(live.native_die_runtime.completed.is_empty());
        assert!(!live.native_die_runtime.ai_dead);
    }
}

#[test]
fn native_die_capsule_completed_live_slot_rejects_capture_and_detached_restore() {
    let (mut source, id) = owner("KeepA");
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut runtime = NativeDieRuntime::default();
    runtime.completed.insert(0); // structurally valid receiving slot, temporally forged
    let record = Record {
        id: id.0,
        declarations: identity(&source.templates["NativeReceiptOwner"]),
        runtime: runtime.clone(),
        score_completed: false,
    };
    let mut offset =
        super::super::combat_projectile_persist::prefix_len(&snapshot.lifecycle_tail).unwrap();
    while &snapshot.lifecycle_tail[offset..offset + 4] != TAG {
        assert!(
            [
                b"CPRJ", b"PCOI", b"GMSH", b"BDKD", b"NLFC", b"ODIS", b"RDCK", b"OLST"
            ]
            .contains(
                &snapshot.lifecycle_tail[offset..offset + 4]
                    .try_into()
                    .unwrap()
            )
        );
        let size = u32::from_le_bytes(
            snapshot.lifecycle_tail[offset + 8..offset + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        offset += 12 + size;
    }
    let old_size = u32::from_le_bytes(
        snapshot.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let payload = bincode_legacy::serialize(&PayloadV2 {
        records: vec![record],
        removals: Vec::new(),
    })
    .unwrap();
    let mut replacement = Vec::new();
    replacement.extend_from_slice(TAG);
    replacement.extend_from_slice(&VERSION.to_le_bytes());
    replacement.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    replacement.extend_from_slice(&payload);
    snapshot
        .lifecycle_tail
        .splice(offset..offset + 12 + old_size, replacement);
    let (mut receiving, _) = owner("KeepA");
    receiving.set_current_frame(555);
    let error = SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap_err();
    assert!(error.to_string().contains("live unstarted Body"));
    assert_eq!(receiving.get_current_frame(), 555);
    assert!(
        receiving
            .host_object(id)
            .unwrap()
            .native_die_runtime
            .completed
            .is_empty()
    );
    source.objects.get_mut(&id).unwrap().native_die_runtime = runtime;
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap_err()
            .to_string()
            .contains("live unstarted Body")
    );
}

#[test]
fn native_die_capsule_actual_standalone_local_receipt_survives_without_world_latch() {
    let (mut source, id) = owner("KeepA");
    let mut template = source.templates["NativeReceiptOwner"].clone();
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&[
            BehaviorModuleDefinition {
                class_name: "FXListDie".into(),
                module_tag: Some("LocalFx".into()),
                attributes: HashMap::new(),
            },
            BehaviorModuleDefinition {
                class_name: "KeepObjectDie".into(),
                module_tag: Some("WorldKeep".into()),
                attributes: HashMap::new(),
            },
        ])
        .into_boxed_slice(),
    );
    source
        .templates
        .insert(template.name.clone(), template.clone());
    recreate_receipt_owner(&mut source, template.clone(), id);
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .take_damage_from_immediate_typed(
            100.,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            0,
            &mut source.health_events,
        );
    let object = source.host_object(id).unwrap();
    assert!(object.status.effectively_dead);
    assert!(!object.status.on_die_started);
    assert_eq!(
        object
            .native_die_runtime
            .completed
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [0]
    );
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut receiving = GameLogic::new();
    receiving.templates.insert(template.name.clone(), template);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert!(!receiving.host_object(id).unwrap().status.on_die_started);
    assert_eq!(
        receiving
            .host_object(id)
            .unwrap()
            .native_die_runtime
            .completed
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [0]
    );
    // A real standalone Body cannot have admitted its world-only Keep slot.
    receiving
        .objects
        .get_mut(&id)
        .unwrap()
        .native_die_runtime
        .completed
        .insert(1);
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&receiving)
            .unwrap_err()
            .to_string()
            .contains("world-only Die slot")
    );
}

#[test]
fn native_die_capsule_ai_receipt_requires_world_latch_despite_receiving_interface() {
    let (mut source, id) = owner("KeepA");
    let template = source.templates.get_mut("NativeReceiptOwner").unwrap();
    template.set_authored_ai_update_interface(Some(true));
    let template = template.clone();
    recreate_receipt_owner(&mut source, template, id);
    assert!(source.objects[&id].has_ai_update_interface());
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut runtime = NativeDieRuntime::default();
    runtime.ai_dead = true;
    let forged = Decoded {
        version: Some(VERSION),
        removals: Vec::new(),
        records: Some(vec![Record {
            id: id.0,
            declarations: identity(&source.templates["NativeReceiptOwner"]),
            runtime: runtime.clone(),
            score_completed: false,
        }]),
        legacy_tail: Cow::Borrowed(&[]),
    };
    assert!(
        validate(&forged, &snapshot, &source, &HashMap::new())
            .unwrap_err()
            .to_string()
            .contains("AI-dead receipt requires world onDie admission")
    );
    source.objects.get_mut(&id).unwrap().native_die_runtime = runtime;
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap_err()
            .to_string()
            .contains("AI-dead receipt requires world onDie admission")
    );
}

#[test]
fn native_die_capsule_actual_physics_crush_receipt_survives_without_world_latch() {
    let (mut source, id) = owner("KeepA");
    let mut template = source.templates["NativeReceiptOwner"].clone();
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&[BehaviorModuleDefinition {
            class_name: "CrushDie".into(),
            module_tag: Some("PhysicsCrush".into()),
            attributes: HashMap::new(),
        }])
        .into_boxed_slice(),
    );
    source
        .templates
        .insert(template.name.clone(), template.clone());
    recreate_receipt_owner(&mut source, template.clone(), id);
    let victim = source.objects.get_mut(&id).unwrap();
    victim.crushable_level = 0;
    victim.selection_radius = 4.;
    let mut crusher = crate::game_logic::Object::new(
        ThingTemplate::new("ReceiptCrusher"),
        ObjectId(77),
        Team::China,
    );
    crusher.crusher_level = 2;
    crusher.set_position(glam::Vec3::new(2.2, 0., 0.));
    crusher.movement.velocity = glam::Vec3::X;
    assert!(crusher.check_for_overlap_collision(0, victim, false, &mut source.health_events));
    assert!(victim.status.effectively_dead);
    assert!(!victim.status.on_die_started);
    assert_eq!(
        victim
            .native_die_runtime
            .completed
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [0]
    );
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut receiving = GameLogic::new();
    receiving.templates.insert(template.name.clone(), template);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert!(!receiving.host_object(id).unwrap().status.on_die_started);
    assert_eq!(
        receiving
            .host_object(id)
            .unwrap()
            .native_die_runtime
            .completed
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [0]
    );
}

#[path = "native_die_body_admission_tests.rs"]
mod body_admission_tests;

fn replace_ndie(
    snapshot: &mut crate::save_load::snapshot::WorldSnapshot,
    version: u32,
    payload: Vec<u8>,
) {
    let mut offset =
        super::super::combat_projectile_persist::prefix_len(&snapshot.lifecycle_tail).unwrap();
    loop {
        let tag = &snapshot.lifecycle_tail[offset..offset + 4];
        let length = u32::from_le_bytes(
            snapshot.lifecycle_tail[offset + 8..offset + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        if tag == TAG {
            let mut replacement = Vec::new();
            replacement.extend_from_slice(TAG);
            replacement.extend_from_slice(&version.to_le_bytes());
            replacement.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            replacement.extend_from_slice(&payload);
            snapshot
                .lifecycle_tail
                .splice(offset..offset + 12 + length, replacement);
            return;
        }
        assert!(
            [
                b"CPRJ", b"PCOI", b"GMSH", b"BDKD", b"NLFC", b"ODIS", b"RDCK", b"OLST"
            ]
            .contains(&tag.try_into().unwrap())
        );
        offset += 12 + length;
    }
}
fn v1(snapshot: &mut crate::save_load::snapshot::WorldSnapshot, source: &GameLogic) {
    let records: Vec<_> = source
        .host_objects()
        .registered()
        .map(|(id, object)| RecordV1 {
            id: id.0,
            declarations: identity(&object.thing().template),
            runtime: object.native_die_runtime.clone(),
        })
        .collect();
    replace_ndie(snapshot, 1, bincode_legacy::serialize(&records).unwrap());
}
#[test]
fn v1_unscored_live_owner_imports_without_fabricating_full_score() {
    let (source, id) = owner("KeepA");
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    v1(&mut snapshot, &source);
    let (mut receiving, _) = owner("KeepA");
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving.objects[&id].kill_score_phase,
        crate::game_logic::kill_credit::KillScorePhase::Ready
    );
    assert!(receiving.pending_object_removal_snapshot().is_empty());
}
#[test]
fn v1_ambiguous_dead_owner_rejects_before_queue_cache_or_owner_mutation() {
    let (mut source, id) = owner("KeepA");
    source
        .apply_owned_damage(
            id,
            1000.,
            Some(id),
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
        )
        .unwrap();
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    v1(&mut snapshot, &source);
    let (mut receiving, same) = owner("KeepA");
    let (peer, peer_id) = owner("KeepA");
    assert_eq!((id, same), (peer_id, peer_id));
    receiving.frame = 915;
    let cache: std::collections::HashSet<_> = receiving.templates.keys().cloned().collect();
    let error = SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("full-score receipt is ambiguous")
    );
    assert_eq!(receiving.frame, 915);
    assert_eq!(receiving.objects[&id].health.current, 100.);
    assert_eq!(
        receiving.objects[&id].kill_score_phase,
        crate::game_logic::kill_credit::KillScorePhase::Ready
    );
    assert!(receiving.pending_object_removal_snapshot().is_empty());
    assert_eq!(cache, receiving.templates.keys().cloned().collect());
    assert_eq!(peer.objects[&id].health.current, 100.);
    assert!(peer.pending_object_removal_snapshot().is_empty());
}
#[test]
fn v2_malformed_pending_removal_graph_rejects_before_live_mutation() {
    let (mut source, id) = owner("KeepA");
    source
        .apply_owned_damage(
            id,
            1000.,
            Some(id),
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
        )
        .unwrap();
    source.destroy_object(id);
    assert!(source.objects[&id].status.on_die_started);
    assert!(source.objects[&id].status.destroyed);
    for removals in [
        vec![crate::game_logic::kill_credit::PendingObjectRemoval {
            id: ObjectId(999),
            killer: None,
            after_death: true,
        }],
        vec![crate::game_logic::kill_credit::PendingObjectRemoval {
            id,
            killer: Some(Team::USA),
            after_death: false,
        }],
        vec![
            crate::game_logic::kill_credit::PendingObjectRemoval {
                id,
                killer: None,
                after_death: true
            };
            2
        ],
    ] {
        let mut snapshot = SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        let records = vec![Record {
            id: id.0,
            declarations: identity(&source.objects[&id].thing().template),
            runtime: source.objects[&id].native_die_runtime.clone(),
            score_completed: true,
        }];
        replace_ndie(
            &mut snapshot,
            VERSION,
            bincode_legacy::serialize(&PayloadV2 { records, removals }).unwrap(),
        );
        let (mut receiving, _) = owner("KeepA");
        receiving.frame = 915;
        let error = SnapshotBuilder::new()
            .restore_from_snapshot(&snapshot, &mut receiving)
            .unwrap_err();
        assert!(error.to_string().contains("pending physical-removal"));
        assert_eq!(receiving.frame, 915);
        assert_eq!(receiving.objects[&id].health.current, 100.);
        assert!(receiving.pending_object_removal_snapshot().is_empty());
    }
}
#[test]
fn save_rejects_inflight_score_and_lifecycle_phases_without_draining_them() {
    let (mut source, id) = owner("KeepA");
    source.objects.get_mut(&id).unwrap().kill_score_phase =
        crate::game_logic::kill_credit::KillScorePhase::Running;
    let error = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap_err();
    assert!(error.to_string().contains("in-flight scoreTheKill"));
    assert_eq!(
        source.objects[&id].kill_score_phase,
        crate::game_logic::kill_credit::KillScorePhase::Running
    );
    source.objects.get_mut(&id).unwrap().kill_score_phase =
        crate::game_logic::kill_credit::KillScorePhase::Ready;
    source.object_lifecycle_callback_depth = 1;
    let error = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap_err();
    assert!(error.to_string().contains("active Object lifecycle"));
    assert_eq!(source.object_lifecycle_callback_depth, 1);
    assert_eq!(source.objects[&id].health.current, 100.);
    assert!(source.pending_object_removal_snapshot().is_empty());
}

#[test]
fn v1_initial_inactive_body_is_not_misclassified_as_an_ambiguous_death() {
    let (mut source, id) = owner("KeepA");
    let mut template = source.templates["NativeReceiptOwner"].clone();
    template.body_kind = Some(crate::game_logic::thing::BodyKind::Inactive);
    template.set_health(0.);
    source
        .templates
        .insert(template.name.clone(), template.clone());
    recreate_receipt_owner(&mut source, template.clone(), id);
    assert!(source.objects[&id].status.effectively_dead);
    assert!(!source.objects[&id].status.on_die_started);
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    v1(&mut snapshot, &source);
    let (mut receiving, _) = owner("KeepA");
    receiving.templates.insert(template.name.clone(), template);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert!(receiving.objects[&id].is_inactive_body());
    assert!(!receiving.objects[&id].status.on_die_started);
    assert_eq!(
        receiving.objects[&id].kill_score_phase,
        crate::game_logic::kill_credit::KillScorePhase::Ready
    );
}
