//! Host tick `impl GameLogic` — `crates`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// When an AI computer unit kills and a money crate is spawned nearby, notify.
    ///
    /// C++ CreateCrateDie: only PLAYER_COMPUTER killers get notifyCrate.

    /// C++ CreateCrateDie::onDie residual for host destruction processing.
    ///
    /// Uses victim template `create_crate_data` list + last_damage_source as killer.
    /// Spawns money crates and notifies computer killers.

    /// C++ SalvageCrateCollide::executeCrateBehavior residual.
    ///
    /// Priority: armor set → weapon set (chance) → level (chance) → money.
    /// Returns (kind label, money granted).

    /// C++ VeterancyCrateCollide::executeCrateBehavior residual (non-pilot AOE).
    ///
    /// Grants `levels` to picker and same-team allies within effect_range
    /// (0 range = picker only). Fail-closed: not full pilot goal-object gate.

    /// Heals all living objects owned by the picker player (C++ healAllObjects).

    /// C++ ShroudCrateCollide::executeCrateBehavior residual.
    ///
    /// Permanently reveals the map for the picker's player (PartitionManager
    /// revealMapForPlayer).

    /// C++ ScriptEngine::transferObjectName residual (host bridge).
    ///
    /// Moves a script-visible name from `from_id` to `to_id` via host name field
    /// + NamedObjectTracker when available.

    /// C++ `targetCanEject`: a target exposes any `EjectPilotDie` interface.
    ///
    /// Vehicle/hijack eligibility remains owned by the surrounding Hijacker
    /// flow.  This final predicate must not infer the interface from a USA
    /// vehicle basename, and OCL/death-filter support is deliberately not
    /// consulted: C++ asks only `getEjectPilotDieInterface()` here.
    pub fn vehicle_supports_hijacker_ride(&self, vehicle_id: ObjectId) -> bool {
        let Some(v) = self.objects.get(&vehicle_id) else {
            return false;
        };
        if !v.is_alive() || v.status.destroyed {
            return false;
        }
        v.thing()
            .template
            .eject_pilot_die
            .as_ref()
            .is_some_and(|metadata| metadata.has_eject_pilot_die_interface())
    }

    /// C++ CommandButtonHuntUpdate::setCommandButton: store the selected
    /// button, then synchronously enter the installed AI provider's aiIdle.
    pub fn start_command_button_hunt(
        &mut self,
        unit_id: ObjectId,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
    ) -> gamelogic::GameLogicResult<bool> {
        self.start_command_button_hunt_with_effects(unit_id, mode, None, &mut Default::default())
    }

    pub(crate) fn start_command_button_hunt_with_effects(
        &mut self,
        unit_id: ObjectId,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
        button: Option<&str>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return Ok(false);
        };
        if !unit.is_alive() {
            return Ok(false);
        }
        unit.arm_command_button_hunt(mode, frame);
        if let Some(button) = button {
            if let Some(hunt) = unit.command_button_hunt.as_mut() {
                hunt.button_name = button.to_string();
                hunt.weapon_slot =
                    crate::game_logic::host_command_button_hunt::weapon_slot_from_button_name(
                        button,
                    );
            }
        }
        // C++ returns early when there is no AIUpdateInterface. Its behavior
        // component retains the selected button, but no AI command is issued.
        let _entered = self.unit_command_ai_idle_owned_with_effects(unit_id, effects)?;
        self.command_button_hunt_reg.record_start();
        Ok(true)
    }

    /// Arm CommandButtonHuntUpdate from a scripted button name (or unit template).
    pub fn start_command_button_hunt_named(
        &mut self,
        unit_id: ObjectId,
        button: Option<&str>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.start_command_button_hunt_named_with_effects(unit_id, button, &mut Default::default())
    }

    pub(crate) fn start_command_button_hunt_named_with_effects(
        &mut self,
        unit_id: ObjectId,
        button: Option<&str>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        use crate::game_logic::host_command_button_hunt::{
            hunt_mode_from_button_name, hunt_mode_from_template,
        };
        let Some(unit) = self.objects.get(&unit_id) else {
            return Ok(false);
        };
        if !unit.is_alive() {
            return Ok(false);
        }
        // C++ doTeamHuntWithCommandButton has no template-name fallback.
        // Named button: leftover hunt_mode_from_button_name only; miss is a no-op.
        let mode = match button {
            Some(name) => hunt_mode_from_button_name(name),
            None => hunt_mode_from_template(&unit.template_name),
        };
        let Some(mode) = mode else {
            return Ok(false);
        };
        self.start_command_button_hunt_with_effects(unit_id, mode, button, effects)
    }

    /// C++ leftover `get_ai_update_interface().is_some()`.
    /// Factory module list when loaded; else combat/mobile heuristic so stunned
    /// infantry and turret structures still receive Guard/Hunt-with-button.
    pub(crate) fn host_unit_has_ai_update(unit: &crate::game_logic::Object) -> bool {
        match leftover_template_has_ai_update(&unit.template_name) {
            Some(has) => has,
            None => {
                if unit.is_kind_of(KindOf::Mine) || unit.is_kind_of(KindOf::Projectile) {
                    return false;
                }
                if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
                    return unit.can_attack() || unit.weapon.is_some();
                }
                true
            }
        }
    }

    /// C++ AIGroup/ScriptActions Guard: AI interface only. No canMove/Immobile/Structure.
    pub(crate) fn host_unit_can_guard(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|u| {
            u.is_alive() && !u.status.destroyed && Self::host_unit_has_ai_update(u)
        })
    }

    /// C++ `doTeamHuntWithCommandButton`: AI + command-set + CommandButtonHuntUpdate.
    /// No `is_mobile` gate (stunned hijackers still arm).
    pub(crate) fn unit_can_team_hunt_with_command_button(
        &self,
        unit_id: ObjectId,
        button: Option<&str>,
    ) -> bool {
        use crate::game_logic::host_command_button_hunt::is_command_button_hunt_template;
        let Some(unit) = self.objects.get(&unit_id) else {
            return false;
        };
        if !unit.is_alive() || unit.status.destroyed {
            return false;
        }
        if !Self::host_unit_has_ai_update(unit) {
            return false;
        }
        match leftover_template_has_command_button_hunt(&unit.template_name) {
            Some(false) => return false,
            Some(true) => {}
            None => {
                if !is_command_button_hunt_template(&unit.template_name) {
                    return false;
                }
            }
        }
        let Some(btn) = button.filter(|s| !s.is_empty()) else {
            return true;
        };
        match self.unit_command_set_contains_button(unit, btn) {
            Some(false) => false,
            Some(true) | None => true,
        }
    }

    fn unit_command_set_contains_button(
        &self,
        unit: &crate::game_logic::Object,
        button: &str,
    ) -> Option<bool> {
        let manager = game_engine::common::ini::ini_command_set::get_command_set_manager()?;
        let set_name = unit
            .command_set_override
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| leftover_template_command_set(&unit.template_name));
        let Some(set_name) = set_name else {
            return leftover_template_known(&unit.template_name).map(|_| false);
        };
        let Some(set) = manager.find_command_set_resolved(&set_name) else {
            return Some(false);
        };
        Some(
            set.get_all_buttons()
                .iter()
                .any(|b| b.eq_ignore_ascii_case(button)),
        )
    }

    /// C++ CommandButtonHuntUpdate::update residual.
    pub fn tick_command_button_hunt_updates(&mut self) {
        self.tick_command_button_hunt_updates_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn tick_command_button_hunt_updates_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_command_button_hunt::{
            HUNT_CMD_FROM_AI, HostCommandButtonHuntData, HostCommandButtonHuntMode,
            hunt_last_command_is_from_ai,
        };

        let frame = self.frame;

        let busy: std::collections::HashSet<ObjectId> =
            self.pending_special_abilities.keys().copied().collect();
        let hunters: Vec<(
            ObjectId,
            HostCommandButtonHuntData,
            Team,
            glam::Vec3,
            AIState,
        )> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                let h = o.command_button_hunt.as_ref()?;
                if !h.due(frame) {
                    return None;
                }
                Some((
                    *id,
                    h.clone(),
                    o.team,
                    o.get_position(),
                    o.ai_state().clone(),
                ))
            })
            .collect();

        for (hunter_id, hunt, hunter_team, hunter_pos, ai_state) in hunters {
            // C++ update(): last command != CMD_FROM_AI permanently ends hunt.
            let last_src = self
                .objects
                .get(&hunter_id)
                .map(|o| o.last_command_source)
                .unwrap_or(HUNT_CMD_FROM_AI);
            if !hunt_last_command_is_from_ai(last_src) {
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.clear_command_button_hunt();
                }
                self.command_button_hunt_reg.record_cancel();
                continue;
            }

            self.command_button_hunt_reg.record_scan();
            if let Some(h) = self
                .objects
                .get_mut(&hunter_id)
                .and_then(|o| o.command_button_hunt.as_mut())
            {
                h.schedule_next(frame);
            }

            match hunt.mode {
                HostCommandButtonHuntMode::FireWeapon => {
                    // C++ huntWeapon: if idle then aiHunt; always temp-lock slot.
                    if matches!(ai_state, AIState::Idle) {
                        let _ = self.unit_command_patrol(hunter_id);
                    }
                    if let Some(u) = self.objects.get_mut(&hunter_id) {
                        let _ = u.set_weapon_lock(
                            hunt.weapon_slot,
                            crate::game_logic::WeaponLockType::LockedTemporarily,
                        );
                        u.last_command_source = HUNT_CMD_FROM_AI;
                    }
                    self.command_button_hunt_reg.record_target();
                    continue;
                }
                HostCommandButtonHuntMode::SpecialPower => {
                    if !matches!(ai_state, AIState::Idle) || busy.contains(&hunter_id) {
                        continue;
                    }
                    let Some(target_id) = self.scan_command_button_hunt_special_target(
                        hunter_id,
                        hunter_team,
                        hunter_pos,
                        &hunt.button_name,
                    ) else {
                        continue;
                    };
                    if self.issue_command_button_hunt_special_with_effects(
                        hunter_id,
                        target_id,
                        &hunt.button_name,
                        presentation_effects,
                    ) {
                        if let Some(u) = self.objects.get_mut(&hunter_id) {
                            u.last_command_source = HUNT_CMD_FROM_AI;
                        }
                        self.command_button_hunt_reg.record_target();
                    }
                    continue;
                }
                HostCommandButtonHuntMode::HijackVehicle
                | HostCommandButtonHuntMode::ConvertToCarBomb
                | HostCommandButtonHuntMode::SabotageBuilding => {}
            }

            if !matches!(ai_state, AIState::Idle) || busy.contains(&hunter_id) {
                continue;
            }

            let Some(target_id) =
                self.scan_command_button_hunt_enter_target(hunter_id, hunter_pos, hunt.mode)
            else {
                continue;
            };

            let ability = match hunt.mode {
                HostCommandButtonHuntMode::HijackVehicle => {
                    PendingSpecialAbility::Hijack { target_id }
                }
                HostCommandButtonHuntMode::ConvertToCarBomb => {
                    PendingSpecialAbility::CarBomb { target_id }
                }
                HostCommandButtonHuntMode::SabotageBuilding => {
                    PendingSpecialAbility::Sabotage { target_id }
                }
                HostCommandButtonHuntMode::SpecialPower | HostCommandButtonHuntMode::FireWeapon => {
                    continue;
                }
            };
            self.queue_pending_special_ability_with_effects(
                hunter_id,
                ability,
                presentation_effects,
            );
            if let Some(tp) = self.objects.get(&target_id).map(|t| t.get_position()) {
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.target = Some(target_id);
                    u.set_ai_state(
                        AIState::SpecialAbility,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    u.last_command_source = HUNT_CMD_FROM_AI;
                }
                let _ = self.assign_unit_path(hunter_id, tp, &[]);
            }
            if let Some(u) = self.objects.get_mut(&hunter_id) {
                u.last_command_source = HUNT_CMD_FROM_AI;
            }
            self.command_button_hunt_reg.record_target();
        }
    }

    /// C++ scanClosestTarget enter branch: first ActionManager-legal near-to-far.
    fn scan_command_button_hunt_enter_target(
        &self,
        hunter_id: ObjectId,
        hunter_pos: glam::Vec3,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_car_bomb::hijack_target_rejected;
        use crate::game_logic::host_command_button_hunt::{
            COMMAND_BUTTON_HUNT_SCAN_RANGE, hunt_enter_action_ok, hunt_same_map_status,
            hunt_stealthed_undetected,
        };
        use gamelogic::common::Relationship;

        let hunter_off = self.hunt_pos_off_map(hunter_pos);
        let mut candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(tid, t)| {
                if *tid == hunter_id || !t.is_alive() {
                    return None;
                }
                let d = hunt_dist_2d(hunter_pos, t.get_position());
                if d > COMMAND_BUTTON_HUNT_SCAN_RANGE {
                    return None;
                }
                Some((*tid, d))
            })
            .collect();
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (tid, _) in candidates {
            let Some(t) = self.objects.get(&tid) else {
                continue;
            };
            if !hunt_same_map_status(hunter_off, self.hunt_pos_off_map(t.get_position())) {
                continue;
            }
            if hunt_stealthed_undetected(t.status.stealthed, t.status.detected) {
                continue;
            }
            let rel = self.hunt_relationship(hunter_id, tid);
            let ok = hunt_enter_action_ok(
                mode,
                rel == Relationship::Enemies,
                rel == Relationship::Neutral,
                t.is_kind_of(KindOf::Vehicle),
                t.is_kind_of(KindOf::Structure),
                t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target,
                t.is_kind_of(KindOf::Drone),
                hijack_target_rejected(t),
                t.status.is_carbomb || t.weapon_set_carbomb,
            );
            if ok {
                return Some(tid);
            }
        }
        None
    }

    fn scan_command_button_hunt_special_target(
        &self,
        hunter_id: ObjectId,
        hunter_team: Team,
        hunter_pos: glam::Vec3,
        button: &str,
    ) -> Option<ObjectId> {
        use super::super::ATTACK_PRIORITY_DISTANCE_MODIFIER;
        use crate::game_logic::host_command_button_hunt::{
            COMMAND_BUTTON_HUNT_SCAN_RANGE, hunt_effective_priority,
            hunt_place_explosive_mine_view_range, hunt_same_map_status, hunt_special_capture_skips,
            hunt_special_is_place_explosive, hunt_stealthed_undetected,
        };
        use gamelogic::common::Relationship;

        let step = {
            let value = self.ai_definitions.data().attack_priority_distance_modifier;
            if value > 0.0 {
                value
            } else {
                ATTACK_PRIORITY_DISTANCE_MODIFIER
            }
        };
        let kind = classify_command_button_hunt_special(button);
        let hunter_off = self.hunt_pos_off_map(hunter_pos);
        let hunter_owner = self.objects.get(&hunter_id).and_then(|h| h.owner_player_id);
        let is_capture = matches!(kind, CommandButtonHuntSpecial::Capture);
        let is_place_explosive = hunt_special_is_place_explosive(button)
            || matches!(
                kind,
                CommandButtonHuntSpecial::Tnt | CommandButtonHuntSpecial::DemoCharge
            );
        let mut best: Option<(ObjectId, i32, i32)> = None;
        for (tid, t) in self.objects.registered() {
            if *tid == hunter_id || !t.is_alive() {
                continue;
            }
            if !hunt_same_map_status(hunter_off, self.hunt_pos_off_map(t.get_position())) {
                continue;
            }
            if hunt_stealthed_undetected(t.status.stealthed, t.status.detected) {
                continue;
            }
            let rel = self.hunt_relationship(hunter_id, *tid);
            if is_capture {
                let same_player = hunter_owner.is_some() && hunter_owner == t.owner_player_id;
                if hunt_special_capture_skips(same_player, rel == Relationship::Allies) {
                    continue;
                }
            } else if rel != Relationship::Enemies {
                // C++ scanClosestTarget default ALLOW_ENEMIES. Car-bomb (ALLOW_NEUTRAL)
                // is the enter scan, not this special-power path.
                continue;
            }
            let is_veh = t.is_kind_of(KindOf::Vehicle);
            let is_str = t.is_kind_of(KindOf::Structure);
            let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
            let ok = match kind {
                CommandButtonHuntSpecial::Capture => is_str,
                CommandButtonHuntSpecial::Snipe => {
                    is_veh && !is_air && !t.is_unmanned() && rel == Relationship::Enemies
                }
                CommandButtonHuntSpecial::Tnt | CommandButtonHuntSpecial::DemoCharge => {
                    is_str || (is_veh && !is_air)
                }
                CommandButtonHuntSpecial::HackVehicle => is_veh && !is_air && !t.is_disabled(),
                CommandButtonHuntSpecial::HackBuilding | CommandButtonHuntSpecial::StealCash => {
                    is_str
                }
                CommandButtonHuntSpecial::Booby => is_str,
                CommandButtonHuntSpecial::Unknown => is_str || is_veh,
            };
            if !ok {
                continue;
            }
            if is_place_explosive
                && self.hunt_owned_mine_near(
                    hunter_id,
                    hunter_owner,
                    t.get_position(),
                    hunt_place_explosive_mine_view_range(button),
                )
            {
                continue;
            }
            let d = hunt_dist_2d(hunter_pos, t.get_position());
            if d > COMMAND_BUTTON_HUNT_SCAN_RANGE {
                continue;
            }
            let raw = if let Some(info) = self.attack_priority_info_for(hunter_id) {
                self.attack_priority_for_target(info, t)
            } else {
                (COMMAND_BUTTON_HUNT_SCAN_RANGE - d) as i32
            };
            if raw == 0 {
                continue;
            }
            let eff = hunt_effective_priority(raw, d, step);
            let better = match best {
                None => true,
                Some((_, be, br)) => eff > be || (eff == be && raw > br),
            };
            if better {
                best = Some((*tid, eff, raw));
            }
        }
        best.map(|(id, _, _)| id)
    }

    fn hunt_pos_off_map(&self, pos: glam::Vec3) -> bool {
        crate::game_logic::host_deliver_payload::is_off_map_residual(
            pos,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        )
    }

    fn hunt_relationship(
        &self,
        hunter_id: ObjectId,
        target_id: ObjectId,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;
        let Some(hunter) = self.objects.get(&hunter_id) else {
            return Relationship::Neutral;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return Relationship::Neutral;
        };
        match (hunter.owner_player_id, target.owner_player_id) {
            (Some(a), Some(b)) => self.player_relationship(a, b),
            _ => {
                if hunter.team == target.team && hunter.team != Team::Neutral {
                    Relationship::Allies
                } else if hunter.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
        }
    }

    fn hunt_owned_mine_near(
        &self,
        hunter_id: ObjectId,
        hunter_owner: Option<u32>,
        target_pos: glam::Vec3,
        view_range: f32,
    ) -> bool {
        self.objects.registered().any(|(mid, m)| {
            if *mid == hunter_id || !m.is_alive() || !m.is_kind_of(KindOf::Mine) {
                return false;
            }
            let same_owner = match (hunter_owner, m.owner_player_id) {
                (Some(a), Some(b)) => a == b,
                _ => self
                    .objects
                    .get(&hunter_id)
                    .is_some_and(|h| h.team == m.team && h.team != Team::Neutral),
            };
            same_owner && hunt_dist_2d(m.get_position(), target_pos) <= view_range
        })
    }

    fn issue_command_button_hunt_special(
        &mut self,
        hunter_id: ObjectId,
        target_id: ObjectId,
        button: &str,
    ) -> bool {
        self.issue_command_button_hunt_special_with_effects(
            hunter_id,
            target_id,
            button,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn issue_command_button_hunt_special_with_effects(
        &mut self,
        hunter_id: ObjectId,
        target_id: ObjectId,
        button: &str,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let kind = classify_command_button_hunt_special(button);
        let Some(tp) = self.objects.get(&target_id).map(|t| t.get_position()) else {
            return false;
        };
        match kind {
            CommandButtonHuntSpecial::Capture => {
                if !self.unit_command_begin_capture(hunter_id, target_id) {
                    return false;
                }
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.target = Some(target_id);
                    u.set_ai_state(
                        AIState::Capturing,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
                let _ = self.assign_unit_path(hunter_id, tp, &[]);
                true
            }
            other => {
                let ability = match other {
                    CommandButtonHuntSpecial::Snipe => {
                        PendingSpecialAbility::SnipeVehicle { target_id }
                    }
                    CommandButtonHuntSpecial::Tnt => {
                        PendingSpecialAbility::PlantTimedDemoCharge { target_id }
                    }
                    CommandButtonHuntSpecial::DemoCharge => {
                        PendingSpecialAbility::PlantRemoteDemoCharge { target_id }
                    }
                    CommandButtonHuntSpecial::HackVehicle => {
                        PendingSpecialAbility::DisableVehicleHack { target_id }
                    }
                    CommandButtonHuntSpecial::HackBuilding => {
                        PendingSpecialAbility::HackerDisableBuilding { target_id }
                    }
                    CommandButtonHuntSpecial::StealCash => {
                        PendingSpecialAbility::StealCashHack { target_id }
                    }
                    CommandButtonHuntSpecial::Booby => {
                        PendingSpecialAbility::PlantBoobyTrap { target_id }
                    }
                    CommandButtonHuntSpecial::Capture | CommandButtonHuntSpecial::Unknown => {
                        PendingSpecialAbility::PlantTimedDemoCharge { target_id }
                    }
                };
                self.queue_pending_special_ability_with_effects(
                    hunter_id,
                    ability,
                    presentation_effects,
                );
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.target = Some(target_id);
                    u.set_ai_state(
                        AIState::SpecialAbility,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
                let _ = self.assign_unit_path(hunter_id, tp, &[]);
                true
            }
        }
    }

    /// C++ DeployStyleAIUpdate pack/unpack timer residual.
    pub fn tick_deploy_style_updates(&mut self) {
        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.deploy_style.is_some())
            .map(|(id, _)| *id)
            .collect();

        // Auto-acquisition retains a normal attack target during DEPLOY so it
        // can resume when ReadyToAttack.  If that victim disappears while the
        // unit is AI_BUSY, clear the pending attack here rather than allowing a
        // stale ObjectId to survive until some unrelated future fire pass.
        // Restrict this to actual attack states so capture/repair/etc. targets
        // remain outside DeployStyle's combat authority.
        let stale_pending_attacks: Vec<ObjectId> = ids
            .iter()
            .copied()
            .filter(|id| {
                let Some(obj) = self.objects.get(id) else {
                    return false;
                };
                if obj.ai_state() != AIState::Attacking
                    || !obj
                        .deploy_style
                        .as_ref()
                        .is_some_and(|deploy| deploy.is_busy())
                {
                    return false;
                }
                let Some(target_id) = obj.target else {
                    return false;
                };
                !self
                    .objects
                    .get(&target_id)
                    .is_some_and(|target| target.is_alive())
            })
            .collect();
        for id in stale_pending_attacks {
            self.stop_attack_decision_aware(id);
        }

        // Complete a timer before evaluating this frame's attack request,
        // matching DeployStyleAIUpdate::update's frame-boundary order.
        for &id in &ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(ds) = obj.deploy_style.as_mut() else {
                continue;
            };
            let (ready_atk, ready_mv) = ds.tick(frame);
            if ready_atk {
                obj.set_deployed(
                    true,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            if ready_mv {
                obj.set_deployed(
                    false,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
        }

        // C++ evaluates its pending attack independently of weapon reload:
        // an in-range current victim starts DEPLOY even when the shot itself
        // will not become ready until later.  This keeps player and mood
        // attacks approaching while distant, but starts the authored timer as
        // soon as their selected weapon is actually in range.
        //
        // C++ DeployStyleAIUpdate::update: `isInRange || isInGuardIdleState`.
        // Guard-idle units pre-deploy for fastest response (AIGuardIdleState).
        let in_range_pending_attacks: Vec<ObjectId> = ids
            .iter()
            .copied()
            .filter(|id| {
                let Some(obj) = self.objects.get(id) else {
                    return false;
                };
                if obj.get_template().deploy_style_metadata.is_none() {
                    return false;
                }
                let trying_to_move =
                    obj.waiting_for_path || !obj.movement.path.is_empty() || obj.status.moving;
                let is_in_guard_idle = matches!(
                    obj.ai_state(),
                    AIState::GuardingArea | AIState::GuardingObject
                ) && !trying_to_move;
                if is_in_guard_idle {
                    return true;
                }
                if !matches!(
                    obj.ai_state(),
                    AIState::Attacking | AIState::AttackingGround
                ) {
                    return false;
                }
                let Some(slot) = obj.selected_weapon_slot() else {
                    return false;
                };
                match (obj.target, obj.target_location) {
                    (Some(target_id), _) => self.objects.get(&target_id).is_some_and(|target| {
                        target.is_alive() && obj.is_within_attack_range_for_slot(slot, target)
                    }),
                    (None, Some(target_location)) => {
                        obj.is_within_attack_range_pos_for_slot(slot, target_location)
                    }
                    (None, None) => false,
                }
            })
            .collect();
        for id in in_range_pending_attacks {
            let started = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                let Some(deploy) = obj.deploy_style.as_mut() else {
                    continue;
                };
                if deploy.begin_deploy(frame) {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.set_status_moving(false);
                    true
                } else {
                    false
                }
            };
            if started {
                self.deploy_style_reg.record_deploy();
                self.queue_resolved_per_unit_sound(
                    id,
                    crate::game_logic::host_deploy_style::DEPLOY_STYLE_DEPLOY_AUDIO,
                    true,
                    false,
                    None,
                    150,
                );
            }
        }
        // C++ ALIGNING_TURRETS + leftover isTurretInNaturalPosition → UNDEPLOY.
        let aligning: Vec<ObjectId> = ids
            .iter()
            .copied()
            .filter(|id| {
                self.objects.get(id).is_some_and(|obj| {
                    obj.deploy_style
                        .as_ref()
                        .is_some_and(|ds| ds.is_aligning_turrets())
                })
            })
            .collect();
        for id in aligning {
            let started_pack = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                let natural = crate::game_logic::host_deploy_style::leftover_host_turret_is_in_natural_position(
                    obj.status.under_construction,
                    obj.turret_angle_deg,
                    obj.turret_pitch_deg,
                    obj.turret_natural_angle_deg,
                    obj.turret_natural_pitch_deg,
                );
                let Some(ds) = obj.deploy_style.as_mut() else {
                    continue;
                };
                if ds.finish_aligning_turrets(frame, natural) {
                    obj.set_deployed(
                        false,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.set_status_moving(false);
                    true
                } else {
                    false
                }
            };
            if started_pack {
                self.deploy_style_reg.record_undeploy();
                self.queue_resolved_per_unit_sound(
                    id,
                    crate::game_logic::host_deploy_style::DEPLOY_STYLE_UNDEPLOY_AUDIO,
                    true,
                    false,
                    None,
                    150,
                );
            }
        }
        // C++ setMyState stamps UNPACKING/PACKING/DEPLOYED on the drawable.
        for &id in &ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(state) = obj.deploy_style.as_ref().map(|d| d.state) else {
                continue;
            };
            crate::game_logic::host_deploy_style::leftover_stamp_deploy_style_conditions(
                &mut obj.model_condition_bits,
                state,
            );
        }
    }

    /// Ensure a source-authored DeployStyle unit is unpacking/unpacked before
    /// fire. Callers must establish a live, in-range attack target (or C++
    /// `isInGuardIdleState`) before invoking this; `DeployStyleAIUpdate::update`
    /// enters `DEPLOY` on in-range attack or guard idle.
    ///
    /// Returns false while the exact parsed `DeployStyleAIUpdate` module is
    /// packing or unpacking. A metadata/runtime mismatch is fail-closed rather
    /// than granting an unverified weapon bypass.
    pub fn ensure_deploy_style_ready_to_fire(&mut self, id: ObjectId) -> bool {
        let frame = self.frame;
        let mut started = false;
        let mut blocked = false;
        let ready = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return true;
            };
            // A unit without the parsed module is not a DeployStyle unit. Do
            // not infer this from template names or KindOf flags.
            if obj.get_template().deploy_style_metadata.is_none() {
                // A stale runtime block without source metadata is also an
                // invalid restore. It must not become an implicit name-based
                // deploy policy or a free fire permission.
                if obj.deploy_style.is_some() {
                    blocked = true;
                    obj.set_status_firing_weapon(false);
                    false
                } else {
                    true
                }
            } else {
                let ready_to_attack = obj.deploy_style.as_ref().map(|ds| ds.is_ready_to_attack());
                let ready = match ready_to_attack {
                    Some(true) => true,
                    Some(false) => {
                        let started_now = obj
                            .deploy_style
                            .as_mut()
                            .map(|ds| ds.begin_deploy(frame))
                            .unwrap_or(false);
                        let state = obj.deploy_style.as_ref().map(|ds| ds.state);
                        if started_now {
                            started = true;
                            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ));
                            obj.set_status_moving(false);
                            if let Some(state) = state {
                                crate::game_logic::host_deploy_style::leftover_stamp_deploy_style_conditions(
                                    &mut obj.model_condition_bits,
                                    state,
                                );
                            }
                        } else {
                            blocked = true;
                        }
                        false
                    }
                    None => {
                        // Object construction/save restore must install the live
                        // state from the metadata. Missing it may not let a
                        // deploy-only turret fire while packed.
                        blocked = true;
                        false
                    }
                };
                if !ready {
                    // Nested AttackStateMachine may have entered its fire
                    // state already. C++ DeployStyle marks it AI_BUSY, so no
                    // actual firing condition may remain set during the timer.
                    obj.set_status_firing_weapon(false);
                }
                ready
            }
        };
        if started {
            self.deploy_style_reg.record_deploy();
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_deploy_style::DEPLOY_STYLE_DEPLOY_AUDIO,
                true,
                false,
                None,
                150,
            );
        }
        if blocked {
            self.deploy_style_reg.record_blocked_fire();
        }
        ready
    }

    /// C++ AssaultTransportAIUpdate wounded-retrieve + healthy re-exit residual.
    pub fn tick_assault_transport_updates(&mut self) -> gamelogic::GameLogicResult<()> {
        self.tick_assault_transport_updates_with_effects(&mut Default::default())
    }
    pub(crate) fn tick_assault_transport_updates_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_troop_crawler::{
            is_assault_member_healthy, is_assault_member_wounded,
        };

        let crawler_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                o.is_troop_crawler_style_container()
                    && o.assault_transport
                        .as_ref()
                        .map(|a| a.active)
                        .unwrap_or(false)
            })
            .map(|(id, _)| *id)
            .collect();

        for crawler_id in crawler_ids {
            // C++ update: isEffectivelyDead → giveFinalOrders + sleep forever.
            let crawler_dead = self
                .objects
                .get(&crawler_id)
                .map(|c| !c.is_alive())
                .unwrap_or(true);
            if crawler_dead {
                self.assault_transport_give_final_orders(crawler_id);
                continue;
            }

            self.assault_transport_prune_members(crawler_id);
            self.assault_transport_add_new_members(crawler_id);

            if self.assault_transport_is_attack_pointless(crawler_id) {
                // C++ aiIdle(CMD_FROM_AI) — idle the transport only.
                self.set_ai_state_decision_aware_with_effects(crawler_id, AIState::Idle, effects)?;
                if let Some(c) = self.objects.get_mut(&crawler_id) {
                    c.set_status_attacking(false);
                }
                continue;
            }

            let (target_raw, members, member_new, is_attack_move, is_attack_object, goal) = {
                let Some(c) = self.objects.get(&crawler_id) else {
                    continue;
                };
                let Some(a) = c.assault_transport.as_ref() else {
                    continue;
                };
                (
                    a.designated_target,
                    a.member_ids.clone(),
                    a.member_new.clone(),
                    a.is_attack_move,
                    a.is_attack_object,
                    a.attack_move_goal_pos,
                )
            };

            let target_id = target_raw.and_then(|raw| {
                let id = ObjectId(raw);
                self.objects.get(&id).filter(|t| t.is_alive()).map(|_| id)
            });

            if let Some(target_id) = target_id {
                for (idx, mid_raw) in members.iter().copied().enumerate() {
                    let mid = ObjectId(mid_raw);
                    let Some(member) = self.objects.get(&mid) else {
                        continue;
                    };
                    if !member.is_alive() {
                        continue;
                    }
                    let contained = member.contained_by == Some(crawler_id);
                    let wounded =
                        is_assault_member_wounded(member.health.current, member.health.maximum);
                    let healthy =
                        is_assault_member_healthy(member.health.current, member.health.maximum);
                    let is_new = member_new.get(idx).copied().unwrap_or(false);
                    let already_enter = matches!(member.ai_state(), AIState::Entering);

                    if contained {
                        // C++: healthy contained eject only when !m_newMember.
                        // aiExit → OpenContain ExitStart/End walk, not a 6wu pop.
                        if healthy && !is_new {
                            self.assault_transport_ai_exit_with_effects(mid, crawler_id, effects);
                            self.troop_crawler.record_healthy_redeploy();
                        }
                        continue;
                    }

                    if wounded {
                        // C++: wounded uncontained members aiEnter unless already AI_ENTER.
                        if !already_enter {
                            self.assault_transport_ai_enter(mid, crawler_id);
                            self.troop_crawler.record_wounded_retrieve();
                        }
                        continue;
                    }

                    if let Some(unit) = self.objects.get(&mid) {
                        if unit.target != Some(target_id) {
                            let _ = self.apply_engagement_decision_aware(mid, target_id);
                        }
                    }
                }
            } else if is_attack_move {
                // C++: target gone + attack-move → continue aiAttackMoveToPosition.
                let already = self
                    .objects
                    .get(&crawler_id)
                    .map(|c| matches!(c.ai_state(), AIState::AttackMoving))
                    .unwrap_or(false);
                if !already {
                    let dest = Vec3::new(goal[0], goal[1], goal[2]);
                    if let Some(c) = self.objects.get_mut(&crawler_id) {
                        let landed_chinook = c.chinook_ai.as_ref().is_some_and(|ai| {
                            ai.flight_status
                                == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                        });
                        c.is_final_goal = !c.is_parachuting()
                            && !landed_chinook
                            && !(c.chinook_ai.is_some() && c.allow_invalid_position)
                            && c.adjust_destinations();
                        c.num_frames_blocked = 0;
                        c.is_blocked_and_stuck = false;
                        c.set_status_moving(true);
                    }
                    let _ = self.assign_unit_path(crawler_id, dest, &[]);
                    if let Some(c) = self.objects.get_mut(&crawler_id) {
                        c.set_auto_acquire_when_idle(true);
                        c.requested_destination = Some(dest);
                        c.set_ai_state(
                            AIState::AttackMoving,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                    }
                }
            } else if is_attack_object {
                // C++: target gone + attack-object → retrieveMembers.
                self.assault_transport_retrieve_members(crawler_id);
            }
        }
        Ok(())
    }

    /// C++ `aiDoCommand` Attack — reset new-member flags so a fresh order can eject.
    pub fn assault_transport_on_player_attack(&mut self, id: ObjectId) {
        let Some(c) = self.objects.get_mut(&id) else {
            return;
        };
        if !c.is_troop_crawler_style_container() {
            return;
        }
        if let Some(a) = c.assault_transport.as_mut() {
            a.on_player_attack();
        }
    }

    /// C++ `aiDoCommand` AttackMove.
    pub fn assault_transport_on_player_attack_move(&mut self, id: ObjectId, dest: Vec3) {
        let Some(c) = self.objects.get_mut(&id) else {
            return;
        };
        if !c.is_troop_crawler_style_container() {
            return;
        }
        match c.assault_transport.as_mut() {
            Some(a) => a.on_player_attack_move([dest.x, dest.y, dest.z]),
            None => {
                let mut a =
                    crate::game_logic::host_troop_crawler::HostAssaultTransportState::default();
                a.on_player_attack_move([dest.x, dest.y, dest.z]);
                c.assault_transport = Some(a);
            }
        }
    }

    /// C++ `aiDoCommand` IDLE: retrieveMembers then reset.
    pub fn assault_transport_on_player_idle(&mut self, id: ObjectId) {
        let active = self
            .objects
            .get(&id)
            .and_then(|c| c.assault_transport.as_ref())
            .map(|a| a.active)
            .unwrap_or(false);
        if !active {
            return;
        }
        if !self
            .objects
            .get(&id)
            .map(|c| c.is_troop_crawler_style_container())
            .unwrap_or(false)
        {
            return;
        }
        self.assault_transport_retrieve_members(id);
        if let Some(c) = self.objects.get_mut(&id) {
            if let Some(a) = c.assault_transport.as_mut() {
                a.clear();
            }
        }
    }

    /// C++ giveFinalOrders: transfer the original order as CMD_FROM_PLAYER.
    pub fn assault_transport_give_final_orders(&mut self, crawler_id: ObjectId) {
        use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER;

        let snapshot = {
            let Some(c) = self.objects.get(&crawler_id) else {
                return;
            };
            let Some(a) = c.assault_transport.as_ref() else {
                return;
            };
            if a.final_orders_given || !a.active {
                return;
            }
            (
                a.member_ids.clone(),
                a.is_attack_object,
                a.is_attack_move,
                a.designated_target,
                a.attack_move_goal_pos,
            )
        };
        if let Some(c) = self.objects.get_mut(&crawler_id) {
            if let Some(a) = c.assault_transport.as_mut() {
                a.final_orders_given = true;
                a.active = false;
            }
        }

        let (members, is_attack_object, is_attack_move, target_raw, goal) = snapshot;
        let target_id = target_raw.and_then(|raw| {
            let id = ObjectId(raw);
            self.objects.get(&id).filter(|t| t.is_alive()).map(|_| id)
        });

        for mid_raw in members {
            let mid = ObjectId(mid_raw);
            let Some(member) = self.objects.get(&mid) else {
                continue;
            };
            if !member.is_alive() {
                continue;
            }
            if is_attack_object {
                if let Some(target_id) = target_id {
                    let engaged = self.apply_engagement_decision_aware(mid, target_id);
                    if let Some(unit) = self.objects.get_mut(&mid) {
                        unit.last_command_source = HUNT_CMD_FROM_PLAYER;
                        if !engaged {
                            unit.set_target_with_operation(
                                Some(target_id),
                                &mut self.attack_events,
                                &mut crate::game_logic::ObjectAudio::queue(
                                    &mut self.queued_audio_events,
                                ),
                                self.frame,
                                &mut crate::game_logic::ScriptPresentationEffects::default()
                                    .reborrow_for_object(&mut self.partition_manager),
                            );
                        }
                    }
                }
            } else if is_attack_move {
                let dest = Vec3::new(goal[0], goal[1], goal[2]);
                if let Some(unit) = self.objects.get_mut(&mid) {
                    let landed_chinook = unit.chinook_ai.as_ref().is_some_and(|ai| {
                        ai.flight_status
                            == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                    });
                    unit.is_final_goal = !unit.is_parachuting()
                        && !landed_chinook
                        && !(unit.chinook_ai.is_some() && unit.allow_invalid_position)
                        && unit.adjust_destinations();
                    unit.num_frames_blocked = 0;
                    unit.is_blocked_and_stuck = false;
                    unit.set_status_moving(true);
                }
                let _ = self.assign_unit_path(mid, dest, &[]);
                if let Some(unit) = self.objects.get_mut(&mid) {
                    unit.set_auto_acquire_when_idle(true);
                    unit.requested_destination = Some(dest);
                    unit.set_ai_state(
                        AIState::AttackMoving,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    unit.last_command_source = HUNT_CMD_FROM_PLAYER;
                }
            } else if let Some(unit) = self.objects.get_mut(&mid) {
                unit.last_command_source = HUNT_CMD_FROM_PLAYER;
            }
        }
    }

    /// C++ retrieveMembers: outside members `aiEnter` the crawler.
    pub fn assault_transport_retrieve_members(&mut self, crawler_id: ObjectId) {
        let members = {
            let Some(c) = self.objects.get(&crawler_id) else {
                return;
            };
            let Some(a) = c.assault_transport.as_ref() else {
                return;
            };
            a.member_ids.clone()
        };
        for mid_raw in members {
            let mid = ObjectId(mid_raw);
            let Some(member) = self.objects.get(&mid) else {
                continue;
            };
            if !member.is_alive() {
                continue;
            }
            if member.contained_by == Some(crawler_id) {
                continue;
            }
            // C++: skip if already AI_ENTER. Never teleport-board.
            if matches!(member.ai_state(), AIState::Entering) {
                continue;
            }
            self.assault_transport_ai_enter(mid, crawler_id);
        }
    }

    /// C++ `ai->aiExit(transport, CMD_FROM_AI)` — ExitStart/End walk.
    fn assault_transport_ai_exit_with_effects(
        &mut self,
        member_id: ObjectId,
        crawler_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;

        if let Some(c) = self.objects.get_mut(&crawler_id) {
            c.remove_occupant_with_partition_owner(
                member_id,
                &mut self.ownership_events,
                &mut self.partition_manager,
            );
        }
        let _ =
            self.unit_command_exit_via_open_contain_with_effects(member_id, crawler_id, effects);
        if let Some(unit) = self.objects.get_mut(&member_id) {
            unit.last_command_source = HUNT_CMD_FROM_AI;
        }
    }

    /// C++ `ai->aiEnter(transport, CMD_FROM_AI)` — walk to hull, board on arrival.
    fn assault_transport_ai_enter(&mut self, member_id: ObjectId, crawler_id: ObjectId) {
        use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;

        let crawler_pos = self.objects.get(&crawler_id).map(|c| c.get_position());
        if let Some(unit) = self.objects.get_mut(&member_id) {
            unit.set_order_target_with_operation(
                Some(crawler_id),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.ignored_obstacle_id = Some(crawler_id);
            unit.last_command_source = HUNT_CMD_FROM_AI;
        }
        if let Some(pos) = crawler_pos {
            self.path_approach_with_state_ignoring(
                member_id,
                pos,
                AIState::Entering,
                Some(crawler_id),
            );
        } else if let Some(unit) = self.objects.get_mut(&member_id) {
            unit.set_ai_state(
                AIState::Entering,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.ignored_obstacle_id = Some(crawler_id);
        }
    }

    fn assault_transport_prune_members(&mut self, crawler_id: ObjectId) {
        use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;

        let members = {
            let Some(c) = self.objects.get(&crawler_id) else {
                return;
            };
            let Some(a) = c.assault_transport.as_ref() else {
                return;
            };
            a.member_ids.clone()
        };
        let mut drop: Vec<usize> = Vec::new();
        for (i, mid_raw) in members.iter().copied().enumerate() {
            let mid = ObjectId(mid_raw);
            let Some(member) = self.objects.get(&mid) else {
                drop.push(i);
                continue;
            };
            if !member.is_alive() || member.last_command_source != HUNT_CMD_FROM_AI {
                drop.push(i);
            }
        }
        if drop.is_empty() {
            return;
        }
        if let Some(c) = self.objects.get_mut(&crawler_id) {
            if let Some(a) = c.assault_transport.as_mut() {
                for i in drop.into_iter().rev() {
                    a.remove_member_at(i);
                }
            }
        }
    }

    fn assault_transport_add_new_members(&mut self, crawler_id: ObjectId) {
        use crate::game_logic::host_troop_crawler::is_assault_member_wounded;

        let occupants = {
            let Some(c) = self.objects.get(&crawler_id) else {
                return;
            };
            c.occupants.clone()
        };
        for occ in occupants {
            let wounded = self
                .objects
                .get(&occ)
                .map(|u| is_assault_member_wounded(u.health.current, u.health.maximum))
                .unwrap_or(false);
            if let Some(c) = self.objects.get_mut(&crawler_id) {
                if let Some(a) = c.assault_transport.as_mut() {
                    let _ = a.try_add_member(occ.0, wounded);
                }
            }
        }
        if let Some(c) = self.objects.get_mut(&crawler_id) {
            if let Some(a) = c.assault_transport.as_mut() {
                a.new_occupants_are_new_members = true;
            }
        }
    }

    fn assault_transport_is_attack_pointless(&self, crawler_id: ObjectId) -> bool {
        let Some(c) = self.objects.get(&crawler_id) else {
            return false;
        };
        if !matches!(c.ai_state(), AIState::Attacking) && !c.status.attacking {
            return false;
        }
        let Some(a) = c.assault_transport.as_ref() else {
            return false;
        };
        if a.member_ids.is_empty() {
            return false;
        }
        // C++: attacking + every member is new → idle.
        (0..a.member_ids.len()).all(|i| a.is_new_member(i))
    }

    /// C++ UndeadBody + BattleBusSlowDeathBehavior first-life / empty-hulk residual.
    pub fn tick_battle_bus_slow_deaths(&mut self) {
        self.tick_battle_bus_slow_deaths_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn tick_battle_bus_slow_deaths_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();

        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_battle_bus::battle_bus_undeath_passenger_damage;

        let frame = self.frame;

        // Snapshot bus state without overlapping borrows.
        let mut bus_snapshots: Vec<(ObjectId, bool, Vec<ObjectId>, usize, f32)> = Vec::new();
        for (id, o) in self.objects.registered() {
            if !o.is_battle_bus_transport {
                continue;
            }
            let Some(body) = o.battle_bus_body.as_ref() else {
                continue;
            };
            bus_snapshots.push((
                *id,
                body.pending_passenger_damage,
                o.occupants.clone(),
                o.occupants.len(),
                o.get_position().z,
            ));
        }

        let mut passenger_hits: Vec<(ObjectId, f32)> = Vec::new();
        for (bus_id, pending, occupants, _count, _z) in &bus_snapshots {
            if !*pending {
                continue;
            }
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            if let Some(bus) = self.objects.get_mut(bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.pending_passenger_damage = false;
                }
            }
            for pid in occupants {
                if let Some(p) = self.objects.get(pid) {
                    let dmg = battle_bus_undeath_passenger_damage(p.health.maximum.max(1.0));
                    passenger_hits.push((*pid, dmg));
                }
            }
            self.battle_bus.record_undeath_detonate();
        }

        for (pid, dmg) in passenger_hits {
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            if let Some(p) = self.objects.get_mut(&pid) {
                if p.is_alive() {
                    let _ = {
                        let owned_victim_id = p.id;
                        let owned_damage_type = DamageType::Explosive;
                        self.apply_residual_damage_owned_with_effects(
                            owned_victim_id,
                            dmg,
                            None,
                            owned_damage_type,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                owned_damage_type,
                            ),
                            None,
                            presentation_effects,
                        )
                    };
                }
            }
        }

        let mut empty_kills: Vec<ObjectId> = Vec::new();
        #[cfg(feature = "game_client")]
        let fx_player = self.local_player_id();
        for (bus_id, _pending, _occ, passenger_count, z) in &bus_snapshots {
            let above = *z > 0.5;
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            let Some(bus) = self.objects.get_mut(bus_id) else {
                continue;
            };
            bus.advance_battle_bus_throw_with_effects(
                frame,
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
            );
            #[cfg(feature = "game_client")]
            let (_landed, empty_kill) = Self::with_object_fx_services(
                &mut self.session_view,
                self.queued_audio_events.policy().clone(),
                fx_player,
                self.world_services.shroud(),
                presentation_effects,
                &fx_operation_environment,
                self.world_services.terrain(),
                &self.logic_random,
                |view| {
                    bus.finish_battle_bus_slow_death_with_fx(
                        frame,
                        *passenger_count,
                        &mut |name, object, at_object| {
                            Self::dispatch_object_phase_fx_with_view(
                                name,
                                object,
                                at_object,
                                fx_player,
                                self.world_services.shroud(),
                                view,
                            )
                        },
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    )
                },
            );
            #[cfg(not(feature = "game_client"))]
            let (_landed, empty_kill) = bus.finish_battle_bus_slow_death_with_fx(
                frame,
                *passenger_count,
                &mut |_name, _object, _at_object| {},
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            if empty_kill {
                empty_kills.push(*bus_id);
            }
        }

        for bus_id in empty_kills {
            self.battle_bus.record_empty_hulk_destruction();
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            if let Some(bus) = self.objects.get_mut(&bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.mark_real_death();
                }
            }
            let _ = self.apply_owned_kill_with_effects(
                bus_id,
                DamageType::Penalty,
                crate::game_logic::host_usa_pilot::HostDeathType::Extra4,
                presentation_effects,
            );
        }
    }

    /// C++ `Thing::isSignificantlyAboveTerrain`: height-above-terrain, not world Y.
    /// Host Y is up; sample terrain (XZ ground) and fall back to the object's ground cache.
    fn vehicle_is_significantly_above_terrain(&self, vehicle: &crate::game_logic::Object) -> bool {
        let pos = vehicle.get_position();
        let terrain_y = self.terrain_height_at(pos).unwrap_or(vehicle.ground_height);
        let height_above_terrain = pos.y - terrain_y;
        height_above_terrain > -(3.0 * 3.0) * crate::game_logic::Object::SHOCK_GRAVITY
    }

    /// Source HijackerUpdate, driven by this exact receiving world.
    pub fn tick_hijacker_updates(&mut self) {
        self.tick_hijacker_updates_with_effects(&mut Default::default())
            .expect("receiving HijackerUpdate failed");
    }
    pub(crate) fn tick_hijacker_updates_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let riders: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, rider)| rider.hijacker_update_active)
            .map(|(id, rider)| {
                (
                    *id,
                    std::sync::Arc::downgrade(&rider.visual_draw_identity_token),
                )
            })
            .collect();
        for (rider_id, rider_birth) in riders {
            let same_rider = |object: &Object| {
                rider_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            };
            let Some(rider) = self
                .objects
                .get(&rider_id)
                .filter(|object| same_rider(object))
            else {
                continue;
            };
            if !rider.hijacker_in_vehicle {
                self.objects
                    .get_mut(&rider_id)
                    .unwrap()
                    .hijacker_was_airborne = false;
                continue;
            }
            let vehicle_id = rider.hijack_vehicle_id;
            // C++ deliberately tests target existence, not effectively-dead.
            // Keep following a retained wreck until the registry removes it.
            let vehicle = vehicle_id.and_then(|id| self.objects.get(&id));
            let Some(vehicle) = vehicle else {
                let eject_pos = rider.hijacker_eject_pos.unwrap_or(rider.get_position());
                let was_airborne = rider.hijacker_was_airborne;
                let has_ai = rider.has_ai_update_interface();
                {
                    let rider = self.objects.get_mut(&rider_id).unwrap();
                    self.partition_manager.attach_receiving_object(rider);
                    let pos = rider.get_position();
                    let footprint = super::collide_dispatch::host_object_footprint(rider);
                    self.partition_manager
                        .register_object_geometry(rider_id.0, pos.x, pos.z, footprint);
                    rider.set_drawable_hidden(false);
                    rider.set_status_no_collisions(false);
                    rider.set_status_masked(false);
                    rider.set_status_unselectable(false);
                }
                // No position restore exists in the original eject branch.
                if has_ai {
                    self.enter_base_ai_idle_owned(rider_id, effects)?;
                }
                if !self
                    .objects
                    .get(&rider_id)
                    .is_some_and(|rider| same_rider(rider))
                {
                    continue;
                }
                if was_airborne {
                    self.put_hijacker_in_airborne_parachute_with_effects(
                        rider_id, eject_pos, effects,
                    )?;
                }
                if let Some(rider) = self
                    .objects
                    .get_mut(&rider_id)
                    .filter(|rider| same_rider(rider))
                {
                    // Source module completion follows the optional containment.
                    rider.hijack_vehicle_id = None;
                    rider.hijacker_in_vehicle = false;
                    rider.hijacker_update_active = false;
                    rider.hijacker_was_airborne = false;
                }
                continue;
            };
            let vehicle_id = vehicle_id.expect("existing target has ID");
            let vehicle_birth = std::sync::Arc::downgrade(&vehicle.visual_draw_identity_token);
            let position = vehicle.get_position();
            if !self.set_owned_object_position_with_effects(rider_id, position, self.frame, effects)
            {
                continue;
            }
            // Position callback has completed. The source rereads these facts
            // from its original receiving target, not a pre-callback snapshot.
            let same_vehicle = |object: &Object| {
                vehicle_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            };
            let Some(vehicle) = self
                .objects
                .get(&vehicle_id)
                .filter(|vehicle| same_vehicle(vehicle))
            else {
                continue;
            };
            let airborne = self.vehicle_is_significantly_above_terrain(vehicle);
            let eject_pos = vehicle.get_position();
            let vehicle_level = vehicle.experience.level;
            let Some(rider) = self
                .objects
                .get_mut(&rider_id)
                .filter(|rider| same_rider(rider))
            else {
                continue;
            };
            rider.hijacker_was_airborne = airborne;
            rider.hijacker_eject_pos = Some(eject_pos);
            let highest = if hijacker_veterancy_rank(vehicle_level)
                > hijacker_veterancy_rank(rider.experience.level)
            {
                vehicle_level
            } else {
                rider.experience.level
            };
            // Original jacker setter precedes target setter, both with feedback.
            let health_bonus = self.veterancy_health_bonus;
            self.with_object_veterancy_feedback_and_locomotors(
                rider_id,
                |rider, audio, feedback, locomotors| {
                    rider.set_exact_veterancy_level_with_health_bonus_table(
                        highest,
                        &health_bonus,
                        audio,
                        feedback,
                        locomotors,
                    );
                },
            );
            if self
                .objects
                .get(&vehicle_id)
                .is_some_and(|vehicle| same_vehicle(vehicle))
            {
                let health_bonus = self.veterancy_health_bonus;
                self.with_object_veterancy_feedback_and_locomotors(
                    vehicle_id,
                    |vehicle, audio, feedback, locomotors| {
                        vehicle.set_exact_veterancy_level_with_health_bonus_table(
                            highest,
                            &health_bonus,
                            audio,
                            feedback,
                            locomotors,
                        );
                    },
                );
            }
        }
        Ok(())
    }
    pub fn transfer_script_object_name(&mut self, from_id: ObjectId, to_id: ObjectId) -> bool {
        let Some(name) = self.objects.get(&from_id).map(|object| object.name.clone()) else {
            return false;
        };
        self.transfer_receiving_script_name(&name, to_id)
    }
    pub fn execute_shroud_crate_behavior(&mut self, picker_id: ObjectId) -> bool {
        // C++ ShroudCrateCollide: other->getControllingPlayer()->getPlayerIndex().
        // getControllingPlayer() always exists for a live game object; the host
        // resolves it via owner id or the faction's unique active player.
        let Some(player_id) = self
            .objects
            .get(&picker_id)
            .filter(|p| p.is_alive())
            .and_then(|p| self.player_owner_for_host_object(p))
        else {
            return false;
        };
        self.partition_manager.reveal_map_for_player(
            &mut self
                .world_services
                .shroud()
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            player_id,
        );
        true
    }
    pub fn execute_heal_crate_behavior(&mut self, picker_id: ObjectId) -> usize {
        // C++ HealCrateCollide: other->getControllingPlayer()->healAllObjects().
        let Some(picker_player) = self
            .objects
            .get(&picker_id)
            .filter(|p| p.is_alive())
            .and_then(|p| self.player_owner_for_host_object(p))
        else {
            return 0;
        };
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.destroyed
                    && self.player_owner_for_host_object(o) == Some(picker_player)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if let Some(o) = self.objects.get_mut(&id) {
                let max = o.health.maximum;
                if o.health.current < max {
                    Self::write_object_health_authority_aware(o, max, &mut self.health_events);
                    n += 1;
                }
            }
        }
        n
    }

    /// C++ UnitCrateCollide::executeCrateBehavior residual.
    ///
    /// Missing ThingTemplate → FALSE. Spawn on picker player default team
    /// via `findPositionAround` maxRadius 20.
    pub fn execute_unit_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        unit_type: &str,
        count: u32,
    ) -> usize {
        self.execute_unit_crate_behavior_with_effects(
            picker_id,
            unit_type,
            count,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_unit_crate_behavior_with_effects(
        &mut self,
        picker_id: ObjectId,
        unit_type: &str,
        count: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> usize {
        let (team, origin, ori) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position(), p.get_orientation()),
            _ => return 0,
        };
        if unit_type.is_empty() || count == 0 {
            return 0;
        }
        // C++ UnitCrateCollide: TheThingFactory->findTemplate NULL → FALSE.
        // The host spawn-template catalog (templates + asset definitions) is
        // the documented ThingFactory equivalent.
        if !self.ensure_host_spawn_template(unit_type) {
            return 0;
        }
        let occupied: Vec<(glam::Vec3, f32)> = self
            .objects
            .registered_values()
            .filter(|o| o.is_alive() && !o.status.destroyed)
            .map(|o| (o.get_position(), o.thing().geometry.radius.max(1.0)))
            .collect();
        let mut spawned = 0usize;
        for i in 0..count {
            let pos = crate::game_logic::host_supply_gather::find_position_around_xz(
                origin,
                &occupied,
                i,
                picker_id.0,
            );
            if let Some(nid) = self.create_object_with_script_effects(
                unit_type,
                team,
                pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&nid) {
                    o.set_orientation_with_partition_owner(ori, &mut self.partition_manager);
                }
                spawned += 1;
            }
        }
        spawned
    }

    /// C++ `VeterancyCrateCollide::executeCrateBehavior`: crate AIUpdate
    /// `getGoalObject() == picker`. No AI / no matching goal → inert.
    pub fn veterancy_crate_ai_goal_matches(&self, crate_id: ObjectId, picker_id: ObjectId) -> bool {
        let Some(crate_obj) = self.objects.get(&crate_id) else {
            return false;
        };
        crate_obj.target == Some(picker_id)
    }

    pub fn execute_veterancy_crate_behavior(
        &mut self,
        crate_id: ObjectId,
        picker_id: ObjectId,
        effect_range: f32,
        levels: u8,
    ) -> usize {
        if !self.veterancy_crate_ai_goal_matches(crate_id, picker_id) {
            return 0;
        }

        let (team, origin, picker_player) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position(), p.owner_player_id),
            _ => return 0,
        };
        // C++ getLevelsToGain: Regular + AddsOwnerVeterancy is 0 → crate invalid.
        if levels == 0 {
            return 0;
        }
        let range_sq = effect_range.max(0.0) * effect_range.max(0.0);
        let targets: Vec<ObjectId> = if effect_range <= 0.0 {
            vec![picker_id]
        } else {
            self.objects
                .registered()
                .filter(|(_, o)| {
                    o.team == team
                        && o.is_alive()
                        && !o.status.destroyed
                        && !o.status.under_construction
                        && !o.is_kind_of(KindOf::Structure)
                        && o.is_trainable()
                        && !matches!(
                            o.experience.level,
                            crate::game_logic::VeterancyLevel::Heroic
                        )
                        && {
                            let p = o.get_position();
                            let dx = p.x - origin.x;
                            let dz = p.z - origin.z;
                            dx * dx + dz * dz <= range_sq
                        }
                        && picker_player
                            .map(|pid| o.owner_player_id == Some(pid))
                            .unwrap_or(true)
                })
                .map(|(id, _)| *id)
                .collect()
        };
        let mut n = 0usize;
        for tid in targets {
            let gained = self
                .with_object_veterancy_feedback_and_locomotors(
                    tid,
                    |object, audio, feedback, locomotors| {
                        if object.is_trainable() {
                            object.gain_exp_for_level_in(levels, true, audio, feedback, locomotors)
                        } else {
                            0
                        }
                    },
                )
                .unwrap_or(0);
            if gained > 0 {
                n += 1;
            }
        }
        n
    }
    pub fn execute_salvage_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        money_provided: u32,
        seed: u32,
    ) -> (&'static str, u32) {
        use crate::game_logic::VeterancyLevel;
        use crate::game_logic::host_gamedata_lobby_residual::{
            SALVAGE_LEVEL_CHANCE_RESIDUAL, SALVAGE_WEAPON_CHANCE_RESIDUAL,
        };
        use crate::game_logic::host_rng_residual::pure_logic_random_real;

        let Some(picker) = self.objects.get_mut(&picker_id) else {
            return ("none", 0);
        };
        // Armor salvager path (no percent).
        if picker.is_kind_of(KindOf::ArmorSalvager) && picker.armor_crate_upgrade < 2 {
            picker.apply_salvage_armor_upgrade();
            return ("armor", 0);
        }
        // Weapon salvager path.
        if picker.is_kind_of(KindOf::WeaponSalvager) && picker.weapon_crate_upgrade < 2 {
            let roll = pure_logic_random_real(seed, 1, 0.0, 1.0);
            if SALVAGE_WEAPON_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_WEAPON_CHANCE_RESIDUAL
            {
                picker.apply_salvage_weapon_upgrade();
                return ("weapon", 0);
            }
        }
        // C++ SalvageCrateCollide::eligibleForLevel: not HEROIC and isTrainable.
        // Untrainable pickers fall through to doMoney instead of burning the crate.
        let can_level =
            picker.is_trainable() && !matches!(picker.experience.level, VeterancyLevel::Heroic);
        if can_level {
            let roll = pure_logic_random_real(seed, 2, 0.0, 1.0);
            if SALVAGE_LEVEL_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_LEVEL_CHANCE_RESIDUAL
            {
                self.with_object_veterancy_feedback_and_locomotors(
                    picker_id,
                    |picker, audio, feedback, locomotors| {
                        picker.apply_salvage_level_gain_in(audio, feedback, locomotors)
                    },
                );
                return ("level", 0);
            }
        }
        // Money fallback.
        let money = if money_provided > 0 {
            money_provided
        } else {
            crate::game_logic::host_create_crate_die::salvage_money_roll(seed, 3)
        };
        ("money", money)
    }
    pub fn try_create_crates_on_die(
        &mut self,
        victim_id: ObjectId,
        victim_pos: glam::Vec3,
        victim_team: Team,
        crate_data: &[String],
        killer_id: Option<ObjectId>,
    ) -> usize {
        self.try_create_crates_on_die_with_effects(
            victim_id,
            victim_pos,
            victim_team,
            crate_data,
            killer_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn try_create_crates_on_die_with_effects(
        &mut self,
        victim_id: ObjectId,
        victim_pos: glam::Vec3,
        victim_team: Team,
        crate_data: &[String],
        killer_id: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> usize {
        if crate_data.is_empty() {
            return 0;
        }
        // Ally kill → no crate (C++ CreateCrateDie::onDie getRelationship==ALLIES).
        // Resolve controlling players (unique-team fallback) — not Team faction equality.
        if let Some(kid) = killer_id {
            let killer_snap = self
                .objects
                .get(&kid)
                .map(|k| (k.owner_player_id, k.team, k.team_instance_name.clone()));
            let victim_snap = self
                .objects
                .get(&victim_id)
                .map(|v| (v.owner_player_id, v.team_instance_name.clone()));
            if let (Some((k_own, k_team, k_inst)), Some((v_own, v_inst))) =
                (killer_snap, victim_snap)
            {
                let k_owner = self.player_owner_for_event(k_own, k_team);
                let v_owner = self.player_owner_for_event(v_own, victim_team);
                if matches!(
                    Self::object_relationship_from_owners(
                        &self.team_factory,
                        &self.players,
                        k_owner,
                        &k_inst,
                        v_owner,
                        &v_inst,
                    ),
                    gamelogic::common::Relationship::Allies,
                ) {
                    return 0;
                }
            }
        }
        let victim_vet = self
            .objects
            .get(&victim_id)
            .map(|v| format!("{:?}", v.experience.level));
        let killer_kind_names: Vec<String> = killer_id
            .and_then(|kid| self.objects.get(&kid))
            .map(|k| {
                k.thing()
                    .template
                    .kind_of
                    .iter()
                    .map(|ko| format!("{ko:?}"))
                    .collect()
            })
            .unwrap_or_default();
        let killer_kind_refs: Vec<&str> = killer_kind_names.iter().map(|s| s.as_str()).collect();
        let killer_sciences: Vec<String> = killer_id
            .and_then(|kid| self.objects.get(&kid).and_then(|k| k.owner_player_id))
            .and_then(|pid| {
                self.players
                    .get(&pid)
                    .map(|p| p.unlocked_sciences.iter().cloned().collect())
            })
            .unwrap_or_default();
        let victim_owner = self.objects.get(&victim_id).and_then(|v| v.owner_player_id);
        let seed = crate::game_logic::host_create_crate_die::crate_die_seed(
            victim_id, killer_id, self.frame,
        );
        let mut spawned = 0usize;
        for (i, name) in crate_data.iter().enumerate() {
            let draw = (i as u32).wrapping_mul(7);
            let gates = crate::game_logic::host_create_crate_die::CrateDieGates {
                victim_veterancy: victim_vet.as_deref(),
                killer_kindof_names: &killer_kind_refs,
                killer_sciences: &killer_sciences,
            };
            let Some(req) = crate::game_logic::host_create_crate_die::try_roll_crate_spawn_gated(
                name,
                seed,
                draw,
                Some(&gates),
            ) else {
                continue;
            };

            // Spawn offset residual (fail-closed vs findPositionAround).
            let ang = (seed.wrapping_add(i as u32) as f32) * 0.7;
            let pos = glam::Vec3::new(
                victim_pos.x + ang.cos() * 5.0,
                victim_pos.y,
                victim_pos.z + ang.sin() * 5.0,
            );
            // Ensure template exists.
            if !self.templates.contains_key(&req.object_name) {
                let mut t = ThingTemplate::new(&req.object_name);
                t.add_kind_of(crate::game_logic::KindOf::Crate);
                // Crates are non-combat pickups.
                self.templates.insert(req.object_name.clone(), t);
            } else if let Some(existing) = self.templates.get_mut(&req.object_name) {
                existing.add_kind_of(crate::game_logic::KindOf::Crate);
            }
            let crate_team = if req.owned_by_maker {
                victim_team
            } else {
                Team::Neutral
            };
            let Some(crate_id) = self.create_object_with_script_effects(
                &req.object_name,
                crate_team,
                pos,
                None,
                presentation_effects,
            ) else {
                continue;
            };
            if req.owned_by_maker {
                if let Some(crate_obj) = self.objects.get_mut(&crate_id) {
                    crate_obj.owner_player_id = victim_owner;
                }
            }
            if let Some(crate_obj) = self.objects.get_mut(&crate_id) {
                crate_obj.apply_crate_terrain_decal();
            }

            if req.is_shroud_crate {
                self.host_money_crates.register_shroud_crate(crate_id);
            } else if req.is_heal_crate {
                self.host_money_crates.register_heal_crate(crate_id);
            } else if req.is_unit_crate {
                self.host_money_crates.register_unit_crate(
                    crate_id,
                    &req.unit_crate_type,
                    req.unit_crate_count,
                );
            } else if req.is_veterancy {
                self.host_money_crates.register_level_up_crate(
                    crate_id,
                    req.veterancy_effect_range,
                    req.veterancy_levels,
                );
            } else if req.object_name.eq_ignore_ascii_case("SalvageCrate") {
                self.host_money_crates
                    .register_salvage_crate(crate_id, req.money_provided);
            } else {
                self.host_money_crates.register(
                    crate_id,
                    req.money_provided,
                    req.building_pickup,
                    if req.building_pickup { 25 } else { 0 },
                );
            }
            self.host_money_crates
                .apply_crate_collide_gates(crate_id, &req.object_name);
            // C++ DeletionUpdate residual on crate object.
            self.host_money_crates.arm_default_deletion(
                crate_id,
                self.frame,
                crate_id.0.wrapping_add(self.frame),
            );
            if let Some(kid) = killer_id {
                let _ = self.notify_computer_killer_of_crate(kid, crate_id);
            }
            spawned += 1;
        }
        spawned
    }
    pub fn notify_computer_killer_of_crate(
        &mut self,
        killer_id: ObjectId,
        crate_id: ObjectId,
    ) -> bool {
        let killer_team = match self.objects.get(&killer_id) {
            Some(k) if k.is_alive() => k.team,
            _ => return false,
        };
        // Computer = non-local player residual.
        let is_computer = self
            .players
            .values()
            .find(|p| p.team == killer_team)
            .map(|p| !p.is_local)
            .unwrap_or(true); // no player record → treat as AI
        if !is_computer {
            return false;
        }
        self.notify_unit_crate(killer_id, crate_id)
    }
    pub fn try_idle_repulse(&mut self, unit_id: ObjectId) -> bool {
        if !self.enable_repulsors {
            return false;
        }
        let (vision, is_idle, can_be, alive, pos) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            if !u.is_kind_of(crate::game_logic::KindOf::CanBeRepulsed) {
                return false;
            }
            // C++ ai->isIdle()
            let idle = matches!(u.ai_state(), crate::game_logic::AIState::Idle)
                && u.target.is_none()
                && u.move_away_from.is_none();
            if !idle {
                return false;
            }
            let vision = u.vision_range.max(50.0);
            (vision, idle, true, true, u.get_position())
        };
        let _ = (is_idle, can_be, alive, pos);
        let Some((rep_id, _)) = self.find_closest_repulsor(unit_id, vision) else {
            return false;
        };
        let rep_pos = match self.objects.get(&rep_id) {
            Some(r) => r.get_position(),
            None => return false,
        };
        self.apply_host_locomotor_set_at_cell(
            unit_id,
            crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Panic,
            true,
        );
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.ai_move_away_from_unit(rep_id, rep_pos);
            if u.ignore_collisions_until_frame > 0 && u.ignore_collisions_until_frame < 100_000 {
                u.ignore_collisions_until_frame = self.frame.saturating_add(60);
            }
            // C++ AIMoveAwayFromRepulsorsState::onEnter (AIStates.cpp:2265).
            u.set_adjust_destinations(false);
            let _ = u.begin_request_safe_path(
                rep_id,
                u.move_away_destination.unwrap_or(rep_pos),
                self.frame,
            );
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy)]
enum CommandButtonHuntSpecial {
    Capture,
    Snipe,
    Tnt,
    DemoCharge,
    HackVehicle,
    HackBuilding,
    StealCash,
    Booby,
    Unknown,
}

fn classify_command_button_hunt_special(button: &str) -> CommandButtonHuntSpecial {
    let n: String = button
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if n.contains("snipe") || n.contains("jarmenkell") {
        CommandButtonHuntSpecial::Snipe
    } else if n.contains("tnt") || n.contains("tankhunter") {
        CommandButtonHuntSpecial::Tnt
    } else if n.contains("remotecharge") || n.contains("remotedemo") {
        CommandButtonHuntSpecial::DemoCharge
    } else if n.contains("timedcharge") || n.contains("timeddemo") || n.contains("burton") {
        CommandButtonHuntSpecial::Tnt
    } else if n.contains("stealcash") {
        CommandButtonHuntSpecial::StealCash
    } else if n.contains("disablevehicle") || n.contains("hackvehicle") {
        CommandButtonHuntSpecial::HackVehicle
    } else if n.contains("hackbuilding") || n.contains("disablebuilding") {
        CommandButtonHuntSpecial::HackBuilding
    } else if n.contains("booby") {
        CommandButtonHuntSpecial::Booby
    } else if n.contains("capture") || n.contains("blacklotus") {
        CommandButtonHuntSpecial::Capture
    } else {
        CommandButtonHuntSpecial::Unknown
    }
}

fn hunt_dist_2d(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn leftover_template_known(template_name: &str) -> Option<()> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    factory.find_template(template_name, false).map(|_| ())
}

fn leftover_template_has_named_module(template_name: &str, module: &str) -> Option<bool> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let want = module.to_ascii_lowercase();
    Some(tmpl.get_behavior_module_info().iter().any(|entry| {
        let name = entry.name.as_str();
        name.eq_ignore_ascii_case(module)
            || (want == "aiupdate" && leftover_module_is_ai_update(name))
    }))
}

