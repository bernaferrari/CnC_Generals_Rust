use super::*;

impl Object {
    /// C++ residual: STEALTHED && !DETECTED && !DISGUISED.
    /// Stealthed-and-undetected units are not legal auto/manual attack targets.
    /// Disguised units are visible as their disguise team (not pure-stealth hide).
    pub fn is_effectively_stealthed(&self) -> bool {
        self.status.stealthed && !self.status.detected && !self.status.disguised
    }

    /// C++ OBJECT_STATUS_DISGUISED residual.
    pub fn is_disguised(&self) -> bool {
        self.status.disguised
    }

    /// Apply Bomb Truck disguise residual (StealthUpdate::disguiseAsObject).
    ///
    /// C++ residual: start DisguiseTransitionTime frames; at halfpoint
    /// `changeVisualDisguise` sets DISGUISED + model. Host residual: arm
    /// pending template/team, tick opacity, commit at halfpoint.
    pub fn apply_disguise(&mut self, template_name: &str, as_team: Team) {
        use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES;
        if self.status.destroyed {
            return;
        }
        self.disguise_pending_template = Some(template_name.to_string());
        self.record_host_ai_request();
        self.disguise_pending_team = Some(as_team);
        // Not fully disguised until halfpoint residual.
        self.set_status_disguised(false);
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        self.status.disguise_transition_frames = BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES;
        self.set_status_disguise_transitioning_to(true);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        // Keep previous appearance until halfpoint if any.
        self.record_host_disguise();
    }

    /// Clear disguise residual (reveal transition).
    ///
    /// C++ residual: DisguiseRevealTransitionTime frames; halfpoint restores
    /// true visual; end clears STEALTHED.
    pub fn clear_disguise(&mut self) {
        use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
        if !self.status.disguised
            && self.disguise_as_template.is_none()
            && self.disguise_pending_template.is_none()
            && self.status.disguise_transition_frames == 0
        {
            return;
        }
        // Begin reveal transition residual (losing disguise look).
        self.status.disguise_transition_frames = BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
        self.set_status_disguise_transitioning_to(false);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        // Keep disguise_as_* until halfpoint swap back.
        self.record_host_disguise();
    }

    /// Force-clear disguise residual immediately (no transition).
    pub fn clear_disguise_instant(&mut self) {
        self.set_status_disguised(false);
        self.disguise_as_template = None;
        self.disguise_as_team = None;
        self.disguise_pending_template = None;
        self.record_host_ai_request();
        self.disguise_pending_team = None;
        self.set_status_stealthed(false);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        self.status.disguise_transition_frames = 0;
        self.set_status_disguise_transitioning_to(false);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        self.record_host_disguise();
    }

    /// C++ StealthUpdate disguise transition residual tick.
    ///
    /// Returns true when halfpoint model-swap residual fired this frame.
    pub fn tick_disguise_transition(&mut self) -> bool {
        if self.status.disguise_transition_frames == 0 {
            return false;
        }
        use crate::game_logic::host_bomb_truck_disguise::{
            BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES, BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES,
        };
        self.status.disguise_transition_frames =
            self.status.disguise_transition_frames.saturating_sub(1);
        let total = if self.status.disguise_transitioning_to {
            BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES.max(1)
        } else {
            BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES.max(1)
        };
        let remaining = self.status.disguise_transition_frames;
        // factor 0 → 1 over transition (C++).
        let factor = 1.0 - (remaining as f32 / total as f32);
        // Opacity: full → none at midpoint → full (fabs(1 - factor*2)).
        let opacity = (1.0 - factor * 2.0).abs();
        self.status.disguise_transition_opacity = opacity;

        let mut halfpoint = false;
        if factor >= 0.5 && !self.status.disguise_halfpoint_reached {
            self.set_status_disguise_halfpoint_reached(true);
            halfpoint = true;
            if self.status.disguise_transitioning_to {
                // changeVisualDisguise residual: commit pending appearance.
                if let Some(tpl) = self.disguise_pending_template.take() {
                    self.disguise_as_template = Some(tpl);
                }
                if let Some(team) = self.disguise_pending_team.take() {
                    self.disguise_as_team = Some(team);
                }
                self.set_status_disguised(true);

                self.record_model_mesh_from_template();
                self.record_kind_of_bits_from_template();
                self.set_status_stealthed(true);
                self.set_status_detected(false);
            } else {
                // Reveal halfpoint: restore true look residual.
                self.set_status_disguised(false);
                self.disguise_as_template = None;
                self.disguise_as_team = None;
                self.disguise_pending_template = None;
                self.record_host_ai_request();
                self.disguise_pending_team = None;
            }
        }

        if remaining == 0 && !self.status.disguise_transitioning_to {
            // Finished removing disguise — clear stealth residual.
            self.set_status_stealthed(false);
            self.set_status_detected(false);
            self.detection_expires_frame = 0;
            self.record_host_stealth_delay();
            self.status.disguise_transition_opacity = 1.0;
        }
        self.record_host_disguise();
        halfpoint
    }

