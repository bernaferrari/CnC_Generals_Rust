//! Mine clearing, detonation and splash.

use super::*;

impl GameLogic {
    /// Safely disarm/clear a residual mine without detonation or area damage.
    /// C++ Weapon DAMAGE_DISARM → LandMineInterface::disarm / destroyObject residual.
    /// Demo traps (KINDOF_DEMOTRAP) are disarmed the same way — no explosion.
    pub fn clear_mine_internal(&mut self, mine_id: ObjectId, clearer_id: ObjectId) -> bool {
        self.clear_mine_internal_with_effects(mine_id, clearer_id, &mut Default::default())
    }

    pub(crate) fn clear_mine_internal_with_effects(
        &mut self,
        mine_id: ObjectId,
        clearer_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        use crate::game_logic::host_mines::{MINE_CLEARED_AUDIO, can_clear_mine_kind};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if !mine.is_alive() {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated || !can_clear_mine_kind(data.kind) {
            return false;
        }
        let clearer_team = self.objects.get(&clearer_id).map(|o| o.team);
        if clearer_team == Some(mine.team) {
            return false;
        }
        let mine_pos = mine.get_position();
        let keep_regen_pad = data.regenerates
            && matches!(
                data.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            );

        if keep_regen_pad {
            use crate::game_logic::host_enum_table_residual::rubble_model_bit;
            use crate::game_logic::host_mines::MINE_MIN_HEALTH;
            let amount = self
                .objects
                .get(&mine_id)
                .map(|obj| obj.health.current - MINE_MIN_HEALTH)
                .unwrap_or(0.0);
            self.apply_guarded_mine_body_drain_with_effects(mine_id, amount, effects);
            if let Some(obj) = self.objects.get_mut(&mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    // CPP disarm's count reset follows the guarded damage callback.
                    let _ = md.disarm_regenerating_pad();
                    md.last_synced_health = Some(obj.health.current);
                }
                obj.model_condition_bits |= 1u128 << rubble_model_bit();
                obj.set_status_masked(true);
            }
        } else if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
                md.proximity_enabled = false;
                md.detonate_at_frame = None;
            }
        }

        self.admit_object_drawable_deselection(mine_id);
        self.mine_residual_clears = self.mine_residual_clears.saturating_add(1);

        self.queue_audio_event(
            AudioEventRequest::new(MINE_CLEARED_AUDIO)
                .with_object(mine_id)
                .with_position(mine_pos)
                .with_priority(160),
        );

        if !keep_regen_pad {
            self.mark_object_for_destruction_with_effects(mine_id, None, effects);
        }

        if let Some(clearer) = self.objects.get_mut(&clearer_id) {
            if clearer.target == Some(mine_id) {
                clearer.target = None;
            }
            if matches!(clearer.ai_state(), AIState::Attacking | AIState::Moving) {
                clearer.set_ai_state(
                    AIState::Idle,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                {}
                clearer.movement.target_position = None;
                clearer.set_status_moving(false);
                clearer.set_status_attacking(false);
            }
        }

        true
    }

    pub(in super::super::super) fn detonate_mine_internal(
        &mut self,
        mine_id: ObjectId,
        reason: crate::game_logic::host_mines::HostMineDetonateReason,
        // C++ MinefieldBehavior::detonateOnce(position): onCollide fires the
        // detonation weapon at the clipped victim point (MinefieldBehavior.cpp:257,
        // :422-424); DemoTrapUpdate::detonate / onDamage pass the object position.
        blast_at: Option<Vec3>,
    ) -> bool {
        self.detonate_mine_internal_with_effects(mine_id, reason, blast_at, &mut Default::default())
    }

    pub(in super::super::super) fn detonate_mine_internal_with_effects(
        &mut self,
        mine_id: ObjectId,
        reason: crate::game_logic::host_mines::HostMineDetonateReason,
        // C++ MinefieldBehavior::detonateOnce(position): onCollide fires the
        // detonation weapon at the clipped victim point (MinefieldBehavior.cpp:257,
        // :422-424); DemoTrapUpdate::detonate / onDamage pass the object position.
        blast_at: Option<Vec3>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        use crate::game_logic::host_mines::{HostMineDetonateReason, damage_at_distance};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if mine.status.sold {
            return false;
        }
        if !mine.is_alive() && !matches!(reason, HostMineDetonateReason::Killed) {
            // Last detonateOnce of a 0-HP non-regen pad still fires + destroyObject.
            let last_virtual = mine.mine_data.as_ref().is_some_and(|md| {
                matches!(
                    md.kind,
                    crate::game_logic::host_mines::HostMineKind::LandMine
                ) && !md.detonated
            });
            if !last_virtual {
                return false;
            }
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated {
            return false;
        }
        if mine.status.under_construction {
            return false;
        }

        let kind = data.kind;
        let damage = data.detonation_damage;
        let radius = data.detonation_radius;
        let demo_profile = data.demo_trap_profile;
        let is_demo_trap = matches!(kind, crate::game_logic::host_mines::HostMineKind::DemoTrap);
        let neutron_blast = data.neutron_blast;
        let mine_team = mine.team;
        let mine_pos = mine.get_position();
        let producer = data.producer_id;
        let frame = self.frame;
        let blast_pos = blast_at.unwrap_or(mine_pos);

        // C++ DemoTrapUpdate.cpp:136-141, 240-251: selecting the detonation
        // weapon slot (command button) calls detonate() which fires the
        // detonation weapon and kills the trap the SAME frame — no SlowDeath
        // warning arm. Only passive paths (proximity scan / death) ride the
        // SlowDeath DestructionDelay residual.
        if is_demo_trap
            && reason != crate::game_logic::host_mines::HostMineDetonateReason::Manual
            && !data.demo_trap_warning_ready(frame)
        {
            if let Some(obj) = self.objects.get_mut(&mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let _ = md.arm_demo_trap_warning(frame);
                }
            }
            self.queue_audio_event(
                AudioEventRequest::new(crate::game_logic::host_mines::DEMO_TRAP_WARNING_AUDIO)
                    .with_object(mine_id)
                    .with_position(mine_pos)
                    .with_priority(180),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponImpact,
                mine_pos,
                frame,
                Some(mine_id),
                None,
            );
            return true;
        }

        // Mark detonated before applying damage.
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
            }
        }

        match reason {
            HostMineDetonateReason::Proximity => {
                self.mine_residual_proximity_detonations =
                    self.mine_residual_proximity_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Timed => {
                self.mine_residual_timed_detonations =
                    self.mine_residual_timed_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Manual | HostMineDetonateReason::Killed => {
                self.mine_residual_manual_detonations =
                    self.mine_residual_manual_detonations.saturating_add(1);
            }
        }

        // Area damage: residual hits enemies + neutrals; demo trap / sticky charges
        // also hit allies (DemoTrap/TNT RadiusDamageAffects SELF ALLIES ENEMIES NEUTRALS).
        let hit_allies = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::DemoTrap
                | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );

        let mut destroy_ids: Vec<(ObjectId, Team, std::sync::Weak<()>)> = Vec::new();
        let mut chain_ids: Vec<ObjectId> = Vec::new();
        if neutron_blast {
            destroy_ids = self.apply_neutron_mine_blast(mine_id, blast_pos, mine_team);
        } else {
            let victim_ids: Vec<ObjectId> = self.objects.registered_keys().copied().collect();
            for vid in victim_ids {
                if vid == mine_id {
                    continue;
                }
                let Some(victim) = self.objects.get(&vid) else {
                    continue;
                };
                if !victim.is_alive() {
                    continue;
                }
                let chain_pad = land_mine_pad_takes_splash(victim);
                if victim.mine_data.is_some() && !chain_pad {
                    continue;
                }
                if victim.team == mine_team && !hit_allies {
                    continue;
                }
                let dist = mine_splash_distance(victim, blast_pos);
                let dmg = if is_demo_trap {
                    crate::game_logic::host_mines::demo_trap_damage_at(demo_profile, dist)
                } else {
                    damage_at_distance(damage, radius, dist)
                };
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(victim) = self.objects.get_mut(&vid) {
                    // C++ DemoTrap/LandMine weapons are EXPLOSION, not UNRESISTABLE.
                    if {
                        let owned_victim_id = victim.id;
                        let owned_damage_type = if matches!(
                            kind,
                            crate::game_logic::host_mines::HostMineKind::LandMine
                        ) {
                            crate::game_logic::combat::DamageType::LandMine
                        } else {
                            crate::game_logic::combat::DamageType::Explosive
                        };
                        self.apply_residual_damage_owned_with_effects(
                            owned_victim_id,
                            dmg,
                            Some(mine_id),
                            owned_damage_type,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                owned_damage_type,
                            ),
                            None,
                            effects,
                        )
                    } && !chain_pad
                    {
                        if let Some(victim) = self.objects.get(&vid) {
                            destroy_ids.push((
                                vid,
                                mine_team,
                                std::sync::Arc::downgrade(&victim.visual_draw_identity_token),
                            ));
                        }
                    } else if chain_pad {
                        chain_ids.push(vid);
                    }
                }
            }
            for id in chain_ids {
                self.chain_detonate_land_mine_from_splash_with_effects(id, effects);
            }
        }

        // Chem DemoTrap residual: spawn MediumPoisonField at detonation.
        if is_demo_trap && demo_profile.spawns_poison() {
            let _ = self.toxin_tractor.spawn_medium_field(
                mine_id,
                mine_team,
                blast_pos,
                self.frame,
                demo_profile.poison_anthrax_tier(),
            );
        }

        // Audio + particle residual.
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(blast_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            blast_pos,
            self.frame,
            Some(mine_id),
            None,
        );

        // Destroy the mine/trap itself.
        self.mark_object_for_destruction_with_effects(
            mine_id,
            producer.map(|_| mine_team),
            effects,
        );
        for (vid, killer, victim_birth) in destroy_ids {
            if !self.objects.get(&vid).is_some_and(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            }) {
                continue;
            }
            // C++ scoreTheKill on the charge; addExperiencePoints sinks to planter.
            self.award_score_the_kill_experience_with_effects_and_interpreter(
                mine_id, vid, None, effects,
            )
            .expect("mine kill XP callback must complete");
            if self.objects.get(&vid).is_some_and(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            }) {
                self.mark_object_for_destruction_with_effects(vid, Some(killer), effects);
            }
        }

        true
    }

    pub(super) fn apply_mine_splash(
        &mut self,
        mine_id: ObjectId,
        blast_pos: Vec3,
        damage: f32,
        radius: f32,
        kind: crate::game_logic::host_mines::HostMineKind,
        mine_team: Team,
        producer: Option<ObjectId>,
    ) {
        self.apply_mine_splash_with_effects(
            mine_id,
            blast_pos,
            damage,
            radius,
            kind,
            mine_team,
            producer,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_mine_splash_with_effects(
        &mut self,
        mine_id: ObjectId,
        blast_pos: Vec3,
        damage: f32,
        radius: f32,
        kind: crate::game_logic::host_mines::HostMineKind,
        mine_team: Team,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_mines::damage_at_distance;
        let neutron_blast = self
            .objects
            .get(&mine_id)
            .and_then(|o| o.mine_data.as_ref())
            .is_some_and(|md| md.neutron_blast);
        if neutron_blast {
            let destroy_ids = self.apply_neutron_mine_blast(mine_id, blast_pos, mine_team);
            self.queue_audio_event(
                AudioEventRequest::new(kind.detonate_audio())
                    .with_object(mine_id)
                    .with_position(blast_pos)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponImpact,
                blast_pos,
                self.frame,
                Some(mine_id),
                None,
            );
            for (vid, killer, victim_birth) in destroy_ids {
                if !self.objects.get(&vid).is_some_and(|victim| {
                    victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                        &victim.visual_draw_identity_token,
                    ))
                }) {
                    continue;
                }
                self.award_score_the_kill_experience_with_effects_and_interpreter(
                    mine_id, vid, None, effects,
                )
                .expect("mine kill XP callback must complete");
                if self.objects.get(&vid).is_some_and(|victim| {
                    victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                        &victim.visual_draw_identity_token,
                    ))
                }) {
                    self.mark_object_for_destruction_with_effects(vid, Some(killer), effects);
                }
            }
            return;
        }
        let hit_allies = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::DemoTrap
                | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );
        let mut destroy_ids: Vec<(ObjectId, Team, std::sync::Weak<()>)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.registered_keys().copied().collect();
        for vid in victim_ids {
            if vid == mine_id
                || (matches!(kind, crate::game_logic::host_mines::HostMineKind::LandMine)
                    && Some(vid) == producer)
            {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() {
                continue;
            }
            let chain_pad = land_mine_pad_takes_splash(victim);
            if victim.mine_data.is_some() && !chain_pad {
                continue;
            }
            if victim.team == mine_team && !hit_allies {
                continue;
            }
            let dist = mine_splash_distance(victim, blast_pos);
            let dmg = damage_at_distance(damage, radius, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                if {
                    let owned_victim_id = victim.id;
                    let owned_damage_type =
                        if matches!(kind, crate::game_logic::host_mines::HostMineKind::LandMine) {
                            crate::game_logic::combat::DamageType::LandMine
                        } else {
                            crate::game_logic::combat::DamageType::Explosive
                        };
                    self.apply_residual_damage_owned_with_effects(
                        owned_victim_id,
                        dmg,
                        Some(mine_id),
                        owned_damage_type,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            owned_damage_type,
                        ),
                        None,
                        effects,
                    )
                } && !chain_pad
                {
                    if let Some(victim) = self.objects.get(&vid) {
                        destroy_ids.push((
                            vid,
                            mine_team,
                            std::sync::Arc::downgrade(&victim.visual_draw_identity_token),
                        ));
                    }
                } else if chain_pad {
                    // Original weapon iteration completes this victim's
                    // onDamage before advancing to the next radius victim.
                    self.chain_detonate_land_mine_from_splash_with_effects(vid, effects);
                }
            }
        }
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(blast_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            blast_pos,
            self.frame,
            Some(mine_id),
            None,
        );
        for (vid, killer, victim_birth) in destroy_ids {
            if !self.objects.get(&vid).is_some_and(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            }) {
                continue;
            }
            self.award_score_the_kill_experience_with_effects_and_interpreter(
                mine_id, vid, None, effects,
            )
            .expect("mine kill XP callback must complete");
            if self.objects.get(&vid).is_some_and(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            }) {
                self.mark_object_for_destruction_with_effects(vid, Some(killer), effects);
            }
        }
    }

    /// C++ MinefieldBehavior::onDamage after splash `attemptDamage`.
    pub(super) fn chain_detonate_land_mine_from_splash(&mut self, mine_id: ObjectId) {
        self.chain_detonate_land_mine_from_splash_with_effects(mine_id, &mut Default::default())
    }

    pub(super) fn chain_detonate_land_mine_from_splash_with_effects(
        &mut self,
        mine_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.synchronize_mine_body_owned_with_effects(mine_id, false, false, effects);
    }

    pub(super) fn synchronize_mine_body_owned(
        &mut self,
        mine_id: ObjectId,
        healing: bool,
        self_drain: bool,
    ) {
        self.synchronize_mine_body_owned_with_effects(
            mine_id,
            healing,
            self_drain,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_neutron_mine_blast(
        &mut self,
        mine_id: ObjectId,
        blast_pos: Vec3,
        mine_team: Team,
    ) -> Vec<(ObjectId, Team, std::sync::Weak<()>)> {
        use crate::game_logic::host_mines::{NeutronMineVictimEffect, neutron_mine_victim_effect};
        let mut destroy_ids: Vec<(ObjectId, Team, std::sync::Weak<()>)> = Vec::new();
        let mut contained_kill: Vec<ObjectId> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.registered_keys().copied().collect();
        for vid in victim_ids {
            if vid == mine_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() || victim.mine_data.is_some() {
                continue;
            }
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - blast_pos.x;
                let dz = vpos.z - blast_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            let airborne = victim.status.airborne_target
                || crate::game_logic::host_mines::is_above_terrain(
                    vpos.y,
                    self.terrain_height_at(vpos).unwrap_or(0.0),
                );
            let effect = neutron_mine_victim_effect(
                dist,
                victim.team == mine_team,
                airborne,
                victim.is_kind_of(KindOf::Infantry),
                victim.is_kind_of(KindOf::Vehicle),
                victim.is_kind_of(KindOf::Drone),
                victim.template_name.to_ascii_lowercase().contains("bike"),
            );
            match effect {
                NeutronMineVictimEffect::None => {}
                NeutronMineVictimEffect::KillInfantry
                | NeutronMineVictimEffect::KillCliffJumper => {
                    contained_kill.extend(victim.contained_units());
                    destroy_ids.push((
                        vid,
                        mine_team,
                        std::sync::Arc::downgrade(&victim.visual_draw_identity_token),
                    ));
                }
                NeutronMineVictimEffect::UnmanVehicle => {
                    contained_kill.extend(victim.contained_units());
                    if let Some(victim) = self.objects.get_mut(&vid) {
                        victim.set_status_disabled_unmanned(true);
                        victim.team = Team::Neutral;
                    }
                    self.admit_host_radar_disabled_edges(vid);
                }
            }
        }
        for cid in contained_kill {
            if let Some(c) = self.objects.get_mut(&cid) {
                if c.is_alive() {
                    destroy_ids.push((
                        cid,
                        mine_team,
                        std::sync::Arc::downgrade(&c.visual_draw_identity_token),
                    ));
                }
            }
        }
        destroy_ids
    }

    /// C++ `Object::getRelationship`. DemoTrap: ENEMIES only.
    /// Land mines: DetonatedBy ENEMIES | NEUTRAL (ctor default).
    pub(super) fn mine_proximity_skips_friendly(
        &self,
        mine_id: ObjectId,
        victim_id: ObjectId,
    ) -> bool {
        use crate::game_logic::host_mines::{
            HostMineKind, demo_trap_proximity_requires_enemies, land_mine_proximity_trips,
        };
        use gamelogic::common::Relationship;

        let Some(mine) = self.objects.get(&mine_id) else {
            return true;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return true;
        };
        let rel = match (
            self.player_owner_for_host_object(mine),
            self.player_owner_for_host_object(victim),
        ) {
            (Some(mine_owner), Some(victim_owner)) => {
                self.player_relationship(mine_owner, victim_owner)
            }
            _ => {
                if victim.team == mine.team {
                    Relationship::Allies
                } else if victim.team == Team::Neutral || mine.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
        };
        let is_land = mine
            .mine_data
            .as_ref()
            .is_some_and(|md| matches!(md.kind, HostMineKind::LandMine));
        if is_land {
            !land_mine_proximity_trips(rel == Relationship::Enemies, rel == Relationship::Neutral)
        } else {
            !demo_trap_proximity_requires_enemies(rel == Relationship::Enemies)
        }
    }
}
