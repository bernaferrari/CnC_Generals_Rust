use super::*;

impl Object {
    /// C++ `WeaponSet::getVictimAntiMask`, expressed against the host's
    /// semantic KindOf set. Ordering is material: small and ballistic
    /// missiles are also projectiles, but C++ chooses their more specific
    /// target mask first.
    pub fn weapon_target_anti_mask(&self) -> u32 {
        use gamelogic::weapon::WeaponAntiMask;

        if self.is_kind_of(KindOf::SmallMissile) {
            WeaponAntiMask::SMALL_MISSILE
        } else if self.is_kind_of(KindOf::BallisticMissile) {
            WeaponAntiMask::BALLISTIC_MISSILE
        } else if self.is_kind_of(KindOf::Projectile) {
            WeaponAntiMask::PROJECTILE
        } else if self.is_kind_of(KindOf::Mine)
            || self.is_kind_of(KindOf::DemoTrap)
            // Existing live mine objects predate semantic Object INI seeding;
            // keep their concrete mine data in the same C++ category.
            || self.is_disarmable_mine()
        {
            WeaponAntiMask::MINE | WeaponAntiMask::GROUND
        } else if self.status.airborne_target {
            if self.is_kind_of(KindOf::Vehicle) {
                WeaponAntiMask::AIRBORNE_VEHICLE
            } else if self.is_kind_of(KindOf::Infantry) {
                WeaponAntiMask::AIRBORNE_INFANTRY
            } else if self.is_kind_of(KindOf::Parachute) {
                WeaponAntiMask::PARACHUTE
            } else {
                // C++ debug-asserts for a non-UNATTACKABLE airborne object
                // without one of these categories, then fails closed.
                0
            }
        } else {
            WeaponAntiMask::GROUND
        }
    }

    /// Whether a concrete host weapon slot can affect a C++ WeaponSet victim
    /// anti-mask. Parsed Weapon.ini data is authoritative; hand-authored host
    /// weapons retain their legacy air/ground booleans only when no exact
    /// store template is available.
    pub fn weapon_allows_target_anti_mask(
        &self,
        weapon: &Weapon,
        slot: Option<u8>,
        target_anti_mask: u32,
    ) -> bool {
        use gamelogic::weapon::{WeaponAntiMask, with_weapon_store};

        if target_anti_mask == 0 {
            return false;
        }

        // Target-mask lookup must use the exact authored WeaponSet entry.
        // `secondary_weapon_name()` intentionally falls back to PRIMARY for
        // several legacy readiness paths, but that fallback is not a second
        // C++ WeaponSet declaration and must not make a hand-authored
        // secondary inherit PRIMARY's AntiMask.
        let authored_weapon_name = |slot| match slot {
            0 => self.primary_weapon_name(),
            1 => self.thing.template.secondary_weapon_name.as_deref(),
            2 => self.thing.template.tertiary_weapon_name.as_deref(),
            _ => None,
        };

        if let Some(template_mask) = slot.and_then(authored_weapon_name).and_then(|name| {
            with_weapon_store(|store| {
                store
                    .find_weapon_template(name)
                    .map(|template| template.get_anti_mask())
            })
            .ok()
            .flatten()
        }) {
            if (template_mask & target_anti_mask) != 0 {
                return true;
            }

            // `FIRE_WEAPON` at a map position is permitted by C++ even when
            // the selected detail weapon only has AntiMine.  The eventual
            // victim lookup still requires that exact MINE mask, so this
            // admits the target-position route without letting the mine
            // weapon harm ordinary ground objects.
            if slot == Some(0)
                && self.weapon_set_mine_clearing_detail
                && self.mine_clearing_primary_weapon.is_some()
                && target_anti_mask == WeaponAntiMask::GROUND
                && (template_mask & WeaponAntiMask::MINE) != 0
            {
                return true;
            }

            return false;
        }

        // Compatibility path for deliberately hand-authored/test weapons
        // that have no WeaponStore name. It never upgrades an unknown
        // projectile/missile category into a ground target.
        if (target_anti_mask & WeaponAntiMask::GROUND) != 0 {
            weapon.can_target_ground
        } else if (target_anti_mask
            & (WeaponAntiMask::AIRBORNE_VEHICLE
                | WeaponAntiMask::AIRBORNE_INFANTRY
                | WeaponAntiMask::PARACHUTE))
            != 0
        {
            weapon.can_target_air
        } else {
            false
        }
    }

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

