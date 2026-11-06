//! Nuke cannon and Overlord gattling add-on fire.

use super::*;

impl GameLogic {
    pub fn apply_nuke_cannon_primary_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        self.apply_nuke_cannon_primary_at_with_effects(
            impact,
            source,
            source_team,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_nuke_cannon_primary_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_nuke_cannon::{
            MEDIUM_RADIATION_AUDIO, NUKE_CANNON_DAMAGE_TYPE, NUKE_CANNON_DEATH_TYPE,
            NUKE_CANNON_FIRE_AUDIO, is_legal_nuke_cannon_splash_target,
            nuke_cannon_primary_damage_at, nuke_cannon_splash_radius,
        };

        let impact_xz = (impact.x, impact.z);
        let max_r = nuke_cannon_splash_radius();
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_nuke_cannon_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if dist <= max_r {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = nuke_cannon_primary_damage_at(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        NUKE_CANNON_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        NUKE_CANNON_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.nuke_cannon_residual.record_primary_blast(hits);

        // Medium radiation field residual at impact.
        let source_id = source.unwrap_or(ObjectId(0));
        let _ = self.nuke_cannon_residual.spawn_radiation_zone(
            source_id,
            source_team,
            impact,
            self.frame,
        );
        self.queue_audio_event(
            AudioEventRequest::new(MEDIUM_RADIATION_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        self.queue_audio_event(
            AudioEventRequest::new(NUKE_CANNON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(160),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }


    /// Residual honesty: Helix transport residual.
    pub fn honesty_helix_transport_ok(&self) -> bool {
        self.overlord_addons.honesty_helix_transport_ok()
    }

    pub fn overlord_addons(
        &self,
    ) -> &crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry {
        &self.overlord_addons
    }

    /// Residual honesty: Nuke Cannon primary area + radiation residual.
    pub fn honesty_nuke_cannon_primary_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_primary_ok()
    }

    pub fn honesty_nuke_cannon_radiation_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_radiation_ok()
    }

    pub fn honesty_nuke_cannon_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_host_path_ok()
    }

    pub fn nuke_cannon_residual(
        &self,
    ) -> &crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry {
        &self.nuke_cannon_residual
    }
}