    /// Whether disguise transition residual is active.
    pub fn is_disguise_transitioning(&self) -> bool {
        self.status.disguise_transition_frames > 0
    }

    /// C++ SpyVisionUpdate::setDisabledUntilFrame residual.
    pub fn apply_spy_vision_disabled_until(&mut self, until_frame: u32) {
        if until_frame > self.status.spy_vision_disabled_until_frame {
            self.status.spy_vision_disabled_until_frame = until_frame;
        }
    }

    /// Whether SpyVision residual is currently disabled by sabotage residual.
    pub fn is_spy_vision_disabled(&self, current_frame: u32) -> bool {
        self.status.spy_vision_disabled_until_frame > current_frame
    }

    /// Expire SpyVision sabotage disable residual when frame passes.
    pub fn tick_spy_vision_disabled(&mut self, current_frame: u32) {
        if self.status.spy_vision_disabled_until_frame > 0
            && current_frame >= self.status.spy_vision_disabled_until_frame
        {
            self.status.spy_vision_disabled_until_frame = 0;
        }
    }

    /// Apparent team residual for a viewer (see host_bomb_truck_disguise).
    pub fn apparent_team_to(&self, viewer_team: Team) -> Team {
        crate::game_logic::host_bomb_truck_disguise::apparent_team_for_viewer(
            self.team,
            self.disguise_as_team,
            self.status.disguised,
            viewer_team,
        )
    }

    /// Effective detection radius for this unit when `is_detector`.
    /// C++: DetectionRange if > 0 else vision range.
    pub fn effective_detection_range(&self) -> f32 {
        if self.detection_range > 0.0 {
            self.detection_range
        } else {
            self.get_template().sight_range
        }
    }

    /// Mark this object as detected until `expires_frame` (logic frame exclusive).
    /// C++ StealthUpdate::markAsDetected residual (`StealthUpdate.cpp:846-912`).
    /// Detection permanently starts disguise reveal (`disguiseAsObject(NULL)`).
    pub fn mark_detected(&mut self, expires_frame: u32) {
        if self.status.disguised
            || self.disguise_as_template.is_some()
            || self.disguise_pending_template.is_some()
        {
            self.clear_disguise();
        }
        self.set_status_detected(true);
        // Keep the later expiry if already detected by another scanner.
        if expires_frame > self.detection_expires_frame {
            self.detection_expires_frame = expires_frame;
            self.record_host_stealth_delay();
        }
    }

    /// C++ StealthUpdate innate mine cloak + `setEffectiveOpacity(0,0)` residual.
    /// Land mines / demo traps / charges start stealthed and stay fully invisible.
    pub fn apply_mine_innate_stealth(&mut self) {
        use crate::game_logic::host_radar_stealth_vision_residual::MINE_STEALTH_OPACITY_RESIDUAL;
        self.innate_stealth = true;
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.stealth_delay_frames = 0;
        self.stealth_allowed_frame = 0;
        self.stealth_delay_pending = false;
        self.camo_friendly_opacity = MINE_STEALTH_OPACITY_RESIDUAL;
        self.record_host_stealth_flags();
        self.record_host_stealth_delay();
        self.record_host_vision_camo();
    }

    /// Clear DETECTED status (stealth may remain active).
    pub fn clear_detected(&mut self) {
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
    }

