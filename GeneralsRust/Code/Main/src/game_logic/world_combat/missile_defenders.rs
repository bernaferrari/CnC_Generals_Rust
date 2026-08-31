//! Host combat `impl GameLogic` — `missile_defenders`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// C++ SpecialAbilityUpdate SpecialObject = LaserBeam residual for MD laser lock.
    pub fn spawn_missile_defender_laser_beam(
        &mut self,
        shooter_id: ObjectId,
        target_id: ObjectId,
        from: glam::Vec3,
        to: glam::Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_missile_defender::{
            LASER_GUIDED_ATTACH_BONE, LASER_GUIDED_BEAM_LIFETIME_FRAMES,
            LASER_GUIDED_BEAM_MAX_HEALTH, LASER_GUIDED_SPECIAL_OBJECT,
        };
        use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;
        use crate::game_logic::{KindOf, ThingTemplate};

        // One active LaserBeam special object per shooter residual.
        let stale: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.missile_defender_laser_beam && o.producer_id == Some(shooter_id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for sid in stale {
            if let Some(o) = self.objects.get_mut(&sid) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.missile_defender_laser_beam = false;
            }
            self.mark_object_for_destruction(sid, None);
        }

        let beam_name = LASER_GUIDED_SPECIAL_OBJECT;
        if !self.templates.contains_key(beam_name) {
            let mut t = ThingTemplate::new(beam_name);
            t.add_kind_of(KindOf::Immobile)
                .set_health(LASER_GUIDED_BEAM_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(beam_name.to_string(), t);
        }
        let team = self
            .objects
            .get(&shooter_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        // Place near muzzle / shooter; presentation freezes full segment via weapon_lasers.
        let place = glam::Vec3::new(from.x, from.y + 8.0, from.z);
        let bid = self.create_object(beam_name, team, place)?;
        let expires = self
            .frame
            .saturating_add(LASER_GUIDED_BEAM_LIFETIME_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&bid) {
            o.missile_defender_laser_beam = true;
            o.producer_id = Some(shooter_id);
            o.missile_defender_laser_beam_expires_frame = Some(expires);
            o.health.maximum = LASER_GUIDED_BEAM_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, LASER_GUIDED_BEAM_MAX_HEALTH);
        }
        self.weapon_lasers
            .retain(|laser| laser.from_id != shooter_id || laser.laser_name != beam_name);
        self.weapon_lasers
            .push(ResidualWeaponLaser::with_bone_lifetime(
                beam_name,
                LASER_GUIDED_ATTACH_BONE,
                shooter_id,
                Some(target_id),
                (from.x, from.y, from.z),
                (to.x, to.y, to.z),
                self.frame,
                LASER_GUIDED_BEAM_LIFETIME_FRAMES,
            ));
        self.missile_defender_laser_beams_spawned =
            self.missile_defender_laser_beams_spawned.saturating_add(1);
        Some(bid)
    }

    pub fn update_missile_defender_laser_beam_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.missile_defender_laser_beam {
                    return None;
                }
                // Expire on prep window end or dead producer.
                if let Some(exp) = o.missile_defender_laser_beam_expires_frame {
                    if exp <= frame {
                        return Some(*id);
                    }
                }
                if let Some(pid) = o.producer_id {
                    let producer_dead = self
                        .objects
                        .get(&pid)
                        .map(|p| !p.is_alive() || p.status.destroyed)
                        .unwrap_or(true);
                    if producer_dead {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.missile_defender_laser_beam = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn honesty_missile_defender_laser_beam_ok(&self) -> bool {
        self.missile_defender_laser_beams_spawned > 0
    }

    pub fn activate_missile_defender_laser_guided(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use crate::game_logic::host_hero_abilities::{
            LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase,
        };
        use crate::game_logic::host_missile_defender::{
            can_activate_laser_guided, is_missile_defender_template, laser_guided_in_start_range,
        };

        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        if !can_activate_laser_guided(
            is_missile_defender_template(&obj.template_name),
            obj.is_alive(),
        ) {
            return false;
        }
        if obj.secondary_weapon.is_none() {
            return false;
        }
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        // C++ update() aborts SPECIAL_MISSILE_DEFENDER_LASER_GUIDED vs structures.
        if target.is_kind_of(KindOf::Structure) {
            return false;
        }
        if target.status.stealthed && !target.status.detected {
            return false;
        }
        let src_pos = obj.get_position();
        let tgt_pos = target.get_position();
        let dist = {
            let dx = src_pos.x - tgt_pos.x;
            let dz = src_pos.z - tgt_pos.z;
            (dx * dx + dz * dz).sqrt()
        };
        if !laser_guided_in_start_range(dist) {
            // C++ initiateIntent then update() approachTarget (aiMoveToObject).
            self.hero_abilities.set_leftover_channel(
                object_id,
                LeftoverSaChannel::new(
                    LeftoverSaKind::LaserGuided,
                    target_id,
                    LeftoverSaPhase::Facing,
                    0,
                ),
            );
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_target(Some(target_id));
                obj.set_ai_state(AIState::SpecialAbility);
            }
            self.path_approach_with_state_ignoring(
                object_id,
                tgt_pos,
                AIState::SpecialAbility,
                Some(target_id),
            );
            return true;
        }

        self.start_leftover_laser_guided_preparation(object_id, target_id, src_pos, tgt_pos);
        true
    }

    /// C++ `SpecialAbilityUpdate::startPreparation` for MD laser.
    fn start_leftover_laser_guided_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        src_pos: glam::Vec3,
        tgt_pos: glam::Vec3,
    ) {
        use crate::game_logic::host_hero_abilities::{
            LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase, leftover_sa_timings,
        };
        use crate::game_logic::host_missile_defender::{
            LASER_GUIDED_ATTACH_BONE, LASER_GUIDED_INITIATE_AUDIO,
        };

        let timings = leftover_sa_timings(LeftoverSaKind::LaserGuided);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_ai_state(AIState::SpecialAbility);
            obj.set_status_using_ability(true);
        }
        let (beam_from, beam_to) = self
            .special_ability_laser_endpoints_from_bone(
                object_id,
                target_id,
                LASER_GUIDED_ATTACH_BONE,
            )
            .unwrap_or((src_pos, tgt_pos));
        // C++ startPreparation creates LaserBeam SpecialObject immediately.
        let special_object_id =
            self.spawn_missile_defender_laser_beam(object_id, target_id, beam_from, beam_to);
        let mut channel = LeftoverSaChannel::new(
            LeftoverSaKind::LaserGuided,
            target_id,
            LeftoverSaPhase::Preparing,
            timings.prep_ms,
        );
        channel.special_object_id = special_object_id;
        self.hero_abilities.set_leftover_channel(object_id, channel);

        self.queue_audio_event(
            AudioEventRequest::new(LASER_GUIDED_INITIATE_AUDIO)
                .with_object(object_id)
                .with_position(src_pos)
                .with_priority(160),
        );
        self.leftover_sa_notify_start_preparation(object_id, LeftoverSaKind::LaserGuided);
    }

    pub(crate) fn update_leftover_laser_guided_channels(&mut self, dt: f32) {
        use crate::game_logic::KindOf;
        use crate::game_logic::host_hero_abilities::{
            LeftoverSaKind, LeftoverSaPhase, leftover_sa_timings, leftover_within_abort_range,
        };
        use crate::game_logic::host_missile_defender::laser_guided_in_start_range;
        const EPS: f32 = 0.000_1;
        let ids: Vec<ObjectId> = self
            .hero_abilities
            .leftover_channels
            .iter()
            .filter_map(|(id, ch)| {
                if ch.kind == LeftoverSaKind::LaserGuided {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for object_id in ids {
            let Some(channel) = self.hero_abilities.leftover_channel(object_id).copied() else {
                continue;
            };
            let timings = leftover_sa_timings(LeftoverSaKind::LaserGuided);
            let Some((src_pos, tgt_pos, tgt_alive, is_structure, stealth_hidden, can_move)) =
                self.objects.get(&object_id).and_then(|src| {
                    self.objects.get(&channel.target_id).map(|tgt| {
                        (
                            src.get_position(),
                            tgt.get_position(),
                            tgt.is_alive(),
                            tgt.is_kind_of(KindOf::Structure),
                            tgt.status.stealthed && !tgt.status.detected,
                            src.can_move(),
                        )
                    })
                })
            else {
                self.leftover_kill_special_objects(object_id);
                self.hero_abilities.take_leftover_channel(object_id);
                continue;
            };
            let dist = {
                let dx = src_pos.x - tgt_pos.x;
                let dz = src_pos.z - tgt_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            // C++ continuePreparation: dead, structure/stealth (update shouldAbort), or
            // getRelationship == ALLIES ("captured by a colleague") kills the laser.
            if !tgt_alive
                || is_structure
                || stealth_hidden
                || self.leftover_sa_target_is_ally(object_id, channel.target_id)
            {
                self.leftover_kill_special_objects(object_id);
                self.hero_abilities.take_leftover_channel(object_id);
                continue;
            }
            if channel.phase != LeftoverSaPhase::Preparing {
                // C++ approachTarget until StartAbilityRange; abort range is prep-only.
                if laser_guided_in_start_range(dist) {
                    self.start_leftover_laser_guided_preparation(
                        object_id,
                        channel.target_id,
                        src_pos,
                        tgt_pos,
                    );
                } else if can_move {
                    self.path_approach_with_state_ignoring(
                        object_id,
                        tgt_pos,
                        AIState::SpecialAbility,
                        Some(channel.target_id),
                    );
                }
                continue;
            }
            if !leftover_within_abort_range(dist, timings.abort_range) {
                self.leftover_kill_special_objects(object_id);
                self.hero_abilities.take_leftover_channel(object_id);
                continue;
            }
            // C++ continuePreparation re-initLaser each prep / persist-prep frame.
            let _ = self.reinit_special_ability_laser(
                object_id,
                channel.target_id,
                channel.special_object_id,
            );
            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
            if remaining > EPS {
                self.hero_abilities.set_leftover_channel(
                    object_id,
                    crate::game_logic::host_hero_abilities::LeftoverSaChannel {
                        remaining_seconds: remaining,
                        ..channel
                    },
                );
                continue;
            }
            // C++ triggerAbilityEffect (SpecialAbilityUpdate.cpp:1276-1293): only
            // while the SECONDARY weapon exists, setWeaponLock(SECONDARY,
            // LOCKED_TEMPORARILY) then aiAttackObject. WeaponSet.cpp:1053-1056
            // refuses a temporary lock under a permanent user lock (that slot
            // keeps firing); reload (Object.cpp:1464-1467) and any player
            // command (GameLogicDispatch.cpp:104-106) release it again.
            if self
                .objects
                .get(&object_id)
                .is_some_and(|obj| obj.weapon_slot(1).is_some())
            {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.set_weapon_lock(1, crate::game_logic::WeaponLockType::LockedTemporarily);
                }
                let _ = self.engage_target_decision_aware(object_id, channel.target_id);
            }
            self.missile_defender_residual_laser_specials = self
                .missile_defender_residual_laser_specials
                .saturating_add(1);
            // PersistentPrepTime 500ms keeps the beam and re-triggers.
            let mut persist = crate::game_logic::host_hero_abilities::LeftoverSaChannel::new(
                LeftoverSaKind::LaserGuided,
                channel.target_id,
                LeftoverSaPhase::Preparing,
                timings.persist_prep_ms,
            );
            persist.special_object_id = channel.special_object_id;
            self.hero_abilities.set_leftover_channel(object_id, persist);
        }
    }

    #[cfg(test)]
    pub fn activate_missile_defender_laser_guided_for_test(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        self.activate_missile_defender_laser_guided(object_id, target_id)
    }

    /// Record Combat Cycle residual rider load honesty.
    pub fn record_combat_cycle_residual_load(&mut self) {
        self.combat_cycle_residual_loads = self.combat_cycle_residual_loads.saturating_add(1);
    }

    /// Apply residual rider weapon switch to a Combat Cycle (RiderChangeContain residual).
    ///
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
    pub fn apply_combat_cycle_rider(
        &mut self,
        object_id: ObjectId,
        rider: crate::game_logic::host_combat_cycle::CombatCycleRider,
    ) -> bool {
        use crate::game_logic::host_combat_cycle::{
            CombatCycleRider, combat_cycle_weapon_for_rider, is_combat_cycle_template,
            is_kell_snipe_transfer_rider, transfer_next_shot_last_fire_time,
        };
        use crate::game_logic::thing::ThingTemplate;

        let occupant_fire = {
            let Some(obj) = self.objects.get(&object_id) else {
                return false;
            };
            obj.contained_units().first().and_then(|oid| {
                self.objects.get(oid).and_then(|o| {
                    let kell = matches!(rider, CombatCycleRider::JarmenKell)
                        || is_kell_snipe_transfer_rider(
                            o.is_kind_of(KindOf::Hero),
                            o.is_kind_of(KindOf::Salvager),
                            &o.template_name,
                        );
                    if !kell {
                        return None;
                    }
                    o.secondary_weapon
                        .as_ref()
                        .or(o.weapon.as_ref())
                        .map(|w| w.last_fire_time)
                })
            })
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let parsed_rider_change = obj.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::RiderChange
            && obj
                .thing
                .template
                .contain_module
                .has_supported_rider_change_roster();
        if !is_combat_cycle_template(&obj.template_name) && !parsed_rider_change {
            return false;
        }
        if !obj.is_combat_cycle_style_container() && !parsed_rider_change {
            obj.install_combat_cycle_transport();
        }

        obj.combat_cycle_rider = rider.as_u8();
        if let Some(name) =
            crate::game_logic::host_combat_cycle::combat_cycle_weapon_name_for_rider(rider)
        {
            let mut weapon = ThingTemplate::weapon_from_store(name)
                .or_else(|| combat_cycle_weapon_for_rider(rider));
            if let Some(w) = &mut weapon {
                // Force residual stats from host residual table.
                if let Some(stats) = combat_cycle_weapon_for_rider(rider) {
                    w.damage = stats.damage;
                    w.range = stats.range;
                    w.min_range = stats.min_range;
                    w.reload_time = stats.reload_time;
                    w.can_target_air = stats.can_target_air;
                    w.can_target_ground = stats.can_target_ground;
                    w.projectile_speed = stats.projectile_speed;
                    w.ammo = stats.ammo;
                }
                if let Some(fire) = occupant_fire {
                    transfer_next_shot_last_fire_time(fire, w);
                }
            }

            let _ = obj.replace_weapon_set_slot(0, weapon);
            if let Some(fire) = occupant_fire {
                if let Some(sec) = obj.secondary_weapon.as_mut() {
                    transfer_next_shot_last_fire_time(fire, sec);
                }
            }
            obj.record_host_weapon_stats();
        } else {
            let _ = obj.replace_weapon_set_slot(0, None);
            obj.record_host_weapon_stats();
            let _ = obj.replace_weapon_set_slot(1, None);
            obj.record_host_weapon_stats();
        }

        self.combat_cycle_residual_rider_switches =
            self.combat_cycle_residual_rider_switches.saturating_add(1);
        true
    }

    /// Refresh Combat Cycle weapon from current occupant residual.
    ///
    /// Empty bike → PRIMARY NONE; single rider → rider weapon residual.
    pub fn refresh_combat_cycle_rider_weapon(&mut self, container_id: ObjectId) {
        use crate::game_logic::host_combat_cycle::{
            CombatCycleRider, combat_cycle_weapon_for_rider, is_combat_cycle_template,
            rider_from_template_name,
        };

        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let parsed_rider_change = container.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::RiderChange
            && container
                .thing
                .template
                .contain_module
                .has_supported_rider_change_roster();
        if parsed_rider_change {
            // An authored RiderChange transaction owns this state.  Never
            // infer the occupant class from a template basename after it has
            // selected a RiderN slot.  The only retained legacy exception is
            // an initial payload with no live occupant yet.
            let active_rider = container
                .rider_change_active_slot
                .map(CombatCycleRider::from_u8)
                .or_else(|| {
                    container
                        .contained_units()
                        .is_empty()
                        .then(|| CombatCycleRider::from_u8(container.combat_cycle_rider))
                });
            if let Some(rider) = active_rider {
                let _ = self.apply_combat_cycle_rider(container_id, rider);
            }
            return;
        }
        if !is_combat_cycle_template(&container.template_name)
            && !container.is_combat_cycle_style_container()
        {
            return;
        }
        let occupants = container.contained_units();
        let rider = if let Some(first) = occupants.first() {
            self.objects
                .get(first)
                .map(|o| rider_from_template_name(&o.template_name))
                .unwrap_or(CombatCycleRider::None)
        } else if container.combat_cycle_rider > 0 {
            // Spawn InitialPayload residual without a live occupant object:
            // keep current rider class when no occupants tracked yet.
            CombatCycleRider::from_u8(container.combat_cycle_rider)
        } else {
            CombatCycleRider::None
        };

        // When occupants present, force rider from them; when empty and no
        // spawn residual, clear weapon.
        let rider = if occupants.is_empty() && container.combat_cycle_rider == 0 {
            CombatCycleRider::None
        } else if !occupants.is_empty() {
            rider
        } else {
            // Occupants empty but spawn rider still set: keep spawn residual.
            CombatCycleRider::from_u8(container.combat_cycle_rider)
        };

        let _ = combat_cycle_weapon_for_rider(rider);
        let _ = self.apply_combat_cycle_rider(container_id, rider);
    }

    /// Apply Combat Cycle residual fire (rider weapon or suicide area).
    pub(in super::super) fn apply_combat_cycle_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_combat_cycle::{
            CombatCycleRider, KELL_DAMAGE_TYPE, KELL_DEATH_TYPE, REBEL_MG_DAMAGE_TYPE,
            REBEL_MG_DEATH_TYPE, RPG_DAMAGE, RPG_DAMAGE_TYPE, RPG_DEATH_TYPE, RPG_SPLASH,
            SUICIDE_DAMAGE_TYPE, SUICIDE_DEATH_TYPE, SUICIDE_SECONDARY_RADIUS,
            combat_cycle_audio_for_rider, is_legal_combat_cycle_target, is_terrorist_suicide_rider,
            rpg_splash_damage_at, suicide_bike_damage_at,
        };

        let (source_team, rider, bike_pos) = source
            .and_then(|sid| {
                self.objects.get(&sid).map(|o| {
                    (
                        o.team,
                        CombatCycleRider::from_u8(o.combat_cycle_rider),
                        o.get_position(),
                    )
                })
            })
            .unwrap_or((Team::Neutral, CombatCycleRider::None, impact));

        // Infer rebel when rider unset but weapon residual looks like MG.
        let rider = if matches!(rider, CombatCycleRider::None) {
            if let Some(sid) = source {
                if let Some(o) = self.objects.get(&sid) {
                    if let Some(w) = o.weapon.as_ref() {
                        if (w.damage - 8.0).abs() < 0.5 {
                            CombatCycleRider::Rebel
                        } else if (w.damage - 40.0).abs() < 0.5 {
                            CombatCycleRider::TunnelDefender
                        } else if (w.damage - 180.0).abs() < 1.0 {
                            CombatCycleRider::JarmenKell
                        } else if w.damage >= 500.0 {
                            CombatCycleRider::Terrorist
                        } else {
                            rider
                        }
                    } else {
                        rider
                    }
                } else {
                    rider
                }
            } else {
                rider
            }
        } else {
            rider
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let mut destroy_self = false;

        if is_terrorist_suicide_rider(rider) {
            // SuicideBikeBomb residual around bike position.
            let center = bike_pos;
            let candidates: Vec<(ObjectId, f32)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if source == Some(*id) {
                        return None;
                    }
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    if !is_legal_combat_cycle_target(
                        obj.is_alive(),
                        false,
                        obj.status.under_construction,
                        combat_kind,
                    ) {
                        return None;
                    }
                    let pos = obj.get_position();
                    let dist = {
                        let dx = center.x - pos.x;
                        let dz = center.z - pos.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    if dist <= SUICIDE_SECONDARY_RADIUS {
                        Some((*id, dist))
                    } else {
                        None
                    }
                })
                .collect();

            for (id, dist) in candidates {
                let dmg = suicide_bike_damage_at(dist);
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(obj) = self.objects.get_mut(&id) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        dmg,
                        source,
                        SUICIDE_DAMAGE_TYPE,
                        SUICIDE_DEATH_TYPE,
                    );
                    hits = hits.saturating_add(1);
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((id, Some(source_team)));
                    }
                }
            }
            destroy_self = true;
            self.combat_cycle_residual_suicides =
                self.combat_cycle_residual_suicides.saturating_add(1);
        } else if matches!(rider, CombatCycleRider::TunnelDefender) {
            // RPG splash residual.
            let impact_xz = (impact.x, impact.z);
            let candidates: Vec<(ObjectId, f32, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if source == Some(*id) {
                        return None;
                    }
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    if !is_legal_combat_cycle_target(
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
                    let is_intended = intended_target == Some(*id);
                    if is_intended || dist <= RPG_SPLASH {
                        Some((*id, dist, is_intended))
                    } else {
                        None
                    }
                })
                .collect();

            for (id, dist, is_intended) in candidates {
                let dmg = rpg_splash_damage_at(is_intended, dist);
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(obj) = self.objects.get_mut(&id) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        dmg,
                        source,
                        RPG_DAMAGE_TYPE,
                        RPG_DEATH_TYPE,
                    );
                    hits = hits.saturating_add(1);
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((id, Some(source_team)));
                    }
                }
            }
            let _ = RPG_DAMAGE;
        } else {
            // Direct residual hit (Rebel MG / Kell sniper).
            if let Some(tid) = intended_target {
                let dmg = source
                    .and_then(|sid| self.objects.get(&sid))
                    .and_then(|o| o.weapon.as_ref())
                    .map(|w| w.damage)
                    .unwrap_or(0.0);
                if dmg > 0.0 {
                    if let Some(obj) = self.objects.get_mut(&tid) {
                        if is_legal_combat_cycle_target(
                            obj.is_alive(),
                            false,
                            obj.status.under_construction,
                            true,
                        ) {
                            let (dt_name, death_name) = match rider {
                                CombatCycleRider::JarmenKell => (KELL_DAMAGE_TYPE, KELL_DEATH_TYPE),
                                CombatCycleRider::TunnelDefender => {
                                    (RPG_DAMAGE_TYPE, RPG_DEATH_TYPE)
                                }
                                _ => (REBEL_MG_DAMAGE_TYPE, REBEL_MG_DEATH_TYPE),
                            };
                            let destroyed = obj.take_damage_from_immediate_residual(
                                dmg, source, dt_name, death_name,
                            );
                            hits = hits.saturating_add(1);
                            if destroyed {
                                any_destroyed = true;
                                destroy_ids.push((tid, Some(source_team)));
                            }
                        }
                    }
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }
        if destroy_self {
            if let Some(sid) = source {
                self.mark_object_for_destruction(sid, Some(source_team));
            }
        }

        self.combat_cycle_residual_fires = self.combat_cycle_residual_fires.saturating_add(1);
        self.combat_cycle_residual_units_hit =
            self.combat_cycle_residual_units_hit.saturating_add(hits);

        if let Some(audio) = combat_cycle_audio_for_rider(rider) {
            self.queue_audio_event(
                AudioEventRequest::new(audio)
                    .with_position(impact)
                    .with_priority(150),
            );
        }
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Toxin Tractor primary stream residual (poison damage radius).
    pub(crate) fn apply_toxin_tractor_stream_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        use crate::game_logic::host_toxin_tractor::{
            AnthraxResidualTier, TOXIN_DAMAGE_TYPE, TOXIN_STREAM_AUDIO, TOXIN_STREAM_RADIUS,
            ToxinTractorSalvageTier, UPGRADE_GLA_ANTHRAX_BETA, UPGRADE_GLA_ANTHRAX_GAMMA,
            UPGRADE_GLA_ANTHRAX_GAMMA_ALT, anthrax_tier_from_flags, is_chem_general_template,
            is_legal_toxin_target, toxin_death_type_name, toxin_stream_damage,
            toxin_stream_damage_at,
        };

        let (anthrax, tier) = source
            .and_then(|sid| self.objects.get(&sid))
            .map(|a| {
                let has_gamma = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                    || a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                    || a.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                    || a.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                let has_beta = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                    || a.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                let anthrax = anthrax_tier_from_flags(
                    has_gamma,
                    has_beta,
                    is_chem_general_template(&a.template_name),
                );
                let tier = if a.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO") {
                    ToxinTractorSalvageTier::Two
                } else if a.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE") {
                    ToxinTractorSalvageTier::One
                } else {
                    ToxinTractorSalvageTier::Base
                };
                (anthrax, tier)
            })
            .unwrap_or((AnthraxResidualTier::None, ToxinTractorSalvageTier::Base));
        let base_dmg = toxin_stream_damage(tier, anthrax);
        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle);
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                if !is_legal_toxin_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                    airborne,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Stream residual: intended always; others within PrimaryDamageRadius.
                if is_intended || dist <= TOXIN_STREAM_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = if is_intended {
                base_dmg
            } else {
                toxin_stream_damage_at(dist, base_dmg)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    TOXIN_DAMAGE_TYPE,
                    toxin_death_type_name(anthrax),
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.toxin_tractor.record_stream_fire(hits);
        if anthrax.is_gamma() {
            self.toxin_tractor.record_gamma_stream_fire();
        }
        self.queue_audio_event(
            AudioEventRequest::new(TOXIN_STREAM_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Toxin Tractor contaminate spray residual + MediumPoisonField spawn.
    pub(in super::super) fn apply_toxin_tractor_spray_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        use crate::game_logic::host_toxin_tractor::{
            AnthraxResidualTier, TOXIN_DAMAGE_TYPE, TOXIN_POISON_AUDIO, TOXIN_SPRAY_AUDIO,
            TOXIN_SPRAY_RADIUS, UPGRADE_GLA_ANTHRAX_BETA, UPGRADE_GLA_ANTHRAX_GAMMA,
            UPGRADE_GLA_ANTHRAX_GAMMA_ALT, anthrax_tier_from_flags, is_chem_general_template,
            is_legal_toxin_target, toxin_death_type_name, toxin_spray_damage,
            toxin_spray_damage_at,
        };

        let anthrax = source
            .and_then(|sid| {
                self.objects.get(&sid).map(|a| {
                    let has_gamma = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                        || a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                        || a.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                        || a.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                    let has_beta = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                        || a.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                    anthrax_tier_from_flags(
                        has_gamma,
                        has_beta,
                        is_chem_general_template(&a.template_name),
                    )
                })
            })
            .unwrap_or(AnthraxResidualTier::None);
        let spray_dmg = toxin_spray_damage(anthrax);
        // Spray residual originates at the tractor; impact is attack target.
        // Fail-closed residual: use impact as field center (contaminate puddle).
        let center = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.get_position()))
            .unwrap_or(impact);
        let center_xz = (center.x, center.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle);
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                if !is_legal_toxin_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                    airborne,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = center_xz.0 - pos.x;
                    let dz = center_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if dist <= TOXIN_SPRAY_RADIUS {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = toxin_spray_damage_at(dist, spray_dmg);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    TOXIN_DAMAGE_TYPE,
                    toxin_death_type_name(anthrax),
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        // C++ FireOCLAfterWeaponCooldown: count secondary shots; field on cooldown.
        if let Some(sid) = source {
            if let Some(obj) = self.objects.get_mut(&sid) {
                let data = obj.fire_ocl_after_cooldown.get_or_insert_with(
                    crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData::new,
                );
                data.record_shot(self.frame);
            }
        }
        self.toxin_tractor.record_spray_fire(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TOXIN_SPRAY_AUDIO)
                .with_position(center)
                .with_priority(150),
        );
        self.queue_audio_event(
            AudioEventRequest::new(TOXIN_POISON_AUDIO)
                .with_position(center)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                center,
                Some(impact),
                self.frame,
                sid,
                None,
            );
        }

        (hits, any_destroyed)
    }

    /// C++ FireOCLAfterWeaponCooldownUpdate residual (toxin spray secondary).
    ///
    /// When secondary spray has fired ≥ MinShots and has been idle past
    /// ContinuousFireCoast, spawn MediumPoisonField with OCL lifetime peel.
    pub fn tick_fire_ocl_after_weapon_cooldown(&mut self) {
        use crate::game_logic::host_toxin_tractor::{
            TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES, UPGRADE_GLA_ANTHRAX_BETA,
            UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT, anthrax_tier_from_flags,
            is_chem_general_template, is_toxin_tractor_template,
        };

        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.fire_ocl_after_cooldown
                    .as_ref()
                    .map(|d| d.valid && d.consecutive_shots > 0)
                    .unwrap_or(false)
                    && is_toxin_tractor_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in ids {
            // Idle residual: not currently firing secondary (ai not attacking with spray).
            let (idle, pos, anthrax) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let last = o
                    .fire_ocl_after_cooldown
                    .as_ref()
                    .map(|d| d.last_shot_frame)
                    .unwrap_or(0);
                // C++: could have shot but didn't (coast idle after last secondary).
                let coasted = frame.saturating_sub(last)
                    >= TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES
                    && last > 0;
                let has_gamma = o.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                    || o.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                    || o.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma");
                let has_beta = o.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                    || o.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                let anthrax = anthrax_tier_from_flags(
                    has_gamma,
                    has_beta,
                    is_chem_general_template(&o.template_name),
                );
                (coasted, o.get_position(), anthrax)
            };
            if !idle {
                continue;
            }
            let lifetime = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let Some(d) = o.fire_ocl_after_cooldown.as_mut() else {
                    continue;
                };
                d.try_fire_ocl_on_cooldown(frame)
            };
            let Some(life) = lifetime else {
                continue;
            };
            let team = self
                .objects
                .get(&id)
                .map(|o| o.team)
                .unwrap_or(Team::Neutral);
            let _ = self
                .toxin_tractor
                .spawn_medium_field_lifetime(id, team, pos, frame, anthrax, life);
            self.toxin_tractor.record_fire_ocl_spawn();
            if let Some(o) = self.objects.get_mut(&id) {
                if let Some(d) = o.fire_ocl_after_cooldown.as_mut() {
                    d.ocl_spawns = d.ocl_spawns.saturating_add(1);
                }
            }
        }
    }

    /// Advance Toxin Tractor poison field residual zones (medium spray + small death).
    pub(in super::super) fn update_toxin_tractor_poison_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                )
            })
            .collect();

        let plans = self
            .toxin_tractor
            .plan_due_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        crate::game_logic::host_poisoned_behavior::poison_weapon_damage_type(),
                        plan.death_type,
                    );
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.toxin_tractor.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.toxin_tractor.prune_expired(frame);
    }

    /// Apply ComancheRocketPodWeapon area residual at impact.
    ///
    /// Returns (units_hit, any_destroyed).
    /// Fail-closed: not full ScatterTarget clip pattern / projectile flight.
    /// C++ ComancheRocketPodWeapon ProjectileObject + ScatterTarget residual.
    pub fn spawn_comanche_rocket_pod_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        to: glam::Vec3,
        shot_index: u32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_comanche_rocket_pods::{
            COMANCHE_ROCKET_POD_PROJECTILE, COMANCHE_ROCKET_POD_PROJECTILE_LIFETIME_FRAMES,
            COMANCHE_ROCKET_POD_PROJECTILE_MAX_HEALTH,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(COMANCHE_ROCKET_POD_PROJECTILE) {
            let mut t = ThingTemplate::new(COMANCHE_ROCKET_POD_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(COMANCHE_ROCKET_POD_PROJECTILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(COMANCHE_ROCKET_POD_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let pid = self.create_object(COMANCHE_ROCKET_POD_PROJECTILE, team, from)?;
        let expires = self
            .frame
            .saturating_add(COMANCHE_ROCKET_POD_PROJECTILE_LIFETIME_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&pid) {
            o.comanche_rocket_pod_projectile = true;
            o.comanche_rocket_pod_projectile_expires_frame = Some(expires);
            o.note_producer(source_id);
            o.health.maximum = COMANCHE_ROCKET_POD_PROJECTILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, COMANCHE_ROCKET_POD_PROJECTILE_MAX_HEALTH);
            let dir = to - from;
            let dist = dir.length().max(0.001);
            let life = COMANCHE_ROCKET_POD_PROJECTILE_LIFETIME_FRAMES.max(1) as f32;
            o.movement.velocity = dir * (dist / life);
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = shot_index;
        self.comanche_rocket_pod_projectiles_spawned = self
            .comanche_rocket_pod_projectiles_spawned
            .saturating_add(1);
        Some(pid)
    }

    pub fn update_comanche_rocket_pod_projectiles(&mut self) {
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.comanche_rocket_pod_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in flying {
            if let Some(o) = self.objects.get_mut(&id) {
                let p = o.get_position();
                let v = o.movement.velocity;
                o.set_position(p + v);
            }
        }
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.comanche_rocket_pod_projectile {
                    if let Some(exp) = o.comanche_rocket_pod_projectile_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.comanche_rocket_pod_projectile = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn honesty_comanche_rocket_pod_projectile_ok(&self) -> bool {
        self.comanche_rocket_pod_projectiles_spawned > 0
    }

    pub fn apply_comanche_rocket_pod_area_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_comanche_rocket_pods::{
            ROCKET_POD_AUDIO, ROCKET_POD_DAMAGE_TYPE, ROCKET_POD_DEATH_TYPE,
            ROCKET_POD_SECONDARY_RADIUS, is_legal_rocket_pod_splash_target,
            rocket_pod_damage_at_distance,
        };

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_rocket_pod_splash_target(
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
                if dist <= ROCKET_POD_SECONDARY_RADIUS {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = rocket_pod_damage_at_distance(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    ROCKET_POD_DAMAGE_TYPE,
                    ROCKET_POD_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.comanche_rocket_pod_residual_area_attacks = self
            .comanche_rocket_pod_residual_area_attacks
            .saturating_add(1);
        self.comanche_rocket_pod_residual_units_hit = self
            .comanche_rocket_pod_residual_units_hit
            .saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(ROCKET_POD_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                None,
            );
        }

        (hits, any_destroyed)
    }

    /// Residual Sentry Drone auto-fire: a gun-equipped Sentry acquires the
    /// nearest enemy in weapon range without a manual AttackObject order.
    /// The acquired victim remains a normal pending attack while the exact
    /// parsed DeployStyleAIUpdate module unpacks; only ReadyToAttack may deal
    /// damage. Fail-closed: no guessed turret alignment/manual animation/LOS.
    pub(in super::super) fn try_sentry_drone_residual_fire(&mut self, sentry_id: ObjectId) {
        use crate::game_logic::host_sentry_drone::{
            SENTRY_GUN_AUDIO, is_legal_sentry_auto_fire_target,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&sentry_id) else {
            return;
        };
        // The name-based residual identifies the Sentry's separate gun-upgrade
        // feature, but DeployStyle weapon authority itself comes solely from
        // the parsed Behavior module. A Sentry-shaped test/mod template
        // without `TurretsFunctionOnlyWhenDeployed = Yes` must not get this
        // deploy-only auto-fire behavior by basename fallback.
        let deploy_only_turret = attacker
            .get_template()
            .deploy_style_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.turrets_function_only_when_deployed);
        if !deploy_only_turret
            || !attacker.is_alive()
            || attacker.weapon.is_none()
            || !attacker.can_attack()
        {
            return;
        }
        let Some(weapon) = attacker.weapon.as_ref() else {
            return;
        };
        if !Object::weapon_ready(weapon, current_time) {
            return;
        }

        let team = attacker.team;
        let range = weapon.range;
        let damage = weapon.damage;
        let fire_pos = attacker.get_position();

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
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
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            sentry_id,
            team,
            fire_pos,
            candidates,
            |_| range,
            |c| {
                let stealthed_hidden = c.effectively_stealthed && c.team != team;
                is_legal_sentry_auto_fire_target(
                    c.is_alive,
                    c.team == team,
                    c.is_neutral,
                    c.under_construction,
                    c.combat_kind,
                    stealthed_hidden,
                )
            },
        );

        let Some((target_id, _, _)) = best else {
            return;
        };

        // Keep an acquired target while the unpack timer runs. The regular
        // combat path owns subsequent target validity checks, so a destroyed
        // target is cleared through stop_attack_decision_aware before it can
        // become a stale post-deploy shot.
        if let Some(attacker) = self.objects.get_mut(&sentry_id) {
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(sentry_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(sentry_id, 2);
            }
        }
        if !self.ensure_deploy_style_ready_to_fire(sentry_id) {
            return;
        }

        let weapon_snap = self.objects.get(&sentry_id).and_then(|a| a.weapon.clone());
        let (destroyed, _) = self.residual_auto_fire_apply_damage(
            sentry_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            0,
        );

        if let Some(attacker) = self.objects.get_mut(&sentry_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                0,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon.as_mut() {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon
                    .as_ref()
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    0,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            // STEALTH_NOT_WHILE_ATTACKING residual.
            if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                attacker.break_stealth();
            }
            if destroyed {
                self.stop_attack_decision_aware(sentry_id);
            }
        }
        let _ = self.record_accepted_weapon_discharge(sentry_id, 0);

        if destroyed {
            self.award_score_the_kill_experience(sentry_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
        }

        let muzzle_pos = self
            .objects
            .get(&sentry_id)
            .map(|a| a.get_position())
            .unwrap_or(fire_pos);
        let impact_pos = self.objects.get(&target_id).map(|t| t.get_position());
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            muzzle_pos,
            impact_pos,
            self.frame,
            sentry_id,
            Some(target_id),
        );
        self.queue_audio_event(
            AudioEventRequest::new(SENTRY_GUN_AUDIO)
                .with_object(sentry_id)
                .with_position(muzzle_pos)
                .with_priority(160),
        );

        self.sentry_drone_residual_auto_fires =
            self.sentry_drone_residual_auto_fires.saturating_add(1);
    }

    /// Residual Hellfire Drone auto-fire: acquires nearest enemy in weapon range
    /// and deals damage without a manual AttackObject order.
    /// Fail-closed: not full SlavedUpdate / LOS / projectile flight matrix.
    pub(in super::super) fn try_hellfire_drone_residual_fire(&mut self, hellfire_id: ObjectId) {
        use crate::game_logic::host_slave_drones::{
            HELLFIRE_FIRE_AUDIO, is_legal_hellfire_auto_fire_target,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&hellfire_id) else {
            return;
        };
        if !attacker.is_alive() || attacker.weapon.is_none() || !attacker.can_attack() {
            return;
        }
        let Some(weapon) = attacker.weapon.as_ref() else {
            return;
        };
        if !Object::weapon_ready(weapon, current_time) {
            return;
        }

        let team = attacker.team;
        let range = weapon.range;
        let damage = weapon.damage;
        let fire_pos = attacker.get_position();

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
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
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            hellfire_id,
            team,
            fire_pos,
            candidates,
            |_| range,
            |c| {
                let stealthed_hidden = c.effectively_stealthed && c.team != team;
                is_legal_hellfire_auto_fire_target(
                    c.is_alive,
                    c.team == team,
                    c.is_neutral,
                    c.under_construction,
                    c.combat_kind,
                    stealthed_hidden,
                )
            },
        );

        let Some((target_id, _, _)) = best else {
            return;
        };

        let weapon_snap = self
            .objects
            .get(&hellfire_id)
            .and_then(|a| a.weapon.clone());
        // C++ Hellfire ScatterRadiusVsInfantry residual: vs infantry may miss.
        let mut destroyed = false;
        let target_is_infantry = self
            .objects
            .get(&target_id)
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut skip_damage = false;
        if target_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                hellfire_id.0,
                target_id.0,
                self.frame,
            );
            let hit_r = self
                .objects
                .get(&target_id)
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            self.hellfire_scatter_applied = self.hellfire_scatter_applied.saturating_add(1);
            if crate::game_logic::host_slave_drones::hellfire_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                self.hellfire_scatter_misses = self.hellfire_scatter_misses.saturating_add(1);
                skip_damage = true;
            }
        }
        if !skip_damage {
            let (d, _) = self.residual_auto_fire_apply_damage(
                hellfire_id,
                target_id,
                damage,
                fire_pos,
                weapon_snap.as_ref(),
                0,
            );
            destroyed = d;
        }

        if let Some(attacker) = self.objects.get_mut(&hellfire_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                0,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon.as_mut() {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon
                    .as_ref()
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    0,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(hellfire_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(hellfire_id, 2);
            }
            if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                attacker.break_stealth();
            }
            if destroyed {
                self.stop_attack_decision_aware(hellfire_id);
            }
        }
        // A scatter miss is still an accepted Hellfire launch: record the
        // actual source slot once, independently of per-victim damage.
        let _ = self.record_accepted_weapon_discharge(hellfire_id, 0);

        if destroyed {
            self.award_score_the_kill_experience(hellfire_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
        }

        let muzzle_pos = self
            .objects
            .get(&hellfire_id)
            .map(|a| a.get_position())
            .unwrap_or(fire_pos);
        let impact_pos = self.objects.get(&target_id).map(|t| t.get_position());
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            muzzle_pos,
            impact_pos,
            self.frame,
            hellfire_id,
            Some(target_id),
        );
        self.queue_audio_event(
            AudioEventRequest::new(HELLFIRE_FIRE_AUDIO)
                .with_object(hellfire_id)
                .with_position(muzzle_pos)
                .with_priority(160),
        );

        self.hellfire_drone_residual_auto_fires =
            self.hellfire_drone_residual_auto_fires.saturating_add(1);
    }

    /// Residual: attach Scout or Hellfire drone to a master vehicle (Humvee residual).
    ///
    /// Spawns the drone near the master, tags the master with the object-upgrade
    /// residual name, and records attach honesty.
    /// Fail-closed: not full ObjectCreationUpgrade ConflictsWith / ProductionUpdate.
    pub fn residual_attach_slave_drone(
        &mut self,
        master_id: ObjectId,
        kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_slave_drones::{
            SlaveDroneKind, drone_spawn_offset_from_master, is_slave_drone_master_template,
        };

        let (team, master_pos, owner_player_id) = {
            let master = self.objects.get(&master_id)?;
            if !master.is_alive() || master.status.under_construction {
                return None;
            }
            if !is_slave_drone_master_template(&master.template_name) {
                return None;
            }
            (master.team, master.get_position(), master.owner_player_id)
        };

        // Ensure drone templates exist with residual kinds.
        let drone_tpl_name = kind.template_name();
        if !self.templates.contains_key(drone_tpl_name) {
            let mut tpl = crate::game_logic::ThingTemplate::new(drone_tpl_name);
            tpl.add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(100.0);
            if matches!(kind, SlaveDroneKind::Hellfire) {
                tpl.set_primary_weapon_name(
                    crate::game_logic::host_slave_drones::HELLFIRE_MISSILE_WEAPON,
                );
            }
            if matches!(kind, SlaveDroneKind::Battle) {
                tpl.set_primary_weapon_name(
                    crate::game_logic::host_slave_drones::BATTLE_DRONE_MACHINE_GUN,
                );
            }
            // Scout: no primary.
            self.templates.insert(drone_tpl_name.to_string(), tpl);
        }
        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();

        let (ox, oz) = drone_spawn_offset_from_master(kind);
        let spawn_pos = Vec3::new(master_pos.x + ox, master_pos.y, master_pos.z + oz);
        let drone_id =
            self.create_object_for_owner_or_team(drone_tpl_name, team, owner_player_id, spawn_pos)?;

        // C++ startSlavedEffects (SlavedUpdate.cpp:700-701) OBJECT_STATUS_UNSELECTABLE.
        if let Some(drone) = self.objects.get_mut(&drone_id) {
            drone.producer_id = Some(master_id);
            drone.set_status_unselectable(true);
            if drone.upgrade_die.is_none() {
                drone.install_upgrade_die(kind.upgrade_name());
            }
        }

        // SpawnBehavior::computeAggregateStates aggregate residual: master and
        // drone sync to the higher rank at attach (SlavedUpdate live tick in
        // world_tick/ai.rs re-applies the same sync every frame).
        let (master_level, drone_level) = match (
            self.objects.get(&master_id),
            self.objects.get(&drone_id),
        ) {
            (Some(m), Some(d)) => (m.experience.level, d.experience.level),
            _ => return Some(drone_id),
        };
        let (sync_master, sync_drone) = crate::game_logic::host_slave_drones::synced_spawn_veterancy(
            master_level,
            drone_level,
        );
        if sync_master != master_level {
            if let Some(master) = self.objects.get_mut(&master_id) {
                master.set_min_veterancy_level(sync_master);
            }
        }
        if sync_drone != drone_level {
            if let Some(drone) = self.objects.get_mut(&drone_id) {
                drone.set_min_veterancy_level(sync_drone);
            }
        }

        if let Some(master) = self.objects.get_mut(&master_id) {
            master.apply_upgrade_tag(kind.upgrade_name());
        }

        match kind {
            SlaveDroneKind::Scout => {
                self.scout_drone_residual_attaches =
                    self.scout_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Hellfire => {
                self.hellfire_drone_residual_attaches =
                    self.hellfire_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Battle => {
                self.battle_drone_residual_attaches =
                    self.battle_drone_residual_attaches.saturating_add(1);
            }
        }

        Some(drone_id)
    }

    /// Residual honesty: bunker-buster blast + garrison kill or bunker damage.
    pub fn honesty_bunker_buster_ok(&self) -> bool {
        self.bunker_buster.honesty_host_path_ok()
    }

    /// Residual honesty: at least one garrison occupant killed by bunker residual.
    pub fn honesty_bunker_buster_garrison_kill_ok(&self) -> bool {
        self.bunker_buster.honesty_garrison_kill_ok()
    }

    /// Residual honesty: amplified bunker structure damage residual applied.
    pub fn honesty_bunker_buster_damage_ok(&self) -> bool {
        self.bunker_buster.honesty_bunker_damage_ok()
    }

    /// Residual honesty: KILL_GARRISONED microwave-style clearer residual.
    pub fn honesty_kill_garrisoned_ok(&self) -> bool {
        self.bunker_buster.honesty_kill_garrisoned_ok()
    }

    /// Host bunker-buster residual registry (tests / HUD honesty).
    pub fn bunker_buster_residual(
        &self,
    ) -> &crate::game_logic::host_bunker_buster::HostBunkerBusterRegistry {
        &self.bunker_buster
    }

    /// Apply BunkerBuster residual to a structure target:
    /// 100 typed occupant damage + force-exit (C++ harmAndForceExitAllContained)
    /// and amplified structure damage vs bunkers.
    ///
    /// Returns (occupants_killed, structure_damage_applied, structure_destroyed).
    pub(in super::super) fn apply_bunker_buster_to_target(
        &mut self,
        target_id: ObjectId,
        attacker_team: Team,
        base_weapon_damage: f32,
        attacker_id: Option<ObjectId>,
    ) -> (u32, f32, bool) {
        use crate::game_logic::host_bunker_buster::{
            BUNKER_BUSTER_AUDIO, BUNKER_BUSTER_HARM_AMOUNT, BUNKER_BUSTER_OCCUPANT_WEAPON,
            BUNKER_BUSTER_SHOCKWAVE_DAMAGE, BUNKER_BUSTER_SHOCKWAVE_RADIUS,
            BUNKER_BUSTER_SHOCKWAVE_WEAPON, STEALTH_JET_MISSILE_DAMAGE_TYPE,
            STEALTH_JET_MISSILE_DEATH_TYPE, bunker_buster_structure_damage,
            is_bunker_structure_name,
        };

        let (mut occupants, is_bunker, target_pos, is_tunnel, target_player) = {
            let Some(target) = self.objects.get(&target_id) else {
                return (0, 0.0, false);
            };
            let occ = target.contained_units();
            let bunker = is_bunker_structure_name(&target.template_name)
                || target
                    .building_data
                    .as_ref()
                    .map(|b| {
                        matches!(
                            b.building_type,
                            crate::game_logic::buildings::BuildingType::Bunker
                        ) || b.max_garrison > 0
                    })
                    .unwrap_or(false);
            let tunnel = target.is_tunnel_network_style_container()
                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                    &target.template_name,
                );
            (
                occ,
                bunker,
                target.get_position(),
                tunnel,
                target.tunnel_system_key(),
            )
        };
        // C++ TunnelContain::getContainedItemsList / harmAndForceExitAllContained
        // (TunnelContain.cpp:95) iterates the shared TunnelTracker pool.
        if is_tunnel {
            for uid in self.tunnel_network.contained_for_player(target_player) {
                if !occupants.contains(&uid) {
                    occupants.push(uid);
                }
            }
        }
        let had_occupants = !occupants.is_empty();
        let mut kills = 0u32;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();

        // Remove occupants from container bookkeeping first.
        if let Some(target) = self.objects.get_mut(&target_id) {
            for &occ_id in &occupants {
                target.remove_occupant(occ_id);
            }
        }
        // C++ OpenContain/TunnelContain::harmAndForceExitAllContained:
        // removeFromContain (expose) then attemptDamage(100 typed). Survivors stay ejected.
        let occupant_damage_type =
            crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
                BUNKER_BUSTER_OCCUPANT_WEAPON,
            );
        let occupant_death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
            Some(BUNKER_BUSTER_OCCUPANT_WEAPON),
            occupant_damage_type,
        );

        let mut ejected: Vec<ObjectId> = Vec::new();
        for occ_id in occupants {
            let Some(occ) = self.objects.get_mut(&occ_id) else {
                continue;
            };
            if !occ.is_alive() {
                continue;
            }
            occ.set_contained_by(None);
            occ.set_ai_state(AIState::Idle);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(occ_id, 0);
            }
            ejected.push(occ_id);
        }

        for &id in &ejected {
            if let Some(player_id) = self.tunnel_network.player_holding_unit(id) {
                if let Some(entry) = self.tunnel_network.record_exit(player_id, id, target_id) {
                    if entry != target_id {
                        if let Some(c) = self.objects.get_mut(&entry) {
                            c.remove_occupant(id);
                        }
                    }
                }
            }
        }

        for occ_id in ejected {
            crate::game_logic::object::prime_live_damage_context(
                attacker_id.and_then(|id| self.objects.get(&id)),
                Some(BUNKER_BUSTER_OCCUPANT_WEAPON),
                occupant_damage_type,
            );
            let Some(occ) = self.objects.get_mut(&occ_id) else {
                continue;
            };
            if !occ.is_alive() {
                continue;
            }
            let killed = occ.take_damage_from_typed_death(
                BUNKER_BUSTER_HARM_AMOUNT,
                attacker_id,
                occupant_damage_type,
                occupant_death_type,
            );
            if killed || !occ.is_alive() || occ.health.current <= 0.0 || occ.status.destroyed {
                kills = kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }

        for id in destroy_ids {
            self.mark_object_for_destruction(id, Some(attacker_team));
        }

        // C++ BunkerBusterBehavior::bustTheBunker FXList::doFXObj(DetonationFX, building).
        // Leftover authored FX_BunkerBusterExplosion — not weapon ProjectileDetonationFX.
        let _ = self.dispatch_fx_list_at_host_object(
            crate::game_logic::host_bunker_buster::BUNKER_BUSTER_DETONATION_FX,
            target_id,
            None,
        );
        let structure_dmg =
            bunker_buster_structure_damage(base_weapon_damage, is_bunker, had_occupants);
        let mut destroyed = false;
        if let Some(target) = self.objects.get_mut(&target_id) {
            destroyed = target.take_damage_from_immediate_residual(
                structure_dmg,
                attacker_id,
                STEALTH_JET_MISSILE_DAMAGE_TYPE,
                STEALTH_JET_MISSILE_DEATH_TYPE,
            );
            if destroyed {
                self.mark_object_for_destruction(target_id, Some(attacker_team));
            }
        }

        // C++ BunkerBusterBehavior::bustTheBunker createAndFireTempWeapon
        // leftover BunkerBusterShockwaveWeaponSmall (10 / r50) at the bunker.
        {
            use crate::game_logic::host_temporary_weapon_behavior::{
                FireWeaponWhenDeadEphemeralWeaponSpec, TemporaryWeaponSlot,
            };
            let spec = FireWeaponWhenDeadEphemeralWeaponSpec {
                module_source_index: 0,
                weapon_template_name: BUNKER_BUSTER_SHOCKWAVE_WEAPON.to_string(),
                weapon_slot: TemporaryWeaponSlot::Primary,
            };
            if self.create_and_fire_temp_weapon(target_id, &spec).is_none() {
                let _ = self.apply_instant_hit_splash_at(
                    target_pos,
                    BUNKER_BUSTER_SHOCKWAVE_DAMAGE,
                    0.0,
                    BUNKER_BUSTER_SHOCKWAVE_RADIUS,
                    0.0,
                    attacker_id.unwrap_or(target_id),
                    attacker_team,
                    target_id,
                    Some(BUNKER_BUSTER_SHOCKWAVE_WEAPON),
                );
            }
        }

        self.bunker_buster.record_bunker_buster_blast(
            kills,
            structure_dmg,
            is_bunker && structure_dmg > base_weapon_damage + 0.01,
        );

        self.queue_audio_event(
            AudioEventRequest::new(BUNKER_BUSTER_AUDIO)
                .with_position(target_pos)
                .with_priority(160),
        );

        (kills, structure_dmg, destroyed)
    }

    /// Apply KILL_GARRISONED residual: kill `floor(damage)` garrisoned occupants.

    /// Consume object-level DAMAGE_KILL_GARRISONED pending count into contain kills.
    pub(in super::super) fn flush_pending_kill_garrisoned(
        &mut self,
        target_id: ObjectId,
        attacker_id: Option<ObjectId>,
        attacker_team: Team,
    ) -> u32 {
        let pending = self
            .objects
            .get_mut(&target_id)
            .map(|o| o.take_pending_kill_garrisoned())
            .unwrap_or(0);
        if pending == 0 {
            return 0;
        }
        self.apply_kill_garrisoned_to_target(target_id, attacker_team, pending as f32, attacker_id)
    }

    /// Fail-closed: no structure HP damage (C++ ActiveBody KillGarrisoned path).
    pub(in super::super) fn apply_kill_garrisoned_to_target(
        &mut self,
        target_id: ObjectId,
        attacker_team: Team,
        damage_amount: f32,
        attacker_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_bunker_buster::{
            BUNKER_BUSTER_OCCUPANT_DAMAGE, kill_garrisoned_count,
        };

        let immune = self
            .objects
            .get(&target_id)
            .is_some_and(|t| !t.is_garrison_contain() || t.is_immune_to_clear_building_attacks());
        if immune {
            return 0;
        }

        let occupants = self
            .objects
            .get(&target_id)
            .map(|t| t.contained_units())
            .unwrap_or_default();
        let kill_n = kill_garrisoned_count(damage_amount, occupants.len());
        if kill_n == 0 {
            return 0;
        }

        let to_kill: Vec<ObjectId> = occupants.into_iter().take(kill_n).collect();
        if let Some(target) = self.objects.get_mut(&target_id) {
            for &occ_id in &to_kill {
                target.remove_occupant(occ_id);
            }
        }

        let mut kills = 0u32;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        for occ_id in to_kill {
            let Some(occ) = self.objects.get_mut(&occ_id) else {
                continue;
            };
            if !occ.is_alive() {
                continue;
            }
            occ.set_contained_by(None);
            occ.set_ai_state(AIState::Idle);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(occ_id, 0);
            }
            let _ = occ.take_damage_from(
                BUNKER_BUSTER_OCCUPANT_DAMAGE.max(occ.health.current * 10.0),
                attacker_id,
            );
            if !occ.is_alive() || occ.health.current <= 0.0 || occ.status.destroyed {
                kills = kills.saturating_add(1);
                destroy_ids.push(occ_id);
            } else {
                let _ = occ.take_damage_from(999_999.0, attacker_id);
                kills = kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }
        for id in destroy_ids {
            if let Some(player_id) = self.tunnel_network.player_holding_unit(id) {
                let _ = self.tunnel_network.record_exit(player_id, id, target_id);
            }
            self.mark_object_for_destruction(id, Some(attacker_team));
        }

        self.bunker_buster.record_kill_garrisoned(kills);
        kills
    }
}
