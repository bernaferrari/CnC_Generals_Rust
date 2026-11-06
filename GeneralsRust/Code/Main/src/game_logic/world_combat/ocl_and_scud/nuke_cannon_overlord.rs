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

    /// Apply Overlord/Helix portable gattling residual at impact.
    ///
    /// Independent portable rider (C++ HelixContain.cpp:340 RIDERS ALWAYS FIRE):
    /// slot 1 = AA `GattlingBuildingGunAir`; slot 0 = ground `GattlingBuildingGun`
    /// only — never stacked onto OverlordTankGun / HelixMinigun host damage.
    pub(crate) fn apply_overlord_gattling_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        self.apply_overlord_gattling_residual_at_with_effects(
            impact,
            source,
            intended_target,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_overlord_gattling_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
            OVERLORD_GATTLING_DEATH_TYPE, OVERLORD_GATTLING_FIRE_AUDIO,
            OVERLORD_GATTLING_GROUND_DAMAGE_TYPE, is_legal_overlord_gattling_target,
            overlord_gattling_ground_damage,
        };

        let chain = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_chain_guns_upgrade(&o.applied_upgrades))
            .unwrap_or(false);

        let (dmg, is_aa) = if slot == 1 {
            let mult = if chain { 1.25 } else { 1.0 };
            (OVERLORD_GATTLING_AIR_DAMAGE * mult, true)
        } else {
            (overlord_gattling_ground_damage(chain), false)
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .registered()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: obj.team == Team::Neutral,
                            under_construction: obj.status.under_construction,
                            combat_kind,
                            effectively_stealthed: obj.is_effectively_stealthed(),
                            is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                            eject_invulnerable: obj.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| {
                        is_legal_overlord_gattling_target(
                            true,
                            false,
                            c.under_construction,
                            c.combat_kind,
                        )
                    },
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        if is_aa {
                            self.overlord_addons.record_gattling_aa_fire(0);
                        } else {
                            self.overlord_addons.record_gattling_ground_fire(0);
                        }
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            let combat_kind = obj.is_kind_of(KindOf::Attackable)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            if is_legal_overlord_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                combat_kind,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (
                        OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                } else {
                    (
                        OVERLORD_GATTLING_GROUND_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        if is_aa {
            self.overlord_addons.record_gattling_aa_fire(hits);
        } else {
            self.overlord_addons.record_gattling_ground_fire(hits);
        }

        let muzzle = source
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(impact);
        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_GATTLING_FIRE_AUDIO)
                .with_position(muzzle)
                .with_priority(140),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// C++ HelixContain.cpp:340 — portable gattling auto-acquires independently
    /// of the host chassis shot (not a piggyback +10 on OverlordTankGun).
    pub(in super::super::super) fn try_overlord_gattling_addon_independent_fire(
        &mut self,
        host_id: ObjectId,
    ) {
        self.try_overlord_gattling_addon_independent_fire_with_effects(
            host_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn try_overlord_gattling_addon_independent_fire_with_effects(
        &mut self,
        host_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_RANGE, OVERLORD_GATTLING_GROUND_RANGE,
            is_legal_overlord_gattling_target, overlord_gattling_slot_for_air,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        let Some(host) = self.objects.get(&host_id) else {
            return;
        };
        if !host.has_overlord_gattling_residual() || !host.is_alive() {
            return;
        }
        if host.contained_by.is_some() {
            return;
        }
        let Some(aa) = host.weapon_slot(1) else {
            return;
        };
        if !Object::weapon_ready(aa, current_time) {
            return;
        }
        let team = host.team;
        let fire_pos = host.get_position();

        let candidates: Vec<_> = self
            .objects
            .registered()
            .filter(|(id, _)| **id != host_id)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            host_id,
            team,
            fire_pos,
            candidates,
            |is_air| {
                if is_air {
                    OVERLORD_GATTLING_AIR_RANGE
                } else {
                    OVERLORD_GATTLING_GROUND_RANGE
                }
            },
            |c| {
                c.is_alive
                    && c.team != team
                    && !c.is_neutral
                    && is_legal_overlord_gattling_target(
                        c.is_alive,
                        false,
                        c.under_construction,
                        c.combat_kind,
                    )
            },
        );
        let Some((target_id, _, _)) = best else {
            return;
        };
        let target_is_air = self
            .objects
            .get(&target_id)
            .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
            .unwrap_or(false);
        let slot = overlord_gattling_slot_for_air(target_is_air);
        let impact = self
            .objects
            .get(&target_id)
            .map(|t| t.get_position())
            .unwrap_or(fire_pos);
        let (hits, _) = self.apply_overlord_gattling_residual_at_with_effects(
            impact,
            Some(host_id),
            Some(target_id),
            slot,
            presentation_effects,
        );
        if hits == 0 {
            return;
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            if let Some(w) = host.weapon_slot_mut(1) {
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
        }
    }

    /// Residual honesty: Overlord/Helix gattling addon install + fire path.
    pub fn honesty_overlord_gattling_ok(&self) -> bool {
        self.overlord_addons.honesty_gattling_install_ok()
            && self.overlord_addons.honesty_gattling_fire_ok()
    }

    /// Residual honesty: Overlord/Helix/Emperor propaganda addon residual.
    pub fn honesty_overlord_propaganda_ok(&self) -> bool {
        self.overlord_addons.honesty_propaganda_install_ok() || self.honesty_propaganda_heal_ok()
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