    /// Break stealth entirely (fire / script residual).
    /// Also clears disguise residual (attack reveal path for bomb truck).
    pub fn break_stealth(&mut self) {
        if self.status.disguised {
            self.clear_disguise();
            return;
        }
        let was_stealthed = self.status.stealthed;
        self.set_status_stealthed(false);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        // CamoNetting / StealthDelay residual: schedule re-cloak gate on reveal.
        if was_stealthed && self.stealth_delay_frames > 0 {
            self.stealth_delay_pending = true;
            self.record_host_stealth_delay();
        }
        // CamoNetting FriendlyOpacity residual: revealed → max opacity.
        if was_stealthed && self.stealth_breaks_on_damage {
            self.camo_friendly_opacity = 1.0;
            self.camo_opacity_pulse_phase = 0.0;
            self.record_host_stealth_delay();
        }
        self.record_host_vision_camo();
    }

    /// C++ StealthUpdate.cpp:178-201 `receiveGrant(true)`:
    /// `OBJECT_STATUS_CAN_STEALTH | OBJECT_STATUS_STEALTHED`,
    /// `m_stealthAllowedFrame = now`. CAN_STEALTH survives later fire destalth.
    pub fn apply_grant_stealth(&mut self) {
        if self.status.destroyed {
            return;
        }
        self.innate_stealth = true;
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        // receiveGrant keeps CAN_STEALTH; fire destalths then re-arms delay.
        if self.stealth_delay_frames == 0 {
            self.stealth_delay_frames = 30;
        }
        self.stealth_allowed_frame = 0;
        self.stealth_delay_pending = false;
        self.record_host_stealth_flags();
        self.record_host_stealth_delay();
    }

    /// C++ StealthUpdate.cpp:717-735 — cloak after `m_stealthAllowedFrame` when allowed.
    pub fn try_recloak_after_stealth_delay(&mut self, now: u32, forbidden: bool) -> bool {
        if !self.innate_stealth || !self.is_alive() || self.status.disguised {
            return false;
        }
        if self.stealth_delay_pending {
            self.stealth_allowed_frame = now.saturating_add(self.stealth_delay_frames);
            self.stealth_delay_pending = false;
            self.record_host_stealth_delay();
        }
        if forbidden || self.status.stealthed {
            return false;
        }
        if self.stealth_allowed_frame > 0 && now < self.stealth_allowed_frame {
            return false;
        }
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.stealth_allowed_frame = 0;
        self.record_host_stealth_delay();
        true
    }

    /// C++ StealthUpdate.cpp:365-373 — non-garrison container destalths occupants.
    pub fn transport_contain_should_destalth(container_is_garrisonable: bool) -> bool {
        !container_is_garrisonable
    }

    /// C++ StealthUpdate.cpp:696-714 — temp grant expires on timer or CMD_FROM_PLAYER.
    pub fn temporary_stealth_grant_should_expire(
        expires_frame: u32,
        now: u32,
        last_command_from_player: bool,
    ) -> bool {
        expires_frame > 0 && (last_command_from_player || now >= expires_frame)
    }

    /// C++ FIRING_PRIMARY last-shot residual: only primary slot destalths Burton.
    pub fn firing_primary_breaks_stealth(
        forbidden_primary_only: bool,
        last_fire_slot: u8,
        last_fire_sim_time: f32,
        firing: bool,
    ) -> bool {
        if !firing {
            return false;
        }
        if !forbidden_primary_only {
            return true;
        }
        last_fire_slot == 0 && last_fire_sim_time > 0.0
    }

    /// C++ Object::setVisionSpied residual (refcounted mask simplified to bitmask).
    /// When on, spying player treats this unit as a temporary looker / revealed target.
    pub fn set_vision_spied_by_player(&mut self, player_id: u32, on: bool) {
        let bit = 1u32 << player_id.min(31);
        if on {
            self.vision_spied_mask |= bit;
        } else {
            self.vision_spied_mask &= !bit;
        }
        self.record_host_vision_camo();
    }

    /// True if `player_id` currently has vision-spied residual on this unit.
    pub fn is_vision_spied_by_player(&self, player_id: u32) -> bool {
        let bit = 1u32 << player_id.min(31);
        (self.vision_spied_mask & bit) != 0
    }

