//! Host scripts `impl GameLogic` — `stealth_mines`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! stealth and detection / mines / demo traps
#![allow(unused_imports, non_snake_case)]
use super::super::*;

mod stealth_detection;
mod mine_placement;
mod mine_detonation;

mod cluster_placement;
mod construction_clearance;
#[cfg(test)]
#[path = "stealth_mines/construction_status_owner_tests.rs"]
mod construction_status_owner_tests;
mod mine_body_callbacks;
mod reveal_response;

impl GameLogic {
    /// Manually detonate a residual demo trap / charge (command residual).
    pub fn manual_detonate_mine(&mut self, mine_id: ObjectId) -> bool {
        self.manual_detonate_mine_with_effects(mine_id, &mut Default::default())
    }

    pub(crate) fn manual_detonate_mine_with_effects(
        &mut self,
        mine_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        use crate::game_logic::host_mines::HostMineDetonateReason;
        self.detonate_mine_internal_with_effects(
            mine_id,
            HostMineDetonateReason::Manual,
            None,
            effects,
        )
    }

    /// Advance residual mines: dozer clear + proximity scan + timed detonation.
    ///
    /// Clear residual (C++ DozerMineDisarmingWeapon DAMAGE_DISARM): only when
    /// WEAPONSET_MINE_CLEARING_DETAIL is armed (MSG_SET_MINE_CLEARING_DETAIL).
    /// Demo-trap skip is DISARM-while-attacking only (DemoTrapUpdate.cpp:181-191).
    /// Proximity fuse is ENEMIES only (DemoTrapUpdate.cpp:196).

    /// C++ StickyBombUpdate::update residual.
    ///
    /// Timed/remote demo charges with `attached_to` follow the target position
    /// (vehicle: ride on roof offset Z; immobile: keep plant XY, refresh ground Y).
    /// If the target dies, the charge is destroyed (C++ destroyObject(self)).
    /// Every LOGICFRAMES_PER_SECOND plays per-unit UnitBombPing.
    pub(crate) fn update_sticky_bomb_attachments(&mut self) {
        self.update_sticky_bomb_attachments_with_effects(&mut Default::default())
    }

    pub(crate) fn update_sticky_bomb_attachments_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let tree_move_frame = self.frame;
        use crate::game_logic::host_mines::{
            sticky_immobile_follow_pos, sticky_vehicle_follow_pos, HostMineKind, STICKY_OFFSET_Z,
            UNIT_BOMB_PING_AUDIO, UNIT_BOMB_PING_FRAMES,
        };

