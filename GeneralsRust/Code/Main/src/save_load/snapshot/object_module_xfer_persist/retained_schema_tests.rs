//! Current OXFR schema controls, separate from the real kept-object save flow.
use super::*;
use crate::game_logic::{KindOf, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn keepobject_oxfr_v7_appends_new_module_state_after_retained_rows() {
    assert_eq!(OXFR_VERSION, 10);
    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("TechOilDerrick");
    template.add_kind_of(KindOf::Structure);
    source.templates.insert("TechOilDerrick".into(), template);
    let kept = source
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .unwrap();
    source.mark_object_for_destruction(kept, None);
    let payload = capture(&source);
    let prefix = bincode_legacy::serialize(&(
        &payload.stun,
        &payload.battle_bus,
        &payload.slow_death,
        &payload.radar,
        &payload.death_start,
    ))
    .unwrap();
    let mut bytes = b"preceding-domain".to_vec();
    append_to_lifecycle_tail(&mut bytes, &source);
    let sentinel = 0xA1B2_C3D4u32.to_le_bytes();
    bytes.extend_from_slice(&sentinel);
    assert_eq!(&bytes[..16], b"preceding-domain");
    let mut suffix = find_oxfr_suffix(&bytes).unwrap();
    assert_eq!(take_u32(&mut suffix).unwrap(), 5);
    let len = take_u32(&mut suffix).unwrap() as usize;
    let encoded = &suffix[..len];
    assert!(
        encoded.starts_with(&prefix),
        "earlier vectors, including exact latch rows, changed"
    );
    let decoded: ObjectModuleXferPersistPayload = bincode_legacy::deserialize(encoded).unwrap();
    assert_eq!(decoded.retained_death.len(), 1);
    assert_eq!(decoded.retained_death[0].object_id, kept.0);
    assert!(decoded.retained_death[0].effectively_dead && decoded.retained_death[0].keep_as_rubble);
    assert!(
        decoded.retained_death[0]
            .keep_object_die
            .as_ref()
            .unwrap()
            .is_rubble
    );
    assert_eq!(&suffix[len..], &sentinel);
}

#[test]
fn keepobject_oxfr_v5_applies_explicit_false_rows_without_using_latch_or_dead_hp() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("RetainedFalse".into(), ThingTemplate::new("RetainedFalse"));
    let id = source
        .create_object("RetainedFalse", Team::USA, Vec3::ZERO)
        .unwrap();
    source.host_object_mut(id).unwrap().health.current = 0.0;
    let payload = capture(&source);
    assert_eq!(payload.retained_death.len(), 1);
    assert!(
        !payload.retained_death[0].effectively_dead && !payload.retained_death[0].keep_as_rubble
    );
    assert!(payload.retained_death[0].keep_object_die.is_none());
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    let object = source.host_object_mut(id).unwrap();
    object.status.effectively_dead = true;
    object.status.keep_as_rubble = true;
    object.status.on_die_started = true;
    let module = object.keep_object_die.get_or_insert_with(Default::default);
    module.is_rubble = true;
    module.rubble_frame = 88;
    apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
    let object = source.host_object(id).unwrap();
    assert!(!object.status.effectively_dead && !object.status.keep_as_rubble);
    assert!(!object.status.on_die_started);
    assert!(object.keep_object_die.is_none());
    assert_eq!(object.health.current, 0.0);
}

#[test]
fn radar_executed_payload_rejects_missing_contributor_rows() {
    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("MalformedRadarState");
    template.radar_upgrade_rules = Some(vec![crate::game_logic::host_radar::RadarUpgradeRules {
        triggered_by: vec!["Upgrade_Example".into()],
        conflicts_with: Vec::new(),
        removes_upgrades: Vec::new(),
        fx_list: None,
        requires_all: false,
        disable_proof: false,
        valid: true,
    }]);
    source.templates.insert(template.name.clone(), template);
    let id = source
        .create_object("MalformedRadarState", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let mut payload = capture(&source);
    payload.radar.push(RadarPersist {
        object_id: id.0,
        executed: vec![true],
        contributors: Vec::new(),
        radar_extend_done_frame: 0,
        radar_extend_complete: false,
        radar_active: false,
    });
    assert!(matches!(
        apply_payload(&mut source, payload),
        Err(SaveLoadError::Corrupted(_))
    ));
}