    /// Whether an enemy of `attacker_team` may target this object.
    /// C++ WeaponSet::getCanAttackObject stealth gate residual + disguise
    /// relationship residual (disguised units appear as disguise team).
    pub fn is_targetable_by_enemy_of(&self, attacker_team: Team) -> bool {
        // C++ WeaponSet applies these two unconditional victim overrides
        // before visibility, relationship, or target acquisition.  Keeping
        // them in this shared query closes the AI/retaliation/area-attack
        // paths that do not enter an explicit player command first.
        if !self.is_alive()
            || !self.is_attackable()
            || self.is_kind_of(KindOf::Unattackable)
            || self.status.masked
        {
            return false;
        }
        // Disguise residual: auto-target uses apparent team (allies of disguise skip).
        if self.status.disguised {
            return crate::game_logic::host_bomb_truck_disguise::is_auto_targetable_as_enemy(
                self.team,
                self.disguise_as_team,
                true,
                attacker_team,
            ) && !self.is_effectively_stealthed();
        }
        if self.team == attacker_team {
            return false;
        }
        // Stealthed and not detected: not a valid target.
        !self.is_effectively_stealthed()
    }

    /// Whether `weapon` can legally hit `target` (air/ground + range + stealth).
    pub fn can_target_with(&self, target: &Object, weapon: &Weapon) -> bool {
        self.can_target_with_slot(target, weapon, None)
    }

    /// Slot-aware can_target (LeechRange uses per-slot active residual).
    pub fn can_target_with_slot(&self, target: &Object, weapon: &Weapon, slot: Option<u8>) -> bool {
        // This helper is used by live acquisition and combat state paths, not
        // merely a range query.  Keep C++ WeaponSet's unconditional target
        // overrides here as well as at the command boundary so a masked or
        // UNATTACKABLE object cannot slip through an AI/ground-acquire path.
        if target.is_kind_of(KindOf::Unattackable) || target.status.masked {
            return false;
        }
        // C++ WeaponSet: stealthed + undetected cannot be attacked
        // (including force-fire against pure stealth; disguise exception not residual).
        // OBJECT_STATUS_IGNORING_STEALTH residual bypasses this gate.
        if target.is_effectively_stealthed()
            && target.team != self.team
            && !self.status.ignoring_stealth
        {
            return false;
        }

        let target_anti_mask = target.weapon_target_anti_mask();
        if !self.weapon_allows_target_anti_mask(weapon, slot, target_anti_mask) {
            return false;
        }

        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;
        // C++ DAMAGE_DISARM estimate residual: only mines/demo/booby are valid.
        {
            let wname = slot.and_then(|weapon_slot| self.weapon_name_for_slot(weapon_slot));
            if wname
                .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                .unwrap_or(false)
            {
                if !target.is_disarmable_mine() {
                    return false;
                }
            }
        }

        // C++ parity (Weapon::isWithinAttackRange): check both minimum
        // and maximum attack range. Ground targets use horizontal (XZ)
        // distance so terrain height does not permanently block fire after
        // a successful march into range.
        let distance = if target_is_air {
            self.thing.get_distance_to(&target.thing)
        } else {
            let a = self.get_position();
            let b = target.get_position();
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };
        if weapon.min_range > 0.0 && distance < weapon.min_range {
            return false;
        }
        // C++ Weapon::hasLeechRange residual: once activated, max range waived
        // for the remainder of the attack cycle.
        let leech = match slot {
            Some(1) => self.leech_range_active_secondary,
            Some(0) => self.leech_range_active_primary,
            // There is no persisted tertiary leech state yet.  It must not
            // inherit primary's range waiver.
            Some(_) => false,
            None => self.leech_range_active_primary || self.leech_range_active_secondary,
        };
        if leech {
            return true;
        }
        // SearchAndDestroy residual: BATTLEPLAN_SEARCHANDDESTROY RANGE 120%.
        let max_range = self.effective_weapon_range(weapon.range);
        distance <= max_range
    }

