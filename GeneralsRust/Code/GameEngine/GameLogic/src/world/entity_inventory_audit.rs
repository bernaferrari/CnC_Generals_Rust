//! Read-inventory audit: envelope tags vs Entity attach vs Main-only gaps.
//!
//! `KNOWN_GAPS` is the locked set of Main flattened bool/frame residuals that
//! sit next to inventoried `Option<Host*Data>` groups (`object/mod.rs` 1104+)
//! and are not envelope tags yet. Adding an untracked field in that window
//! fails the Main source-scan. Inventoring a gap requires removing it here
//! and adding the matching tag to Main `INVENTORY_TAGS`.

/// Main-only flattened bool/frame residuals not yet in the envelope inventory.
/// Window: `fire_weapon_when_dead_fired` … `weapon_laser_beam_expires_frame`.
/// Later projectile clusters are the grouped `ProjectileFlightResiduals` TODO.
pub const KNOWN_GAPS: &[&str] = &[
    "fire_weapon_when_dead_fired",
    "pending_transition_damage_fx",
    "pending_death_fx",
    "pending_death_audio",
    "pending_create_object_die_spawns",
    "create_object_die_transfer_damage",
    "carpet_bomb_payload",
    "artillery_barrage_shell",
    "a10_strike_missile",
    "leaflet_transport_target",
    "leaflet_container",
    "paradrop_transport_target",
    "paradrop_parachute",
    "daisy_cutter_bomb",
    "anthrax_bomb_payload",
    "sneak_tunnel_start",
    "cluster_mines_bomb",
    "emp_pulse_bomb",
    "emp_pulse_spheroid",
    "emp_pulse_spheroid_expires_frame",
    "particle_trail_remnant",
    "particle_trail_remnant_expires_frame",
    "nuke_radiation_field",
    "nuke_radiation_field_expires_frame",
    "anthrax_toxin_field",
    "anthrax_toxin_field_expires_frame",
    "spectre_howitzer_shell",
    "spectre_howitzer_shell_expires_frame",
    "particle_orbital_laser",
    "particle_orbital_laser_expires_frame",
    "particle_connector_laser",
    "particle_connector_laser_expires_frame",
    "point_defense_laser_beam",
    "point_defense_laser_beam_expires_frame",
    "missile_defender_laser_beam",
    "missile_defender_laser_beam_expires_frame",
    "booby_trap_special",
    "booby_trap_attached_to",
    "countermeasure_flare",
    "countermeasure_flare_expires_frame",
    "angry_mob_member",
    "angry_mob_nexus_id",
    "weapon_laser_beam",
    "weapon_laser_beam_expires_frame",
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
        EntityLifecycleEnvelope, EntityModuleState, EntityStore, TemplateRef, Transform,
        ENTITY_LIFECYCLE_ENVELOPE_VERSION,
    };

    fn unique(items: &[&str]) -> bool {
        let mut seen = items.to_vec();
        seen.sort_unstable();
        seen.dedup();
        seen.len() == items.len()
    }

    #[test]
    fn known_gaps_are_unique_and_declaration_ordered() {
        assert_eq!(KNOWN_GAPS.len(), 44);
        assert_eq!(KNOWN_GAPS[0], "fire_weapon_when_dead_fired");
        assert_eq!(KNOWN_GAPS[43], "weapon_laser_beam_expires_frame");
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
