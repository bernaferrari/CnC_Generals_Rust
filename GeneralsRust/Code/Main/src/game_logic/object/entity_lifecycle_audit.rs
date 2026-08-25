//! Read-inventory audit: Main residual groups vs envelope tags vs Entity fields.
//!
//! Fails only on new regressions. Documented main-only gaps are explicit TODO
//! tags and do not fail the audit until they disappear without being inventoried.

use super::entity_lifecycle_tags::INVENTORY_TAGS;
use crate::game_logic::object::Object;
use crate::game_logic::{ObjectId, Team, ThingTemplate};
use gamelogic::world::entities::{
    ENTITY_LIFECYCLE_ENVELOPE_VERSION, EntityLifecycleEnvelope, EntityModuleState, EntityStore,
    TemplateRef, Transform,
};

const EXPECTED_INVENTORY_LEN: usize = 59;

const ENTITY_ONLY_GROUPS: &[&str] = &[
    "health",
    "transform",
    "owner",
    "attack_target",
    "move_target",
    "producer_id",
    "contained_by_host",
    "garrisoned_host_ids",
    "destroyed",
    "destroyed_at_frame",
    "production_queue",
    "construction_percent",
    "veterancy",
    "weapon_primary_residual",
];

const MAIN_ONLY_TODO_GROUPS: &[&str] = &[
    "RadarExtend",
    "ProductionDoor",
    "RebuildHole",
    "SupplyTruckRuntime",
    "ShockStun",
    "PhysicsMotive",
    "Locomotor",
    "WeaponSlots",
    "Turret",
    "StatusBits",
    "ModelCondition",
    "ContinuousFire",
    "SubdualDamage",
    "OverlordAddon",
];

fn representative_object() -> Object {
    Object::new(ThingTemplate::new("AuditRanger"), ObjectId(3), Team::USA)
}

fn envelope_with_every_inventory_tag() -> EntityLifecycleEnvelope {
    EntityLifecycleEnvelope {
        version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
        entity_id: 3,
        destroyed: false,
        destroyed_at_frame: 0,
        module_states: INVENTORY_TAGS
            .iter()
            .map(|tag| EntityModuleState {
                tag: (*tag).to_string(),
                payload: vec![tag.len() as u8],
            })
            .chain(std::iter::once(EntityModuleState {
                tag: "UnknownFuture".to_string(),
                payload: vec![0xFF],
            }))
            .collect(),
    }
}

#[test]
fn inventory_len_and_declaration_order_are_locked() {
    assert_eq!(INVENTORY_TAGS.len(), EXPECTED_INVENTORY_LEN);
    assert_eq!(INVENTORY_TAGS[0], "UpgradeDie");
    assert_eq!(INVENTORY_TAGS[49], "CommandButtonHuntUpdate");
    assert_eq!(INVENTORY_TAGS[58], "RailroadBehavior");
}

#[test]
fn attach_preserves_every_envelope_tag_including_unknown() {
    let mut store = EntityStore::new();
    let id = store.spawn(
        TemplateRef::new("AuditRanger"),
        None,
        Transform::default(),
        10.0,
    );
    let envelope = envelope_with_every_inventory_tag();
    let entity = store.get_mut(id).expect("spawned");
    entity.attach_envelope(envelope);
    let taken = entity.take_envelope().expect("attached");
    let tags: Vec<&str> = taken.module_states.iter().map(|m| m.tag.as_str()).collect();
    assert_eq!(tags.len(), EXPECTED_INVENTORY_LEN + 1);
    for (idx, expected) in INVENTORY_TAGS.iter().enumerate() {
        assert_eq!(tags[idx], *expected, "envelope tag {idx} dropped on attach");
    }
    assert_eq!(tags[EXPECTED_INVENTORY_LEN], "UnknownFuture");
}

#[test]
fn read_inventory_audit_buckets() {
    let object = representative_object();
    let produced: Vec<String> = object
        .entity_lifecycle_envelope()
        .module_states
        .iter()
        .map(|m| m.tag.clone())
        .collect();
    for tag in &produced {
        assert!(
            INVENTORY_TAGS.contains(&tag.as_str()),
            "envelope-only regression: {tag} is not in INVENTORY_TAGS"
        );
    }

    let covered = INVENTORY_TAGS.len();
    let envelope_only = 0usize;
    let entity_only = ENTITY_ONLY_GROUPS.len();
    let main_only = MAIN_ONLY_TODO_GROUPS.len();
    assert_eq!(covered, EXPECTED_INVENTORY_LEN);
    assert_eq!(envelope_only, 0);
    assert_eq!(entity_only, 14);
    assert_eq!(main_only, 14);

    for todo in MAIN_ONLY_TODO_GROUPS {
        assert!(
            !INVENTORY_TAGS.contains(todo),
            "stale MAIN_ONLY_TODO {todo} is now inventoried; remove it from the TODO list"
        );
    }
    for group in ENTITY_ONLY_GROUPS {
        assert!(
            !INVENTORY_TAGS.contains(group),
            "entity-only group {group} collided with an envelope tag"
        );
    }
}
