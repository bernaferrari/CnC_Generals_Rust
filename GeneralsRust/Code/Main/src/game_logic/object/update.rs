use super::*;

impl Object {
    pub fn tick_timers(&mut self, dt: f32) -> bool {
        if self.cheer_timer > 0.0 {
            self.cheer_timer -= dt;
            if self.cheer_timer <= 0.0 && self.ai_state == AIState::SpecialAbility {
                self.set_ai_state(AIState::Idle);
                self.cheer_timer = 0.0;
                self.record_host_demo_mine_cheer();
            }
        }

        if self.prone_timer > 0.0 {
            self.prone_timer -= dt;
            if self.prone_timer <= 0.0 {
                self.prone_timer = 0.0;
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    self.model_condition_bits &= !(1u128 << bit);
                    // Wave 487: clear prone model bit into GW.
                    self.record_host_model_condition();
                }
            }
        }

        if self.emoticon_frames_left > 0 {
            // dt is seconds; logic is 30Hz — consume fractional frames.
            let frames = (dt * 30.0).max(0.0);
            let next = self.emoticon_frames_left as f32 - frames;
            if next <= 0.0 {
                self.emoticon_frames_left = 0;
                self.emoticon_name.clear();
            } else {
                self.emoticon_frames_left = next.ceil() as i32;
            }
        }