        let frame = self.frame;
        let sticky_ids: Vec<(ObjectId, Option<ObjectId>)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if !md.is_active() || !obj.is_alive() {
                    return None;
                }
                if !matches!(
                    md.kind,
                    HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
                ) {
                    return None;
                }
                Some((*id, md.attached_to))
            })
            .collect();

        let mut destroy_charges: Vec<ObjectId> = Vec::new();
        let mut moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();
        let mut pings: Vec<(ObjectId, glam::Vec3)> = Vec::new();

        for (charge_id, target_id) in sticky_ids {
            if let Some(target_id) = target_id {
                let Some(target) = self.objects.get(&target_id) else {
                    destroy_charges.push(charge_id);
                    continue;
                };
                if !target.is_alive() || target.status.effectively_dead {
                    destroy_charges.push(charge_id);
                    continue;
                }
                let tpos = target.get_position();
                let immobile =
                    target.is_kind_of(KindOf::Structure) || target.is_kind_of(KindOf::Immobile);
                let charge_pos = self
                    .objects
                    .get(&charge_id)
                    .map(|c| c.get_position())
                    .unwrap_or(tpos);
                let ground_y = self.terrain_ground_height_at(charge_pos).unwrap_or(0.0);
                let new_pos = if immobile {
                    // C++ IMMOBILE: keep bomber plant XY, only refresh Z/Y to terrain.
                    sticky_immobile_follow_pos(charge_pos, ground_y)
                } else {
                    sticky_vehicle_follow_pos(tpos, STICKY_OFFSET_Z)
                };
                moves.push((charge_id, new_pos));
            }

            if let Some(md) = self
                .objects
                .get(&charge_id)
                .and_then(|o| o.mine_data.as_ref())
            {
                if md.next_ping_frame.is_some_and(|at| frame >= at) {
                    let pos = self
                        .objects
                        .get(&charge_id)
                        .map(|o| o.get_position())
                        .unwrap_or(Vec3::ZERO);
                    pings.push((charge_id, pos));
                }
            }
        }

        for (charge_id, pos) in moves {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                obj.set_position_with_effects_and_pose_owners(
                    pos,
                    tree_move_frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
            }
            self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
        }
        for (charge_id, _pos) in pings {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let next = md.next_ping_frame.unwrap_or(frame);
                    md.next_ping_frame = Some(
                        next.saturating_add(UNIT_BOMB_PING_FRAMES)
                            .max(frame.saturating_add(UNIT_BOMB_PING_FRAMES)),
                    );
                }
            }
            // C++ StickyBombUpdate::update: getPerUnitSound("UnitBombPing") + setObjectID.
            self.queue_resolved_per_unit_sound(
                charge_id,
                UNIT_BOMB_PING_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
        for charge_id in destroy_charges {
            self.sticky_bomb_target_deaths = self.sticky_bomb_target_deaths.saturating_add(1);
            self.destroy_object_with_effects(charge_id, effects);
        }
    }

    fn capture_booby_plant_xy(&mut self) {
        let snaps: Vec<(ObjectId, ObjectId)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?)))
            .collect();
        for (sid, cid) in snaps {
            if let Some(pos) = self.objects.get(&cid).map(|o| o.get_position()) {
                self.booby_trap.capture_plant_xy_if_unset(sid, pos.x, pos.z);
            }
        }
    }

    fn restore_booby_plant_xy_and_ping_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let tree_move_frame = self.frame;
        use crate::game_logic::host_mines::{UNIT_BOMB_PING_AUDIO, UNIT_BOMB_PING_FRAMES};
        let frame = self.frame;
        let jobs: Vec<(ObjectId, ObjectId)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?)))
            .collect();
        for (sid, cid) in jobs {
            let ground_y = self
                .objects
                .get(&cid)
                .map(|o| {
                    self.terrain_ground_height_at(o.get_position())
                        .unwrap_or(0.0)
                })
                .unwrap_or(0.0);
            if let Some(pos) = self.booby_trap.immobile_follow_pos(sid, ground_y) {
                if let Some(obj) = self.objects.get_mut(&cid) {
                    obj.set_position_with_effects_and_pose_owners(
                        pos,
                        tree_move_frame,
                        &mut effects.reborrow_for_object(&mut self.partition_manager),
                        &self.host_trigger_world,
                        &self.team_factory,
                    );
                }
            }
        }
        let ping_ids: Vec<(ObjectId, ObjectId, u32)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?, p.next_ping_frame?)))
            .collect();
        let mut due = Vec::new();
        for (sid, cid, at) in ping_ids {
            if frame < at {
                continue;
            }
            if let Some(pos) = self.objects.get(&cid).map(|o| o.get_position()) {
                due.push((sid, cid, pos));
            }
        }
        for (sid, cid, _pos) in due {
            if let Some(p) = self.booby_trap.plant_mut(sid) {
                p.next_ping_frame = Some(frame.saturating_add(UNIT_BOMB_PING_FRAMES));
            }
            self.queue_resolved_per_unit_sound(cid, UNIT_BOMB_PING_AUDIO, true, false, None, 140);
        }
    }

    /// C++ RemoteC4Charge SpecialObjectsPersistWhenOwnerDies = No residual.
    /// TimedC4Charge persists (BURTON_TIMED_PERSIST_WHEN_OWNER_DIES = true).
    pub fn cleanup_remote_charges_when_owner_dies(&mut self) {
        self.cleanup_remote_charges_when_owner_dies_with_effects(&mut Default::default())
    }

    pub(crate) fn cleanup_remote_charges_when_owner_dies_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_mines::HostMineKind;
        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if md.detonated || !obj.is_alive() {
                    return None;
                }
                if md.kind != HostMineKind::RemoteDemoCharge {
                    return None;
                }
                let pid = md.producer_id.or(obj.producer_id)?;
                let owner_dead = self
                    .objects
                    .get(&pid)
                    .map(|p| !p.is_alive() || p.status.destroyed || p.status.effectively_dead)
                    .unwrap_or(true);
                if owner_dead {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                if let Some(md) = o.mine_data.as_mut() {
                    md.detonated = true;
                }
            }
            self.mark_object_for_destruction_with_effects(id, None, effects);
        }
    }

    /// C++ MinefieldBehavior::update producer-death drain + AutoHeal + onDamage virtual sync.
    fn update_land_mine_regen_and_virtual(&mut self) {
        self.update_land_mine_regen_and_virtual_with_effects(&mut Default::default())
    }

    fn update_land_mine_regen_and_virtual_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_mines::{HostMineKind, MINE_CREATOR_DEATH_CHECK_FRAMES};
        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if data.detonated
                    || !matches!(data.kind, HostMineKind::LandMine)
                    || (!obj.is_alive() && !data.defers_lethal_body_destroy())
                {
                    return None;
                }
                Some(*id)
            })
            .collect();
        for id in ids {
            let producer = self.objects.get(&id).and_then(|obj| {
                obj.mine_data
                    .as_ref()
                    .and_then(|data| data.producer_id)
                    .or(obj.producer_id)
            });
            let producer_dead = producer.is_some_and(|producer| {
                self.objects
                    .get(&producer)
                    .is_none_or(|object| object.status.effectively_dead)
            });
            let (amount, healing, self_drain, needs_sync) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                let health = obj.health.current;
                let max_health = obj.health.maximum.max(obj.max_health).max(1.0);
                let Some(data) = obj.mine_data.as_mut() else {
                    continue;
                };
                if data.regenerates
                    && data.stops_regen_after_creator_dies
                    && frame >= data.next_death_check_frame
                {
                    data.next_death_check_frame =
                        frame.saturating_add(MINE_CREATOR_DEATH_CHECK_FRAMES);
                    if producer_dead {
                        data.note_producer_dead(frame);
                        if let Some(auto_heal) = obj.default_auto_heal.as_mut() {
                            auto_heal.stopped = true;
                            auto_heal.pending_damage = false;
                        }
                    }
                }
                let (amount, healing, self_drain) = if data.draining {
                    (data.drain_health_amount(max_health), false, true)
                } else {
                    (
                        data.tick_regen_auto_heal(frame, health, max_health) - health,
                        true,
                        false,
                    )
                };
                let needs_sync = data
                    .last_synced_health
                    .is_none_or(|last| (last - health).abs() >= 1e-4);
                (amount, healing, self_drain, needs_sync)
            };
            if amount > 0.0 {
                if healing {
                    // CPP AutoHealBehavior284: direct healing, self benefactor.
                    let _ = self.apply_owned_healing_with_effects(id, amount, Some(id), effects);
                } else {
                    self.apply_residual_damage_owned_with_effects(
                        id,
                        amount,
                        Some(id),
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        None,
                        effects,
                    );
                }
            }
            // Finish this pad's real onHealing/onDamage callback before the
            // next actor. No batch of synthetic HP or delayed virtual blasts.
            if amount > 0.0 || needs_sync {
                self.synchronize_mine_body_owned_with_effects(
                    id,
                    amount > 0.0 && healing,
                    amount > 0.0 && self_drain,
                    effects,
                );
            }
        }
        self.drain_masked_object_selection();
    }

    pub fn update_mines_and_demo_traps(&mut self) {
        self.update_mines_and_demo_traps_with_effects(&mut Default::default())
    }

    pub(crate) fn update_mines_and_demo_traps_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_mines::{
            can_clear_mine_kind, demo_trap_skips_dozer_disarm_while_attacking, is_mine_clearer,
            land_mine_geometry_contacts, mine_clear_allowed_for_order, minefield_skips_worker,
            victim_mine_collide_radius, HostMineDetonateReason, HostMineKind,
            DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE,
        };

        let frame = self.frame;
        self.update_land_mine_scoot_with_effects(effects);

        self.update_land_mine_regen_and_virtual_with_effects(effects);

        // C++ StickyBombUpdate::update residual — stick to target / die with target.
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        {
            self.update_sticky_bomb_attachments_with_effects(effects);
        }
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        {
            self.capture_booby_plant_xy();
            self.update_booby_trap_special_attachments_with_effects(effects);
            self.restore_booby_plant_xy_and_ping_with_effects(effects);
        }
        // C++ SpecialObjectsPersistWhenOwnerDies = No for RemoteC4Charge residual.
        self.cleanup_remote_charges_when_owner_dies_with_effects(effects);
        let mut due: Vec<(ObjectId, HostMineDetonateReason)> = Vec::new();
        let mut clear_due: Vec<(ObjectId, ObjectId)> = Vec::new(); // (mine_id, clearer_id)
        let mut approach: Vec<(ObjectId, Vec3)> = Vec::new(); // clearer moves toward mine
                                                              // Clearers inside clear range winding up PreAttackDelay (not yet clear_due).
                                                              // C++ isClearingMines() && getGoalObject() != NULL keeps them immune while engaged.
        let mut windup: Vec<(ObjectId, ObjectId)> = Vec::new();
        // C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
        for (id, obj) in self.objects.registered() {
            let Some(data) = obj.mine_data.as_ref() else {
                continue;
            };
            if crate::game_logic::host_mines::should_detonate_when_killed(
                data.kind,
                crate::game_logic::host_mines::DEMO_TRAP_DETONATE_WHEN_KILLED,
                !obj.is_alive(),
                data.detonated,
                obj.status.under_construction || obj.status.sold,
            ) {
                due.push((*id, HostMineDetonateReason::Killed));
            }
        }

        // Collect active mine positions + params first (avoid borrow issues).
        let mines: Vec<(
            ObjectId,
            Team,
            Vec3,
            f32,
            bool,
            Option<u32>,
            bool,
            HostMineKind,
        )> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if !data.is_active() || !obj.is_alive() {
                    return None;
                }
                // C++ DemoTrapUpdate::update returns on OBJECT_STATUS_SOLD.
                if obj.status.sold {
                    return None;
                }
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    data.trigger_range,
                    data.proximity_enabled,
                    data.detonate_at_frame,
                    obj.status.under_construction,
                    data.kind,
                ))
            })
            .collect();

        // Mine clearers: only after player/AI issued MINE_CLEARING_DETAIL.
        let clearers: Vec<(ObjectId, Team, Vec3, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                if !is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name) {
                    return None;
                }
                if !mine_clear_allowed_for_order(obj.weapon_set_mine_clearing_detail) {
                    return None;
                }
                let busy = matches!(
                    obj.ai_state(),
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Capturing
                        | AIState::SpecialAbility
                );
                Some((*id, obj.team, obj.get_position(), busy))
            })
            .collect();

        // Potential victims: dozers skip only DISARM + IS_ATTACKING (C++ :181-191).
        let victims: Vec<(ObjectId, Team, Vec3, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                let is_dozer = is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name);
                let weapon_disarm = obj
                    .weapon_name_for_slot(0)
                    .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                    .unwrap_or(false)
                    || obj.weapon_set_mine_clearing_detail;
                let attacking =
                    obj.status.attacking || matches!(obj.ai_state(), AIState::Attacking);
                if demo_trap_skips_dozer_disarm_while_attacking(is_dozer, weapon_disarm, attacking)
                {
                    return None;
                }
                let combatant = obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Attackable);
                if !combatant {
                    return None;
                }
                let ground_y = self.terrain_height_at(obj.get_position()).unwrap_or(0.0);
                if crate::game_logic::host_mines::is_above_terrain(obj.get_position().y, ground_y) {
                    return None;
                }
                let geom = &(*obj.geometry_info());
                let victim_r = victim_mine_collide_radius(
                    geom.authored,
                    geom.bounding_circle_radius(),
                    obj.selection_radius,
                );
                Some((*id, obj.team, obj.get_position(), victim_r))
            })
            .collect();

        // Dozer/Worker clear + approach residual before proximity (so clear wins).
        // C++: only enemy/neutral mines (not ally/own) — residual uses team inequality.
        let clear_range_sqr = DOZER_MINE_CLEAR_RANGE * DOZER_MINE_CLEAR_RANGE;
        let scan_range_sqr = DOZER_MINE_CLEAR_SCAN_RANGE * DOZER_MINE_CLEAR_SCAN_RANGE;
        for (cid, cteam, cpos, busy) in &clearers {
            if *busy {
                continue;
            }
            // Pure residual acquire: nearest clearable enemy/neutral mine in scan range (XZ).
            let mine_cands: Vec<_> = mines
                .iter()
                .filter(|(_, mine_team, _, _, _, _, under_construction, kind)| {
                    !*under_construction && can_clear_mine_kind(*kind) && *mine_team != *cteam
                })
                .map(|(mine_id, mine_team, mine_pos, _, _, _, _, _)| {
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: *mine_id,
                        team: *mine_team,
                        position: *mine_pos,
                        is_alive: true,
                        is_neutral: *mine_team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    }
                })
                .collect();
            let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(*cid),
                (cpos.x, cpos.z),
                mine_cands,
                DOZER_MINE_CLEAR_SCAN_RANGE,
                |_| true,
            )
            .map(|(mine_id, dist, _)| {
                let mine_pos = mines
                    .iter()
                    .find(|(id, _, _, _, _, _, _, _)| *id == mine_id)
                    .map(|(_, _, p, _, _, _, _, _)| *p)
                    .unwrap_or(*cpos);
                (mine_id, dist * dist, mine_pos)
            });
            // Keep scan_range_sqr referenced for residual parity with prior sqr gate.
            let _ = scan_range_sqr;
            if let Some((mine_id, dist_sqr, mine_pos)) = best {
                if dist_sqr <= clear_range_sqr {
                    let delay = self
                        .objects
                        .get(cid)
                        .map(|o| {
                            crate::game_logic::host_mines::mine_clear_pre_attack_frames(
                                o.is_kind_of(KindOf::Worker),
                                &o.template_name,
                            )
                        })
                        .unwrap_or(
                            crate::game_logic::host_mines::DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES,
                        );
                    let ready = self
                        .objects
                        .get_mut(&mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.begin_or_ready_clear_pre_attack(*cid, frame, delay));
                    if !clear_due.iter().any(|(m, _)| *m == mine_id) {
                        if ready {
                            clear_due.push((mine_id, *cid));
                        } else {
                            windup.push((mine_id, *cid));
                        }
                    }
                } else {
                    if let Some(obj) = self.objects.get_mut(&mine_id) {
                        if let Some(md) = obj.mine_data.as_mut() {
                            if md.clear_pre_attack_clearer == Some(*cid) {
                                md.reset_clear_pre_attack();
                            }
                        }
                    }
                    // Approach residual: move idle clearer toward nearest mine.
                    approach.push((*cid, mine_pos));
                }
            }
        }

        // C++ MinefieldBehavior::update expire immunities (collideTime + 2).
        for (mine_id, _, _, _, _, _, _, kind) in &mines {
            if *kind != HostMineKind::LandMine {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    md.expire_immunes(frame);
                }
            }
        }

        // Clearers with a goal mine are immune to neighboring pads (MAX_IMMUNITY).
        let mut clearing_ids: Vec<ObjectId> = Vec::new();
        for (mine_id, cid) in &clear_due {
            clearing_ids.push(*cid);
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if matches!(md.kind, HostMineKind::LandMine) {
                        md.grant_immunity(*cid, frame);
                    }
                }
            }
        }
        for (mine_id, cid) in &windup {
            clearing_ids.push(*cid);
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if matches!(md.kind, HostMineKind::LandMine) {
                        md.grant_immunity(*cid, frame);
                    }
                }
            }
        }
        for (cid, mine_pos) in &approach {
            clearing_ids.push(*cid);
            // Grant immunity on every land mine the clearer is currently overlapping.
            for (mine_id, _, mpos, trigger, .., kind) in &mines {
                if *kind != HostMineKind::LandMine {
                    continue;
                }
                let dx = mine_pos.x - mpos.x;
                let dz = mine_pos.z - mpos.z;
                let r = trigger.max(DOZER_MINE_CLEAR_SCAN_RANGE);
                if dx * dx + dz * dz <= r * r {
                    if let Some(obj) = self.objects.get_mut(mine_id) {
                        if let Some(md) = obj.mine_data.as_mut() {
                            md.grant_immunity(*cid, frame);
                        }
                    }
                }
            }
        }

        let mut land_trips: Vec<(ObjectId, ObjectId, Vec3)> = Vec::new();
        for (
            mine_id,
            _mine_team,
            mine_pos,
            trigger_range,
            proximity,
            detonate_at,
            under_construction,
            kind,
        ) in &mines
        {
            if *under_construction {
                continue;
            }
            // C++ MinefieldBehavior::onCollide :360-362 — inert until scoot ends.
            if self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.is_scooting())
            {
                continue;
            }

            // Already scheduled for safe clear this frame — do not also detonate.
            if clear_due.iter().any(|(m, _)| *m == *mine_id) {
                continue;
            }
            if let Some(at) = detonate_at {
                if frame >= *at {
                    due.push((*mine_id, HostMineDetonateReason::Timed));
                    continue;
                }
            }
            let warning_ready = self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.demo_trap_warning_ready(frame));
            if warning_ready {
                due.push((*mine_id, HostMineDetonateReason::Proximity));
                continue;
            }
            if self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.demo_trap_warning_armed())
            {
                continue;
            }
            if !proximity || *trigger_range <= 0.0 {
                continue;
            }
            let range_sqr = trigger_range * trigger_range;
            let workers_detonate = self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .map(|m| m.workers_detonate)
                .unwrap_or(false);
            for (vid, _vteam, vpos, vgeom) in &victims {
                if *vid == *mine_id {
                    continue;
                }
                if self.mine_proximity_skips_friendly(*mine_id, *vid) {
                    continue;
                }
                // Land mines: C++ MinefieldBehavior::onCollide (geometry overlap).
                // Demo traps: C++ DemoTrapUpdate FROM_CENTER_2D trigger range.
                let in_contact = if *kind == HostMineKind::LandMine {
                    land_mine_geometry_contacts(*mine_pos, *trigger_range, *vpos, *vgeom)
                } else {
                    let dx = vpos.x - mine_pos.x;
                    let dz = vpos.z - mine_pos.z;
                    dx * dx + dz * dz <= range_sqr
                };
                if !in_contact {
                    continue;
                }
                if *kind == HostMineKind::LandMine {
                    let Some(victim) = self.objects.get(vid) else {
                        continue;
                    };
                    if minefield_skips_worker(
                        workers_detonate,
                        victim.is_kind_of(KindOf::Infantry),
                        victim.is_kind_of(KindOf::Dozer),
                    ) {
                        continue;
                    }
                    if clearing_ids.contains(vid) {
                        if let Some(obj) = self.objects.get_mut(mine_id) {
                            if let Some(md) = obj.mine_data.as_mut() {
                                md.grant_immunity(*vid, frame);
                            }
                        }
                        continue;
                    }
                    let immune = self
                        .objects
                        .get_mut(mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.refresh_immune(*vid, frame));
                    if immune {
                        continue;
                    }
                    let allow = self
                        .objects
                        .get_mut(mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.allow_repeat_detonate(*vid, *vpos));
                    if !allow {
                        continue;
                    }
                    land_trips.push((*mine_id, *vid, *vpos));
                    break;
                }
                due.push((*mine_id, HostMineDetonateReason::Proximity));
                break;
            }
        }

        // Safe clears first (mine gone, no splash).
        for (mine_id, clearer_id) in clear_due {
            let _ = self.clear_mine_internal_with_effects(mine_id, clearer_id, effects);
        }

        // Idle clearer approach: set move toward nearest enemy mine.
        for (clearer_id, mine_pos) in approach {
            if let Some(obj) = self.objects.get_mut(&clearer_id) {
                // Don't clobber an explicit non-idle order already in flight.
                if matches!(
                    obj.ai_state(),
                    AIState::Idle | AIState::Moving | AIState::Attacking
                ) || obj.target.is_none()
                {
                    obj.set_ai_state_at_frame(
                        AIState::Moving,
                        frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    obj.movement.target_position = Some(mine_pos);
                    self.movement_events.record(
                        frame,
                        clearer_id,
                        Some([mine_pos.x, mine_pos.y, mine_pos.z]),
                    );
                    obj.set_status_moving(true);
                }
            }
        }

        for (mine_id, _victim, vpos) in land_trips {
            let _ = self.trip_virtual_land_mine_with_effects(mine_id, vpos, effects);
        }

        for (mine_id, reason) in due {
            let _ = self.detonate_mine_internal_with_effects(mine_id, reason, None, effects);
        }
    }

    /// C++ MinefieldBehavior::detonateOnce — one virtual mine, pad may persist.
    fn trip_virtual_land_mine(&mut self, mine_id: ObjectId, victim_pos: Vec3) -> bool {
        self.trip_virtual_land_mine_with_effects(mine_id, victim_pos, &mut Default::default())
    }

    fn trip_virtual_land_mine_with_effects(
        &mut self,
        mine_id: ObjectId,
        victim_pos: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        use crate::game_logic::host_enum_table_residual::rubble_model_bit;
        use crate::game_logic::host_mines::{
            clip_point_to_mine_footprint, LAND_MINE_GEOMETRY_RADIUS, MINE_MIN_HEALTH,
        };

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        // C++ detonateOnce from onDamage while the pad may already be at 0 HP.
        let allow_zero_hp = mine
            .mine_data
            .as_ref()
            .is_some_and(|md| md.defers_lethal_body_destroy());
        if !mine.is_alive() && !allow_zero_hp {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if !data.is_active()
            || !matches!(
                data.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            )
        {
            return false;
        }
        let mine_pos = mine.get_position();
        let blast_pos =
            clip_point_to_mine_footprint(mine_pos, victim_pos, LAND_MINE_GEOMETRY_RADIUS);
        // CPP detonateOnce fires the weapon synchronously before decrementing
        // virtual mines. Neighbor onDamage callbacks therefore observe the
        // still-current source charge and may complete nested detonations.
        let (damage, radius, kind, mine_team, producer) = (
            data.detonation_damage,
            data.detonation_radius,
            data.kind,
            mine.team,
            data.producer_id,
        );
        self.mine_residual_proximity_detonations =
            self.mine_residual_proximity_detonations.saturating_add(1);
        self.apply_mine_splash_with_effects(
            mine_id, blast_pos, damage, radius, kind, mine_team, producer, effects,
        );
        let destroy_pad = {
            let Some(obj) = self.objects.get_mut(&mine_id) else {
                return false;
            };
            let Some(md) = obj.mine_data.as_mut() else {
                return false;
            };
            md.consume_virtual_mine()
        };
        if destroy_pad {
            // This blast already ran. CPP requests deletion, not another
            // weapon fire or a synthetic lethal Body/onDie operation.
            self.destroy_object_with_effects(mine_id, effects);
        }

        // CPP MinefieldBehavior.cpp:271-292: persistent pads drain through Body.
        let amount = self
            .objects
            .get(&mine_id)
            .and_then(|obj| {
                obj.mine_data.as_ref().map(|md| {
                    let desired =
                        (md.virtual_health_fraction() * obj.health.maximum).max(MINE_MIN_HEALTH);
                    obj.health.current - desired
                })
            })
            .unwrap_or(0.0);
        if !destroy_pad {
            self.apply_guarded_mine_body_drain_with_effects(mine_id, amount, effects);
        }
        // Re-resolve the canonical pad after callbacks. The guard suppresses
        // only Minefield's recursive virtual-count reaction, not other modules.
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            let hp = obj.health.current;
            let Some(md) = obj.mine_data.as_mut() else {
                return true;
            };
            md.last_synced_health = Some(hp);
            let empty = md.virtual_mines_remaining == 0;
            let bit = 1u128 << rubble_model_bit();
            if empty {
                obj.model_condition_bits |= bit;
                obj.set_status_masked(true);
            } else {
                obj.model_condition_bits &= !bit;
                obj.set_status_masked(false);
            }
        }
        self.admit_object_drawable_deselection(mine_id);
        true
    }

    /// CPP Minefield detonation/disarm ignoreDamage guards only its own
    /// virtual-count reaction; the admitted Body hit still reaches AutoHeal.
    fn apply_guarded_mine_body_drain(&mut self, mine_id: ObjectId, amount: f32) {
        self.apply_guarded_mine_body_drain_with_effects(mine_id, amount, &mut Default::default())
    }

    fn apply_guarded_mine_body_drain_with_effects(
        &mut self,
        mine_id: ObjectId,
        amount: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if amount <= 0.0 {
            return;
        }
        let previous_ignore = {
            let Some(data) = self
                .objects
                .get_mut(&mine_id)
                .and_then(|obj| obj.mine_data.as_mut())
            else {
                return;
            };
            let previous = data.ignore_damage;
            data.ignore_damage = true;
            previous
        };
        self.apply_residual_damage_owned_with_effects(
            mine_id,
            amount,
            Some(mine_id),
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::None,
            None,
            effects,
        );
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            let hp = obj.health.current;
            if let Some(data) = obj.mine_data.as_mut() {
                data.ignore_damage = previous_ignore;
                data.last_synced_health = Some(hp);
            }
        }
    }

}

