use super::*;

impl Object {
    /// Resolve a concrete C++ WeaponSet slot to its source Weapon.ini name.
    ///
    /// This deliberately has no fallback for TERTIARY.  A missing third slot
    /// is not a primary weapon, and treating it as one makes a `FIRE_WEAPON
    /// TERTIARY` command silently discharge the wrong weapon.
    pub fn weapon_name_for_slot(&self, slot: u8) -> Option<&str> {
        match slot {
            0 => self.primary_weapon_name(),
            1 => self.secondary_weapon_name(),
            2 => self.tertiary_weapon_name(),
            _ => None,
        }
    }

    /// First concrete WeaponSet slot currently bound on this object.
    pub fn first_available_weapon_slot(&self) -> Option<u8> {
        [0u8, 1u8, 2u8]
            .into_iter()
            .find(|slot| self.weapon_slot(*slot).is_some())
    }

    /// The concrete slot currently selected for an attack.
    ///
    /// A weapon lock is authoritative over the displayed active slot.  This
    /// returns `None` instead of falling back when a restored or otherwise
    /// invalid slot is requested, so an explicit TERTIARY command can never
    /// silently discharge PRIMARY.
    pub fn selected_weapon_slot(&self) -> Option<u8> {
        let slot = if self.weapon_lock_type != WeaponLockType::NotLocked {
            self.weapon_lock_slot
        } else {
            self.active_weapon_slot
        };
        self.weapon_slot(slot).map(|_| slot)
    }

    /// C++ WeaponSet model-condition residual for PREATTACK/FIRING/BETWEEN/RELOADING A/B/C.
    ///
    /// Maps `weapon_fire_status` + active slot onto ModelConditionFlags bits
    /// (ALLOW_SURRENDER-off layout: PREATTACK_A=35 .. RELOADING_C=46).

    /// C++ Object::getAmmoPipShowingInfo residual.
    ///
    /// Returns `(clip_size, remaining_ammo)` for the first ShowsAmmoPips weapon.

