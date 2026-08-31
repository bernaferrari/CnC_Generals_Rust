use super::*;

impl Object {
    // Command system compatibility methods
    pub fn can_move(&self) -> bool {
        // C++ Object::isMobile: KINDOF_IMMOBILE or isDisabled() → false.
        // weapons_jammed intentionally does NOT block movement (weapons-only residual).
        // Docked aircraft may move (takeoff/sortie residual).
        // Shock flailing residual: block commanded move while STUNNED_FLAILING
        // (stun_frames > 15). Settled STUNNED phase may still stagger via velocity.
        let parked_aircraft = self.is_parked_at_airfield();
        let flailing = self.shock_stun_frames > 15;
        self.is_mobile()
            && self.is_alive()
            && !self.status.deployed
            && !self.is_disabled()
            && !flailing
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned))
    }

    pub fn set_destination(&mut self, destination: Vec3) {
        let _ = self.takeoff_from_airfield_parking();
        // C++ DeployStyle: ordered move packs up (undeploy) residual.
        if self.status.deployed {
            self.set_deployed(false);
        }
        self.move_to(destination);
    }

    pub fn set_target(&mut self, target: Option<ObjectId>) {
        if target.is_some() {
            let _ = self.takeoff_from_airfield_parking();
        }
        self.target = target;
        if target.is_some() {
            self.target_location = None;
            self.record_host_target_location();
            self.set_ai_state(AIState::Attacking);
            self.set_status_attacking(true);
        } else {
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Idle);
            self.set_status_attacking(false);
        }
        crate::game_logic::host_attack_log::record(self.id, target);
    }

    /// Set order target without forcing AIState::Attacking.
    /// Used by capture/hijack/gather/special-ability pathing where
    /// `path_to_goal_with_state` owns the AI state residual.
    /// Still last-writes host_attack_log + target_location clear.
    pub fn set_order_target(&mut self, target: Option<ObjectId>) {
        if target.is_some() {
            let _ = self.takeoff_from_airfield_parking();
        }
        self.target = target;
        self.target_location = None;
        self.record_host_target_location();
        self.set_status_force_attack(false);
        self.set_status_attacking(false);
        crate::game_logic::host_attack_log::record(self.id, target);
    }

    /// Check whether this object can fire the requested special power.
    ///
    /// Per-power residual: only this power's timer must be clear (other SWs may
    /// still be reloading). Aggregate `special_power_ready` is refreshed for HUD.
    pub fn is_special_power_ready(&self, power: &SpecialPowerType) -> bool {
        if !self.is_alive() || self.is_disabled() {
            return false;
        }
        // C++ SpecialPowerModule::isReady requires m_pausedCount == 0.
        if self.is_special_power_countdown_paused(power) {
            return false;
        }
        let remaining = self
            .special_power_cooldowns
            .get(power)
            .copied()
            .unwrap_or(0.0);
        remaining <= 0.0
    }

    /// C++ SpecialPowerModule::pauseCountdown — refcount, not a set.
    ///
    /// First pause increments the count. Nested pauses only increment.
    /// Final unpause (count → 0) leaves remaining cooldown unchanged: live
    /// ticks already freeze while paused, so a power that was ready stays
    /// ready (`availableOnFrame` slides with pause duration in C++).
    pub fn pause_special_power_countdown(&mut self, power: &SpecialPowerType, pause: bool) {
        if pause {
            let count = self.special_power_paused.entry(power.clone()).or_insert(0);
            *count = count.saturating_add(1);
        } else if let Some(count) = self.special_power_paused.get_mut(power) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                self.special_power_paused.remove(power);
            }
        }
    }

    /// C++ `m_pausedCount > 0` residual.
    pub fn is_special_power_countdown_paused(&self, power: &SpecialPowerType) -> bool {
        self.special_power_paused.get(power).copied().unwrap_or(0) > 0
    }

    /// C++ `SpecialPowerModule::setReadyFrame` residual.
    ///
    /// `seconds` is remaining countdown (`LOGICFRAMES_PER_SECOND * seconds` in
    /// C++ ScriptActions.cpp:4094-4113). Zero or negative means ready now.
    pub fn set_special_power_ready_seconds(&mut self, power: &SpecialPowerType, seconds: f32) {
        if seconds <= 0.0 {
            self.special_power_cooldowns.remove(power);
        } else {
            self.special_power_cooldowns.insert(power.clone(), seconds);
        }
        self.refresh_special_power_aggregate_cooldown();
    }

    /// Remaining per-power countdown seconds (0 = ready).
    pub fn special_power_countdown_seconds(&self, power: &SpecialPowerType) -> f32 {
        self.special_power_cooldowns
            .get(power)
            .copied()
            .unwrap_or(0.0)
    }

    /// C++ Object::setWeaponBonusCondition(PLAYER_UPGRADE) residual.
    pub fn set_weapon_bonus_player_upgrade(&mut self, enabled: bool) {
        self.weapon_bonus_player_upgrade = enabled;
    }

    /// C++ Object::setWeaponBonusCondition(DRONE_SPOTTING) residual.
    pub fn set_weapon_bonus_drone_spotting(&mut self, enabled: bool) {
        self.weapon_bonus_drone_spotting = enabled;
    }

    /// C++ BodyModule::setArmorSetFlag(ARMORSET_PLAYER_UPGRADE) residual.
    pub fn set_armor_set_player_upgrade(&mut self, enabled: bool) {
        self.armor_set_player_upgrade = enabled;
    }

    /// C++ AIUpdateInterface::setLocomotorUpgrade residual.
    pub fn set_locomotor_upgrade(&mut self, enabled: bool) {
        self.locomotor_upgrade = enabled;
    }

    /// C++ Drawable::setTerrainDecal(TERRAIN_DECAL_CHEMSUIT) residual.
    pub fn set_terrain_decal_chemsuit(&mut self, enabled: bool) {
        use crate::game_logic::host_battlemaster::{TERRAIN_DECAL_CHEMSUIT, TERRAIN_DECAL_NONE};
        self.terrain_decal_chemsuit = enabled;
        if enabled {
            self.set_terrain_decal(TERRAIN_DECAL_CHEMSUIT);
            if self.terrain_decal_size <= 0.0 {
                let major = if self.thing.template.geometry_info.authored {
                    self.thing.template.geometry_info.major_radius
                } else {
                    self.selection_radius.max(8.0)
                };
                self.set_terrain_decal_size(major * 2.0, major * 2.0);
            }
            self.terrain_decal_opacity = 1.0;
            self.terrain_decal_fade_rate = 0.0;
        } else if self.terrain_decal_type == TERRAIN_DECAL_CHEMSUIT {
            self.set_terrain_decal(TERRAIN_DECAL_NONE);
            self.terrain_decal_opacity = 0.0;
        }
    }

    /// C++ Drawable::setTerrainDecal residual (HordeUpdate rings).
    pub fn set_terrain_decal(&mut self, decal_type: u8) {
        if self.terrain_decal_type == decal_type {
            return;
        }
        self.terrain_decal_type = decal_type;
    }

    /// C++ Drawable::getTerrainDecalType residual.
    pub fn get_terrain_decal_type(&self) -> u8 {
        self.terrain_decal_type
    }

    /// C++ Drawable::setTerrainDecalSize residual.
    pub fn set_terrain_decal_size(&mut self, x: f32, _y: f32) {
        self.terrain_decal_size = x;
    }

    /// C++ Drawable::setTerrainDecalFadeTarget residual.
    pub fn set_terrain_decal_fade_target(&mut self, target: f32, rate: f32) {
        self.terrain_decal_fade_target = target;
        self.terrain_decal_fade_rate = rate;
    }

    /// C++ `Drawable::update` LERP `m_decalOpacity` by fade rate; clear at 0.
    pub fn tick_terrain_decal_fade(&mut self) {
        use crate::game_logic::host_battlemaster::TERRAIN_DECAL_NONE;
        if self.terrain_decal_type == TERRAIN_DECAL_NONE || self.terrain_decal_fade_rate == 0.0 {
            return;
        }
        self.terrain_decal_opacity += self.terrain_decal_fade_rate;
        if self.terrain_decal_fade_rate < 0.0 && self.terrain_decal_opacity <= 0.0 {
            self.terrain_decal_opacity = 0.0;
            self.terrain_decal_fade_rate = 0.0;
            self.set_terrain_decal(TERRAIN_DECAL_NONE);
        } else if self.terrain_decal_fade_rate > 0.0 && self.terrain_decal_opacity >= 1.0 {
            self.terrain_decal_opacity = 1.0;
            self.terrain_decal_fade_rate = 0.0;
        }
    }

    /// C++ CreateCrateDie.cpp:219-223 crate glow.
    pub fn apply_crate_terrain_decal(&mut self) {
        use crate::game_logic::host_battlemaster::{
            CRATE_DECAL_FADE_IN_RATE, CRATE_DECAL_SIZE_MULT, TERRAIN_DECAL_CRATE,
        };
        let major = if self.thing.template.geometry_info.authored {
            self.thing.template.geometry_info.major_radius
        } else {
            self.selection_radius.max(4.0)
        };
        let size = CRATE_DECAL_SIZE_MULT * major.max(1.0);
        self.set_terrain_decal(TERRAIN_DECAL_CRATE);
        self.set_terrain_decal_size(size, size);
        self.terrain_decal_opacity = 0.0;
        self.set_terrain_decal_fade_target(1.0, CRATE_DECAL_FADE_IN_RATE);
    }

    /// C++ `Drawable::friend_bindToObject` / `changedTeam` for `KINDOF_FS_FAKE`.
    pub fn apply_fake_building_terrain_decal(&mut self) {
        use crate::game_logic::KindOf;
        use crate::game_logic::host_battlemaster::TERRAIN_DECAL_SHADOW_TEXTURE;
        if !self.is_kind_of(KindOf::FSFake) {
            return;
        }
        self.set_terrain_decal(TERRAIN_DECAL_SHADOW_TEXTURE);
        if self.terrain_decal_size <= 0.0 {
            let major = if self.thing.template.geometry_info.authored {
                self.thing.template.geometry_info.major_radius
            } else {
                self.selection_radius.max(20.0)
            };
            self.set_terrain_decal_size(major * 2.0, major * 2.0);
        }
        self.terrain_decal_opacity = 1.0;
        self.terrain_decal_fade_rate = 0.0;
    }

    /// C++ HordeUpdate terrain-decal type / size / fade matrix.
    pub fn apply_horde_terrain_decal(
        &mut self,
        was_in_horde: bool,
        now_in_horde: bool,
        draw_icon_ui: bool,
    ) {
        use crate::game_logic::host_battlemaster::{
            TERRAIN_DECAL_NONE, has_fanaticism_upgrade, hide_leftover_horde_flag_subobjects,
            is_portable_structure_template, leftover_horde_decal_fade, leftover_horde_decal_type,
            leftover_horde_fanaticism_bonus, leftover_horde_major_radius,
            leftover_infantry_horde_decal_size_or_bbox, leftover_template_shadow_size,
            leftover_unit_has_horde_flag_subobjects, leftover_vehicle_horde_decal_size,
        };
        if leftover_unit_has_horde_flag_subobjects(&self.template_name) {
            hide_leftover_horde_flag_subobjects(&mut self.sub_object_visibility);
        }
        if !self.is_alive() {
            return;
        }
        let is_infantry = self.is_kind_of(crate::game_logic::KindOf::Infantry);
        if draw_icon_ui {
            if now_in_horde && !is_portable_structure_template(&self.template_name) {
                let has_nationalism = self.weapon_bonus_nationalism;
                let has_fanaticism = leftover_horde_fanaticism_bonus(
                    has_nationalism,
                    has_fanaticism_upgrade(&self.applied_upgrades),
                );
                let ty = leftover_horde_decal_type(is_infantry, has_nationalism, has_fanaticism);
                if is_infantry {
                    let (sx, sy) = leftover_template_shadow_size(
                        &self.template_name,
                        self.thing.template.shadow_size_x,
                        self.thing.template.shadow_size_y,
                    );
                    let geom = &self.thing.template.geometry_info;
                    let major = leftover_horde_major_radius(
                        geom.authored,
                        geom.major_radius,
                        self.selection_radius,
                    );
                    let minor = if geom.authored && geom.minor_radius > 0.0 {
                        geom.minor_radius
                    } else {
                        major
                    };
                    let size = leftover_infantry_horde_decal_size_or_bbox(sx, sy, major, minor);
                    self.set_terrain_decal_size(size, size);
                } else {
                    let geom = &self.thing.template.geometry_info;
                    let major = leftover_horde_major_radius(
                        geom.authored,
                        geom.major_radius,
                        self.selection_radius,
                    );
                    let size = leftover_vehicle_horde_decal_size(major);
                    self.set_terrain_decal_size(size, size);
                }
                if self.get_terrain_decal_type() != ty {
                    self.set_terrain_decal(ty);
                }
            }
        } else {
            self.set_terrain_decal(TERRAIN_DECAL_NONE);
        }

        if let Some((target, rate)) = leftover_horde_decal_fade(was_in_horde, now_in_horde) {
            self.set_terrain_decal_fade_target(target, rate);
        }
    }

    /// C++ SpecialPowerCompletionDie::setCreator residual.
    pub fn set_special_power_completion(
        &mut self,
        special_power_name: impl Into<String>,
        creator_id: u32,
    ) {
        if self
            .special_power_completion
            .as_ref()
            .map(|d| d.creator_set)
            .unwrap_or(false)
        {
            return;
        }
        self.special_power_completion = Some(
            crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionDieData::new(
                special_power_name,
                creator_id,
            ),
        );
    }

    /// C++ ObjectCreationList.cpp:386-393 / Weapon.cpp:1103-1113 setCreator.
    /// Stamps the creator only when this template carries SpecialPowerCompletionDie.
    pub fn bind_special_power_completion_creator(&mut self, creator_id: u32) {
        let Some(power) =
            crate::game_logic::host_special_power_completion_die::completion_die_power_for_template(
                &self.template_name,
            )
        else {
            return;
        };
        self.set_special_power_completion(power, creator_id);
    }

    /// C++ SpecialPowerModule::startPowerRecharge residual (non-SharedNSync path).
    ///
    /// Sets this power's cooldown to full ReloadTime so PublicTimer SWs start
    /// charging when the structure is created/completed — not ready-to-fire.
    pub fn start_power_recharge(&mut self, power: &crate::command_system::SpecialPowerType) {
        let cd = crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
            power,
        )
        .unwrap_or(self.special_power_cooldown)
        .max(0.0);
        if cd > 0.0 {
            self.special_power_cooldowns.insert(power.clone(), cd);
            // Legacy aggregate timer residual for single-slot HUD paths.
            self.special_power_cooldown = cd;
            self.special_power_cooldown_remaining = cd;
            self.set_special_power_ready(false);
        } else {
            self.special_power_cooldowns.remove(power);
            // Flag-only: a zero-reload arm must not express force-ready for
            // the whole object (set_special_power_ready(true) clears every
            // power's countdown map entry).
            self.special_power_ready = true;
            self.special_power_cooldown_remaining = 0.0;
        }
        self.refresh_special_power_aggregate_cooldown();
    }

    /// Start one authored SpecialPower timer with its parsed C++ ReloadTime.
    ///
    /// The legacy helper above still services broad residual powers whose
    /// timings live in a handwritten table.  Paired Object INI abilities must
    /// never borrow that table (or `ThingTemplate::special_power_cooldown`)
    /// merely because they share a command enum.
    pub fn start_power_recharge_with_frames(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        reload_time_frames: u32,
    ) {
        let seconds = reload_time_frames as f32 / 30.0;
        if seconds > 0.0 {
            self.special_power_cooldowns.insert(power.clone(), seconds);
            self.special_power_cooldown = seconds;
            self.special_power_cooldown_remaining = seconds;
            self.set_special_power_ready(false);
        } else {
            self.special_power_cooldowns.remove(power);
        }
        self.refresh_special_power_aggregate_cooldown();
    }

    /// Consume a charge for the special power and start per-power cooldown.
    pub fn consume_special_power_charge(&mut self, power: &SpecialPowerType) {
        if !self.is_special_power_ready(power) {
            return;
        }
        // Prefer retail SpecialPower ReloadTime residual when known; else template cooldown.
        let cd = crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
            power,
        )
        .unwrap_or(self.special_power_cooldown)
        .max(0.0);
        if cd > 0.0 {
            self.special_power_cooldowns.insert(power.clone(), cd);
        } else {
            self.special_power_cooldowns.remove(power);
        }
        self.refresh_special_power_aggregate_cooldown();
        if crate::game_logic::host_missile_launcher_building_update::missile_launcher_special_power(
            &self.template_name,
        )
        .as_ref()
            == Some(power)
        {
            if let Some(data) = self.missile_launcher_building.as_mut() {
                data.pending_initiate = true;
            } else if let Some(ini) = crate::game_logic::host_missile_launcher_building_update::missile_launcher_ini_for_template(
                &self.template_name,
            ) {
                let mut data = crate::game_logic::host_missile_launcher_building_update::HostMissileLauncherBuildingUpdateData::from_ini(ini);
                data.pending_initiate = true;
                self.missile_launcher_building = Some(data);
            }
        }
        self.set_ai_state(AIState::Idle);
    }

    /// Refresh legacy aggregate ready/remaining from per-power residual timers.
    pub fn refresh_special_power_aggregate_cooldown(&mut self) {
        let mut max_rem = 0.0_f32;
        self.special_power_cooldowns.retain(|_, r| {
            if *r > max_rem {
                max_rem = *r;
            }
            *r > 0.0
        });
        // Also consider legacy single timer if still non-zero (older save residual).
        if self.special_power_cooldown_remaining > max_rem {
            max_rem = self.special_power_cooldown_remaining;
        }
        self.special_power_cooldown_remaining = max_rem;
        self.special_power_ready = max_rem <= 0.0;
        self.record_host_special_power();
    }

    pub fn apply_upgrade_tag(&mut self, upgrade: &str) {
        if upgrade.is_empty() {
            return;
        }
        if self.applied_upgrades.insert(upgrade.to_string()) {
            if let Some(add) =
                crate::game_logic::host_unit_training::add_xp_scalar_for_upgrade(upgrade)
            {
                self.add_experience_scalar(add);
            }
        }
    }

    /// C++ Object::removeUpgrade residual.
    pub fn remove_upgrade_tag(&mut self, upgrade: &str) -> bool {
        self.applied_upgrades.remove(upgrade)
    }

    pub fn has_upgrade_tag(&self, upgrade: &str) -> bool {
        self.applied_upgrades.contains(upgrade)
    }

    /// C++ Object::hasUpgrade for a completed OBJECT-scoped upgrade tag.
    pub fn has_object_upgrade_complete(&self, upgrade: &str) -> bool {
        self.applied_upgrades
            .iter()
            .any(|name| name.eq_ignore_ascii_case(upgrade))
    }

    /// C++ Object::affectedByUpgrade residual for host objects (no leftover modules).
    /// Leftover UpgradeMux::wouldUpgrade already matches; drone ObjectCreationUpgrade
    /// ConflictsWith is the live name residual (one Scout/Battle/Hellfire per vehicle).
    pub fn affected_by_object_upgrade(&self, upgrade: &str) -> bool {
        crate::game_logic::host_slave_drones::slave_drone_affected_by_upgrade(
            upgrade,
            self.applied_upgrades.iter().map(String::as_str),
        )
    }

    /// C++ ProductionUpdate::queueUpgrade OBJECT: hasUpgrade || !affectedByUpgrade.
    pub fn refuses_object_upgrade(&self, upgrade: &str) -> bool {
        self.has_object_upgrade_complete(upgrade) || !self.affected_by_object_upgrade(upgrade)
    }

    /// Install C++ HighlanderBody residual.
    pub fn install_highlander_body(&mut self) {
        self.highlander_body = true;
    }

    /// Install C++ UpgradeDie residual.
    pub fn install_upgrade_die(&mut self, upgrade_to_remove: impl Into<String>) {
        self.upgrade_die =
            Some(crate::game_logic::host_upgrade_die::HostUpgradeDieData::new(upgrade_to_remove));
    }

    pub fn set_target_location(&mut self, location: Option<Vec3>) {
        self.target_location = location;
        if location.is_some() {
            self.target = None;
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
        } else {
            self.set_status_force_attack(false);
        }
        self.record_host_target_location();
    }

    pub fn set_force_attack(&mut self, force: bool) {
        self.set_status_force_attack(force);
    }

    pub fn stop(&mut self) {
        // Stop all current actions
        self.stop_moving();
        self.stop_attack();
    }

    pub fn set_guard_position(&mut self, position: Option<Vec3>) {
        self.guard_position = position;
        // A fresh position guard is not a polygon area unless a caller stamps
        // `guard_area_trigger` after this (TEAM_GUARD_AREA / execute_guard_area).
        self.guard_area_trigger = None;
        if position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        }
        self.record_host_guard();
    }

    pub fn set_guard_mode(&mut self, mode: GuardMode) {
        self.guard_mode = mode;
        self.record_host_guard();
    }

    pub fn set_guard_target(&mut self, target: Option<ObjectId>) {
        self.guard_target = target;
        self.guard_area_trigger = None;
        if target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        }
        self.record_host_guard();
    }

    /// Drop AIGuard inner/outer/aggressor chase exits (player order or return).
    pub fn clear_guard_chase(&mut self) {
        self.guard_chase_phase = 0;
        self.guard_chase_give_up_frame = 0;
    }

    /// C++ AIUpdateInterface::privateGuardRetaliate residual.
    ///
    /// Clears current goal, anchors at `pos` (unit position if None), sets
    /// goal victim, enters GuardRetaliating, optional max shots.

    /// C++ AIUpdateInterface::notifyCrate residual.
    pub fn notify_crate(&mut self, crate_id: ObjectId) {
        self.crate_created = Some(crate_id);
        self.record_host_ai_request();
    }

    /// C++ AIUpdateInterface::checkForCrateToPickup residual.
    ///
    /// Saves id, clears marker (C++ clears before lookup — host saves first so
    /// the crate can actually be found), returns crate id if still pending.
    pub fn check_for_crate_to_pickup(&mut self) -> Option<ObjectId> {
        let id = self.crate_created.take()?;
        Some(id)
    }

    pub fn begin_guard_retaliate(
        &mut self,
        victim: ObjectId,
        anchor: Option<glam::Vec3>,
        max_shots: Option<i32>,
    ) {
        if !self.is_alive() || self.status.destroyed {
            return;
        }
        if self.is_kind_of(KindOf::Immobile) || self.is_kind_of(KindOf::Structure) {
            return;
        }
        let anchor_pos = anchor.unwrap_or_else(|| self.get_position());
        self.guard_retaliate_victim = Some(victim);
        self.record_host_ai_request();
        self.guard_retaliate_anchor = Some(anchor_pos);
        // Preserve ordinary guard anchors if already guarding.
        if self.guard_position.is_none() && self.guard_target.is_none() {
            self.guard_position = Some(anchor_pos);
        }
        self.target = Some(victim);
        self.target_location = None;
        self.set_ai_state(AIState::GuardRetaliating);
        // C++ AttackAggressor onEnter: chase timer + radius/owner leash.
        // Give-up frame is stamped on the first GuardRetaliate tick (needs GameLogic.frame).
        self.guard_chase_phase = crate::game_logic::GUARD_CHASE_PHASE_RETALIATE;
        self.guard_chase_give_up_frame = 0;
        // C++ JetAIUpdate::aiDoCommand GUARD_RETALIATE sets
        // ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD.
        self.mark_jet_command_for_reload_interrupt(true);
        self.status.attacking = true;
        if let Some(max) = max_shots {
            self.max_shots_to_fire = max;
            self.record_host_combat_attack();
        }
        crate::game_logic::host_attack_log::record(self.id, Some(victim));
        self.record_host_guard();
    }

    /// Clear GuardRetaliate residual and return to guard/idle.
    pub fn end_guard_retaliate(&mut self) {
        self.guard_retaliate_victim = None;
        self.guard_retaliate_anchor = None;
        self.target = None;
        self.status.attacking = false;
        if self.guard_target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        } else if self.guard_position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        } else {
            self.set_ai_state(AIState::Idle);
        }
        crate::game_logic::host_attack_log::record(self.id, None);
    }

    /// Tick GuardRetaliate: drop when victim gone; return toward anchor if far.
    ///
    /// C++ AIGuardRetaliate RETURN stays inside the machine until idle finds
    /// nothing. Do not `move_to` (that flips to Moving and abandons Guard).
    pub fn tick_guard_retaliate(&mut self, victim_alive: bool, victim_pos: Option<glam::Vec3>) {
        if !matches!(self.ai_state, AIState::GuardRetaliating) {
            return;
        }
        if !victim_alive || self.guard_retaliate_victim.is_none() {
            if let Some(anchor) = self.guard_retaliate_anchor {
                let us = self.get_position();
                let dx = us.x - anchor.x;
                let dz = us.z - anchor.z;
                // CLOSE_ENOUGH = 25 residual
                if dx * dx + dz * dz > 25.0 * 25.0 && self.can_move() {
                    self.movement.target_position = Some(anchor);
                    self.set_status_moving(true);
                    self.target = None;
                    self.status.attacking = false;
                    crate::game_logic::host_move_log::record(
                        self.id,
                        Some([anchor.x, anchor.y, anchor.z]),
                    );
                    return;
                }
            }
            self.end_guard_retaliate();
            return;
        }
        // Keep target locked on victim.
        if let Some(vid) = self.guard_retaliate_victim {
            if self.target != Some(vid) {
                self.target = Some(vid);
                self.status.attacking = true;
            }
        }
        let _ = victim_pos;
    }

    pub fn can_repair(&self) -> bool {
        // C++ ActionManager::canRepairObject: only KINDOF_DOZER can repair.
        self.can_move() && self.is_kind_of(KindOf::Dozer)
    }

    pub fn can_construct(&self) -> bool {
        // C++ ActionManager::canResumeConstructionOf: only KINDOF_DOZER.
        self.can_move() && self.is_kind_of(KindOf::Dozer)
    }

    pub fn is_railed_transport(&self) -> bool {
        self.thing.template.dock_kind == crate::game_logic::DockKind::RailedTransport
            || self.thing.template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RailedTransport
    }

    pub fn can_contain(&self) -> bool {
        if !self.is_alive() {
            return false;
        }
        // C++ RiderChangeContain refuses a new rider after onRemoving starts
        // the delayed scuttle.  The object may still be alive for those
        // frames, so ordinary alive/capacity checks alone are insufficient.
        if self.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::RiderChange
            && self.rider_change_scuttled_on_frame != 0
        {
            return false;
        }
        // China Overlord residual: only containable once BattleBunker residual
        // capacity is installed (Some(n>0)). Without bunker (Some(0)) reject.
        if self.is_overlord_style_container() {
            return self.overlord_bunker_slot_capacity() > 0;
        }
        // GLA Tunnel Network residual: TunnelContain entrance (shared team pool).
        if self.is_tunnel_network_style_container()
            || self.thing.template.contain_module.kind.is_tunnel_contain()
            || self.thing.template.contain_module.kind.is_cave_contain()
        {
            return self.is_kind_of(KindOf::Structure);
        }
        // C++ HealContain: barracks/hospital OpenContain, not a garrison.
        if self.thing.template.contain_module.kind.is_heal_contain() {
            return self.thing.template.contain_module.slots.unwrap_or(0) > 0
                || self
                    .building_data
                    .as_ref()
                    .is_some_and(|b| b.max_garrison > 0);
        }
        // `InternetHackContain` is a real structure-side transport
        // interface.  Its exact admission/controller checks remain in the
        // normal Enter authority path; a generic structure cannot borrow it.
        if self.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::InternetHack
        {
            return self.max_transport > 0;
        }
        // `RailedTransportContain` is not a generic vehicle transport in
        // retail (the AutoFerry, for example, is KINDOF_TRANSPORT).  Its
        // explicit Slots field is nevertheless a real containment interface
        // once the separate RailedTransportDockUpdate has accepted a dock.
        if self.is_railed_transport() {
            return self.thing.template.railed_transport_slots.is_some();
        }
        // C++ `canEnterObject` asks for a real ContainModuleInterface.  A
        // vehicle's footprint or selection radius cannot fabricate one.  The
        // parsed module path covers arbitrary retail TransportContain and
        // RiderChangeContain objects; the explicit flags below cover the host
        // implementations that retain their own specialized state.
        if self
            .thing
            .template
            .contain_module
            .kind
            .is_mobile_container()
        {
            return self.max_transport > 0;
        }
        if self.is_helix_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_combat_cycle_transport
            || self.is_humvee_transport
            || self.is_troop_crawler_transport
            || self.is_combat_chinook_transport
            || self.is_listening_outpost_transport
            || self.chinook_ai.is_some()
        {
            return self.max_transport > 0;
        }
        // Vehicle residual transport: any KINDOF_VEHICLE is a real container
        // (host residual; C++ TransportContain admission still applies through
        // has_capacity_for / Enter slot weighting).  Authored zero-capacity
        // modules above already returned; this covers hand-built fixtures and
        // late bootstrap where no named installer ran.
        if self.is_kind_of(KindOf::Vehicle) {
            return true;
        }

        // Structures: only garrisonable buildings with residual capacity > 0.
        // Fail-closed: faction producers / non-bunker structures reject Enter.
        if self.is_kind_of(KindOf::Structure) {
            if self.is_garrison_contain() && !self.garrison_container_accepts_entry() {
                return false;
            }
            return self
                .building_data
                .as_ref()
                .map(|b| b.max_garrison > 0)
                .unwrap_or(false);
        }
        false
    }

    /// C++ GarrisonContain vs Heal/Tunnel/transport structures.
    pub fn is_garrison_contain(&self) -> bool {
        self.thing.template.contain_module.kind == crate::game_logic::ContainModuleKind::Garrison
            || (self.is_kind_of(KindOf::Structure)
                && self
                    .building_data
                    .as_ref()
                    .is_some_and(|b| b.max_garrison > 0)
                && !self.thing.template.contain_module.kind.is_heal_contain()
                && !self.thing.template.contain_module.kind.is_tunnel_contain()
                && !self.thing.template.contain_module.kind.is_cave_contain())
    }

    /// C++ GarrisonContain::isImmuneToClearBuildingAttacks.
    pub fn is_immune_to_clear_building_attacks(&self) -> bool {
        if self.thing.template.contain_module.kind == crate::game_logic::ContainModuleKind::Garrison
        {
            return self
                .thing
                .template
                .contain_module
                .immune_to_clear_building_attacks;
        }
        // C++ OpenContain default is true; only garrisonable modules are cleared.
        true
    }

    /// C++ GarrisonContainModuleData::m_isEnclosingContainer (default true).
    pub fn is_enclosing_garrison_container(&self) -> bool {
        if !self.is_garrison_contain() {
            return true;
        }
        self.thing.template.contain_module.is_enclosing_container
    }

    /// C++ `ContainModuleInterface::isEnclosingContainerFor`.
    /// OpenContain default TRUE. Exceptions: Parachute always FALSE,
    /// Overlord first passenger / portable bunker FALSE, Helix portable
    /// FALSE, Garrison reads `IsEnclosingContainer` (Fire Base No).
    pub fn is_enclosing_container_for(&self, victim: &Object) -> bool {
        let name = self.template_name.to_ascii_lowercase();
        if self.paradrop_parachute || name.contains("parachute") {
            return false;
        }
        if self.is_overlord_style_container() {
            if self.overlord_portable_occupant == Some(victim.id) {
                return false;
            }
            if self
                .contained_units()
                .first()
                .is_some_and(|&id| id == victim.id)
            {
                return false;
            }
            if crate::game_logic::host_battlemaster::is_portable_structure_template(
                &victim.template_name,
            ) {
                return false;
            }
        }
        if self.is_helix_transport
            && crate::game_logic::host_battlemaster::is_portable_structure_template(
                &victim.template_name,
            )
        {
            return false;
        }
        if self.is_garrison_contain() {
            return self.is_enclosing_garrison_container();
        }
        true
    }

    /// C++ GarrisonContain::onContaining / onRemoving OBJECT_STATUS_CAN_ATTACK.
    pub fn set_garrison_can_attack(&mut self, enabled: bool) {
        if enabled {
            let _ = self.apply_status_bits_upgrade_masks(&["CAN_ATTACK"], &[]);
        } else {
            let _ = self.apply_status_bits_upgrade_masks(&[], &["CAN_ATTACK"]);
        }
    }

    pub fn garrison_evac_disposition(&self) -> u8 {
        self.building_data
            .as_ref()
            .map(|b| b.evac_disposition)
            .filter(|&d| d > 0)
            .unwrap_or(3)
    }

    pub fn set_garrison_evac_disposition(&mut self, disposition: u8) {
        if let Some(bd) = self.building_data.as_mut() {
            bd.evac_disposition = disposition;
        }
    }

    /// C++ GarrisonContain::isValidContainerFor health / ReallyDamaged gates.
    pub fn garrison_container_accepts_entry(&self) -> bool {
        if self.health.current <= 0.0 {
            return false;
        }
        if self.body_damage_state
            == crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
            && !self.is_kind_of(KindOf::GarrisonableUntilDestroyed)
        {
            return false;
        }
        true
    }

    /// C++ GarrisonContain::onContaining setTeam + hide-from-nonallies seed.
    /// Hide uses `KINDOF_STEALTH_GARRISON`, not the current STEALTHED bit.
    pub fn note_garrison_occupant_entered(
        &mut self,
        occupant_team: Team,
        occupant_owner: Option<u32>,
        occupant_stealth_garrison: bool,
        occupant_detected: bool,
    ) {
        if let Some(bd) = self.building_data.as_mut() {
            if bd.original_team.is_none() {
                bd.original_team = Some(self.team);
            }
            bd.hide_garrisoned_state = occupant_stealth_garrison && !occupant_detected;
        }
        self.set_team_and_owner(occupant_team, occupant_owner);
    }

    /// C++ GarrisonContain::onRemoving last-occupant original-team restore.
    pub fn restore_garrison_original_team_if_empty(&mut self) {
        let empty = self
            .building_data
            .as_ref()
            .map(|b| b.garrisoned_units.is_empty())
            .unwrap_or(true);
        if !empty {
            return;
        }
        let orig = self.building_data.as_ref().and_then(|b| b.original_team);
        if let Some(bd) = self.building_data.as_mut() {
            bd.original_team = None;
            bd.hide_garrisoned_state = false;
            bd.garrison_guns.clear();
            bd.garrison_point_occupant.clear();
        }
        self.set_garrison_can_attack(false);
        if let Some(orig) = orig {
            self.set_team(orig);
        }
    }

    /// C++ `Object::getTransportSlotCount` subset.  The raw Object INI value
    /// is zero by default in C++; missing metadata therefore must not let a
    /// source unit enter a capacity-checked normal transport.
    #[inline]
    pub fn transport_slot_count(&self) -> usize {
        self.thing.template.transport_slot_count.unwrap_or(0)
    }

    /// Frozen/typed `AllowInsideKindOf` decision for normal player Enter.
    /// Specialized host transports are kept explicit rather than inferred from
    /// their template spelling; parsed source data takes precedence whenever a
    /// concrete module was retained.
    pub fn normal_enter_admission(&self) -> crate::game_logic::ContainAdmission {
        use crate::game_logic::{ContainAdmission, ContainModuleKind};

        if self.is_tunnel_network_style_container() {
            return ContainAdmission::InfantryOrVehicle;
        }
        if self.is_overlord_style_container() && self.overlord_bunker_slot_capacity() > 0 {
            return ContainAdmission::InfantryOnly;
        }
        // `RiderChangeContain` is not an ordinary one-seat transport.  It is
        // admitted only when the exact parsed roster and its bounded Combat
        // Cycle transaction are available; an old name-based Combat Cycle
        // marker alone remains deliberately fail-closed.
        if self.thing.template.contain_module.kind == ContainModuleKind::RiderChange {
            return if self.supports_authored_rider_change_normal_enter() {
                self.thing.template.contain_module.admission
            } else {
                ContainAdmission::Unsupported
            };
        }
        if self.is_combat_cycle_style_container() {
            return ContainAdmission::Unsupported;
        }
        if self.thing.template.contain_module.kind != ContainModuleKind::None {
            return self.thing.template.contain_module.admission;
        }
        if self.is_humvee_style_container()
            || self.is_battle_bus_style_container()
            || self.is_technical_style_container()
            || self.is_combat_cycle_style_container()
            || self.is_listening_outpost_style_container()
            || self.is_troop_crawler_style_container()
        {
            return ContainAdmission::InfantryOnly;
        }
        if self.is_combat_chinook_style_container() || self.is_helix_transport {
            return ContainAdmission::InfantryOrVehicle;
        }
        ContainAdmission::Unsupported
    }

    #[inline]
    pub fn supports_normal_enter(&self) -> bool {
        self.normal_enter_admission() != crate::game_logic::ContainAdmission::Unsupported
    }

    #[inline]
    pub fn normal_enter_requires_infantry(&self) -> bool {
        self.normal_enter_admission() == crate::game_logic::ContainAdmission::InfantryOnly
    }

    #[inline]
    pub fn normal_enter_forbids_aircraft(&self) -> bool {
        matches!(
            self.normal_enter_admission(),
            crate::game_logic::ContainAdmission::InfantryOnly
                | crate::game_logic::ContainAdmission::InfantryOrVehicle
        )
    }

    /// `TransportContain::isValidContainerFor` is stricter than
    /// `OpenContain`: an ordinary transport, Helix, or railed transport only
    /// accepts a rider controlled by the *same player*.  This is deliberately
    /// not an alliance/faction test: two same-faction skirmish slots do not
    /// share seats.  GarrisonContain and TunnelContain retain their separate
    /// C++ rules.
    #[inline]
    pub fn normal_enter_requires_exact_controller(&self) -> bool {
        use crate::game_logic::ContainModuleKind;

        if self.is_tunnel_network_style_container() {
            return false;
        }
        if self.is_overlord_style_container() && self.overlord_bunker_slot_capacity() > 0 {
            return true;
        }
        match self.thing.template.contain_module.kind {
            ContainModuleKind::Transport
            | ContainModuleKind::RiderChange
            | ContainModuleKind::RailedTransport
            | ContainModuleKind::InternetHack => true,
            ContainModuleKind::Garrison
            | ContainModuleKind::Heal
            | ContainModuleKind::Cave
            | ContainModuleKind::Tunnel => false,
            ContainModuleKind::None => {
                self.is_helix_transport
                    || self.is_battle_bus_transport
                    || self.is_technical_transport
                    || self.is_combat_cycle_transport
                    || self.is_humvee_transport
                    || self.is_troop_crawler_transport
                    || self.is_combat_chinook_transport
                    || self.is_listening_outpost_transport
            }
        }
    }

    /// Whether normal Enter capacity is measured in the rider's authored
    /// `TransportSlotCount` rather than one contained body.  C++
    /// TransportContain maintains `m_extraSlotsInUse`; GarrisonContain and
    /// TunnelTracker instead count contained objects.
    #[inline]
    pub fn normal_enter_uses_transport_slots(&self) -> bool {
        use crate::game_logic::ContainModuleKind;

        // RiderChange replaces its sole payload atomically and explicitly
        // ignores TransportContain capacity, matching C++ isValidContainerFor
        // with CHECK_CAPACITY=false.
        if self.thing.template.contain_module.kind == ContainModuleKind::RiderChange {
            return false;
        }
        if self.thing.template.contain_module.kind == ContainModuleKind::InternetHack {
            return true;
        }
        if self.is_tunnel_network_style_container()
            || self.is_kind_of(KindOf::Structure)
            || self.thing.template.contain_module.kind == ContainModuleKind::Garrison
        {
            return false;
        }
        matches!(
            self.thing.template.contain_module.kind,
            ContainModuleKind::Transport | ContainModuleKind::RailedTransport
        ) || self.is_overlord_style_container()
            || self.is_helix_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_combat_cycle_transport
            || self.is_humvee_transport
            || self.is_troop_crawler_transport
            || self.is_combat_chinook_transport
            || self.is_listening_outpost_transport
    }

    /// C++ OpenContain relationship gates.  Without retained module metadata
    /// the specialized host containers use C++'s all-true OpenContain defaults.
    pub fn allows_normal_enter_from_team(&self, source_team: Team) -> bool {
        let relationship = if source_team == self.team {
            gamelogic::common::Relationship::Allies
        } else if source_team == Team::Neutral || self.team == Team::Neutral {
            gamelogic::common::Relationship::Neutral
        } else {
            gamelogic::common::Relationship::Enemies
        };
        self.allows_normal_enter_for_relationship(relationship)
    }

    /// C++ OpenContain relationship gate with ownership-aware relationship
    /// resolution supplied by GameLogic.  Keep the old team-only method above
    /// solely for genuinely legacy callers; normal Enter must call this form.
    pub fn allows_normal_enter_for_relationship(
        &self,
        relationship: gamelogic::common::Relationship,
    ) -> bool {
        let metadata = &self.thing.template.contain_module;
        if metadata.kind == crate::game_logic::ContainModuleKind::None {
            return true;
        }
        match relationship {
            gamelogic::common::Relationship::Allies => metadata.allow_allies_inside,
            gamelogic::common::Relationship::Enemies => metadata.allow_enemies_inside,
            gamelogic::common::Relationship::Neutral => metadata.allow_neutral_inside,
        }
    }

    pub fn has_capacity_for(&self, count: usize) -> bool {
        // InternetHackContain is a structure, but its authored `Slots` are
        // transport slots rather than GarrisonContain bodies.  Its normal
        // Enter authority already computes weighted slot availability; this
        // arrival-side guard must not reinterpret it as `max_garrison`.
        if self.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::InternetHack
        {
            let cap = self.transport_capacity();
            return cap > 0 && self.contained_units().len().saturating_add(count) <= cap;
        }
        if let Some(building) = &self.building_data {
            if building.max_garrison == 0 {
                return false;
            }
            building.garrisoned_units.len() + count <= building.max_garrison
        } else if self.is_kind_of(KindOf::Vehicle)
            // C++ TransportContain::getContainMax (TransportContain.cpp:105)
            // is slot-based, not KindOf-based: aircraft transports such as
            // the Combat Chinook (TransportContain Slots=8, KINDOF_AIRCRAFT)
            // admit riders through the same m_slotCapacity arithmetic
            // (TransportContain.cpp:183-186) as ground vehicles.
            || self.is_kind_of(KindOf::Aircraft)
            || self.thing.template.dock_kind == crate::game_logic::DockKind::RailedTransport
        {
            let cap = self.transport_capacity();
            if cap == 0 {
                return false;
            }
            self.occupants.len() + count <= cap
        } else {
            false
        }
    }

    /// Residual garrison capacity (structures only). 0 = not garrisonable.
    pub fn garrison_capacity(&self) -> usize {
        self.building_data
            .as_ref()
            .map(|b| b.max_garrison)
            .unwrap_or(0)
    }

    /// True when this vehicle uses OverlordContain residual semantics
    /// (`overlord_bunker_capacity` is `Some(...)`).
    pub fn is_overlord_style_container(&self) -> bool {
        self.overlord_bunker_capacity.is_some()
    }

    /// Residual BattleBunker infantry slots on an Overlord-style vehicle.
    /// `0` when not overlord-style or bunker residual not installed.
    pub fn overlord_bunker_slot_capacity(&self) -> usize {
        self.overlord_bunker_capacity.unwrap_or(0)
    }

    /// Install residual BattleBunker capacity (C++ OCL_OverlordBattleBunker →
    /// ChinaTankOverlordBattleBunker TransportContain Slots=5).
    /// Fail-closed: does not spawn a real portable-structure passenger object.
    /// Conflicts residual: clears gattling/propaganda addons (exclusive payload).
    pub fn install_overlord_battle_bunker(&mut self, slots: usize) {
        self.overlord_bunker_capacity = Some(slots);
        // Exclusive ConflictsWith residual (not Emperor innate propaganda).
        let emperor =
            crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name);
        self.has_overlord_gattling_addon = false;
        if !emperor {
            self.has_overlord_propaganda_addon = false;
        }
        self.record_host_overlord();
        self.sync_overlord_addon_body_damage();
    }

    /// Install residual portable GattlingCannon addon
    /// (C++ OCL_OverlordGattlingCannon / OCL_HelixGattlingCannon).
    /// Equips AA secondary + passenger ground residual on primary fires.
    /// Fail-closed: not full portable-structure passenger object.
    pub fn install_overlord_gattling_addon(&mut self) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            is_emperor_template, overlord_gattling_air_weapon,
        };
        // Exclusive ConflictsWith residual vs bunker / propaganda (except Emperor).
        let emperor = is_emperor_template(&self.template_name);
        if !emperor {
            self.has_overlord_propaganda_addon = false;
            // Keep overlord-style marker but zero bunker slots.
            if self.overlord_bunker_capacity.is_some() {
                self.overlord_bunker_capacity = Some(0);
            }
        }
        self.has_overlord_gattling_addon = true;
        let _ = self.set_weapon_set_flag(0, true);
        let chain = has_chain_guns_upgrade(&self.applied_upgrades);
        let _ = self.replace_weapon_set_slot(1, Some(overlord_gattling_air_weapon(0, chain)));
        self.continuous_fire_consecutive = 0;
        self.continuous_fire_level = 0;
        self.continuous_fire_coast_until_frame = 0;
        self.continuous_fire_victim = 0;
        self.record_host_combat_attack();
        self.record_host_continuous_fire();
        self.record_host_weapon_set();
        self.record_host_overlord();
        self.sync_overlord_addon_body_damage();
    }

    /// Install residual portable PropagandaTower addon
    /// (C++ OCL_OverlordPropagandaTower / OCL_HelixPropagandaTower).
    /// Fail-closed: not full portable tower object / PulseFX.
    pub fn install_overlord_propaganda_addon(&mut self) {
        // Exclusive ConflictsWith residual vs gattling / bunker.
        self.has_overlord_gattling_addon = false;
        if self.overlord_bunker_capacity.is_some() {
            self.overlord_bunker_capacity = Some(0);
        }
        self.has_overlord_propaganda_addon = true;
        self.record_host_overlord();
        self.sync_overlord_addon_body_damage();
    }

    /// Install residual HelixContain transport (Slots=5).
    /// C++ OpenContain default `m_passengersAllowedToFire = FALSE`.
    /// `PassengersFireUpgrade` (Battle Bunker) sets the flag later.
    /// HelixContain still applies WEAPONBONUSCONDITION_GARRISONED on enter.
    pub fn install_helix_transport(&mut self) {
        self.is_helix_transport = true;
        self.max_transport = crate::game_logic::host_overlord_addons::HELIX_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        // Helix can hold infantry / vehicle / portable structure residual.
        // Fail-closed: allow_inside matrix simplified to transport capacity.
        self.record_host_contain_capacity();
        self.record_host_overlord();
        self.record_host_stealth_flags();
    }

    pub fn has_portable_overlord_addon(&self) -> bool {
        crate::game_logic::host_overlord_addon_damage::portable_addon_installed(
            self.has_overlord_gattling_addon,
            self.has_overlord_propaganda_addon,
            self.overlord_bunker_slot_capacity(),
            crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name),
        )
    }

    pub fn sync_overlord_addon_body_damage(&mut self) {
        if !self.has_portable_overlord_addon() {
            self.overlord_addon_body_damage_state =
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine;
            return;
        }
        if let Some(state) =
            crate::game_logic::host_overlord_addon_damage::overlord_addon_mirrored_damage_state(
                self.body_damage_state,
            )
        {
            self.overlord_addon_body_damage_state = state;
        }
    }

    pub fn apply_overlord_addon_set_damage_state(
        &mut self,
        new_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) {
        if matches!(
            new_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
        ) {
            return;
        }
        self.health.current =
            crate::game_logic::host_overlord_addon_damage::overlord_addon_set_damage_state_health(
                self.health.maximum.max(0.0),
                new_state,
            );
        self.refresh_model_condition_bits();
    }

    /// True when portable gattling residual is active on this host.
    pub fn has_overlord_gattling_residual(&self) -> bool {
        self.has_overlord_gattling_addon
    }

    /// True when portable / innate propaganda residual is active on this host.
    pub fn has_overlord_propaganda_residual(&self) -> bool {
        self.has_overlord_propaganda_addon
            || crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name)
    }

    /// Install residual GLA Battle Bus transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-door exit / SlowDeath undeath SECOND_LIFE.
    pub fn install_battle_bus_transport(&mut self) {
        self.is_battle_bus_transport = true;
        self.max_transport = crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        self.thing
            .template
            .contain_module
            .weapon_bonus_passed_to_passengers =
            crate::game_logic::host_battle_bus::BATTLE_BUS_WEAPON_BONUS_PASSED_TO_PASSENGERS;
        if self.battle_bus_body.is_none() {
            self.battle_bus_body =
                Some(crate::game_logic::host_battle_bus::HostBattleBusBodyData::new());
        }
        // First-life max health residual (UndeadBody / ActiveBody).
        if self.health.maximum < crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH {
            self.health.maximum = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
            self.health.current = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
        }
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a Battle Bus residual transport.
    pub fn is_battle_bus_style_container(&self) -> bool {
        self.is_battle_bus_transport
    }

    /// C++ UndeadBody::startSecondLife + BattleBus first-death begin residual.
    pub fn start_battle_bus_second_life(&mut self) {
        use crate::game_logic::host_battle_bus::{
            BATTLE_BUS_MC_BIT_SECOND_LIFE, BATTLE_BUS_SECOND_LIFE_MAX_HEALTH,
            BATTLE_BUS_THROW_FORCE, HostBattleBusBodyData, battle_bus_start_undeath_fx_name,
        };
        let frame = crate::game_logic::host_historic_bonus::logic_frame();
        let body = self
            .battle_bus_body
            .get_or_insert_with(HostBattleBusBodyData::new);
        if body.is_second_life && !body.is_in_first_death {
            // Already converted.
            return;
        }
        body.begin_first_life_undeath(frame);
        self.health.maximum = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.health.current = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.armor_set_second_life = true;
        self.status.destroyed = false;
        self.status.effectively_dead = false;
        // C++ applyShock throwForce.z (up). Host is Y-up, so +Y.
        // scrubVelocity2D then throw — do not stop_moving (that zeroes the hop).
        let _ = self.apply_shock_wave_impulse(glam::Vec3::new(0.0, BATTLE_BUS_THROW_FORCE, 0.0));
        self.apply_shock_random_rotation(frame);
        self.movement.velocity.x = 0.0;
        self.movement.velocity.z = 0.0;
        self.movement.target_position = None;
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.status.attacking = false;
        let _ = BATTLE_BUS_MC_BIT_SECOND_LIFE; // set on land
        self.record_host_weapon_set();
        // Leftover `execute_fx_at_object_id` / C++ `FXList::doFXObj(m_fxStartUndeath, me)`.
        let fx = battle_bus_start_undeath_fx_name(&self.template_name);
        crate::game_logic::publish_host_fx_object(
            self.id.0,
            self.get_position(),
            self.get_orientation(),
            self.owner_player_id.map(|p| p as i32).unwrap_or(-1),
        );
        let _ = crate::game_logic::dispatch_fx_list_at_object(&fx, self.id.0, None);
    }

    /// Tick BattleBusSlowDeath first-death air time + empty hulk arming.
    /// Returns (landed_this_tick, empty_hulk_kill).
    pub fn tick_battle_bus_slow_death(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
    ) -> (bool, bool) {
        use crate::game_logic::host_battle_bus::{
            BATTLE_BUS_MC_BIT_SECOND_LIFE, battle_bus_hit_ground_fx_name,
        };
        if self.battle_bus_body.is_none() {
            return (false, false);
        }
        // Integrate residual throw height (host world-Y up).
        let (in_first, throw_vz) = self
            .battle_bus_body
            .as_ref()
            .map(|b| (b.is_in_first_death, b.throw_vz))
            .unwrap_or((false, 0.0));
        if in_first && throw_vz.abs() > 0.001 {
            let pos = self.get_position();
            let ground = self.ground_height;
            let mut y = pos.y + throw_vz;
            let mut new_vz = throw_vz - 0.5; // residual gravity peel
            if new_vz < 0.0 && y <= ground {
                y = ground;
                new_vz = 0.0;
            }
            self.set_position(glam::Vec3::new(pos.x, y.max(ground), pos.z));
            if let Some(body) = self.battle_bus_body.as_mut() {
                body.throw_vz = new_vz;
            }
        }
        let above = self.get_position().y - self.ground_height > 0.5;
        let landed = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.try_land_first_death(current_frame, above))
            .unwrap_or(false);
        if landed {
            // C++ setModelConditionState(MODELCONDITION_SECOND_LIFE) + DISABLED_HELD.
            self.model_condition_bits |= 1u128 << BATTLE_BUS_MC_BIT_SECOND_LIFE;
            self.set_status_disabled_held(true);
            self.stop_moving();
            self.set_ai_state(AIState::Idle);
            self.refresh_model_condition_bits();
            // Leftover `finish_first_death` / C++ `FXList::doFXObj(m_fxHitGround, me)`.
            let fx = battle_bus_hit_ground_fx_name(&self.template_name);
            crate::game_logic::publish_host_fx_object(
                self.id.0,
                self.get_position(),
                self.get_orientation(),
                self.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            );
            let _ = crate::game_logic::dispatch_fx_list_at_object(&fx, self.id.0, None);
        }
        let empty_kill = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.tick_empty_hulk(passenger_count, current_frame))
            .unwrap_or(false);
        (landed, empty_kill)
    }

    /// True when UndeadBody should intercept a lethal hit (first life only).
    /// `raw_amount` is C++ `DamageInfo.in.m_amount` (PRE-armor).
    pub fn battle_bus_should_intercept_lethal(
        &self,
        damage_type: crate::game_logic::combat::DamageType,
        raw_amount: f32,
    ) -> bool {
        if !self.is_battle_bus_transport {
            return false;
        }
        // C++ UndeadBody.cpp:58-62 — only DAMAGE_UNRESISTABLE and
        // !IsHealthDamagingDamage (Damage.h:110-127) skip second-life
        // intercept. DAMAGE_PENALTY is ordinary HP and must trigger it.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) || !damage_type.is_health_damaging()
        {
            return false;
        }
        let second = self
            .battle_bus_body
            .as_ref()
            .map(|b| b.is_second_life)
            .unwrap_or(false);
        !second && raw_amount >= self.health.current && self.health.current > 0.0
    }

    /// Install residual GLA Tunnel Network structure:
    /// C++ TunnelContain shared MaxTunnelCapacity=10 per player.
    /// Fail-closed: not GuardTunnelNetwork AI / TimeForFullHeal / CaveSystem.
    pub fn install_tunnel_network_residual(&mut self) {
        self.is_tunnel_network = true;
        if let Some(bd) = self.building_data.as_mut() {
            // Local max is the shared pool cap; GameLogic enforces per-player count.
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
        } else {
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
            self.building_data = Some(bd);
            self.record_host_building_type();
        }
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// C++ `getControllingPlayer()->getTunnelSystem()` key.
    #[inline]
    pub fn tunnel_system_key(&self) -> u32 {
        crate::game_logic::host_tunnel_network::tunnel_system_key(self.owner_player_id, self.team)
    }

    /// True when this structure is a GLA Tunnel Network residual entrance.
    pub fn is_tunnel_network_style_container(&self) -> bool {
        self.is_tunnel_network || self.thing.template.contain_module.kind.is_tunnel_contain()
    }

    /// C++ CaveContain: CaveIndex-shared CaveSystem tracker.
    pub fn install_cave_contain_residual(&mut self, cave_index: i32) {
        self.is_cave_contain = true;
        self.cave_index = cave_index;
        if let Some(bd) = self.building_data.as_mut() {
            bd.max_garrison = crate::game_logic::host_cave_system::MAX_CAVE_CAPACITY;
        } else {
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = crate::game_logic::host_cave_system::MAX_CAVE_CAPACITY;
            self.building_data = Some(bd);
            self.record_host_building_type();
        }
        self.record_host_contain_capacity();
    }

    pub fn is_cave_style_container(&self) -> bool {
        self.is_cave_contain
            || self.thing.template.contain_module.kind.is_cave_contain()
            || crate::game_logic::host_cave_system::is_cave_template(&self.template_name)
    }

    /// Install residual GLA Technical transport:
    /// C++ TransportContain Slots=5, AllowInsideKindOf=INFANTRY.
    /// Passengers ride (bed garrison residual) but do **not** fire
    /// (`PassengersAllowedToFire` unset in retail).
    /// Fail-closed: not chassis reskin / W3D gunner matrix.
    pub fn install_technical_transport(&mut self) {
        self.is_technical_transport = true;
        self.max_transport = crate::game_logic::host_technical::TECHNICAL_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a GLA Technical residual transport.
    pub fn is_technical_style_container(&self) -> bool {
        self.is_technical_transport
    }

    /// Install residual GLA Combat Cycle RiderChangeContain:
    /// C++ Slots=1, AllowInsideKindOf=INFANTRY, passengers do not fire
    /// (bike itself switches WeaponSet to rider weapon residual).
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle matrix.
    pub fn install_combat_cycle_transport(&mut self) {
        self.is_combat_cycle_transport = true;
        self.max_transport = crate::game_logic::host_combat_cycle::COMBAT_CYCLE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a GLA Combat Cycle residual transport.
    pub fn is_combat_cycle_style_container(&self) -> bool {
        self.is_combat_cycle_transport
    }

    /// True only for the parsed, bounded RiderChangeContain path.  The
    /// specialized Combat Cycle residual is an *effect* implementation, not
    /// admission evidence: the complete Object INI roster is sufficient here;
    /// no template basename/legacy transport flag is used to enable Enter.
    #[inline]
    pub fn supports_authored_rider_change_normal_enter(&self) -> bool {
        self.rider_change_scuttled_on_frame == 0
            && self
                .thing
                .template
                .contain_module
                .has_supported_rider_change_roster()
    }

    /// Exact, case-insensitive authored RiderN identity lookup.  C++ also
    /// accepts reskin/build-variation equivalence; that source relationship is
    /// not retained in the active ThingTemplate graph, so those variants stay
    /// fail-closed rather than being approximated from their name.
    #[inline]
    pub fn authored_rider_change_rider_for_template(
        &self,
        template_name: &str,
    ) -> Option<&crate::game_logic::RiderChangeRiderMetadata> {
        self.thing
            .template
            .contain_module
            .supported_rider_change_rider_for_template(template_name)
    }

    /// Install residual America Humvee transport:
    /// C++ TransportContain Slots=5, PassengersAllowedToFire=Yes,
    /// AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
    pub fn install_humvee_transport(&mut self) {
        self.is_humvee_transport = true;
        self.max_transport = crate::game_logic::host_humvee::HUMVEE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is an America Humvee residual transport.
    pub fn is_humvee_style_container(&self) -> bool {
        self.is_humvee_transport
    }

    /// Install residual China Troop Crawler transport:
    /// C++ TransportContain Slots=8, AllowInsideKindOf=INFANTRY,
    /// InitialPayload Redguard×8, GoAggressiveOnExit residual (exit-to-fight).
    /// Passengers do **not** fire from inside (`PassengersAllowedToFire` unset).
    /// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
    pub fn install_troop_crawler_transport(&mut self) {
        self.is_troop_crawler_transport = true;
        self.max_transport = crate::game_logic::host_troop_crawler::TROOP_CRAWLER_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a China Troop Crawler residual transport.
    pub fn is_troop_crawler_style_container(&self) -> bool {
        self.is_troop_crawler_transport
    }

    /// Install residual Air Force Combat Chinook transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY VEHICLE.
    /// Fail-closed: leftover dual-world ropes/drawable still require OBJECT_REGISTRY.
    pub fn install_combat_chinook_transport(&mut self) {
        self.is_combat_chinook_transport = true;
        self.max_transport = crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        self.thing
            .template
            .contain_module
            .weapon_bonus_passed_to_passengers = crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_WEAPON_BONUS_PASSED_TO_PASSENGERS;
        {
            use game_engine::common::system::kind_of::KindOfMask;
            let contain = &mut self.thing.template.contain_module;
            contain.allow_inside_kind_of = KindOfMask::INFANTRY.bits() | KindOfMask::VEHICLE.bits();
            contain.forbid_inside_kind_of =
                KindOfMask::AIRCRAFT.bits() | KindOfMask::HUGE_VEHICLE.bits();
        }
        // Combat Chinook KindOf includes CAN_ATTACK residual (vanilla Chinook does not).
        self.thing.template.add_kind_of(KindOf::Attackable);
        let p = self.get_position();
        self.chinook_ai = Some(
            crate::game_logic::host_combat_chinook::HostChinookAI::new_combat([p.x, p.z, p.y]),
        );
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE
        // (ListeningOutpostUpgradedDummyWeapon). Strip kind-based Weapon::default.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// Vanilla AmericaVehicleChinook: TransportContain Slots=8 + ChinookAI, no passenger fire.
    pub fn install_chinook_transport(&mut self) {
        self.max_transport = crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        {
            use game_engine::common::system::kind_of::KindOfMask;
            let contain = &mut self.thing.template.contain_module;
            contain.allow_inside_kind_of = KindOfMask::INFANTRY.bits() | KindOfMask::VEHICLE.bits();
            contain.forbid_inside_kind_of =
                KindOfMask::AIRCRAFT.bits() | KindOfMask::HUGE_VEHICLE.bits();
        }
        let p = self.get_position();
        if self.chinook_ai.is_none() {
            self.chinook_ai = Some(
                crate::game_logic::host_combat_chinook::HostChinookAI::new_vanilla([p.x, p.z, p.y]),
            );
        }
        self.record_host_contain_capacity();
    }

    /// True when this vehicle is an AirF Combat Chinook residual transport.
    pub fn is_combat_chinook_style_container(&self) -> bool {
        self.is_combat_chinook_transport
    }

    /// Install residual China Listening Outpost transport + detect residual:
    /// C++ TransportContain Slots=2, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY,
    /// StealthDetectorUpdate DetectionRange=300, InnateStealth=Yes.
    /// Fail-closed: not multi-door exit / IR FX / RIDERS_ATTACKING uncloak matrix.
    pub fn install_listening_outpost_transport(&mut self) {
        self.is_listening_outpost_transport = true;
        self.max_transport =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        // Detector residual (DetectionRange = 300).
        self.is_detector = true;
        self.detection_range =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;
        // C++ StealthUpdate ctor: InnateStealth sets CAN_STEALTH only, not STEALTHED
        // (StealthUpdate.cpp:110-137). Spawn arms StealthDelay in create_object.
        self.innate_stealth = true;
        self.stealth_breaks_on_move = true;
        // Fire does not break stealth on the vehicle itself (passengers fire residual).
        self.stealth_breaks_on_attack = false;
        self.stealth_delay_frames =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_STEALTH_DELAY_FRAMES;
        self.stealth_delay_pending = false;
        self.record_host_stealth_delay();
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
        // KindOf residual includes CAN_ATTACK (for dummy weapon range residual).
        self.thing.template.add_kind_of(KindOf::Attackable);
        self.record_host_detector();
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a China Listening Outpost residual transport.
    pub fn is_listening_outpost_style_container(&self) -> bool {
        self.is_listening_outpost_transport
    }

    /// C++ TransportContain ExitDelay frames for this hull.
    pub fn transport_exit_delay_frames(&self) -> u32 {
        if self.is_humvee_style_container() {
            crate::game_logic::host_humvee::HUMVEE_EXIT_DELAY_FRAMES
        } else if self.is_battle_bus_style_container() {
            crate::game_logic::host_battle_bus::BATTLE_BUS_EXIT_DELAY_FRAMES
        } else if self.is_troop_crawler_style_container() {
            crate::game_logic::host_troop_crawler::TROOP_CRAWLER_EXIT_DELAY_FRAMES
        } else if self.is_combat_chinook_style_container()
            || self.template_name.to_ascii_lowercase().contains("chinook")
        {
            crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_EXIT_DELAY_FRAMES
        } else if self.is_listening_outpost_style_container() {
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_EXIT_DELAY_FRAMES
        } else {
            0
        }
    }

    /// C++ TransportContain `DelayExitInAir` — Battle Bus only.
    pub fn transport_delay_exit_in_air(&self) -> bool {
        self.is_battle_bus_style_container()
    }

    /// C++ TransportContain `GoAggressiveOnExit`.
    pub fn transport_go_aggressive_on_exit(&self) -> bool {
        self.is_humvee_style_container()
            || self.is_battle_bus_style_container()
            || self.is_troop_crawler_style_container()
            || self.is_combat_chinook_style_container()
            || self.is_listening_outpost_style_container()
            || self.is_technical_style_container()
            || self.template_name.to_ascii_lowercase().contains("chinook")
    }

    /// C++ TransportContain `KeepContainerVelocityOnExit`.
    /// Default FALSE; only the authored store field enables the hull kick.
    pub fn transport_keep_container_velocity_on_exit(&self) -> bool {
        self.thing
            .template
            .contain_module
            .keep_container_velocity_on_exit
    }

    /// C++ OpenContainModuleData::m_numberOfExitPaths.
    pub fn transport_number_of_exit_paths(&self) -> i32 {
        if self.is_humvee_style_container() {
            crate::game_logic::host_humvee::HUMVEE_NUMBER_OF_EXIT_PATHS as i32
        } else if self.is_battle_bus_style_container() {
            crate::game_logic::host_battle_bus::BATTLE_BUS_NUMBER_OF_EXIT_PATHS as i32
        } else if self.is_troop_crawler_style_container() {
            crate::game_logic::host_troop_crawler::TROOP_CRAWLER_NUMBER_OF_EXIT_PATHS as i32
        } else if self.is_listening_outpost_style_container() {
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_NUMBER_OF_EXIT_PATHS as i32
        } else if self.is_combat_chinook_style_container() {
            crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_NUMBER_OF_EXIT_PATHS as i32
        } else {
            1
        }
    }

    /// C++ `Object::isAboveTerrain` residual for DelayExitInAir.
    pub fn is_above_terrain_for_exit(&self) -> bool {
        self.status.airborne_target
            || (self.ground_height_from_terrain
                && self.get_position().y > self.ground_height + 0.01)
            || self.get_position().y > 0.5
    }

    /// C++ TransportContain::isExitBusy.
    pub fn is_transport_exit_busy(&self, frame: u32) -> bool {
        if self.transport_delay_exit_in_air() && self.is_above_terrain_for_exit() {
            return true;
        }
        frame < self.frame_exit_not_busy
    }

    /// TransportContain-style vehicles that honor ExitDelay / DelayExitInAir.
    pub fn uses_transport_contain_exit_busy(&self) -> bool {
        if self.is_garrison_contain()
            || self.is_tunnel_network_style_container()
            || self.is_cave_style_container()
            || self.is_kind_of(KindOf::Structure)
        {
            return false;
        }
        self.can_contain()
            || self.is_humvee_style_container()
            || self.is_battle_bus_style_container()
            || self.is_troop_crawler_style_container()
            || self.is_combat_chinook_style_container()
            || self.is_listening_outpost_style_container()
            || self.is_technical_style_container()
            || self.is_helix_transport
    }

    /// Retained transport capacity.  Capacity comes only from an authored
    /// Contain module or an explicit specialized host transport installation;
    /// never from an object's visual footprint.
    pub fn transport_capacity(&self) -> usize {
        if self.thing.template.contain_module.kind
            == crate::game_logic::ContainModuleKind::InternetHack
        {
            return self.max_transport;
        }
        if self.is_kind_of(KindOf::Structure) {
            return 0;
        }
        let is_railed_transport =
            self.thing.template.dock_kind == crate::game_logic::DockKind::RailedTransport;
        if !self.is_kind_of(KindOf::Vehicle)
            && !self.is_kind_of(KindOf::Aircraft)
            && !is_railed_transport
        {
            return 0;
        }
        // Railed transports must use their authored `Slots`; they do not gain
        // the generic vehicle footprint capacity when a custom object omitted
        // RailedTransportContain.
        if is_railed_transport {
            return self.max_transport;
        }
        // Overlord BattleBunker residual: bunker slots only (0 without bunker).
        if let Some(cap) = self.overlord_bunker_capacity {
            return cap;
        }
        if self
            .thing
            .template
            .contain_module
            .kind
            .is_mobile_container()
            || self.is_helix_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_combat_cycle_transport
            || self.is_humvee_transport
            || self.is_troop_crawler_transport
            || self.is_combat_chinook_transport
            || self.is_listening_outpost_transport
        {
            return self.max_transport;
        }
        // Vehicle residual: a plain KINDOF_VEHICLE with no authored contain
        // module and no named installer keeps a minimal two-slot transport
        // floor (host MoveToAndEvacuate/Exit residual fixtures and late
        // bootstrap).  Authored zero-capacity modules above stay at 0.
        if self.is_kind_of(KindOf::Vehicle) {
            return self.max_transport.max(2);
        }
        0
    }

    /// Current transport occupant count (vehicles only; structures use garrison).
    pub fn transport_count(&self) -> usize {
        if self.is_kind_of(KindOf::Structure) {
            0
        } else {
            self.occupants.len()
        }
    }

    /// Current garrison/transport occupant count.
    pub fn garrison_count(&self) -> usize {
        self.contained_units().len()
    }

    pub fn set_contained_by(&mut self, container: Option<ObjectId>) {
        // Default enclosing=true matches C++ OpenContain::isEnclosingContainerFor.
        self.set_contained_by_enclosing(container, true);
    }

    /// C++ Object::onContainedBy / onRemovedFrom.
    /// Enter always sets UNSELECTABLE; MASKED only when the container encloses
    /// the occupant (Fire Base `IsEnclosingContainer=No` stays visible/targetable).
    /// Exit clears both.
    pub fn set_contained_by_enclosing(&mut self, container: Option<ObjectId>, enclosing: bool) {
        if container.is_none() && self.experience_sink == self.contained_by {
            // C++ never unsinks the portable structure (it never leaves).
            // Infantry exiting the BattleBunker residual must not keep
            // forwarding later kills to a tank they no longer ride.
            self.set_experience_sink(None);
        }
        if container.is_some() {
            self.set_status_unselectable(true);
            self.set_status_masked(enclosing);
        } else {
            self.set_status_unselectable(false);
            self.set_status_masked(false);
        }
        self.contained_by = container;
        crate::game_logic::host_contain_log::record_contained_by(self.id, container);
        // C++ OpenContain::onContaining setDrawableHidden(true) when enclosing.
        if enclosing {
            self.set_drawable_hidden(container.is_some());
        }
    }

    pub fn add_occupant(&mut self, unit_id: ObjectId) -> bool {
        if !self.can_contain() || !self.has_capacity_for(1) {
            return false;
        }
        if let Some(building) = self.building_data.as_mut() {
            if building.garrisoned_units.contains(&unit_id) {
                return true;
            }
            building.garrisoned_units.push(unit_id);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &building.garrisoned_units,
                building.max_garrison.min(u16::MAX as usize) as u16,
            );
            true
        } else {
            if self.occupants.contains(&unit_id) {
                return true;
            }
            self.occupants.push(unit_id);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &self.occupants,
                self.occupants.len().min(u16::MAX as usize) as u16,
            );
            true
        }
    }

    pub fn contained_units(&self) -> Vec<ObjectId> {
        if let Some(building) = &self.building_data {
            building.garrisoned_units.clone()
        } else {
            self.occupants.clone()
        }
    }

    pub fn remove_occupant(&mut self, unit_id: ObjectId) -> bool {
        if let Some(building) = self.building_data.as_mut() {
            if let Some(pos) = building
                .garrisoned_units
                .iter()
                .position(|&id| id == unit_id)
            {
                building.garrisoned_units.remove(pos);
                // C++ GarrisonContain::onRemoving → removeObjectFromGarrisonPoint
                // (or station) for this occupant, not only when the building
                // becomes empty. Survivors must inherit the freed window.
                building.free_garrison_point_for(unit_id);
                crate::game_logic::host_contain_log::record_garrison(
                    self.id,
                    &building.garrisoned_units,
                    building.max_garrison.min(u16::MAX as usize) as u16,
                );
                let empty = building.garrisoned_units.is_empty();
                if empty {
                    self.restore_garrison_original_team_if_empty();
                }
                return true;
            }
        }
        if let Some(pos) = self.occupants.iter().position(|&id| id == unit_id) {
            self.occupants.remove(pos);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &self.occupants,
                self.occupants.len().min(u16::MAX as usize) as u16,
            );
            return true;
        }
        false
    }

    /// Begin containing an occupant (transport/garrison bookkeeping).
    pub fn enter_transport(&mut self, unit_id: ObjectId) -> bool {
        self.add_occupant(unit_id)
    }

    /// Remove an occupant from this transport/garrison.
    pub fn exit_transport(&mut self, unit_id: ObjectId) -> bool {
        self.remove_occupant(unit_id)
    }
}
