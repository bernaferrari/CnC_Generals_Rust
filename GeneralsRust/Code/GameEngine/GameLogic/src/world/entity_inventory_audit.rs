//! Read-inventory audit: envelope tags vs Entity attach vs Main-only gaps.
//!
//! `KNOWN_GAPS` is the locked set of Main flattened bool/frame residuals that
//! sit next to inventoried `Option<Host*Data>` groups (`object/mod.rs` 1104+)
//! and are not envelope tags yet. Adding an untracked field in that window
//! fails the Main source-scan. Inventoring a gap requires removing it here
//! and adding the matching tag to Main `INVENTORY_TAGS`.

/// Transient presentation drain queues in the residual window.
/// These are not C++ Object::xfer fields (Object.cpp:3995-4364); C++ FX/audio
/// is module-side or client-side and is drained same-frame by Main.
pub const KNOWN_GAPS: &[&str] = &[
    "pending_transition_damage_fx",
    "pending_death_fx",
    "pending_death_audio",
    "pending_create_object_die_spawns",
];

/// Entity flattened write-surface families that are not envelope tags.
pub const ENTITY_ONLY_GROUPS: &[&str] = &[
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::entities::{
        ENTITY_LIFECYCLE_ENVELOPE_VERSION, EntityLifecycleEnvelope, EntityModuleState, EntityStore,
        TemplateRef, Transform,
    };

    fn unique(items: &[&str]) -> bool {
        let mut seen = items.to_vec();
        seen.sort_unstable();
        seen.dedup();
        seen.len() == items.len()
    }

    #[test]
    fn known_gaps_are_unique_and_declaration_ordered() {
        assert_eq!(KNOWN_GAPS.len(), 4);
        assert_eq!(KNOWN_GAPS[0], "pending_transition_damage_fx");
        assert_eq!(KNOWN_GAPS[3], "pending_create_object_die_spawns");
        assert!(unique(KNOWN_GAPS), "KNOWN_GAPS must stay unique");
        for gap in KNOWN_GAPS {
            assert!(
                !ENTITY_ONLY_GROUPS.contains(gap),
                "gap {gap} collided with an entity-only flattened family"
            );
        }
    }

    #[test]
    fn take_envelope_round_trip_equals_attached() {
        let mut store = EntityStore::new();
        let id = store.spawn(
            TemplateRef::new("AuditUnit"),
            None,
            Transform::default(),
            10.0,
        );
        let attached = EntityLifecycleEnvelope {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: id.get(),
            destroyed: false,
            destroyed_at_frame: 0,
            module_states: vec![
                EntityModuleState {
                    tag: "UpgradeDie".to_string(),
                    payload: vec![1, 2, 3],
                },
                EntityModuleState {
                    tag: "UnknownFuture".to_string(),
                    payload: vec![0xFF],
                },
            ],
        };
        let entity = store.get_mut(id).expect("spawned");
        entity.attach_envelope(attached.clone());
        assert!(entity.entity_modules.is_none());
        let taken = entity.take_envelope().expect("attached");
        assert_eq!(taken, attached);
    }
}