    /// C++ Weapon::getPercentReadyToFire residual for one slot (0.0..1.0).
    pub fn weapon_slot_percent_ready_to_fire(&self, slot: u8, current_time: f32) -> f32 {
        let Some(weapon) = self.weapon_slot(slot) else {
            return 0.0;
        };
        let name = self.weapon_name_for_slot(slot);
        // Prefer live WeaponFireStatus when this is the active slot.
        let status = if slot == self.active_weapon_slot {
            self.weapon_fire_status
        } else {
            // Approximate status from ammo/reload without mutating.
            if !Self::weapon_has_ammo_for_shot(weapon, name) {
                WeaponFireStatus::OutOfAmmo
            } else {
                let reload = self.effective_weapon_reload(weapon.reload_time);
                if current_time - weapon.last_fire_time < reload - 1e-6 {
                    if weapon.clip_size > 0
                        && weapon.ammo == Some(weapon.clip_size)
                        && weapon.clip_reload_time > reload + 1e-4
                    {
                        WeaponFireStatus::ReloadingClip
                    } else {
                        WeaponFireStatus::BetweenFiringShots
                    }
                } else {
                    WeaponFireStatus::ReadyToFire
                }
            }
        };
        match status {
            WeaponFireStatus::OutOfAmmo | WeaponFireStatus::PreAttack => 0.0,
            WeaponFireStatus::ReadyToFire => 1.0,
            WeaponFireStatus::BetweenFiringShots | WeaponFireStatus::ReloadingClip => {
                let reload =
                    if status == WeaponFireStatus::ReloadingClip && weapon.clip_reload_time > 0.0 {
                        weapon.clip_reload_time
                    } else {
                        self.effective_weapon_reload(weapon.reload_time)
                    };
                if reload <= 1e-6 {
                    return 1.0;
                }
                let elapsed = (current_time - weapon.last_fire_time).max(0.0);
                if elapsed >= reload {
                    1.0
                } else {
                    (elapsed / reload).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// C++ Object::getMostPercentReadyToFireAnyWeapon residual (0..100).
    pub fn get_most_percent_ready_to_fire_any_weapon(&self, current_time: f32) -> u32 {
        let mut most = 0u32;
        for slot in [0u8, 1u8, 2u8] {
            if self.weapon_slot(slot).is_none() {
                continue;
            }
            let pct = (self.weapon_slot_percent_ready_to_fire(slot, current_time) * 100.0) as u32;
            if pct > most {
                most = pct;
            }
            if most >= 100 {
                return 100;
            }
        }
        most.min(100)
    }

    pub fn get_ammo_pip_showing_info(&self) -> Option<(u32, u32)> {
        use crate::game_logic::weapon_bootstrap::host_shows_ammo_pips_for_weapon_name;
        for slot in [0u8, 1u8, 2u8] {
            let Some(w) = self.weapon_slot(slot) else {
                continue;
            };
            let name = self.weapon_name_for_slot(slot);
            let Some(n) = name else {
                continue;
            };
            if !host_shows_ammo_pips_for_weapon_name(n) {
                continue;
            }
            let total = if w.clip_size > 0 {
                w.clip_size
            } else {
                w.ammo.unwrap_or(0)
            };
            if total == 0 {
                continue;
            }
            let full = w.ammo.unwrap_or(total).min(total);
            return Some((total, full));
        }
        None
    }

    /// C++ Object::findWaypointFollowingCapableWeapon residual (slot index).
    ///
    /// Scans TERTIARY, SECONDARY then PRIMARY (C++ WEAPONSLOT_COUNT-1 .. PRIMARY).
    pub fn find_waypoint_following_capable_weapon_slot(&self) -> Option<u8> {
        use crate::game_logic::weapon_bootstrap::host_capable_of_following_waypoint_for_weapon_name;
        for slot in [2u8, 1u8, 0u8] {
            let Some(_w) = self.weapon_slot(slot) else {
                continue;
            };
            let name = self.weapon_name_for_slot(slot);
            if name
                .map(host_capable_of_following_waypoint_for_weapon_name)
                .unwrap_or(false)
            {
                return Some(slot);
            }
        }
        None
    }

    pub fn sync_weapon_model_conditions_from_status(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_C, MC_BIT_FIRING_A, MC_BIT_FIRING_B, MC_BIT_FIRING_C,
            MC_BIT_PREATTACK_A, MC_BIT_PREATTACK_B, MC_BIT_PREATTACK_C, MC_BIT_RELOADING_A,
            MC_BIT_RELOADING_B, MC_BIT_RELOADING_C,
        };
        const WEAPON_MC_BITS: [u32; 12] = [
            MC_BIT_PREATTACK_A,
            MC_BIT_FIRING_A,
            MC_BIT_BETWEEN_FIRING_SHOTS_A,
            MC_BIT_RELOADING_A,
            MC_BIT_PREATTACK_B,
            MC_BIT_FIRING_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_RELOADING_B,
            MC_BIT_PREATTACK_C,
            MC_BIT_FIRING_C,
            MC_BIT_BETWEEN_FIRING_SHOTS_C,
            MC_BIT_RELOADING_C,
        ];
        let before = self.model_condition_bits;
        for b in WEAPON_MC_BITS {
            self.model_condition_bits &= !(1u128 << b);
        }
        let base = match self.active_weapon_slot {
            1 => 4usize,
            2 => 8usize,
            _ => 0usize,
        };
        let idx = match self.weapon_fire_status {
            WeaponFireStatus::PreAttack => Some(base),
            WeaponFireStatus::BetweenFiringShots => Some(base + 2),
            WeaponFireStatus::ReloadingClip => Some(base + 3),
            WeaponFireStatus::ReadyToFire | WeaponFireStatus::OutOfAmmo => {
                if self.status.is_firing_weapon {
                    Some(base + 1)
                } else {
                    None
                }
            }
        };
        if let Some(i) = idx {
            self.model_condition_bits |= 1u128 << WEAPON_MC_BITS[i];
        } else if self.status.is_firing_weapon {
            self.model_condition_bits |= 1u128 << WEAPON_MC_BITS[base + 1];
        }
        // Wave 487: weapon fire model bits must reach GW before model-condition writeback.
        if self.model_condition_bits != before {
            self.record_host_model_condition();
        }
    }

    /// C++ Weapon::getStatus residual refresh for the active/primary slot.
    pub fn refresh_weapon_fire_status(&mut self, current_time: f32) {
        // Pre-attack wind-up wins while armed.
        if self.pre_attack_ready_at > current_time + 1e-6 {
            self.weapon_fire_status = WeaponFireStatus::PreAttack;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        let slot = self.active_weapon_slot;
        let Some(weapon) = self.weapon_slot(slot) else {
            self.weapon_fire_status = WeaponFireStatus::OutOfAmmo;
            self.sync_weapon_model_conditions_from_status();
            return;
        };
        let name = self.weapon_name_for_slot(slot);
        let reload = self.effective_weapon_reload(weapon.reload_time);
        if !Self::weapon_has_ammo_for_shot(weapon, name) {
            self.weapon_fire_status = WeaponFireStatus::OutOfAmmo;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        if weapon.clip_size > 0 {
            let clip_reload = if weapon.clip_reload_time > 0.0 {
                weapon.clip_reload_time
            } else {
                reload
            };
            if current_time - weapon.last_fire_time < reload - 1e-6 {
                if weapon.ammo == Some(weapon.clip_size)
                    && clip_reload > reload + 1e-4
                    && current_time - weapon.last_fire_time < clip_reload
                {
                    self.weapon_fire_status = WeaponFireStatus::ReloadingClip;
                    self.sync_weapon_model_conditions_from_status();
                    return;
                }
                self.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
                self.sync_weapon_model_conditions_from_status();
                return;
            }
        } else if current_time - weapon.last_fire_time < reload - 1e-6 {
            self.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        self.weapon_fire_status = WeaponFireStatus::ReadyToFire;
        self.sync_weapon_model_conditions_from_status();
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        // C++ Object::canFireWeapon: DISABLED_SUBDUED / weapons_jammed residual.
        // Shock stun residual blocks weapon fire while flailing/stunned.
        if self.status.weapons_jammed || self.is_disabled() || self.is_shock_stunned() {
            return false;
        }
        let primary_name = self.thing.template.primary_weapon_name.clone();
        let secondary_name = self.thing.template.secondary_weapon_name.clone();
        let tertiary_name = self.thing.template.tertiary_weapon_name.clone();
        if let Some(weapon) = &self.weapon {
            let reload = self.effective_weapon_reload(weapon.reload_time);
            if Self::weapon_ready_named(weapon, current_time, primary_name.as_deref(), reload) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            let reload = self.effective_weapon_reload(weapon.reload_time);
            let name = secondary_name.as_deref().or(primary_name.as_deref());
            if Self::weapon_ready_named(weapon, current_time, name, reload) {
                return true;
            }
        }
        if let Some(weapon) = &self.tertiary_weapon {
            let reload = self.effective_weapon_reload(weapon.reload_time);
            if Self::weapon_ready_named(weapon, current_time, tertiary_name.as_deref(), reload) {
                return true;
            }
        }
        false
    }

    /// Fail-closed residual combat weapon choice (not full AutoChoose/PreferredAgainst).
    ///
    /// Slot: `0` = primary, `1` = secondary, `2` = tertiary.
    /// Rules:
    /// - Explicit player lock wins when its concrete slot is ready + in range.
    /// - PreferredAgainst residual (damage + kind heuristic, not full INI matrix):
    ///   - Structures: prefer secondary when damage ≥ primary (or primary cannot fire).
    ///   - Infantry: prefer secondary when damage > primary (FlashBang residual).
    ///   - Vehicles: prefer secondary when damage > primary (TOW residual).
    ///   - Neutron residual: active secondary with neutron upgrade vs infantry/vehicle
    ///     prefers secondary when player locked or secondary is the only ready slot;
    ///     also when primary cannot fire and secondary is ready.
    /// - Else primary when ready + in range; else secondary (alternate fire residual).
    pub fn select_combat_weapon_slot(&self, target: &Object, current_time: f32) -> Option<u8> {
        // C++ WeaponSet lock: locked slot wins while ready/in-range.
        if self.weapon_lock_type != WeaponLockType::NotLocked {
            let slot = self.weapon_lock_slot;
            if let Some(w) = self.weapon_slot(slot) {
                let target_faerie = target.is_faerie_fire();
                if self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                    && self.can_target_with_slot(target, w, Some(slot))
                {
                    return Some(slot);
                }
            }
            // Temporary lock may fall through if clip empty / not ready.
            if self.weapon_lock_type == WeaponLockType::LockedPermanently {
                return Some(self.weapon_lock_slot);
            }
        }
        let target_faerie = target.is_faerie_fire();
        let primary_ok = self.weapon.as_ref().is_some_and(|w| {
            self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                && self.can_target_with_slot(target, w, Some(0))
        });
        let secondary_ok = self.secondary_weapon.as_ref().is_some_and(|w| {
            self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                && self.can_target_with_slot(target, w, Some(1))
        });

        // Manual weapon-slot toggle (command residual).
        if self.active_weapon_slot == 2 {
            // TERTIARY is intentionally not an auto-choice candidate.  This
            // handles an explicit selected/locked manual weapon only.
            let tertiary_ok = self.tertiary_weapon.as_ref().is_some_and(|w| {
                self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                    && self.can_target_with_slot(target, w, Some(2))
            });
            if tertiary_ok {
                return Some(2);
            }
            return None;
        }

        if !primary_ok && !secondary_ok {
            return None;
        }

        if self.active_weapon_slot == 1 {
            if secondary_ok {
                return Some(1);
            }
            if primary_ok {
                return Some(0);
            }
            return None;
        }

        let target_is_structure =
            target.object_type == ObjectType::Building || target.is_kind_of(KindOf::Structure);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let target_is_vehicle =
            target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft);
        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        let primary_damage = self.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0);
        let secondary_damage = self
            .secondary_weapon
            .as_ref()
            .map(|w| w.damage)
            .unwrap_or(0.0);

        // SCUD residual: PreferredAgainst SECONDARY INFANTRY (toxin warhead)
        // even though secondary primary-damage is lower than explosive.
        let scud_prefer_toxin =
            crate::game_logic::host_scud_launcher::scud_prefer_secondary_vs_infantry(
                crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                    &self.template_name,
                ),
                target_is_infantry,
            );

        // Quad Cannon residual: airborne targets prefer AA secondary slot.
        let quad_prefer_aa =
            crate::game_logic::host_quad_cannon::is_quad_cannon_template(&self.template_name)
                && target_is_air;

        // Avenger residual: airborne targets prefer air laser secondary.
        let avenger_prefer_aa = crate::game_logic::host_avenger::avenger_prefer_air_laser(
            crate::game_logic::host_avenger::is_avenger_template(&self.template_name),
            target_is_air,
        );

        // Humvee residual: airborne targets prefer air TOW after TOW upgrade.
        let humvee_prefer_aa = crate::game_logic::host_humvee::humvee_prefer_air_tow(
            crate::game_logic::host_humvee::is_humvee_template(&self.template_name),
            self.has_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW)
                || self.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
            target_is_air,
        );

        if secondary_ok {
            if scud_prefer_toxin || quad_prefer_aa || avenger_prefer_aa || humvee_prefer_aa {
                return Some(1);
            }
            // PreferredAgainst residual by target kind + relative damage.
            if target_is_structure && (secondary_damage >= primary_damage || !primary_ok) {
                return Some(1);
            }
            if target_is_infantry && (secondary_damage > primary_damage || !primary_ok) {
                // FlashBang residual (35 > 5). Neutron secondary damage is 1.0 so
                // only wins here when primary cannot fire unless slot-locked.
                return Some(1);
            }
            if target_is_vehicle && (secondary_damage > primary_damage || !primary_ok) {
                // TOW residual (30 > 10 Humvee gun).
                return Some(1);
            }
        }

        // Default / alternate: primary first, then secondary if only it is ready.
        // TERTIARY intentionally is never chosen here: retail Comanche rocket
        // pods declare `AutoChooseSources = TERTIARY NONE`.
        if primary_ok {
            Some(0)
        } else if secondary_ok {
            Some(1)
        } else {
            None
        }
    }

    pub fn weapon_slot(&self, slot: u8) -> Option<&Weapon> {
        match slot {
            0 => self.weapon.as_ref(),
            1 => self.secondary_weapon.as_ref(),
            2 => self.tertiary_weapon.as_ref(),
            _ => None,
        }
    }

    pub fn weapon_slot_mut(&mut self, slot: u8) -> Option<&mut Weapon> {
        match slot {
            0 => self.weapon.as_mut(),
            1 => self.secondary_weapon.as_mut(),
            2 => self.tertiary_weapon.as_mut(),
            _ => None,
        }
    }

    /// C++ PartitionManager::getRelativeAngle2D residual to a world position.

    /// Normalize angle to (-PI, PI].
    pub fn normalize_angle_rad(a: f32) -> f32 {
        let mut x = a % (std::f32::consts::TAU);
        if x > std::f32::consts::PI {
            x -= std::f32::consts::TAU;
        } else if x <= -std::f32::consts::PI {
            x += std::f32::consts::TAU;
        }
        x
    }

    /// C++ TurretAI::friend_turnTowardsAngle residual.
    ///
    /// `desired_rel_rad` is desired world-relative aim angle of the body-to-target
    /// relative heading; host stores turret yaw in degrees absolute-ish residual
    /// matching Strategy Center path (body-relative when body ori is applied).
    /// Returns true when |angle - desired| <= rel_thresh.
    pub fn turn_turret_towards_angle_rad(
        &mut self,
        desired_rel_rad: f32,
        rate_modifier: f32,
        rel_thresh: f32,
    ) -> bool {
        let desired = Self::normalize_angle_rad(desired_rel_rad);
        let orig = self.turret_angle_deg.to_radians();
        let mut actual = Self::normalize_angle_rad(orig);
        let turn_rate = (self.turret_turn_rate_rad * rate_modifier.max(0.0)).max(0.0);
        let angle_diff = Self::normalize_angle_rad(desired - actual);
        if angle_diff.abs() < turn_rate {
            actual = desired;
            self.turret_rotating = false;
        } else {
            if angle_diff > 0.0 {
                actual += turn_rate;
            } else {
                actual -= turn_rate;
            }
            actual = Self::normalize_angle_rad(actual);
            self.turret_rotating = true;
        }
        self.turret_angle_deg = actual.to_degrees();
        let aligned = Self::normalize_angle_rad(actual - desired).abs() <= rel_thresh.max(0.0);
        self.record_host_turret();
        aligned
    }

    /// C++ TurretAI::setTurretTargetObject residual (object-local).

    /// C++ TurretAI::friend_turnTowardsPitch residual.
    pub fn turn_turret_towards_pitch_rad(
        &mut self,
        desired_pitch_rad: f32,
        rate_modifier: f32,
    ) -> bool {
        let desired = Self::normalize_angle_rad(desired_pitch_rad);
        let mut actual = Self::normalize_angle_rad(self.turret_pitch_deg.to_radians());
        let pitch_rate = (self.turret_turn_rate_rad * rate_modifier.max(0.0)).max(0.0);
        let diff = Self::normalize_angle_rad(desired - actual);
        if diff.abs() < pitch_rate {
            actual = desired;
        } else if diff > 0.0 {
            actual = Self::normalize_angle_rad(actual + pitch_rate);
        } else {
            actual = Self::normalize_angle_rad(actual - pitch_rate);
        }
        self.turret_pitch_deg = actual.to_degrees();
        let aligned = Self::normalize_angle_rad(actual - desired).abs() <= 1e-4;
        self.record_host_turret();
        aligned
    }

    pub fn set_turret_target_object(&mut self, victim: Option<ObjectId>, force_attacking: bool) {
        if !self.turret_enabled {
            return;
        }
        match victim {
            None => {
                self.turret_target_id = None;
                self.turret_force_attacking = false;
                if matches!(
                    self.turret_substate,
                    TurretSubState::Aim | TurretSubState::Fire
                ) {
                    self.turret_substate = TurretSubState::Hold;
                }
            }
            Some(id) => {
                self.turret_target_id = Some(id);
                self.turret_force_attacking = force_attacking;
                if !matches!(
                    self.turret_substate,
                    TurretSubState::Aim | TurretSubState::Fire
                ) {
                    self.turret_substate = TurretSubState::Aim;
                }
            }
        }
    }

    /// C++ TurretAI::isTryingToAimAtTarget residual.
    pub fn is_trying_to_aim_at_target(&self, victim: ObjectId) -> bool {
        self.turret_substate == TurretSubState::Aim && self.turret_target_id == Some(victim)
    }

    pub fn relative_angle_2d_to(&self, target_pos: Vec3) -> f32 {
        crate::game_logic::weapon_bootstrap::relative_angle_2d(
            self.get_position(),
            self.get_orientation(),
            target_pos,
        )
    }

    /// Resolve AcceptableAimDelta for the active/named weapon slot (radians).
    pub fn aim_delta_for_slot(&self, slot: u8) -> f32 {
        let name = self.weapon_name_for_slot(slot);
        name.map(crate::game_logic::weapon_bootstrap::host_aim_delta_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::AIM_DELTA_REL_THRESH_RAD)
    }

    /// C++ AIStates aim gate: facing within AcceptableAimDelta of target.
    pub fn is_aimed_at_position(&self, target_pos: Vec3, slot: u8) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        // Omni-fire residual (~180°): always aimed.
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim)
    }