    /// True if primary **or** secondary can currently hit the target.
    pub fn can_target(&self, target: &Object) -> bool {
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }
        if let Some(weapon) = self.weapon_slot(0) {
            if self.can_target_with_slot(target, weapon, Some(0)) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            if self.can_target_with_slot(target, weapon, Some(1)) {
                return true;
            }
        }
        if let Some(weapon) = &self.tertiary_weapon {
            if self.can_target_with_slot(target, weapon, Some(2)) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod stealth_grant_tests {
    use super::{Object, ObjectId, Team, ThingTemplate};


    #[test]
    fn gps_grant_keeps_can_stealth_and_recloaks_after_fire() {
        // C++ StealthUpdate.cpp:198 receiveGrant stays CAN_STEALTH.
        let mut unit = Object::new(ThingTemplate::new("TestTank"), super::ObjectId(1), super::Team::GLA);
        unit.apply_grant_stealth();
        assert!(unit.status.stealthed);
        assert!(unit.innate_stealth, "receiveGrant must set CAN_STEALTH");

        unit.stealth_breaks_on_attack = true;
        unit.stealth_delay_frames = 30;
        unit.break_stealth();
        assert!(!unit.status.stealthed);
        assert!(unit.innate_stealth, "fire must not strip CAN_STEALTH");
        assert!(unit.stealth_delay_pending);

        assert!(!unit.try_recloak_after_stealth_delay(0, false));
        assert!(!unit.status.stealthed, "must wait StealthDelay");
        assert!(unit.try_recloak_after_stealth_delay(30, false));
        assert!(unit.status.stealthed);
        assert!(unit.innate_stealth);
    }

    #[test]
    fn hero_spawn_delay_does_not_cloak_before_allowed_frame() {
        // C++ StealthUpdate.cpp:111 ctor delay before STEALTHED.
        let mut hero = Object::new(
            ThingTemplate::new("AmericaInfantryColonelBurton"),
            super::ObjectId(2),
            super::Team::USA,
        );
        hero.innate_stealth = true;
        hero.stealth_delay_frames = 60;
        hero.stealth_allowed_frame = 60;
        assert!(!hero.status.stealthed);
        assert!(!hero.try_recloak_after_stealth_delay(0, false));
        assert!(!hero.status.stealthed);
        assert!(hero.try_recloak_after_stealth_delay(60, false));
        assert!(hero.status.stealthed);
    }

    #[test]
    fn camo_unlock_rearms_delay_not_instant_cloak() {
        // C++ StealthUpgrade.cpp:31 CAN_STEALTH; StealthUpdate.cpp:739 re-arm.
        let mut rebel = Object::new(
            ThingTemplate::new("GLAInfantryRebel"),
            super::ObjectId(3),
            super::Team::GLA,
        );
        rebel.innate_stealth = true;
        rebel.stealth_delay_frames = 75;
        rebel.stealth_allowed_frame = 75;
        rebel.set_status_stealthed(false);
        assert!(!rebel.try_recloak_after_stealth_delay(10, false));
        assert!(!rebel.status.stealthed);
        assert!(rebel.try_recloak_after_stealth_delay(75, false));
        assert!(rebel.status.stealthed);
    }
    #[test]
    fn transport_destalths_non_garrison() {
        assert!(Object::transport_contain_should_destalth(false));
        assert!(!Object::transport_contain_should_destalth(true));
    }

    #[test]
    fn temp_grant_strips_on_player_order() {
        assert!(Object::temporary_stealth_grant_should_expire(100, 50, true));
        assert!(!Object::temporary_stealth_grant_should_expire(100, 50, false));
        assert!(Object::temporary_stealth_grant_should_expire(100, 100, false));
        assert!(!Object::temporary_stealth_grant_should_expire(0, 50, true));
    }

    #[test]
    fn burton_primary_only_breaks_stealth() {
        assert!(Object::firing_primary_breaks_stealth(true, 0, 1.0, true));
        assert!(!Object::firing_primary_breaks_stealth(true, 1, 1.0, true));
        assert!(!Object::firing_primary_breaks_stealth(true, 0, 0.0, true));
        assert!(!Object::firing_primary_breaks_stealth(true, 0, 1.0, false));
        assert!(Object::firing_primary_breaks_stealth(false, 1, 1.0, true));
    }
}
