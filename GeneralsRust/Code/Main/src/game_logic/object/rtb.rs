use super::*;

impl Object {
    /// Weapon ready on reload timer (not range).
    ///
    /// C++ AutoReloadsClip residual via weapon-name peel:
    /// - Auto: empty clip becomes ready after clip reload (refill on fire).
    /// - Manual / ReturnToBase: empty stays OUT_OF_AMMO until `rearm_weapon_full`.
    pub fn weapon_ready(weapon: &Weapon, current_time: f32) -> bool {
        // Without a name peel, treat as Auto (legacy).
        current_time - weapon.last_fire_time >= weapon.reload_time
            && Self::weapon_has_ammo_for_shot(weapon, None)
    }

    /// Name-aware ready check (preferred).
    /// `effective_reload` = WeaponBonus RATE_OF_FIRE adjusted interval (seconds).
    pub fn weapon_ready_named(
        weapon: &Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
        effective_reload: f32,
    ) -> bool {
        current_time - weapon.last_fire_time >= effective_reload
            && Self::weapon_has_ammo_for_shot(weapon, weapon_name)
    }

    pub fn weapon_has_ammo_for_shot(weapon: &Weapon, weapon_name: Option<&str>) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);
        match rt {
            HostReloadType::Auto => true,
            HostReloadType::Manual | HostReloadType::ReturnToBase => match weapon.ammo {
                Some(0) => false,
                Some(_) => true,
                None => true, // unlimited residual
            },
        }
    }

    /// C++ clip residual: consume one round.
    pub fn consume_ammo_on_fire(weapon: &mut Weapon, current_time: f32) {
        Self::consume_ammo_on_fire_named(weapon, current_time, None);
    }

    pub fn consume_ammo_on_fire_named(
        weapon: &mut Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
    ) {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        weapon.last_fire_time = current_time;
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);

        if weapon.clip_size == 0 {
            if let Some(a) = weapon.ammo.as_mut() {
                if *a > 0 {
                    *a -= 1;
                }
            }
            return;
        }

        match rt {
            HostReloadType::Auto => {
                if weapon.ammo == Some(0) || weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    *a = a.saturating_sub(1);
                    if *a == 0 {
                        // Ready check uses ClipReloadTime / RATE_OF_FIRE while
                        // ammo is empty. Keep last_fire_time at the shot so
                        // that interval is measured from this frame.
                        weapon.last_fire_time = current_time;
                    }
                }
            }
            HostReloadType::Manual | HostReloadType::ReturnToBase => {
                if weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    if *a > 0 {
                        *a = a.saturating_sub(1);
                    }
                }
            }
        }
    }

    /// Whether the shot just reached an auto-reload clip boundary.
    ///
    /// Retail `Weapon::fireWeapon` returns `true` when an auto-reloading
    /// clip was exhausted and immediately reloaded; callers use that edge to
    /// release a temporary WeaponSet lock.  The host represents that pending
    /// reload as an empty `ammo` counter until the next ready fire, so keep
    /// the equivalent test centralized here.  Manual and return-to-base
    /// clips intentionally do not report this edge.
    pub fn auto_reloaded_clip_after_firing(weapon: &Weapon, weapon_name: Option<&str>) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };

        if weapon.clip_size == 0 || weapon.ammo != Some(0) {
            return false;
        }
        let reload_type = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);
        reload_type == HostReloadType::Auto
    }

    pub fn rearm_weapon_full(weapon: &mut Weapon) {
        if weapon.clip_size > 0 {
            weapon.ammo = Some(weapon.clip_size);
        } else if let Some(a) = weapon.ammo {
            weapon.ammo = Some(a.max(1));
        }
        weapon.last_fire_time = -1.0e6;
    }

    pub(super) fn primary_weapon_name(&self) -> Option<&str> {
        if self.weapon_set_mine_clearing_detail && self.mine_clearing_primary_weapon.is_some() {
            return self
                .thing
                .template
                .mine_clearing_primary_weapon_name
                .as_deref();
        }
        self.thing.template.primary_weapon_name.as_deref()
    }

    pub(super) fn secondary_weapon_name(&self) -> Option<&str> {
        self.thing.template.secondary_weapon_name.as_deref().or(self
            .thing
            .template
            .primary_weapon_name
            .as_deref())
    }

    /// TERTIARY has no primary fallback: a missing third WeaponSet entry must
    /// remain unavailable rather than masquerading as the unit's main gun.
    pub(super) fn tertiary_weapon_name(&self) -> Option<&str> {
        self.thing.template.tertiary_weapon_name.as_deref()
    }

    /// C++ `JetOrHeliReloadAmmoState::onEnter` duration: max over slots of
    /// `clipReloadTime * (clipSize - remaining) / clipSize` (logic frames).
    /// Caller clamps to at least 1 frame when entering ReloadAmmo.
    pub fn airfield_rearm_clip_reload_frames(&self) -> u32 {
        let mut frames = 0u32;
        let consider = |w: &Weapon, acc: &mut u32| {
            if w.clip_size == 0 {
                return;
            }
            let remaining = w.ammo.unwrap_or(w.clip_size);
            let needed = w.clip_size.saturating_sub(remaining);
            if needed == 0 {
                return;
            }
            let raw = if w.clip_reload_time > 0.0 {
                (w.clip_reload_time * 30.0).round() as u32
            } else {
                0
            };
            let biased = (raw as u64 * needed as u64 / w.clip_size as u64) as u32;
            *acc = (*acc).max(biased);
        };
        if let Some(w) = self.weapon.as_ref() {
            consider(w, &mut frames);
        }
        if let Some(w) = self.secondary_weapon.as_ref() {
            consider(w, &mut frames);
        }
        if let Some(w) = self.tertiary_weapon.as_ref() {
            consider(w, &mut frames);
        }
        frames
    }

    /// C++ `Weapon::setClipPercentFull` (Weapon.cpp:1845) — floor, no reduce
    /// unless `allow_reduction`.
    pub fn set_weapon_clip_percent_full(
        weapon: &mut Weapon,
        percent: f32,
        allow_reduction: bool,
    ) {
        if weapon.clip_size == 0 {
            return;
        }
        let ammo = (weapon.clip_size as f32 * percent.clamp(0.0, 1.0)).floor() as u32;
        let current = weapon.ammo.unwrap_or(0);
        if ammo > current || (allow_reduction && ammo < current) {
            weapon.ammo = Some(ammo);
            weapon.last_fire_time = -1.0e6;
        }
    }

    fn weapons_need_clip_fill(&self) -> bool {
        let short = |w: &Weapon| w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(false);
        self.weapon.as_ref().is_some_and(short)
            || self.secondary_weapon.as_ref().is_some_and(short)
            || self.tertiary_weapon.as_ref().is_some_and(short)
    }

    /// C++ `JetOrHeliReloadAmmoState::onEnter` — remaining-biased duration, min 1.
    pub fn begin_parked_airfield_rearm(&mut self, now: u32) {
        if self.airfield_rearm_ready_frame.is_some() {
            return;
        }
        if !self.weapons_need_clip_fill() {
            return;
        }
        let frames = self.airfield_rearm_clip_reload_frames().max(1);
        self.airfield_rearm_duration_frames = frames;
        self.airfield_rearm_ready_frame = Some(now.saturating_add(frames));
    }

    /// C++ `JetOrHeliReloadAmmoState::update` progressive `setClipPercentFull`.
    /// Returns true when every clip is full.
    pub fn tick_parked_airfield_rearm(&mut self, now: u32) -> bool {
        let Some(done) = self.airfield_rearm_ready_frame else {
            return false;
        };
        let duration = self.airfield_rearm_duration_frames.max(1);
        let progress = if now >= done {
            1.0
        } else {
            let remaining = done - now;
            duration.saturating_sub(remaining) as f32 / duration as f32
        };
        if let Some(w) = self.weapon.as_mut() {
            Self::set_weapon_clip_percent_full(w, progress, false);
        }
        if let Some(w) = self.secondary_weapon.as_mut() {
            Self::set_weapon_clip_percent_full(w, progress, false);
        }
        if let Some(w) = self.tertiary_weapon.as_mut() {
            Self::set_weapon_clip_percent_full(w, progress, false);
        }
        if now >= done || !self.weapons_need_clip_fill() {
            if !self.weapons_need_clip_fill() {
                self.airfield_rearm_ready_frame = None;
                self.airfield_rearm_duration_frames = 0;
                return true;
            }
        }
        false
    }

    pub fn needs_return_to_base_rearm(&self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let empty_rtb = |w: &Weapon, name: Option<&str>| {
            let rt = name
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            rt == HostReloadType::ReturnToBase && matches!(w.ammo, Some(0))
        };
        self.weapon
            .as_ref()
            .is_some_and(|w| empty_rtb(w, self.primary_weapon_name()))
            || self
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| empty_rtb(w, self.secondary_weapon_name()))
            || self
                .tertiary_weapon
                .as_ref()
                .is_some_and(|w| empty_rtb(w, self.tertiary_weapon_name()))
    }

    /// C++ `JetAIUpdate::isOutOfSpecialReloadAmmo` — all ReturnToBase clips empty.
    pub fn is_out_of_special_reload_ammo(&self) -> bool {
        self.needs_return_to_base_rearm()
    }

    /// Attack orders keep flying; only idle / guard-hunt interrupt auto-RTB.
    pub fn jet_empty_clip_should_auto_rtb(&self) -> bool {
        if !self.needs_return_to_base_rearm() {
            return false;
        }
        if self.return_to_base_requested {
            return true;
        }
        if self.jet_ai.allow_interrupt_for_reload || self.jet_ai.has_pending_command {
            return true;
        }
        matches!(self.ai_state, AIState::Idle)
            && self.target.is_none()
            && !self.hunting
            && self.guard_position.is_none()
            && self.guard_target.is_none()
    }


    pub fn rearm_return_to_base_weapons(&mut self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let mut any = false;
        let pri = self.primary_weapon_name().map(|s| s.to_string());
        let sec = self.secondary_weapon_name().map(|s| s.to_string());
        let ter = self.tertiary_weapon_name().map(str::to_owned);
        if let Some(w) = self.weapon.as_mut() {
            let rt = pri
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        if let Some(w) = self.secondary_weapon.as_mut() {
            let rt = sec
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        if let Some(w) = self.tertiary_weapon.as_mut() {
            let rt = ter
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        any
    }

    /// C++ JetAIUpdate `OutOfAmmoDamagePerSecond` residual (fraction of max HP / sec).
    /// Retail JetAIUpdate OutOfAmmoDamagePerSecond = **10%**.
    pub const OUT_OF_AMMO_DAMAGE_PER_SECOND: f32 = 0.10;

    /// Apply one logic-frame of out-of-ammo damage while RTB weapons are empty.
    ///
    /// C++ JetOrHeliCirclingDeadAirfieldState:
    /// `damageRate = pct * SECONDS_PER_LOGICFRAME * maxHealth`, DAMAGE_UNRESISTABLE.
    /// Returns damage applied (0 if not eligible).
    pub fn apply_out_of_ammo_damage_frame(&mut self) -> f32 {
        if !self.is_alive() {
            return 0.0;
        }
        // Aircraft / jet residual only.
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return 0.0;
        }
        if !self.needs_return_to_base_rearm() {
            return 0.0;
        }
        // No damage while docked at airfield / garrisoned.
        if matches!(
            self.ai_state,
            AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
        ) {
            return 0.0;
        }

        const LOGIC_DT: f32 = 1.0 / 30.0;
        let max_hp = self.health.maximum.max(1.0);
        let dmg = Self::OUT_OF_AMMO_DAMAGE_PER_SECOND * LOGIC_DT * max_hp;
        if dmg <= 0.0 {
            return 0.0;
        }
        self.take_damage(dmg);
        dmg
    }

    /// Distance to another object (3D residual; pathfinding often 2D).
    pub fn distance_to_object(&self, other: &Object) -> f32 {
        self.get_position().distance(other.get_position())
    }

    /// Distance to world position.
    pub fn distance_to_pos(&self, pos: glam::Vec3) -> f32 {
        self.get_position().distance(pos)
    }

    /// C++ Weapon::isWithinAttackRange residual for one concrete WeaponSet slot.
    ///
    /// Unknown or unbound slots fail closed.  Tertiary deliberately has no
    /// inherited leech-range state: the host only persists the retail A/B
    /// leech flags, and aliasing C to either one would change another
    /// weapon's targeting behaviour.
    pub fn is_within_attack_range_for_slot(&self, slot: u8, other: &Object) -> bool {
        self.is_within_attack_range_at_distance(slot, self.distance_to_object(other))
    }

    /// C++ Weapon::isWithinAttackRange residual for a world position and one
    /// concrete WeaponSet slot.
    pub fn is_within_attack_range_pos_for_slot(&self, slot: u8, pos: glam::Vec3) -> bool {
        self.is_within_attack_range_at_distance(slot, self.distance_to_pos(pos))
    }

    fn is_within_attack_range_at_distance(&self, slot: u8, dist: f32) -> bool {
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        if weapon.min_range > 0.0 && dist + 1e-4 < weapon.min_range {
            return false;
        }
        if self.leech_range_active_for_slot(slot) {
            return true;
        }
        dist <= self.effective_weapon_range(weapon.range) + 1e-3
    }

    /// C++ Weapon::isWithinAttackRange residual across all concrete slots.
    /// When LeechRange is active for a slot, max range is waived (C++ hasLeechRange).
    /// Max range includes WeaponBonus RANGE field (garrison / SearchAndDestroy / …).
    pub fn is_within_attack_range(&self, other: &Object) -> bool {
        [0u8, 1u8, 2u8]
            .into_iter()
            .any(|slot| self.is_within_attack_range_for_slot(slot, other))
    }

    /// C++ Weapon::isWithinAttackRange for a position.
    pub fn is_within_attack_range_pos(&self, pos: glam::Vec3) -> bool {
        [0u8, 1u8, 2u8]
            .into_iter()
            .any(|slot| self.is_within_attack_range_pos_for_slot(slot, pos))
    }

    /// C++ canPursue residual (simplified — no turret matrix).

    /// C++ Weapon::hasLeechRange residual for one concrete slot.
    pub fn leech_range_active_for_slot(&self, slot: u8) -> bool {
        match slot {
            0 => self.leech_range_active_primary,
            1 => self.leech_range_active_secondary,
            // There is no persisted tertiary leech state.  Do not alias it.
            _ => false,
        }
    }

    /// C++ Weapon::hasLeechRange residual (primary or secondary active).
    pub fn leech_range_active(&self) -> bool {
        self.leech_range_active_primary || self.leech_range_active_secondary
    }

    pub fn can_pursue_target(&self, victim: &Object) -> bool {
        // Need victim physics (velocity).
        let victim_speed = victim.forward_speed_2d().abs();
        let our_max = self.effective_max_speed();
        if our_max <= 0.0 {
            return false;
        }
        // Crush residual: vehicles always pursue crushable infantry if AI computer — fail-closed skip player type.
        if self.can_crush_only(victim, false) {
            return true;
        }
        // Too close residual: min_range
        if let Some(w) = &self.weapon {
            let dist = self.distance_to_object(victim);
            if w.min_range > 0.0 && dist < w.min_range {
                return false;
            }
        }
        if victim_speed >= our_max {
            return false;
        }
        if victim_speed < our_max / 10.0 {
            return false;
        }
        // Victim moving away residual.
        let us = self.get_position();
        let them = victim.get_position();
        let dx = them.x - us.x;
        let dz = them.z - us.z;
        let vdir = victim.unit_direction_vector_2d();
        if dx * vdir.x + dz * vdir.y < 0.0 {
            return false; // moving toward us
        }
        true
    }

    /// Face toward a world position (AI_FACE_POSITION residual).
    pub fn face_position(&mut self, pos: glam::Vec3, dt: f32) -> bool {
        if !self.can_move() {
            return false;
        }
        let (_t, rel) = self.rotate_towards_position(pos, dt);
        rel.abs() < 0.05 // facing success residual (~3 deg)
    }

    /// Face toward another object.
    pub fn face_object(&mut self, other: &Object, dt: f32) -> bool {
        self.face_position(other.get_position(), dt)
    }
}