    /// C++ setLocomotorGoalOrientation residual: rotate toward target (in-place turn).
    ///
    /// `max_step_rad` caps per-call turn (default generous for host residual).
    /// Returns true when already within aim delta after the step.
    pub fn turn_toward_position(&mut self, target_pos: Vec3, slot: u8, max_step_rad: f32) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        if crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim) {
            return true;
        }
        let step = max_step_rad.max(0.0);
        let turn = rel.clamp(-step, step);
        let new_ori = self.get_orientation() + turn;
        self.set_orientation(new_ori);
        let rel2 = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel2, aim)
    }

    /// C++ Weapon::getPreAttackDelay residual: whether PreAttackDelay applies this shot.
    pub fn pre_attack_delay_applies(
        &self,
        slot: u8,
        target_id: ObjectId,
        prefire: crate::game_logic::weapon_bootstrap::HostPrefireType,
        pre_delay: f32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::HostPrefireType;
        if pre_delay <= 0.0 {
            return false;
        }
        match prefire {
            HostPrefireType::PerShot => true,
            HostPrefireType::PerAttack => {
                // Only the first shot of an engagement against this victim.
                !(self.consecutive_shot_target == Some(target_id)
                    && self.consecutive_shots_at_target > 0)
            }
            HostPrefireType::PerClip => match self.weapon_slot(slot) {
                Some(w) if w.clip_size > 0 => {
                    let ammo = w.ammo.unwrap_or(w.clip_size);
                    ammo >= w.clip_size
                }
                // Unlimited clip residual: treat like per-shot.
                _ => true,
            },
        }
    }

    /// Record a successful discharge for PreAttackType PER_ATTACK bookkeeping.
    pub fn record_shot_at_target(&mut self, target_id: ObjectId) {
        if self.consecutive_shot_target == Some(target_id) {
            self.consecutive_shots_at_target = self.consecutive_shots_at_target.saturating_add(1);
            self.record_host_combat_attack();
        } else {
            self.consecutive_shot_target = Some(target_id);
            self.consecutive_shots_at_target = 1;
            self.record_host_combat_attack();
        }
        // PER_SHOT: force next fire_at to re-arm delay by clearing ready stamp into the past.
        self.pre_attack_ready_at = 0.0;
        self.record_host_combat_attack();
        self.update_continuous_fire_after_shot(target_id);
    }

    /// C++ FiringTracker continuous-fire MEAN/FAST residual (non-gattling path).
    /// Gattling buildings/tanks may overwrite level via specialized advance helpers.
    pub fn update_continuous_fire_after_shot(&mut self, target_id: ObjectId) {
        let one = self.continuous_fire_one_shots;
        let two = self.continuous_fire_two_shots;
        if one == 0 || one == u32::MAX {
            return;
        }
        let c = self.consecutive_shots_at_target;
        self.continuous_fire_victim = target_id.0;
        self.continuous_fire_consecutive = c;
        let level = self.continuous_fire_level;
        self.continuous_fire_level = if level == 1 {
            if c < one {
                0
            } else if two != u32::MAX && c > two {
                2
            } else {
                1
            }
        } else if level == 2 {
            if two != u32::MAX && c < two {
                0
            } else {
                2
            }
        } else if c > one {
            1
        } else {
            0
        };
        self.record_host_continuous_fire();
    }

    /// Stamp ContinuousFireCoast deadline after a shot (C++ m_frameToStartCoolDown).
    pub fn stamp_continuous_fire_coast(&mut self, frame: u32) {
        if self.continuous_fire_level == 0 {
            self.continuous_fire_coast_until_frame = 0;
            return;
        }
        let coast = self.continuous_fire_coast_frames;
        if coast == 0 {
            // No coast configured — keep spin until explicit cool-down.
            return;
        }
        self.continuous_fire_coast_until_frame = frame.saturating_add(coast);
    }

    /// C++ FiringTracker::update cool-down after ContinuousFireCoast idle.
    pub fn tick_continuous_fire_coast(&mut self, frame: u32) {
        self.tick_fire_sound_loop(frame);
        let _ = frame;
        self.tick_subdual_damage();
        if self.continuous_fire_level == 0 {
            return;
        }
        let until = self.continuous_fire_coast_until_frame;
        if until == 0 || frame < until {
            return;
        }
        // coolDown residual: clear MEAN/FAST straight to zero.
        self.continuous_fire_level = 0;
        self.continuous_fire_consecutive = 0;
        self.consecutive_shots_at_target = 0;
        self.consecutive_shot_target = None;
        self.continuous_fire_victim = 0;
        self.continuous_fire_coast_until_frame = 0;
        self.record_host_continuous_fire();
    }

    /// Stamp AutoReloadWhenIdle deadline after a shot (C++ m_frameToForceReload).
    pub fn stamp_auto_reload_when_idle(&mut self, frame: u32) {
        let delay = self.auto_reload_when_idle_frames;
        if delay == 0 {
            return;
        }
        // Only meaningful when clip is partially empty.
        let partial = self
            .weapon
            .as_ref()
            .is_some_and(|w| w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(false));
        if partial {
            self.frame_to_force_reload = frame.saturating_add(delay);
        } else {
            self.frame_to_force_reload = 0;
        }
    }

    /// C++ Object::reloadAllAmmo(TRUE) residual — refill all concrete WeaponSet clips.
    pub fn reload_all_ammo(&mut self) {
        for slot in [0u8, 1u8, 2u8] {
            if let Some(w) = self.weapon_slot_mut(slot) {
                if w.clip_size > 0 {
                    w.ammo = Some(w.clip_size);
                }
            }
        }
        self.frame_to_force_reload = 0;
    }

    /// C++ FiringTracker::update force-reload-when-idle residual.
    pub fn tick_force_reload_when_idle(&mut self, frame: u32) {
        let until = self.frame_to_force_reload;
        if until == 0 || frame < until {
            return;
        }
        let needs =
            self.weapon.as_ref().is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(true)
            }) || self.secondary_weapon.as_ref().is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(true)
            }) || self.tertiary_weapon.as_ref().is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(true)
            });
        if needs {
            self.reload_all_ammo();
        } else {
            self.frame_to_force_reload = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weapon(damage: f32) -> Weapon {
        Weapon {
            damage,
            range: 200.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        }
    }

    #[test]
    fn auto_chooser_never_promotes_a_tertiary_weapon() {
        let mut attacker = Object::new(
            ThingTemplate::new("ThreeSlotAttacker"),
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(weapon(5.0));
        attacker.tertiary_weapon = Some(weapon(999.0));
        let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));

        assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(0));

        attacker.weapon = None;
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.0),
            None,
            "a tertiary-only unit needs an explicit player slot selection"
        );

        attacker.set_active_weapon_slot(2);
        assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(2));
    }

    #[test]
    fn unknown_weapon_slot_fails_closed() {
        let mut object = Object::new(
            ThingTemplate::new("ThreeSlotAttacker"),
            ObjectId(1),
            Team::USA,
        );
        object.weapon = Some(weapon(5.0));
        assert!(object.weapon_slot(99).is_none());
        assert!(object.weapon_slot_mut(99).is_none());
        assert!(!object.set_weapon_lock(99, WeaponLockType::LockedPermanently));
    }
}