        let was_ready = self.special_power_ready;
        // C++ SpecialPowerModule::getReadyFrame residual: while isDisabled (or
        // pauseCountdown), availableOnFrame slides with the logic frame — countdown
        // does not advance. SharedNSync player timers are separate and keep ticking.
        let freeze_special_power = self.is_disabled();
        // Under SPECIAL_POWER_AUTHORITY+shadow, GameWorld sole-ticks countdown;
        // host only refreshes ready aggregate after writeback (Wave 618 ready-log).
        let sole_sp = crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled();
        if dt > 0.0 && !freeze_special_power && !sole_sp && !self.special_power_cooldowns.is_empty()
        {
            let paused: Vec<_> = self
                .special_power_cooldowns
                .keys()
                .filter(|power| self.is_special_power_countdown_paused(power))
                .cloned()
                .collect();
            for (power, rem) in &mut self.special_power_cooldowns {
                if !paused.iter().any(|p| p == power) {
                    *rem = (*rem - dt).max(0.0);
                }
            }
        }
        // Legacy single-timer residual (older paths / saves).
        if dt > 0.0
            && !freeze_special_power
            && !sole_sp
            && self.special_power_cooldown_remaining > 0.0
        {
            self.special_power_cooldown_remaining =
                (self.special_power_cooldown_remaining - dt).max(0.0);
        }
        self.refresh_special_power_aggregate_cooldown();
        let became_ready = !was_ready && self.special_power_ready;
        // GameWorld last-writer residual: publish SP timer after every tick that
        // may have advanced/frozen countdown or flipped ready.
        if became_ready
            || self.special_power_cooldown_remaining > 0.0
            || !self.special_power_cooldowns.is_empty()
            || was_ready != self.special_power_ready
        {
            self.record_host_special_power();
        }
        became_ready
    }

    /// C++ MissileLauncherBuildingUpdate::update — door bits keyed to ready-frame.
    /// Returns `(DoorOpenIdleAudio play, stop)` for GameLogic to queue.
    pub fn tick_missile_launcher_building(&mut self, now: u32) -> (Option<String>, bool) {
        if self.status.under_construction {
            return (None, false);
        }
        use crate::game_logic::host_missile_launcher_building_update::{
            HostMissileLauncherBuildingUpdateData, missile_launcher_ini_for_template,
            missile_launcher_special_power,
        };
        if self.missile_launcher_building.is_none() {
            let Some(ini) = missile_launcher_ini_for_template(&self.template_name) else {
                return (None, false);
            };
            self.missile_launcher_building =
                Some(HostMissileLauncherBuildingUpdateData::from_ini(ini));
        }
        let power = missile_launcher_special_power(&self.template_name);
        let (ready_frame, is_ready) = if let Some(power) = power.as_ref() {
            let rem = self.special_power_countdown_seconds(power);
            let ready = rem <= 0.0 && self.is_special_power_ready(power);
            let ready_frame = if ready {
                now
            } else {
                now.saturating_add((rem * 30.0).ceil() as u32)
            };
            (ready_frame, ready)
        } else {
            (0, false)
        };
        let Some(data) = self.missile_launcher_building.as_mut() else {
            return (None, false);
        };
        let before = data.door_state;
        data.update(now, ready_frame, is_ready);
        // Leftover `TheFXList::do_fx_at_position` on every door-state enter
        // (C++ `FXList::doFXPos` at the building). `pending_fx` is the name.
        let pending_fx = data.pending_fx.take();
        let pending_idle = data.pending_idle_audio.take();
        let stop_idle = std::mem::take(&mut data.stop_idle_audio);
        if data.door_state != before || data.pending_initiate {
            // pending_initiate is consumed inside update; bits still need apply.
        }
        self.apply_missile_launcher_door_bits();
        if let Some(fx) = pending_fx {
            if !fx.is_empty() && !fx.eq_ignore_ascii_case("None") {
                let _ = crate::game_logic::dispatch_fx_list_at_pos(&fx, self.get_position());
            }
        }
        (pending_idle, stop_idle)
    }

    fn apply_missile_launcher_door_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        use crate::game_logic::host_missile_launcher_building_update::HostMissileLauncherDoorState;
        let Some(state) = self
            .missile_launcher_building
            .as_ref()
            .map(|d| d.door_state)
        else {
            return;
        };
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        let before = self.model_condition_bits;
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
        match state {
            HostMissileLauncherDoorState::Opening => {
                self.model_condition_bits |= 1u128 << open_b;
            }
            HostMissileLauncherDoorState::Open => {
                self.model_condition_bits |= 1u128 << wait_b;
            }
            HostMissileLauncherDoorState::WaitingToClose => {
                self.model_condition_bits |= 1u128 << wait_close_b;
            }
            HostMissileLauncherDoorState::Closing => {
                self.model_condition_bits |= 1u128 << close_b;
            }
            HostMissileLauncherDoorState::Closed => {}
        }
        if self.model_condition_bits != before {
            self.record_host_model_condition();
        }
    }

    pub fn update_construction(&mut self, dt: f32) {
        if self.status.under_construction {
            let build_rate = 1.0 / self.thing.template.build_time;
            self.construction_percent += build_rate * dt;

            // C++ DozerAIUpdate.cpp:1708+526: start 1 HP, then
            // +maxHealth / framesToBuild each logic frame — including the
            // completing frame. Completion (cpp:536-561) does not snap HP.
            let frames = (self.thing.template.build_time * 30.0).max(1.0);
            let per_frame = self.health.maximum / frames;
            let logic_frames = (dt * 30.0).max(0.0);
            self.health.current =
                (self.health.current.max(1.0) + per_frame * logic_frames).min(self.health.maximum);

            if self.construction_percent >= 1.0 {
                self.construction_percent = 1.0;
                self.set_status_under_construction(false);
            }
        }
    }

    /// Leftover unused `UnitAIUpdate::get_locomotor_distance_to_goal` 3D gate:
    /// leftover `FLAG_CLOSE_ENOUGH_3D` or `KINDOF_PROJECTILE`.
    #[inline]
    pub fn host_uses_close_enough_dist_3d(&self) -> bool {
        self.close_enough_dist_3d
            || self.is_kind_of(KindOf::Projectile)
            || self.object_type == ObjectType::Projectile
    }

    /// Leftover unused `get_locomotor_distance_to_goal` metric to `goal`.
    /// 3D only when leftover flag or projectile; else 2D / aircraft flight-dist.
    pub fn host_locomotor_distance_to_goal(&self, current: Vec3, goal: Vec3) -> f32 {
        if self.host_uses_close_enough_dist_3d() {
            return current.distance(goal);
        }
        let treat_as_aircraft = !crate::game_logic::PathfindingGrid::is_doing_ground_movement(self)
            || matches!(self.loco_appearance, LocomotorAppearance::Hover);
        let dx = goal.x - current.x;
        let dz = goal.z - current.z;
        let dist_2d = (dx * dx + dz * dz).sqrt();
        if !treat_as_aircraft {
            return dist_2d;
        }
        let flight = if self.movement.path.is_empty() {
            dist_2d
        } else {
            crate::game_logic::PathfindingSystem::compute_flight_dist_to_goal(
                current,
                &self.movement.path[self.movement.current_path_index.saturating_sub(1)..],
            )
        };
        if flight * flight > dist_2d * dist_2d {
            dist_2d
        } else {
            flight
        }
    }

    pub fn update_movement(&mut self, dt: f32) {
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.movement.target_position = None;
            self.movement.velocity = Vec3::ZERO;
            return;
        }

        // C++ Locomotor::setPhysicsOptions residual each move tick.
        self.set_locomotor_physics_options();

        // Stunned residual: no loco move while shock-stunned.
        if self.shock_stun_frames > 0 {
            return;
        }
        if self.locomotor_goal_type == LocoGoalType::Angle {
            // C++ doLocomotor ANGLE — Face leftover-marches via
            // locoUpdate_moveTowardsAngle; do not maintain/follow a path.
            return;
        }

        if matches!(self.ai_state, AIState::AttackMoving) && self.target.is_some() {
            let dest_walk = self.requested_destination.map(|dest| {
                let near = |p: Vec3| {
                    let dx = p.x - dest.x;
                    let dz = p.z - dest.z;
                    dx * dx + dz * dz < 16.0
                };
                self.movement.path.last().copied().is_some_and(near)
                    || self.movement.target_position.is_some_and(near)
            });
            if dest_walk.unwrap_or(true) {
                self.movement.velocity = Vec3::ZERO;
                self.set_status_moving(false);
                return;
            }
        }

        // C++ fixInvalidPosition residual when on invalid terrain.
        if self.fix_invalid_position() {
            return;
        }

        if self.movement.target_position.is_none() {
            // C++ maintainCurrentPosition when no move order.
            // ground_y unknown here — use current y as layer residual.
            let gy = self.get_position().y;
            let _ = self.loco_maintain_current_position(gy, dt);
            return;
        }

        // Moving: invalidate maintain pos residual.
        self.maintain_pos_valid = false;
        // C++ locoUpdate_moveTowardsPosition applyMotiveForce(0) (Locomotor.cpp:1010-1014).
        self.apply_motive_force(glam::Vec3::ZERO);

        if let Some(target_pos) = self.movement.target_position {
            let current_pos = self.get_position();
            let dx = target_pos.x - current_pos.x;
            let dz = target_pos.z - current_pos.z;
            let dist_2d = (dx * dx + dz * dz).sqrt();

            if dist_2d < 1.0e-4 {
                // Advance path or stop.
                let next_waypoint =
                    if self.movement.current_path_index + 1 < self.movement.path.len() {
                        self.movement.current_path_index += 1;
                        Some(self.movement.path[self.movement.current_path_index])
                    } else {
                        None
                    };
                if let Some(waypoint) = next_waypoint {
                    self.movement.target_position = Some(waypoint);
                } else {
                    self.commit_completed_waypoint_labels();
                    self.stop_moving();
                }
                return;
            }

            // C++ locoUpdate_moveTowardsPosition residual (treads-like host default).
            let max_speed = self.effective_max_speed().max(0.0);
            let mut desired_speed = max_speed * self.group_speed_factor.clamp(0.0, 1.0);
            // Cap by blocked speed residual (convert frame→sec: blocked is per-frame).
            if self.is_blocked && self.cur_max_blocked_speed.is_finite() {
                let blocked_per_sec = self.cur_max_blocked_speed * 30.0;
                desired_speed = desired_speed.min(blocked_per_sec);
            }

            // C++ getIsDownhillOnly residual: refuse uphill goals.
            if self.downhill_only {
                let us_y = current_pos.y;
                let goal_y = target_pos.y;
                if us_y < goal_y - 0.05 {
                    return;
                }
            }

            // Legs wander residual: bias desired heading before rotate.
            let mut rotate_goal = target_pos;
            if matches!(
                self.loco_appearance,
                LocomotorAppearance::LegsTwo | LocomotorAppearance::Climber
            ) && self.wander_width_factor != 0.0
            {
                let actual = self.forward_speed_2d().abs();
                let wobble = self.tick_wander_angle_offset(actual);
                let us = self.get_position();
                let base = (-dz).atan2(dx) + wobble;
                rotate_goal = glam::Vec3::new(
                    us.x + base.cos() * 100.0,
                    us.y,
                    us.z + (-base.sin()) * 100.0,
                );
            }

            // C++ rotateTowardsPosition residual.
            let (_turning, angle_diff) = self.rotate_towards_position(rotate_goal, dt);

            // Appearance-specific speed residual (C++ moveTowardsPosition*).
            let quarter_pi = std::f32::consts::FRAC_PI_4;
            let mut angle_coeff = angle_diff.abs() / quarter_pi;
            if angle_coeff > 1.0 {
                angle_coeff = 1.0;
            }

            // Wheels: can only turn while moving — cap to minTurnSpeed when turning.
            if matches!(
                self.loco_appearance,
                LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle
            ) {
                let turn_speed = self.wheeled_turn_speed_floor();
                let small_turn = std::f32::consts::PI / 20.0;
                if angle_diff.abs() > small_turn && desired_speed > turn_speed {
                    desired_speed = turn_speed;
                }
                // Reverse residual when goal is behind and can_move_backward.
                if self.can_move_backward
                    && actual_speed_is_zero(self)
                    && angle_diff.abs() > std::f32::consts::FRAC_PI_2
                {
                    self.moving_backwards = true;
                    self.record_host_locomotor();
                }
                if self.moving_backwards && angle_diff.abs() < std::f32::consts::FRAC_PI_2 {
                    self.moving_backwards = false;
                    self.record_host_locomotor();
                }
            }

            let mut goal_speed = match self.loco_appearance {
                LocomotorAppearance::LegsTwo
                | LocomotorAppearance::Climber
                | LocomotorAppearance::Treads => (1.0 - angle_coeff) * desired_speed,
                LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle => desired_speed,
                LocomotorAppearance::Hover
                | LocomotorAppearance::Wings
                | LocomotorAppearance::Thrust
                | LocomotorAppearance::Other => desired_speed,
            };

            // Braking residual near destination (unless NO_SLOW_DOWN).
            let actual_speed = self.forward_speed_2d().abs();
            let braking = self.braking.max(1.0e-3);
            let slow_down_dist =
                calc_slow_down_dist(actual_speed, self.min_speed.max(0.0), braking);
            if !self.no_slow_down_as_approaching_dest {
                if dist_2d < slow_down_dist && !self.is_braking {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                if dist_2d > PATHFIND_CELL_SIZE_F_RESIDUAL && dist_2d > 2.0 * slow_down_dist {
                    self.is_braking = false;
                    self.braking_factor = 1.0;
                }
                if self.is_braking {
                    let floor = self.min_speed.max(0.0);
                    goal_speed = goal_speed
                        .min(actual_speed * 0.85 / self.braking_factor.max(1.0))
                        .max(floor);
                }
            }
            // Treads near-goal tight turn residual.
            if matches!(self.loco_appearance, LocomotorAppearance::Treads)
                && dist_2d < 2.0 * PATHFIND_CELL_SIZE_F_RESIDUAL
                && angle_coeff > 0.05
            {
                goal_speed = actual_speed * 0.6;
            }

            // Wings/Thrust specialized residual (may set position itself).
            if matches!(self.loco_appearance, LocomotorAppearance::Thrust) {
                self.move_towards_thrust(target_pos, dist_2d, goal_speed, dt);
                let _ = self.handle_behavior_z(self.get_position().y, Some(target_pos.y));
            } else if matches!(self.loco_appearance, LocomotorAppearance::Wings) {
                // 2D other-like + preferred height via BehaviorZ.
                self.apply_forward_speed_force(goal_speed, dt);
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);
                let _ = self.handle_behavior_z(new_position.y, Some(target_pos.y));
            } else {
                // Force/velocity apply residual (legs/wheels/treads/hover/other).
                self.apply_forward_speed_force(goal_speed, dt);

                // Arm motive window so collide forces stay lateral while driving.
                if goal_speed.abs() > 0.1 {
                    self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL;
                    self.record_host_physics_motive();
                }

                // Position integrate (host dt seconds).
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);

                // C++ handleBehaviorZ residual after loco XY step.
                let ground_y = new_position.y; // caller/physics motion step samples terrain
                let _ = self.handle_behavior_z(ground_y, Some(target_pos.y));
            }

            // Arrival residual.
            // C++ Locomotor::getCloseEnoughDist after SET_STOPPING_DISTANCE
            // (`setCloseEnoughDist`, ignore values < 0.5). Host default 2.0
            // is the pre-script residual arrival band.
            // Leftover get_locomotor_distance_to_goal: 3D only when
            // CloseEnoughDist3D or KINDOF_PROJECTILE; else 2D / flight-dist.
            let arrive_dist = self
                .close_enough_dist
                .filter(|d| d.is_finite() && *d >= 0.5)
                .unwrap_or(2.0);
            let distance_to_target = self.host_locomotor_distance_to_goal(current_pos, target_pos);
            if distance_to_target < arrive_dist {
                let next_waypoint =
                    if self.movement.current_path_index + 1 < self.movement.path.len() {
                        self.movement.current_path_index += 1;
                        Some(self.movement.path[self.movement.current_path_index])
                    } else {
                        None
                    };
                if let Some(waypoint) = next_waypoint {
                    self.movement.target_position = Some(waypoint);
                    self.is_braking = false;
                } else {
                    self.commit_completed_waypoint_labels();
                    self.stop_moving();
                    self.is_braking = false;
                }
            }
        }
        self.record_host_movement();
    }

    /// C++ SalvageCrateCollide::doWeaponSet residual.
    pub fn apply_salvage_weapon_upgrade(&mut self) {
        if self.weapon_crate_upgrade >= 2 {
            return;
        }
        self.weapon_crate_upgrade = self.weapon_crate_upgrade.saturating_add(1);
        self.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        self.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        let tag = if self.weapon_crate_upgrade >= 2 {
            "WEAPONSET_CRATEUPGRADE_TWO"
        } else {
            "WEAPONSET_CRATEUPGRADE_ONE"
        };
        self.applied_upgrades.insert(tag.to_string());
        use crate::game_logic::host_enum_table_residual::{
            weaponset_crateupgrade_one_model_bit, weaponset_crateupgrade_two_model_bit,
        };
        let one = weaponset_crateupgrade_one_model_bit();
        let two = weaponset_crateupgrade_two_model_bit();
        self.model_condition_bits &= !(1u128 << one);
        self.model_condition_bits &= !(1u128 << two);
        if self.weapon_crate_upgrade >= 2 {
            self.model_condition_bits |= 1u128 << two;
        } else {
            self.model_condition_bits |= 1u128 << one;
        }
        self.record_host_model_condition();
        // C++ SalvageCrateCollide::doWeaponSet → setWeaponSetFlag → updateWeaponSet.
        let crate_condition = if self.weapon_crate_upgrade >= 2 {
            "CRATEUPGRADE_TWO"
        } else {
            "CRATEUPGRADE_ONE"
        };
        self.adopt_weapon_set_lock_share_for_condition(crate_condition);
        self.release_weapon_lock_on_set_change();
        if let Some(wname) = crate::game_logic::host_car_bomb::crateupgrade_primary_weapon(
            &self.template_name,
            self.weapon_crate_upgrade,
        ) {
            if let Some(weapon) = crate::game_logic::thing::ThingTemplate::weapon_from_store(&wname)
            {
                let _ = self.replace_weapon_set_slot(0, Some(weapon));
            }
        }
        self.record_host_ai_request();
    }

    /// C++ SalvageCrateCollide::doArmorSet residual.
    pub fn apply_salvage_armor_upgrade(&mut self) {
        if self.armor_crate_upgrade >= 2 {
            return;
        }
        self.armor_crate_upgrade = self.armor_crate_upgrade.saturating_add(1);
        self.applied_upgrades.remove("ARMORSET_CRATEUPGRADE_ONE");
        self.applied_upgrades.remove("ARMORSET_CRATEUPGRADE_TWO");
        let tag = if self.armor_crate_upgrade >= 2 {
            "ARMORSET_CRATEUPGRADE_TWO"
        } else {
            "ARMORSET_CRATEUPGRADE_ONE"
        };
        self.applied_upgrades.insert(tag.to_string());
        self.validate_armor_and_damage_fx();
        self.record_host_ai_request();
    }

    /// C++ ActiveBody::validateArmorAndDamageFX + crate model bits.
    pub(crate) fn validate_armor_and_damage_fx(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            armorset_crateupgrade_one_model_bit, armorset_crateupgrade_two_model_bit,
        };
        let one = armorset_crateupgrade_one_model_bit();
        let two = armorset_crateupgrade_two_model_bit();
        self.model_condition_bits &= !(1u128 << one);
        self.model_condition_bits &= !(1u128 << two);
        match self.armor_crate_upgrade {
            1 => self.model_condition_bits |= 1u128 << one,
            n if n >= 2 => self.model_condition_bits |= 1u128 << two,
            _ => {}
        }
        self.record_host_model_condition();
    }

    /// C++ ActiveBody::onVeterancyLevelChanged armor-set flag switch.
    fn set_veterancy_armor_set_flags(&mut self, level: VeterancyLevel) {
        self.armor_set_veteran = false;
        self.armor_set_elite = false;
        self.armor_set_hero = false;
        match level {
            VeterancyLevel::Rookie => {}
            VeterancyLevel::Veteran => self.armor_set_veteran = true,
            VeterancyLevel::Elite => self.armor_set_elite = true,
            VeterancyLevel::Heroic => self.armor_set_hero = true,
        }
    }

    /// C++ `Object::onVeterancyLevelChanged` exclusive WEAPONSET + WEAPONBONUSCONDITION.
    fn set_veterancy_weapon_set_and_bonus_flags(&mut self, level: VeterancyLevel) {
        let (vet, elite, hero) =
            crate::game_logic::host_unit_training::veterancy_weapon_set_flags(level);
        self.weapon_set_veteran = vet;
        self.weapon_set_elite = elite;
        self.weapon_set_hero = hero;
        self.weapon_bonus_veteran = vet;
        self.weapon_bonus_elite = elite;
        self.weapon_bonus_hero = hero;
        self.stamp_veterancy_weaponset_model_bits();
        self.record_host_weapon_set();
    }

    /// C++ `TheWeaponSetTypeToModelConditionTypeMap` WEAPONSET_VETERAN/ELITE/HERO.
    fn stamp_veterancy_weaponset_model_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            weaponset_elite_model_bit, weaponset_hero_model_bit, weaponset_veteran_model_bit,
        };
        let vet_b = weaponset_veteran_model_bit();
        let elite_b = weaponset_elite_model_bit();
        let hero_b = weaponset_hero_model_bit();
        self.model_condition_bits &= !(1u128 << vet_b);
        self.model_condition_bits &= !(1u128 << elite_b);
        self.model_condition_bits &= !(1u128 << hero_b);
        if self.weapon_set_hero {
            self.model_condition_bits |= 1u128 << hero_b;
        } else if self.weapon_set_elite {
            self.model_condition_bits |= 1u128 << elite_b;
        } else if self.weapon_set_veteran {
            self.model_condition_bits |= 1u128 << vet_b;
        }
        self.record_host_model_condition();
    }

    /// C++ WeaponSet::updateWeaponSet: unless the incoming set has
    /// WeaponLockSharedAcrossSets, release permanent lock and return to PRIMARY.
    pub(in crate::game_logic::object) fn release_weapon_lock_on_set_change(&mut self) {
        if self.thing.template.weapon_lock_shared_across_sets {
            return;
        }
        self.release_weapon_lock(WeaponLockType::LockedPermanently);
        self.set_active_weapon_slot(0);
    }

    /// C++ checks `WeaponTemplateSet::isWeaponLockSharedAcrossSets` on the *new* set.
    pub(in crate::game_logic::object) fn adopt_weapon_set_lock_share_for_condition(
        &mut self,
        condition: &str,
    ) {
        let Some(manager) = crate::assets::get_asset_manager() else {
            return;
        };
        let Ok(guard) = manager.lock() else {
            return;
        };
        let Some(definition) = guard.get_object_definition(&self.template_name) else {
            return;
        };
        let Some(set) = definition.weapon_sets.iter().find(|set| {
            set.conditions.iter().any(|row| {
                row.split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | '|'))
                    .any(|token| {
                        let token = token.trim().trim_start_matches("WEAPONSET_");
                        token.eq_ignore_ascii_case(condition)
                    })
            })
        }) else {
            return;
        };
        self.thing.template.weapon_lock_shared_across_sets =
            set.attributes.iter().any(|(key, value)| {
                (key.eq_ignore_ascii_case("WeaponLockSharedAcrossSets")
                    || key.eq_ignore_ascii_case("ShareWeaponLock"))
                    && matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "yes" | "true" | "1"
                    )
            });
    }

    /// Bind authored WeaponTemplateSet for this rank when INI declares one.
    /// Returns true when a slot was replaced (skip in-place damage/ROF scale).
    fn try_bind_authored_veterancy_weapon_set(&mut self, level: VeterancyLevel) -> bool {
        let Some((primary, secondary, tertiary)) =
            crate::game_logic::host_unit_training::authored_veterancy_weapon_set(
                &self.template_name,
                level,
            )
        else {
            return false;
        };
        let mut rebound = false;
        if let Some(name) = primary {
            if let Some(weapon) = crate::game_logic::thing::ThingTemplate::weapon_from_store(&name)
            {
                let _ = self.replace_weapon_set_slot(0, Some(weapon));
                rebound = true;
            }
        }
        if let Some(name) = secondary {
            if let Some(weapon) = crate::game_logic::thing::ThingTemplate::weapon_from_store(&name)
            {
                let _ = self.replace_weapon_set_slot(1, Some(weapon));
                rebound = true;
            }
        }
        if let Some(name) = tertiary {
            if let Some(weapon) = crate::game_logic::thing::ThingTemplate::weapon_from_store(&name)
            {
                let _ = self.replace_weapon_set_slot(2, Some(weapon));
                rebound = true;
            }
        }
        rebound
    }

    /// C++ SalvageCrateCollide::doLevelGain residual.
    pub fn apply_salvage_level_gain(&mut self) {
        use crate::game_logic::VeterancyLevel;
        let cur = self.experience.level;
        if matches!(cur, VeterancyLevel::Heroic) {
            return;
        }
        let need = match cur {
            VeterancyLevel::Rookie => self.thing.template.veterancy_xp_thresholds[0],
            VeterancyLevel::Veteran => self.thing.template.veterancy_xp_thresholds[1],
            VeterancyLevel::Elite => self.thing.template.veterancy_xp_thresholds[2],
            VeterancyLevel::Heroic => return,
        };
        let add = (need - self.experience.current).max(1.0);
        self.gain_experience(add);
    }

    /// C++ ExperienceTracker::gainExpForLevel residual.
    ///
    /// Grants just enough XP to gain `levels` veterancy ranks (clamped to Heroic).
    /// `can_level_up` false skips (non-trainable residual).
    pub fn gain_exp_for_level(&mut self, levels: u8, can_level_up: bool) -> u8 {
        if levels == 0 || !can_level_up {
            return 0;
        }
        use crate::game_logic::VeterancyLevel;
        let mut gained = 0u8;
        for _ in 0..levels {
            if matches!(self.experience.level, VeterancyLevel::Heroic) {
                break;
            }
            self.apply_salvage_level_gain();
            gained += 1;
        }
        gained
    }

    pub fn record_host_experience(&self) {
        crate::game_logic::host_experience_log::record(self.id, self.experience.current.max(0.0));
    }

    pub(super) fn record_host_veterancy_level(&self) {
        let ordinal = match self.experience.level {
            crate::game_logic::VeterancyLevel::Rookie => 0u8,
            crate::game_logic::VeterancyLevel::Veteran => 1,
            crate::game_logic::VeterancyLevel::Elite => 2,
            crate::game_logic::VeterancyLevel::Heroic => 3,
        };
        crate::game_logic::host_veterancy_log::record(self.id, ordinal);
    }

    /// C++ ThingTemplate::isTrainable.
    pub fn is_trainable(&self) -> bool {
        self.thing.template.is_trainable
    }

    /// C++ ExperienceTracker::isAcceptingExperiencePoints.
    pub fn is_accepting_experience_points(&self) -> bool {
        self.is_trainable() || self.experience_sink.is_some()
    }

    /// C++ ExperienceTracker::getExperienceValue + UNDER_CONSTRUCTION gate.
    ///
    /// Ally / same-controller kills must pass `killer_is_ally_or_own = true`
    /// (C++ `getRelationship == ALLIES` and `controller == victimController`).
    pub fn kill_experience_value(&self) -> f32 {
        self.kill_experience_value_from_killer(false)
    }

    /// C++ ExperienceTracker::getExperienceValue(killer).
    pub fn kill_experience_value_from_killer(&self, killer_is_ally_or_own: bool) -> f32 {
        if killer_is_ally_or_own || self.status.under_construction {
            return 0.0;
        }
        self.thing
            .template
            .experience_value_for_level(self.experience.level)
    }

    /// C++ ThingTemplate::getSkillPointValue(victimLevel).
    /// Unset SkillPointValue uses ExperienceValue (`USE_EXP_VALUE_FOR_SKILL_VALUE`).
    pub fn kill_skill_point_value(&self) -> i32 {
        if self.status.under_construction {
            return 0;
        }
        self.thing
            .template
            .skill_point_value_for_level(self.experience.level)
    }

    /// C++ ExperienceTracker::setExperienceSink.
    pub fn set_experience_sink(&mut self, sink: Option<ObjectId>) {
        self.experience_sink = sink;
    }

    /// C++ Weapon.cpp:3021 / NeutronMissileUpdate.cpp:219: projectiles sink XP to launcher.
    pub fn note_producer(&mut self, source: ObjectId) {
        self.producer_id = Some(source);
        if self.is_kind_of(crate::game_logic::KindOf::Projectile) {
            self.set_experience_sink(Some(source));
        }
    }

    /// C++ ExperienceTracker::setExperienceScalar.
    pub fn set_experience_scalar(&mut self, scalar: f32) {
        self.experience_scalar = if scalar.is_finite() { scalar } else { 1.0 };
    }

    /// C++ ExperienceScalarUpgrade::upgradeImplementation (get + AddXPScalar).
    pub fn add_experience_scalar(&mut self, add: f32) {
        if add.is_finite() && add != 0.0 {
            self.set_experience_scalar(self.experience_scalar + add);
        }
    }

    pub fn gain_experience(&mut self, amount: f32) {
        // C++ addExperiencePoints: untrainable objects keep no XP (sink
        // forwarding is handled by GameLogic::award_experience).
        if !self.is_trainable() {
            return;
        }
        // C++ amountToGain *= m_experienceScalar when canScaleForBonus.
        let scalar = if self.experience_scalar.is_finite() && self.experience_scalar > 0.0 {
            self.experience_scalar
        } else {
            1.0
        };
        let amount = amount * scalar;
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let projected = self.experience.current + amount;

        // C++ parity: veterancy thresholds are per-template (Object::ExperienceValues
        // in INI).  Use template-defined thresholds, falling back to defaults.
        let thresholds = self.thing.template.veterancy_xp_thresholds;

        // Check for level up against projected XP (even when HP/XP authority defers current).
        let previous_level = self.experience.level;
        let new_level = if projected >= thresholds[2] {
            VeterancyLevel::Heroic
        } else if projected >= thresholds[1] {
            VeterancyLevel::Elite
        } else if projected >= thresholds[0] {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Rookie
        };

        if new_level != previous_level {
            self.experience.level = new_level;
            // Apply veterancy bonuses
            self.apply_veterancy_bonuses(previous_level, new_level);
            self.record_host_veterancy_level();
        }

        // GameWorld residual authority: log absolute XP; defer host current mutate.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_experience_log::record(self.id, projected.max(0.0));
        } else {
            self.experience.current = projected;
            self.record_host_experience();
        }
    }

    /// C++ parity (GameData.ini veterancy bonuses):
    ///   Veteran: +10% dmg, +20% RoF, +20% HP
    ///   Elite:   +20% dmg, +40% RoF, +30% HP
    ///   Heroic:  +30% dmg, +60% RoF, +50% HP
    /// Returns (health_multiplier, damage_multiplier, rof_multiplier).
    fn veterancy_bonuses(level: VeterancyLevel) -> (f32, f32, f32) {
        crate::game_logic::host_unit_training::veterancy_bonus_multipliers(level)
    }

    /// Wave 79: true when AdvancedTraining ExperienceScalar residual tag is present.
    pub fn has_advanced_training_xp_scalar(&self) -> bool {
        use crate::game_logic::host_unit_training::{
            UPGRADE_AMERICA_ADVANCED_TRAINING, is_advanced_training_upgrade,
        };
        self.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING)
            || self.has_upgrade_tag("UpgradeAdvancedTraining")
            || self
                .applied_upgrades
                .iter()
                .any(|u| is_advanced_training_upgrade(u))
    }

    pub fn record_host_max_health(&self) {
        crate::game_logic::host_max_health_log::record(
            self.id,
            self.max_health.max(self.health.maximum).max(1.0),
        );
    }

    /// C++ `TheGlobalData->m_healthBonus[new] / m_healthBonus[old]`.
    /// Leftover GlobalData when INI-populated; else GameData.ini residual 1.0/1.2/1.3/1.5.
    fn veterancy_health_bonus_scale(
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) -> f32 {
        let idx = |level: VeterancyLevel| match level {
            VeterancyLevel::Rookie => 0usize,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        if let Some(data) = game_engine::common::ini::get_global_data() {
            let guard = data.read();
            let old = guard
                .health_bonus
                .get(idx(previous_level))
                .copied()
                .unwrap_or(1.0);
            let new = guard
                .health_bonus
                .get(idx(new_level))
                .copied()
                .unwrap_or(1.0);
            // Constructor default is 1.0 for every rank. Treat that as "INI not loaded".
            if old > 0.0
                && new > 0.0
                && ((old - 1.0).abs() > f32::EPSILON || (new - 1.0).abs() > f32::EPSILON)
            {
                return new / old;
            }
        }
        crate::game_logic::host_unit_training::veterancy_health_scale(previous_level, new_level)
    }

    /// C++ ActiveBody::onVeterancyLevelChanged → setMaxHealth(..., PRESERVE_RATIO).
    fn apply_veterancy_max_health_scale(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) {
        if previous_level == new_level {
            return;
        }
        let scale = Self::veterancy_health_bonus_scale(previous_level, new_level);
        let old_max = self.health.maximum.max(self.max_health).max(1.0);
        let ratio = self.health.current / old_max;
        let new_max = (old_max * scale).max(1.0);
        self.set_body_max_health(new_max);
        self.health.current = (new_max * ratio).clamp(0.0, new_max);
    }

    pub(crate) fn apply_veterancy_bonuses(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) {
        self.apply_veterancy_bonuses_with_feedback(previous_level, new_level, true);
    }

    pub(crate) fn apply_veterancy_bonuses_with_feedback(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) {
        let (_, old_damage_bonus, old_rof_bonus) = Self::veterancy_bonuses(previous_level);
        let (_, damage_bonus, rof_bonus) = Self::veterancy_bonuses(new_level);

        // C++ Object::onVeterancyLevelChanged: exclusive WEAPONSET + WEAPONBONUSCONDITION.
        self.set_veterancy_weapon_set_and_bonus_flags(new_level);
        if let Some(condition) =
            crate::game_logic::host_unit_training::veterancy_weapon_set_condition(new_level)
        {
            self.adopt_weapon_set_lock_share_for_condition(condition);
        }
        self.release_weapon_lock_on_set_change();
        let rebound_authored = self.try_bind_authored_veterancy_weapon_set(new_level);

        // C++ updateUpgradeModules + giveUpgrade(findVeterancyUpgrade(newLevel)).
        if let Some(name) = crate::game_logic::host_unit_training::veterancy_upgrade_name(new_level)
        {
            self.apply_upgrade_tag(name);
            let _ = crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade(
                self, name,
            );
        }

        // C++ ActiveBody::setMaxHealth(m_maxHealth * (newBonus/oldBonus), PRESERVE_RATIO).
        // Scale the *current* body max so Composite Armor / difficulty HP survive rank-up.
        self.apply_veterancy_max_health_scale(previous_level, new_level);

        // In-place damage/ROF scale is the host residual for units that keep
        // the rookie WeaponSet. Authored VETERAN/ELITE/HERO sets already
        // replace the Weapon instances; GameData WeaponBonus then stacks at
        // fire time from the exclusive condition flags.
        if !rebound_authored {
            if let Some(weapon) = &mut self.weapon {
                let dmg_scale = if old_damage_bonus > 0.0 {
                    damage_bonus / old_damage_bonus
                } else {
                    1.0
                };
                weapon.damage *= dmg_scale;
                // C++ parity: RoF bonus reduces reload time (faster firing).
                // Scale relative to previous level so multi-level transitions work.
                let rof_scale = rof_bonus / old_rof_bonus;
                weapon.reload_time *= rof_scale;
            }
        }
        self.record_host_veterancy_level();
        self.set_body_max_health(self.health.maximum.max(1.0));
        self.record_host_max_health();
        self.set_veterancy_armor_set_flags(new_level);
        self.validate_armor_and_damage_fx();

        // C++ ActiveBody SoundPromoted* + Object LevelGain Anim2D + MiscAudio UnitPromoted.
        self.queue_veterancy_promote_fx(previous_level, new_level, provide_feedback);
    }

    fn queue_veterancy_promote_fx(
        &self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) {
        use crate::game_logic::host_unit_training::{
            hide_promote_fx_for_stealth, record_promote_fx, should_queue_promote_fx,
        };
        let hide = hide_promote_fx_for_stealth(
            self.is_locally_controlled_for_promote_fx(),
            self.status.stealthed,
            self.status.detected,
            self.status.disguised,
        );
        if !should_queue_promote_fx(
            previous_level,
            new_level,
            self.is_kind_of(crate::game_logic::KindOf::IgnoredInGui),
            hide,
            gamelogic::helpers::TheGameLogic::get_draw_icon_ui(),
            provide_feedback,
        ) {
            return;
        }
        let pos = self.get_health_box_position();
        record_promote_fx(self.id, pos, 0, new_level);
    }

    /// C++ `Object::isLocallyControlled` residual for promote FX.
    /// Unset owner (unit tests) is local; player 0 is the live local slot.
    fn is_locally_controlled_for_promote_fx(&self) -> bool {
        match self.owner_player_id {
            None => true,
            Some(id) => id == 0,
        }
    }

    /// C++ ExperienceTracker::setMinVeterancyLevel residual (VeterancyGainCreate).
    ///
    /// Never lowers rank. Seeds residual XP so gain_experience does not demote.
    /// Applies health / weapon bonuses when promoting.
    pub fn set_min_veterancy_level(&mut self, level: VeterancyLevel) -> bool {
        fn rank(level: VeterancyLevel) -> u8 {
            match level {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            }
        }
        fn xp_seed(level: VeterancyLevel, thresholds: [f32; 3]) -> f32 {
            match level {
                VeterancyLevel::Rookie => 0.0,
                VeterancyLevel::Veteran => thresholds[0],
                VeterancyLevel::Elite => thresholds[1],
                VeterancyLevel::Heroic => thresholds[2],
            }
        }

        let previous = self.experience.level;
        let thresholds = self.thing.template.veterancy_xp_thresholds;
        if rank(level) <= rank(previous) {
            // Still seed XP if level already matches but XP is below threshold.
            let seed = xp_seed(previous, thresholds);
            if self.experience.current < seed {
                self.experience.current = seed;
            }
            return false;
        }
        self.experience.level = level;
        let seed = xp_seed(level, thresholds);
        self.experience.current = self.experience.current.max(seed);
        self.apply_veterancy_bonuses(previous, level);
        true
    }

    /// C++ `ExperienceTracker::setVeterancyLevel` used by
    /// `RiderChangeContain`.  Unlike `set_min_veterancy_level`, rider swaps
    /// may lower the bike back to Rookie before applying the new rider's rank,
    /// so this assigns the exact level and resets XP to this template's level
    /// threshold rather than carrying arbitrary source XP across templates.
    pub(crate) fn set_rider_change_veterancy_level(&mut self, level: VeterancyLevel) {
        let threshold = match level {
            VeterancyLevel::Rookie => 0.0,
            VeterancyLevel::Veteran => self.thing.template.veterancy_xp_thresholds[0],
            VeterancyLevel::Elite => self.thing.template.veterancy_xp_thresholds[1],
            VeterancyLevel::Heroic => self.thing.template.veterancy_xp_thresholds[2],
        };
        let previous = self.experience.level;
        self.experience.level = level;
        self.experience.current = threshold.max(0.0);
        if previous != level {
            self.apply_veterancy_bonuses_with_feedback(previous, level, false);
        } else {
            self.record_host_veterancy_level();
            self.record_host_experience();
        }
    }
}