fn leftover_module_is_ai_update(name: &str) -> bool {
    name.eq_ignore_ascii_case("AIUpdateInterface")
        || name.eq_ignore_ascii_case("AIUpdate")
        || name.to_ascii_lowercase().ends_with("aiupdate")
}

fn leftover_template_has_ai_update(template_name: &str) -> Option<bool> {
    leftover_template_has_named_module(template_name, "AIUpdate")
}

fn leftover_template_has_command_button_hunt(template_name: &str) -> Option<bool> {
    leftover_template_has_named_module(template_name, "CommandButtonHuntUpdate")
}

fn leftover_template_command_set(template_name: &str) -> Option<String> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let cs = tmpl.get_command_set_string();
    let s = cs.as_str();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod hq_tb5nn_tests {
    use super::*;
    use crate::game_logic::host_command_button_hunt::{
        HUNT_CMD_FROM_AI, HostCommandButtonHuntMode,
    };
    use crate::game_logic::{
        KindOf, Object, ObjectId, Team, ThingTemplate, Weapon, WeaponLockType,
    };

    /// hq-tb5nn: C++ huntWeapon UPDATE_SLEEP_NONE re-arms LOCKED_TEMPORARILY
    /// every frame after fireCurrentWeapon releases the temp lock on clip reload.
    #[test]
    fn fire_weapon_command_button_hunt_relocks_every_frame() {
        let mut logic = GameLogic::new();
        let id = ObjectId(4401);
        let mut ranger_template = ThingTemplate::new("AmericaInfantryRanger");
        ranger_template.add_kind_of(KindOf::Infantry);
        let mut ranger = Object::new(ranger_template, id, Team::USA);
        ranger.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        ranger.secondary_weapon = Some(Weapon {
            range: 80.0,
            damage: 5.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        ranger.arm_command_button_hunt(HostCommandButtonHuntMode::FireWeapon, 0);
        if let Some(h) = ranger.command_button_hunt.as_mut() {
            h.weapon_slot = 1;
        }
        ranger.last_command_source = HUNT_CMD_FROM_AI;
        logic.objects.insert(id, ranger);

        logic.frame = 5;
        logic.tick_command_button_hunt_updates();
        {
            let u = logic.objects.get(&id).expect("ranger");
            assert_eq!(u.weapon_lock_type, WeaponLockType::LockedTemporarily);
            assert_eq!(u.weapon_lock_slot, 1);
        }

        if let Some(u) = logic.objects.get_mut(&id) {
            u.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
        assert_eq!(
            logic.objects.get(&id).expect("ranger").weapon_lock_type,
            WeaponLockType::NotLocked
        );

        // Next frame — not +30. Clip-complete release must be re-armed immediately.
        logic.frame = 6;
        logic.tick_command_button_hunt_updates();
        let u = logic.objects.get(&id).expect("ranger");
        assert_eq!(
            u.weapon_lock_type,
            WeaponLockType::LockedTemporarily,
            "FireWeapon hunt must re-lock every frame, not every 30"
        );
        assert_eq!(u.weapon_lock_slot, 1);
    }
}

fn hijacker_veterancy_rank(level: crate::game_logic::VeterancyLevel) -> u8 {
    use crate::game_logic::VeterancyLevel;
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

#[cfg(test)]
#[path = "crates/hijacker_owner_tests.rs"]
mod hijacker_owner_tests;

#[path = "crates/parachute_owner.rs"]
mod parachute_owner;