/// C++ `createAndFireTempWeapon` hits other `KINDOF_MINE` pads (Weapon.cpp:1283).
/// Their `MinefieldBehavior::onDamage` then `detonateOnce` until health bands match.
fn land_mine_pad_takes_splash(victim: &Object) -> bool {
    victim.mine_data.as_ref().is_some_and(|md| {
        !md.detonated
            && matches!(
                md.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            )
            && md.has_virtual_charge()
    })
}

/// C++ `DAMAGE_RANGE_CALC_TYPE = FROM_BOUNDINGSPHERE_3D` (Weapon.cpp:70).
/// Barracks-ring pads sit ~30 wu apart with GeometryMajorRadius 30 — center
/// distance misses the r5 StructureMineWeapon, surface gap does not.
fn mine_splash_distance(victim: &Object, blast_pos: Vec3) -> f32 {
    if land_mine_pad_takes_splash(victim) {
        let sphere = crate::game_logic::combat::victim_splash_sphere_radius(victim)
            .max(crate::game_logic::host_mines::LAND_MINE_GEOMETRY_RADIUS);
        crate::game_logic::combat::splash_from_bounding_sphere_3d(
            blast_pos,
            victim.get_position(),
            sphere,
        )
    } else {
        let vpos = victim.get_position();
        let dx = vpos.x - blast_pos.x;
        let dz = vpos.z - blast_pos.z;
        (dx * dx + dz * dz).sqrt()
    }
}

#[cfg(test)]
#[path = "virtual_mine_damage_owner_tests.rs"]
mod virtual_mine_damage_owner_tests;
