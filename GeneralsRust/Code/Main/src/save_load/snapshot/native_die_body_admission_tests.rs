//! Cross-domain admission uses the same saved Body kind as object reconstruction.
use super::*;

fn inactive() -> (GameLogic, ObjectId, ThingTemplate) {
    let (mut source, id) = owner("InactiveLocalFx");
    let mut template = source.templates["NativeReceiptOwner"].clone();
    template.body_kind = Some(crate::game_logic::thing::BodyKind::Inactive);
    template.set_health(0.);
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&[BehaviorModuleDefinition {
            class_name: "FXListDie".into(),
            module_tag: Some("LocalFx".into()),
            attributes: HashMap::new(),
        }])
        .into_boxed_slice(),
    );
    source
        .templates
        .insert(template.name.clone(), template.clone());
    source.objects.insert(
        id,
        crate::game_logic::Object::new(template.clone(), id, Team::USA),
    );
    assert!(source.host_object(id).unwrap().is_inactive_body());
    assert!(source.host_object(id).unwrap().status.effectively_dead);
    assert!(!source.host_object(id).unwrap().status.on_die_started);
    template.body_kind = None; // Supported receiving legacy/fresh catalog admission.
    (source, id, template)
}
fn replace_receipt(snapshot: &mut WorldSnapshot, record: Option<Record>) {
    let mut offset =
        super::super::super::combat_projectile_persist::prefix_len(&snapshot.lifecycle_tail)
            .unwrap();
    loop {
        let tag: &[u8; 4] = snapshot.lifecycle_tail[offset..offset + 4]
            .try_into()
            .unwrap();
        let size = u32::from_le_bytes(
            snapshot.lifecycle_tail[offset + 8..offset + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        if tag == TAG {
            let mut bytes = Vec::new();
            if let Some(record) = record {
                let payload = bincode_legacy::serialize(&PayloadV2 {
                    records: vec![record],
                    removals: Vec::new(),
                })
                .unwrap();
                bytes.extend_from_slice(TAG);
                bytes.extend_from_slice(&VERSION.to_le_bytes());
                bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                bytes.extend_from_slice(&payload);
            }
            snapshot
                .lifecycle_tail
                .splice(offset..offset + 12 + size, bytes);
            return;
        }
        assert!([b"CPRJ", b"BDKD", b"NLFC", b"ODIS", b"RDCK", b"OLST"].contains(&tag));
        offset += 12 + size;
    }
}
#[test]
fn native_die_saved_inactive_unset_receiving_body_restores_present_and_absent_receipts() {
    let (source, id, template) = inactive();
    let builder = SnapshotBuilder::new();
    for absent in [false, true] {
        let mut snapshot = builder.create_world_snapshot(&source).unwrap();
        if absent {
            replace_receipt(&mut snapshot, None);
        }
        let mut receiving = GameLogic::new();
        receiving
            .templates
            .insert(template.name.clone(), template.clone());
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .unwrap();
        let restored = receiving.host_object(id).unwrap();
        assert!(restored.is_inactive_body());
        assert!(restored.status.effectively_dead);
        assert!(!restored.status.on_die_started);
        assert!(restored.native_die_runtime.completed.is_empty());
    }
}
#[test]
fn native_die_saved_inactive_unset_receiving_body_rejects_forged_fx_before_mutation() {
    let (source, id, template) = inactive();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut runtime = NativeDieRuntime::default();
    runtime.completed.insert(0);
    replace_receipt(
        &mut snapshot,
        Some(Record {
            score_completed: false,
            id: id.0,
            declarations: identity(&source.templates["NativeReceiptOwner"]),
            runtime,
        }),
    );
    let mut receiving = GameLogic::new();
    receiving.templates.insert(template.name.clone(), template);
    receiving.set_current_frame(779);
    let error = builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap_err();
    assert!(error.to_string().contains("live unstarted Body"));
    assert_eq!(receiving.get_current_frame(), 779);
    assert!(receiving.host_objects().registered_is_empty());
    // Explicitly incompatible receiving Body metadata must still fail BDKD.
    receiving
        .templates
        .get_mut("NativeReceiptOwner")
        .unwrap()
        .body_kind = Some(crate::game_logic::thing::BodyKind::Active);
    let valid = builder.create_world_snapshot(&source).unwrap();
    assert!(
        builder
            .restore_from_snapshot(&valid, &mut receiving)
            .unwrap_err()
            .to_string()
            .contains("Body class differs from saved module")
    );
    assert_eq!(receiving.get_current_frame(), 779);
    assert!(receiving.host_objects().registered_is_empty());
}