    /// Restore the C++ client-only `Weapon::m_suspendFXFrame` parallel
    /// snapshot tail without exposing slot storage to save/load code.
    pub(crate) fn restore_weapon_suspend_fx_frame_for_slot(
        &mut self,
        slot: u8,
        frame: u32,
    ) -> bool {
        let weapon = match slot {
            0 => self.weapon.as_mut(),
            1 => self.secondary_weapon.as_mut(),
            2 => self.tertiary_weapon.as_mut(),
            _ => None,
        };
        let Some(weapon) = weapon else {
            return false;
        };
        weapon.set_suspend_fx_frame(frame);
        true
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
            // C++ getStatus is per-weapon; ShareWeaponReloadTime copies
            // RELOADING_CLIP onto siblings even while their clip is full.
            if weapon.reloading_clip {
                WeaponFireStatus::ReloadingClip
            } else if !Self::weapon_has_ammo_for_shot(weapon, name) {
                WeaponFireStatus::OutOfAmmo
            } else {
                let reload = self.effective_weapon_reload(weapon.reload_time);
                if current_time - weapon.last_fire_time < reload - 1e-6 {
                    WeaponFireStatus::BetweenFiringShots
                } else {
                    WeaponFireStatus::ReadyToFire
                }
            }
        };
        match status {
            WeaponFireStatus::OutOfAmmo | WeaponFireStatus::PreAttack => 0.0,
            WeaponFireStatus::ReadyToFire => 1.0,
            WeaponFireStatus::BetweenFiringShots | WeaponFireStatus::ReloadingClip => {
                let reload = if status == WeaponFireStatus::ReloadingClip {
                    // C++ reloadWithBonus uses ClipReloadTime verbatim; 0 is ready now.
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
            let remaining_zero = w.reloading_clip
                || (slot == self.active_weapon_slot
                    && self.weapon_fire_status == WeaponFireStatus::ReloadingClip)
                || (w.clip_size > 0 && w.ammo == Some(0));
            let full = if remaining_zero {
                0
            } else {
                w.ammo.unwrap_or(total).min(total)
            };
            return Some((total, full));
        }
        None
    }

    /// C++ `WeaponSet::findWaypointFollowingCapableWeapon` slot index.
    ///
    /// Scans TERTIARY, SECONDARY then PRIMARY (C++ WEAPONSLOT_COUNT-1 .. PRIMARY)
    /// on leftover store `CapableOfFollowingWaypoints`. Name seeds are never consulted.
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

    /// C++ `Weapon::reloadWithBonus` (Weapon.cpp:1884-1912) refills the clip
    /// at reload start. Live host defers that refill until `ClipReloadTime`
    /// elapses so `ammo == 0` can still mark `RELOADING_CLIP`. Completing here
    /// matches leftover `Weapon::update` late refill + C++ `getStatus` READY.
    fn complete_elapsed_auto_reload_clips(&mut self, current_time: f32) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            HostReloadType, host_reload_type_for_weapon_name,
        };
        let rof = self.weapon_bonus_fields().2;
        let mut completed = false;
        for slot in [0u8, 1, 2] {
            let name = self.weapon_name_for_slot(slot).map(str::to_owned);
            let reload_type = name
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if reload_type != HostReloadType::Auto {
                continue;
            }
            let Some(weapon) = self.weapon_slot(slot) else {
                continue;
            };
            if weapon.clip_size == 0 {
                continue;
            }
            let waiting = weapon.reloading_clip || weapon.ammo == Some(0);
            if !waiting {
                continue;
            }
            let reload = self.live_reload_interval(weapon, name.as_deref(), rof);
            if current_time - weapon.last_fire_time + 1e-6 < reload {
                continue;
            }
            if let Some(weapon) = self.weapon_slot_mut(slot) {
                weapon.ammo = Some(weapon.clip_size);
                weapon.reloading_clip = false;
            }
        }
        completed
    }

    /// C++ Weapon::getStatus residual refresh for the active/primary slot.
    pub fn refresh_weapon_fire_status(&mut self, current_time: f32) {
        self.apply_weapon_bonus_rof_restart(current_time);
        if self.complete_elapsed_auto_reload_clips(current_time) {
            self.record_host_weapon_stats();
        }
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
        let reload = self.live_reload_interval(weapon, name, self.weapon_bonus_fields().2);
        if !Self::weapon_has_ammo_for_shot(weapon, name) {
            self.weapon_fire_status = WeaponFireStatus::OutOfAmmo;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        if current_time - weapon.last_fire_time < reload - 1e-6 {
            if weapon.reloading_clip || (weapon.clip_size > 0 && weapon.ammo == Some(0)) {
                self.weapon_fire_status = WeaponFireStatus::ReloadingClip;
            } else {
                self.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
            }
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        self.weapon_fire_status = WeaponFireStatus::ReadyToFire;
        self.sync_weapon_model_conditions_from_status();
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        // C++ Object::canFireWeapon: DISABLED_SUBDUED / weapons_jammed residual.
        // Shock stun residual blocks weapon fire while flailing/stunned.
        // C++ Object::isAbleToAttack: UNDER_CONSTRUCTION / SOLD cannot fire.
        if self.status.weapons_jammed
            || self.is_disabled()
            || self.is_shock_stunned()
            || self.status.under_construction
            || self.status.sold
        {
            return false;
        }
        let primary_name = self.primary_weapon_name().map(str::to_owned);
        let secondary_name = self.thing.template.secondary_weapon_name.clone();
        let tertiary_name = self.thing.template.tertiary_weapon_name.clone();
        let rof = self.weapon_bonus_fields().2;
        if let Some(weapon) = self.weapon_slot(0) {
            let reload = self.live_reload_interval(weapon, primary_name.as_deref(), rof);
            if Self::weapon_ready_named(weapon, current_time, primary_name.as_deref(), reload) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            let name = secondary_name.as_deref().or(primary_name.as_deref());
            let reload = self.live_reload_interval(weapon, name, rof);
            if Self::weapon_ready_named(weapon, current_time, name, reload) {
                return true;
            }
        }
        if let Some(weapon) = &self.tertiary_weapon {
            let reload = self.live_reload_interval(weapon, tertiary_name.as_deref(), rof);
            if Self::weapon_ready_named(weapon, current_time, tertiary_name.as_deref(), reload) {
                return true;
            }
        }
        false
    }

    /// Combat weapon choice.
    ///
    /// Slot: `0` = primary, `1` = secondary, `2` = tertiary.
    /// Leftover `choose_best_weapon_for_target` gates: lock early-return,
    /// OUT_OF_AMMO+!AutoReloadsClip skip, pitch elimination, turret-aim
    /// ready suppression, zero-damage (DAMAGE_UNRESISTABLE exception),
    /// and unready-but-valid backup.
    pub fn select_combat_weapon_slot(&self, target: &Object, current_time: f32) -> Option<u8> {
        // C++ WeaponSet.cpp:782-783 / leftover weapon_set.rs:683-685 —
        // locked current slot stays until someone unlocks. A reloading
        // FireWeapon/flashbang/snipe waits; it does not auto-choose PRIMARY.
        if let Some(slot) = gamelogic::weapon::choose_best_locked_slot(
            self.weapon_lock_type != WeaponLockType::NotLocked,
            self.weapon_lock_slot,
        ) {
            return Some(slot);
        }
        let target_faerie = target.is_faerie_fire();
        let primary_valid = self.leftover_choose_best_slot_valid(0, target);
        let secondary_valid = self.leftover_choose_best_slot_valid(1, target);
        let primary_ok =
            self.leftover_choose_best_slot_ready(0, target, current_time, target_faerie);
        let secondary_ok =
            self.leftover_choose_best_slot_ready(1, target, current_time, target_faerie);

        // Manual weapon-slot toggle (command residual).
        if self.active_weapon_slot == 2 {
            // TERTIARY is intentionally not an auto-choice candidate.  This
            // handles an explicit selected/locked manual weapon only.
            let tertiary_ok = self.tertiary_weapon.as_ref().is_some_and(|w| {
                self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                    && self.can_target_with_slot(target, w, Some(2))
                    && self.is_slot_within_target_pitch(2, target)
                    && !self.leftover_choose_best_eliminates_zero_damage(2, target)
            });
            if tertiary_ok {
                return Some(2);
            }
            return None;
        }

        if !primary_ok && !secondary_ok {
            if gamelogic::weapon::choose_best_uses_backup(false, primary_valid || secondary_valid) {
                return self.leftover_choose_best_backup_slot(
                    target,
                    primary_valid,
                    secondary_valid,
                );
            }
            return None;
        }

        if self.active_weapon_slot == 1 {
            // Explicit secondary (flashbang / TOW) even if AutoChoose is NONE.
            let secondary_explicit = self.secondary_weapon.as_ref().is_some_and(|w| {
                self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                    && self.can_target_with_slot(target, w, Some(1))
                    && self.is_slot_within_target_pitch(1, target)
                    && !self.leftover_choose_best_eliminates_zero_damage(1, target)
            });
            if secondary_explicit {
                return Some(1);
            }
            if primary_ok {
                return Some(0);
            }
            return None;
        }

        // C++ WeaponSet.cpp:869-877 PreferredAgainst override. Primary wins
        // ties because C++ walks slots backwards and `damage >= best`.
        if self.ini_preferred_slot_usable(0, target) {
            return Some(0);
        }
        if self.ini_preferred_slot_usable(1, target) {
            return Some(1);
        }

        let target_is_structure =
            target.object_type == ObjectType::Building || target.is_kind_of(KindOf::Structure);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let target_is_vehicle =
            target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft);
        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        // C++ chooseBestWeaponForTarget compares estimateWeaponDamage
        // (armor-adjusted), not raw PrimaryDamage.
        let primary_damage = self.estimated_slot_damage_vs(0, target);
        let secondary_damage = self.estimated_slot_damage_vs(1, target);

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

    /// Leftover chooseBest fitness: AutoChoose + OUT_OF_AMMO+!autoReload
    /// + anti-mask + pitch + zero-damage.
    fn leftover_choose_best_slot_valid(&self, slot: u8, target: &Object) -> bool {
        if !self.thing.template.slot_allows_auto_choose(slot) {
            return false;
        }
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        // C++ WeaponSet.cpp:834-836 / leftover weapon_set.rs:732-737 —
        // empty clip that does not AutoReloadsClip is skipped entirely
        // (not even backup). Live maps that to weapon_has_ammo_for_shot
        // (HostReloadType::Manual | ReturnToBase + ammo == 0).
        if !Self::weapon_has_ammo_for_shot(weapon, self.weapon_name_for_slot(slot)) {
            return false;
        }
        // C++ WeaponSet.cpp:826-833: chooseBest elimination is anti-mask only.
        // The isWithinAttackRange check is deliberately commented out in C++
        // ("Being out of range does not mean this weapon can not affect the
        // target!") — the attack machine approaches out-of-range victims.
        if !self.weapon_allows_target_anti_mask(
            weapon,
            Some(slot),
            target.weapon_target_anti_mask(),
        ) {
            return false;
        }
        if !self.is_slot_within_target_pitch(slot, target) {
            return false;
        }
        !self.leftover_choose_best_eliminates_zero_damage(slot, target)
    }

    /// Leftover chooseBest ready: valid + READY_TO_FIRE, turret-aim demotes.
    fn leftover_choose_best_slot_ready(
        &self,
        slot: u8,
        target: &Object,
        current_time: f32,
        target_faerie: bool,
    ) -> bool {
        if !self.leftover_choose_best_slot_valid(slot, target) {
            return false;
        }
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        let status_ready = self.weapon_ready_vs_target_bonused(weapon, current_time, target_faerie);
        gamelogic::weapon::choose_best_ready_after_turret_aim(
            status_ready,
            self.is_weapon_slot_on_turret_and_aiming_at_target(slot, target),
        )
    }

    fn leftover_choose_best_eliminates_zero_damage(&self, slot: u8, target: &Object) -> bool {
        let damage = self.estimated_slot_damage_vs(slot, target);
        let damage_type = self
            .leftover_slot_damage_type(slot)
            .unwrap_or(gamelogic::damage::DamageType::Explosion);
        gamelogic::weapon::choose_best_eliminates_zero_damage(damage, damage_type)
    }

    fn leftover_slot_damage_type(&self, slot: u8) -> Option<gamelogic::damage::DamageType> {
        let name = self.weapon_name_for_slot(slot)?;
        if name.is_empty() {
            return None;
        }
        gamelogic::weapon::with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|wt| wt.get_damage_type())
        })
        .ok()
        .flatten()
    }

    /// Leftover PreferMostDamage backup: preferred first, then highest estimate.
    /// Primary wins ties (`damage >= best` while walking slots backwards).
    fn leftover_choose_best_backup_slot(
        &self,
        target: &Object,
        primary_valid: bool,
        secondary_valid: bool,
    ) -> Option<u8> {
        if self.ini_preferred_slot_usable(0, target) {
            return Some(0);
        }
        if self.ini_preferred_slot_usable(1, target) {
            return Some(1);
        }
        match (primary_valid, secondary_valid) {
            (true, true) => {
                let primary = self.estimated_slot_damage_vs(0, target);
                let secondary = self.estimated_slot_damage_vs(1, target);
                if secondary > primary {
                    Some(1)
                } else {
                    Some(0)
                }
            }
            (true, false) => Some(0),
            (false, true) => Some(1),
            (false, false) => None,
        }
    }

    /// Leftover chooseBest no-victim: lock keeps slot, else PRIMARY.
    pub fn leftover_choose_best_ground_slot(&self) -> u8 {
        gamelogic::weapon::choose_best_ground_attack_slot(
            self.weapon_lock_type != WeaponLockType::NotLocked,
            self.weapon_lock_slot,
        )
    }

    /// Leftover chooseBest no-victim: unlocked units reset PRIMARY.
    pub fn leftover_choose_best_reset_primary_for_ground(&mut self) {
        if gamelogic::weapon::choose_best_resets_primary_for_ground(
            self.weapon_lock_type != WeaponLockType::NotLocked,
        ) {
            self.set_active_weapon_slot(0);
        }
    }

    /// Leftover `Weapon::isWithinTargetPitch` via leftover-backed loft limits.
    fn is_slot_within_target_pitch(&self, slot: u8, victim: &Object) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_is_contact_weapon_name, host_target_pitch_limits_for_weapon_name,
            is_pitch_within_limits_geom,
        };
        let name = self.weapon_name_for_slot(slot).unwrap_or("");
        if !name.is_empty() && host_is_contact_weapon_name(name) {
            return true;
        }
        let limits = host_target_pitch_limits_for_weapon_name(name);
        let src = self.get_position();
        let tgt = victim.get_position();
        let src_half = self.thing.template.geometry_info.height.max(0.0) * 0.5;
        let tgt_above = victim
            .thing
            .template
            .geometry_info
            .max_height_above_position();
        let tgt_below = victim.thing.template.geometry_info.height.max(0.0) * 0.5;
        is_pitch_within_limits_geom(src, tgt, &limits, src_half, tgt_above, tgt_below)
    }

    /// Leftover `AIUpdateInterface::isWeaponSlotOnTurretAndAimingAtTarget`.
    pub fn is_weapon_slot_on_turret_and_aiming_at_target(&self, slot: u8, victim: &Object) -> bool {
        self.is_weapon_slot_on_turret(slot) && self.is_trying_to_aim_at_target(victim.id)
    }

    /// Leftover `TurretAI::isWeaponSlotOnTurret` (ControlledWeaponSlots bit).
    pub fn is_weapon_slot_on_turret(&self, slot: u8) -> bool {
        if !self.turret_enabled {
            return false;
        }
        let mask = self.leftover_turret_controlled_weapon_slots_mask();
        (mask & (1u32 << slot)) != 0
    }

    fn leftover_turret_controlled_weapon_slots_mask(&self) -> u32 {
        if let Some(mask) = leftover_parse_controlled_weapon_slots(&self.template_name) {
            return mask;
        }
        // leftover TurretAI default when INI is absent: PRIMARY.
        1
    }

    /// C++ WeaponSet.cpp:869-877 — PreferredAgainst KindOf match, ready unless
    /// the clip is empty (`OUT_OF_AMMO`). AutoChoose NONE slots stay button-only.
    fn ini_preferred_slot_usable(&self, slot: u8, target: &Object) -> bool {
        if !self.thing.template.slot_allows_auto_choose(slot) {
            return false;
        }
        if !self
            .thing
            .template
            .slot_preferred_against(slot, |kind| target.is_kind_of(kind))
        {
            return false;
        }
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        // C++ PreferredAgainst leg (WeaponSet.cpp:869-877) magnifies damage and
        // range; it never applies an in-range veto either.
        if !self.weapon_allows_target_anti_mask(
            weapon,
            Some(slot),
            target.weapon_target_anti_mask(),
        ) {
            return false;
        }
        if !self.is_slot_within_target_pitch(slot, target) {
            return false;
        }
        if self.leftover_choose_best_eliminates_zero_damage(slot, target) {
            return false;
        }
        !(weapon.clip_size > 0 && weapon.ammo == Some(0) && !weapon.reloading_clip)
    }

    /// C++ `Weapon::estimateWeaponDamage` for one live slot vs a victim.
    fn estimated_slot_damage_vs(&self, slot: u8, target: &Object) -> f32 {
        let Some(weapon) = self.weapon_slot(slot) else {
            return 0.0;
        };
        let name = self.weapon_name_for_slot(slot).unwrap_or("");
        // C++ WeaponSet.cpp:847-851 estimates the WEAPON TEMPLATE's
        // PrimaryDamage, not a host-stamped Object copy. Retail STATUS paint
        // weapons (AvengerTargetDesignator) carry their authored duration
        // damage in Weapon.ini; host paint stamps zero the Object copy for
        // the no-HP-damage residual, which must not feed chooseBest.
        let template_damage = gamelogic::weapon::with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|wt| wt.primary_damage)
        })
        .ok()
        .flatten()
        .filter(|d| *d > 0.0)
        .unwrap_or(weapon.damage);
        let est =
            crate::game_logic::weapon_bootstrap::host_estimate_weapon_from_name(name, template_damage);
        let dt = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(name);
        let victim = crate::game_logic::weapon_bootstrap::host_estimate_victim_from_object(
            target,
            target.garrison_count() as u32,
            crate::game_logic::host_armor_residual::apply_residual_armor(target, dt, 1.0),
        );
        crate::game_logic::weapon_bootstrap::estimate_weapon_template_damage(&est, Some(&victim))
    }

    /// C++ Weapon.cpp:2655-2665 / 1898-1909 ShareWeaponReloadTime.
    ///
    /// Copy the firing slot's next-shot time (`now + DelayBetweenShots` or
    /// clip reload) onto every slot. Ready checks still use each slot's own
    /// interval, so `last_fire` is back-computed to become ready at that
    /// shared frame.
    pub fn sync_shared_weapon_reload(&mut self) {
        if !self.thing.template.share_weapon_reload_time {
            return;
        }
        let rof = self.weapon_bonus_fields().2;
        let mut latest = f32::NEG_INFINITY;
        let mut firing_slot = 0u8;
        for slot in [0u8, 1, 2] {
            if let Some(weapon) = self.weapon_slot(slot) {
                if weapon.last_fire_time >= latest {
                    latest = weapon.last_fire_time;
                    firing_slot = slot;
                }
            }
        }
        if !latest.is_finite() {
            return;
        }
        let firing_interval = {
            let weapon = match self.weapon_slot(firing_slot) {
                Some(weapon) => weapon,
                None => return,
            };
            let name = self.weapon_name_for_slot(firing_slot);
            self.live_reload_interval(weapon, name, rof)
        };
        let next_ready =
            crate::game_logic::weapon_bootstrap::shared_next_ready_time(latest, firing_interval);
        // C++ `weapon->setStatus(RELOADING_CLIP)` / `BETWEEN_FIRING_SHOTS`
        // on every slot *before* ready checks re-read status.
        let firing_reloading = self.weapon_slot(firing_slot).is_some_and(|weapon| {
            weapon.reloading_clip || (weapon.clip_size > 0 && weapon.ammo == Some(0))
        });
        for slot in [0u8, 1, 2] {
            if let Some(weapon) = self.weapon_slot_mut(slot) {
                weapon.reloading_clip = firing_reloading;
            }
        }
        let mut stamp = [None; 3];
        for slot in [0u8, 1, 2] {
            if let Some(weapon) = self.weapon_slot(slot) {
                let name = self.weapon_name_for_slot(slot);
                let interval = self.live_reload_interval(weapon, name, rof);
                stamp[slot as usize] = Some(
                    crate::game_logic::weapon_bootstrap::last_fire_time_matching_shared_ready(
                        next_ready, interval,
                    ),
                );
            }
        }
        for slot in [0u8, 1, 2] {
            if let (Some(last_fire), Some(weapon)) =
                (stamp[slot as usize], self.weapon_slot_mut(slot))
            {
                weapon.last_fire_time = last_fire;
            }
        }
    }

    /// C++ `Weapon::onWeaponBonusChange` (Weapon.cpp:1935-1974).
    ///
    /// When RATE_OF_FIRE changes while a slot is still RELOADING_CLIP or
    /// BETWEEN_FIRING_SHOTS, C++ restarts that wait from *now* with the new
    /// bonus delay. ShareWeaponReloadTime then copies the new next-shot stamp
    /// and RELOADING_CLIP onto every sibling (1961-1972).
    pub(in crate::game_logic::object) fn apply_weapon_bonus_rof_restart(
        &mut self,
        current_time: f32,
    ) {
        let rof = self.weapon_bonus_fields().2;
        let mut restart_slots = [false; 3];
        let mut any_restart = false;
        for slot in [0u8, 1, 2] {
            let Some(weapon) = self.weapon_slot(slot) else {
                continue;
            };
            let prev = if weapon.last_bonus_rof <= 0.0 {
                1.0
            } else {
                weapon.last_bonus_rof
            };
            if (prev - rof).abs() <= 1e-4 {
                continue;
            }
            let name = self.weapon_name_for_slot(slot);
            if self.slot_waiting_on_bonus_delay(weapon, name, current_time, prev) {
                restart_slots[slot as usize] = true;
                any_restart = true;
            }
        }
        if any_restart {
            let share = self.thing.template.share_weapon_reload_time;
            for slot in [0u8, 1, 2] {
                if let Some(weapon) = self.weapon_slot_mut(slot) {
                    if share || restart_slots[slot as usize] {
                        weapon.last_fire_time = current_time;
                        if share {
                            weapon.reloading_clip = true;
                        }
                    }
                }
            }
        }
        for slot in [0u8, 1, 2] {
            if let Some(weapon) = self.weapon_slot_mut(slot) {
                weapon.last_bonus_rof = rof;
            }
        }
    }

    /// True when this slot is still inside a C++ RELOADING_CLIP / BETWEEN wait
    /// measured with `prev_rof`. Never-fired `last_fire_time == 0` is READY,
    /// not BETWEEN (C++ starts OUT_OF_AMMO then reloads to READY).
    fn slot_waiting_on_bonus_delay(
        &self,
        weapon: &Weapon,
        name: Option<&str>,
        current_time: f32,
        prev_rof: f32,
    ) -> bool {
        let clip_wait = weapon.reloading_clip || (weapon.clip_size > 0 && weapon.ammo == Some(0));
        if !clip_wait && weapon.last_fire_time <= 0.0 && self.last_fire_frame == 0 {
            return false;
        }
        let interval = self.live_reload_interval(weapon, name, prev_rof);
        current_time - weapon.last_fire_time < interval - 1e-6
    }

    pub fn weapon_slot(&self, slot: u8) -> Option<&Weapon> {
        match slot {
            0 if self.weapon_set_mine_clearing_detail => self
                .mine_clearing_primary_weapon
                .as_ref()
                .or(self.weapon.as_ref()),
            0 => self.weapon.as_ref(),
            1 => self.secondary_weapon.as_ref(),
            2 => self.tertiary_weapon.as_ref(),
            _ => None,
        }
    }

    pub fn weapon_slot_mut(&mut self, slot: u8) -> Option<&mut Weapon> {
        match slot {
            0 if self.weapon_set_mine_clearing_detail
                && self.mine_clearing_primary_weapon.is_some() =>
            {
                self.mine_clearing_primary_weapon.as_mut()
            }
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
        let now = crate::game_logic::host_historic_bonus::logic_frame();
        let within_coast = self.continuous_fire_coast_until_frame != 0
            && now < self.continuous_fire_coast_until_frame;
        if self.consecutive_shot_target == Some(target_id) || within_coast {
            self.consecutive_shots_at_target = self.consecutive_shots_at_target.saturating_add(1);
            self.consecutive_shot_target = Some(target_id);
            self.record_host_combat_attack();
        } else {
            self.consecutive_shot_target = Some(target_id);
            self.consecutive_shots_at_target = 1;
            self.record_host_combat_attack();
        }
        // PER_SHOT: force next fire_at to re-arm delay by clearing ready stamp into the past.
        self.pre_attack_ready_at = 0.0;
        self.record_host_combat_attack();
        self.sync_shared_weapon_reload();
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
            if two != u32::MAX && c < two { 0 } else { 2 }
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
    /// C++ FiringTracker::shotFired reads the FIRED weapon's template each shot
    /// (`weaponFired->getAutoReloadWhenIdleFrames()`). A zero delay on that
    /// weapon does not fall back to the PRIMARY construct-time bind.
    pub fn stamp_auto_reload_when_idle(&mut self, frame: u32) {
        self.stamp_auto_reload_when_idle_from_slot(self.last_fire_slot, frame);
    }

    /// Bind the force-reload delay from the weapon that just fired.
    pub fn stamp_auto_reload_when_idle_from_slot(&mut self, slot: u8, frame: u32) {
        let delay = match self.weapon_name_for_slot(slot) {
            Some(name) => {
                crate::game_logic::thing::ThingTemplate::weapon_tracker_from_store(name)
                    .auto_reload_when_idle_frames
            }
            None if slot == 0 => self.auto_reload_when_idle_frames,
            None => 0,
        };
        if delay == 0 {
            return;
        }
        let partial = [0u8, 1, 2].iter().any(|&slot| {
            self.weapon_slot(slot).is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(false)
            })
        });
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
                    w.reloading_clip = false;
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
            self.weapon_slot(0).is_some_and(|w| {
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

/// Leftover TurretAIData::parseTWS — `ControlledWeaponSlots = PRIMARY SECONDARY`.
fn leftover_parse_controlled_weapon_slots(template_name: &str) -> Option<u32> {
    let manager = crate::assets::get_asset_manager()?;
    let guard = manager.lock().ok()?;
    let definition = guard
        .get_object_definition(template_name)
        .or_else(|| guard.resolve_object_definition(template_name, None))?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module.attribute("ControlledWeaponSlots").is_some())?;
    let raw = module.attribute("ControlledWeaponSlots")?;
    let mut mask = 0u32;
    for token in raw.split_whitespace() {
        let bit = match token.to_ascii_uppercase().as_str() {
            "PRIMARY" => 1u32 << 0,
            "SECONDARY" => 1u32 << 1,
            "TERTIARY" => 1u32 << 2,
            _ => continue,
        };
        mask |= bit;
    }
    (mask != 0).then_some(mask)
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

    #[test]
    fn preferred_against_primary_infantry_beats_higher_damage_secondary() {
        // C++ WeaponSet.cpp:869-877 — Comanche cannon vs Hellfire: PRIMARY
        // PreferredAgainst INFANTRY wins even when secondary damage is larger.
        let mut attacker = Object::new(
            {
                let mut t = ThingTemplate::new("AmericaVehicleComanche");
                t.preferred_against[0] = vec![KindOf::Infantry];
                t
            },
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(weapon(10.0));
        attacker.secondary_weapon = Some(weapon(100.0));
        let mut target = Object::new(
            {
                let mut t = ThingTemplate::new("Infantry");
                t.add_kind_of(KindOf::Infantry);
                t
            },
            ObjectId(2),
            Team::GLA,
        );
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(0));
    }

    #[test]
    fn preferred_against_secondary_infantry_beats_higher_damage_primary() {
        // C++ SCUD toxin: PreferredAgainst SECONDARY INFANTRY.
        let mut attacker = Object::new(
            {
                let mut t = ThingTemplate::new("GLAVehicleSCUDLauncher");
                t.preferred_against[1] = vec![KindOf::Infantry];
                t
            },
            ObjectId(1),
            Team::GLA,
        );
        attacker.weapon = Some(weapon(300.0));
        attacker.secondary_weapon = Some(weapon(50.0));
        let mut target = Object::new(
            {
                let mut t = ThingTemplate::new("Infantry");
                t.add_kind_of(KindOf::Infantry);
                t
            },
            ObjectId(2),
            Team::USA,
        );
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(1));
    }

    #[test]
    fn share_weapon_reload_time_syncs_sibling_last_fire() {
        // C++ Weapon.cpp:2655-2665 ShareWeaponReloadTime copies next-shot frame.
        let mut attacker = Object::new(
            {
                let mut t = ThingTemplate::new("SharedReload");
                t.share_weapon_reload_time = true;
                t
            },
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(Weapon {
            last_fire_time: 4.0,
            ..weapon(10.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 0.0,
            ..weapon(10.0)
        });
        attacker.sync_shared_weapon_reload();
        assert_eq!(attacker.weapon.as_ref().unwrap().last_fire_time, 4.0);
        assert_eq!(
            attacker.secondary_weapon.as_ref().unwrap().last_fire_time,
            4.0
        );
    }

    #[test]
    fn share_weapon_reload_time_copies_firing_slot_next_shot() {
        // C++ Weapon.cpp:2655-2665: siblings wait the firing slot delay,
        // not their own DelayBetweenShots / ClipReloadTime.
        let mut attacker = Object::new(
            {
                let mut t = ThingTemplate::new("AmericaVehicleComanche");
                t.share_weapon_reload_time = true;
                t
            },
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(Weapon {
            last_fire_time: 4.0,
            reload_time: 0.2,
            ..weapon(10.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 0.0,
            reload_time: 3.0,
            ..weapon(10.0)
        });
        attacker.sync_shared_weapon_reload();
        let gun = attacker.weapon.as_ref().unwrap();
        let pods = attacker.secondary_weapon.as_ref().unwrap();
        let gun_ready = gun.last_fire_time + 0.2;
        let pods_ready = pods.last_fire_time + 3.0;
        assert!(
            (gun_ready - pods_ready).abs() < 1e-4,
            "{gun_ready} vs {pods_ready}"
        );
        assert!((gun.last_fire_time - 4.0).abs() < 1e-4);
    }

    #[test]
    fn auto_choose_none_secondary_is_not_picked_while_primary_reloads() {
        let mut attacker = Object::new(
            ThingTemplate::new("GLAInfantryJarmenKell"),
            ObjectId(1),
            Team::GLA,
        );
        assert!(
            !attacker.thing.template.slot_allows_auto_choose(1),
            "Jarmen secondary is AutoChooseSources NONE"
        );
        attacker.weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            ..weapon(10.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: -10.0,
            reload_time: 0.0,
            damage: 100.0,
            range: 200.0,
            ..Weapon::default()
        });
        let mut target = Object::new(ThingTemplate::new("Tank"), ObjectId(2), Team::USA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.1),
            Some(0),
            "leftover backup keeps reloading PRIMARY; AutoChoose NONE must not snipe"
        );
    }

    #[test]
    fn share_weapon_reload_marks_sibling_reloading_clip() {
        let mut attacker = Object::new(
            {
                let mut t = ThingTemplate::new("AmericaVehicleComanche");
                t.share_weapon_reload_time = true;
                t
            },
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(Weapon {
            last_fire_time: 4.0,
            clip_size: 12,
            ammo: Some(0),
            clip_reload_time: 2.0,
            reloading_clip: true,
            ..weapon(10.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 0.0,
            clip_size: 8,
            ammo: Some(8),
            clip_reload_time: 10.0,
            ..weapon(40.0)
        });
        attacker.sync_shared_weapon_reload();
        assert!(
            attacker.secondary_weapon.as_ref().unwrap().reloading_clip,
            "C++ setStatus(RELOADING_CLIP) on every sibling"
        );
        assert_eq!(attacker.secondary_weapon.as_ref().unwrap().ammo, Some(8));
        attacker.active_weapon_slot = 1;
        attacker.refresh_weapon_fire_status(4.5);
        assert!(
            attacker.secondary_weapon.as_ref().unwrap().reloading_clip,
            "sibling ReloadingClip must survive getStatus refresh"
        );
        assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReloadingClip);
    }

    #[test]
    fn clip_reload_elapse_refills_and_clears_reloading() {
        let mut attacker = Object::new(ThingTemplate::new("ClipWait"), ObjectId(1), Team::USA);
        attacker.weapon = Some(Weapon {
            last_fire_time: 1.0,
            clip_size: 4,
            ammo: Some(0),
            clip_reload_time: 1.0,
            reloading_clip: true,
            reload_time: 0.2,
            ..weapon(10.0)
        });
        attacker.refresh_weapon_fire_status(1.5);
        assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReloadingClip);
        assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(0));
        attacker.refresh_weapon_fire_status(2.05);
        assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(4));
        assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
        assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    }

    #[test]
    fn clip_reload_time_zero_is_ready_same_frame() {
        let mut attacker = Object::new(ThingTemplate::new("InstantClip"), ObjectId(1), Team::USA);
        attacker.weapon = Some(Weapon {
            last_fire_time: 1.0,
            clip_size: 2,
            ammo: Some(0),
            clip_reload_time: 0.0,
            reloading_clip: true,
            reload_time: 1.0,
            ..weapon(10.0)
        });
        attacker.refresh_weapon_fire_status(1.0);
        assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(2));
        assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
        assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    }

    #[test]
    fn rof_bonus_change_restarts_in_progress_reload() {
        let mut attacker = Object::new(ThingTemplate::new("RofWait"), ObjectId(1), Team::USA);
        attacker.weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            last_bonus_rof: 1.0,
            ..weapon(10.0)
        });
        attacker.weapon_bonus_veteran = true;
        attacker.refresh_weapon_fire_status(1.5);
        let last = attacker.weapon.as_ref().unwrap().last_fire_time;
        assert!(
            (last - 1.5).abs() < 1e-4,
            "C++ onWeaponBonusChange restarts the wait from now, last={last}"
        );
    }

    #[test]
    fn weaponset_change_unlocks_unless_shared_across_sets() {
        let mut attacker = Object::new(
            ThingTemplate::new("AmericaVehicleHumvee"),
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(weapon(10.0));
        attacker.secondary_weapon = Some(weapon(30.0));
        assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedPermanently));
        attacker.apply_veterancy_bonuses(
            crate::game_logic::VeterancyLevel::Rookie,
            crate::game_logic::VeterancyLevel::Veteran,
        );
        assert!(!attacker.is_weapon_locked());
        assert_eq!(attacker.active_weapon_slot, 0);

        attacker.thing.template.weapon_lock_shared_across_sets = true;
        assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedPermanently));
        attacker.apply_veterancy_bonuses(
            crate::game_logic::VeterancyLevel::Veteran,
            crate::game_logic::VeterancyLevel::Elite,
        );
        assert!(attacker.is_weapon_locked());
        assert_eq!(attacker.weapon_lock_slot, 1);
    }

    #[test]
    fn retarget_inside_continuous_fire_coast_keeps_consecutive_shots() {
        let mut attacker = Object::new(
            ThingTemplate::new("ChinaGattlingTank"),
            ObjectId(1),
            Team::China,
        );
        attacker.consecutive_shot_target = Some(ObjectId(2));
        attacker.consecutive_shots_at_target = 5;
        attacker.continuous_fire_coast_until_frame = u32::MAX;
        attacker.record_shot_at_target(ObjectId(3));
        assert_eq!(attacker.consecutive_shots_at_target, 6);
        assert_eq!(attacker.consecutive_shot_target, Some(ObjectId(3)));

        attacker.continuous_fire_coast_until_frame = 0;
        attacker.record_shot_at_target(ObjectId(4));
        assert_eq!(attacker.consecutive_shots_at_target, 1);
        assert_eq!(attacker.consecutive_shot_target, Some(ObjectId(4)));
    }

    #[test]
    fn leftover_choose_best_skips_zero_damage_unless_unresistable() {
        let mut attacker =
            Object::new(ThingTemplate::new("ZeroDmgChooser"), ObjectId(1), Team::USA);
        attacker.weapon = Some(weapon(0.0));
        attacker.secondary_weapon = Some(weapon(10.0));
        let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.0),
            Some(1),
            "zero-damage PRIMARY is eliminated so SECONDARY wins"
        );

        const UNRES: &str = "__RustChooseBestZeroUnresistable";
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(UNRES.to_string());
            template.primary_damage = 0.0;
            template.damage_type = gamelogic::damage::DamageType::Unresistable;
            template.attack_range = 200.0;
            store.add_weapon_template(template);
        });
        attacker.thing.template.set_primary_weapon_name(UNRES);
        attacker.weapon = Some(weapon(0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.0),
            Some(0),
            "DAMAGE_UNRESISTABLE may keep a zero-damage slot"
        );
    }

    #[test]
    fn leftover_choose_best_turret_aim_demotes_ready_slot() {
        let mut attacker = Object::new(ThingTemplate::new("TurretChooser"), ObjectId(1), Team::USA);
        attacker.weapon = Some(weapon(100.0));
        attacker.secondary_weapon = Some(weapon(10.0));
        attacker.turret_enabled = true;
        attacker.turret_substate = TurretSubState::Aim;
        attacker.turret_target_id = Some(ObjectId(2));
        let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert!(attacker.is_weapon_slot_on_turret_and_aiming_at_target(0, &target));
        assert!(!attacker.is_weapon_slot_on_turret_and_aiming_at_target(1, &target));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.0),
            Some(1),
            "aiming turret PRIMARY is demoted so hull SECONDARY can fire"
        );
    }

    #[test]
    fn leftover_choose_best_skips_slot_outside_target_pitch() {
        const LOFT: &str = "__RustChooseBestLoftLimited";
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(LOFT.to_string());
            template.primary_damage = 100.0;
            template.attack_range = 200.0;
            template.min_target_pitch = (-15f32).to_radians();
            template.max_target_pitch = 15f32.to_radians();
            store.add_weapon_template(template);
        });
        let mut t = ThingTemplate::new("PitchChooser");
        t.set_primary_weapon_name(LOFT);
        let mut attacker = Object::new(t, ObjectId(1), Team::USA);
        attacker.weapon = Some(weapon(100.0));
        attacker.secondary_weapon = Some(weapon(10.0));
        let mut target = Object::new(ThingTemplate::new("HighTarget"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(10.0, 50.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.0),
            Some(1),
            "loft-failing PRIMARY is eliminated so SECONDARY wins"
        );
    }

    #[test]
    fn leftover_choose_best_backup_picks_unready_valid_slot() {
        let mut attacker = Object::new(ThingTemplate::new("BackupChooser"), ObjectId(1), Team::USA);
        attacker.weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            ..weapon(10.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            ..weapon(40.0)
        });
        let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.1),
            Some(1),
            "when every auto-choose slot is mid-reload leftover keeps the best backup"
        );
    }

    #[test]
    fn leftover_choose_best_skips_empty_no_auto_reload_slot() {
        // C++ WeaponSet.cpp:834-836: OUT_OF_AMMO && !getAutoReloadsClip()
        // continues past the slot; leftover weapon_set.rs:732-737 ports it.
        // An empty no-auto-reload PRIMARY must not win leftover backup.
        const EMPTY: &str = "__RustChooseBestEmptyNoReload";
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(EMPTY.to_string());
            template.primary_damage = 100.0;
            template.attack_range = 200.0;
            template.reload_type = gamelogic::weapon::WeaponReloadType::NoReload;
            store.add_weapon_template(template);
        });
        let mut t = ThingTemplate::new("EmptyNoReloadChooser");
        t.set_primary_weapon_name(EMPTY);
        let mut attacker = Object::new(t, ObjectId(1), Team::USA);
        attacker.weapon = Some(Weapon {
            ammo: Some(0),
            clip_size: 1,
            last_fire_time: 1.0,
            reload_time: 1.0,
            ..weapon(100.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            ..weapon(10.0)
        });
        let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.1),
            Some(1),
            "empty no-auto-reload PRIMARY is skipped so leftover backup keeps SECONDARY"
        );
    }

    #[test]
    fn leftover_choose_best_temp_lock_keeps_unready_slot() {
        // C++ WeaponSet.cpp:782-783: isCurWeaponLocked → keep current slot.
        // Reloading FireWeapon/flashbang/snipe must not fall through to PRIMARY.
        let mut attacker = Object::new(
            ThingTemplate::new("AmericaInfantryRanger"),
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(Weapon {
            last_fire_time: -10.0,
            reload_time: 0.0,
            ..weapon(5.0)
        });
        attacker.secondary_weapon = Some(Weapon {
            last_fire_time: 1.0,
            reload_time: 1.0,
            ammo: Some(0),
            clip_size: 1,
            ..weapon(35.0)
        });
        assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
        let mut target = Object::new(
            {
                let mut t = ThingTemplate::new("Infantry");
                t.add_kind_of(KindOf::Infantry);
                t
            },
            ObjectId(2),
            Team::GLA,
        );
        target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(
            attacker.select_combat_weapon_slot(&target, 1.1),
            Some(1),
            "temp-locked reloading SECONDARY must wait, not auto-choose PRIMARY"
        );
    }

    #[test]
    fn leftover_choose_best_ground_resets_primary_unless_locked() {
        let mut attacker = Object::new(
            ThingTemplate::new("AmericaVehicleHumvee"),
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(weapon(10.0));
        attacker.secondary_weapon = Some(weapon(30.0));
        attacker.set_active_weapon_slot(1);
        assert_eq!(attacker.leftover_choose_best_ground_slot(), 0);
        attacker.leftover_choose_best_reset_primary_for_ground();
        assert_eq!(attacker.active_weapon_slot, 0);

        assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
        assert_eq!(attacker.leftover_choose_best_ground_slot(), 1);
        attacker.leftover_choose_best_reset_primary_for_ground();
        assert_eq!(attacker.active_weapon_slot, 1);
        assert_eq!(attacker.weapon_lock_slot, 1);
    }
}
