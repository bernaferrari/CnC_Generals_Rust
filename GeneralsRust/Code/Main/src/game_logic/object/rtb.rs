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
            HostReloadType, host_reload_type_for_weapon_name,
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
            HostReloadType, host_reload_type_for_weapon_name,
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
            HostReloadType, host_reload_type_for_weapon_name,
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
    pub fn set_weapon_clip_percent_full(weapon: &mut Weapon, percent: f32, allow_reduction: bool) {
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
        let short =
            |w: &Weapon| w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(false);
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
            HostReloadType, host_reload_type_for_weapon_name,
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

    /// C++ `Object::isOutOfAmmo` / leftover `WeaponSet::isOutOfAmmo`.
    /// Present empty clips (not ReloadingClip) are OUT_OF_AMMO. A host unit
    /// with no weapon slots is not exhausted (uninitialized clip ≠ empty clip).
    pub fn is_out_of_ammo(&self) -> bool {
        let slot_out = |w: &Weapon| w.ammo == Some(0) && !w.reloading_clip;
        let mut any = false;
        if let Some(w) = self.weapon.as_ref() {
            any = true;
            if !slot_out(w) {
                return false;
            }
        }
        if let Some(w) = self.secondary_weapon.as_ref() {
            any = true;
            if !slot_out(w) {
                return false;
            }
        }
        if let Some(w) = self.tertiary_weapon.as_ref() {
            any = true;
            if !slot_out(w) {
                return false;
            }
        }
        any
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
            HostReloadType, host_reload_type_for_weapon_name,
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

    /// C++ `FROM_BOUNDINGSPHERE_2D` (Weapon.cpp ATTACK_RANGE_IS_2D).
    /// Host ground plane is XZ (Y-up); subtract both bounding-circle radii.
    pub fn distance_to_object(&self, other: &Object) -> f32 {
        let a = self.get_position();
        let b = other.get_position();
        let dx = a.x - b.x;
        let dz = a.z - b.z;
        let center = (dx * dx + dz * dz).sqrt();
        let ra = self.thing.template.geometry_info.bounding_circle_radius();
        let rb = other.thing.template.geometry_info.bounding_circle_radius();
        (center - ra - rb).max(0.0)
    }

    /// Distance to world position.
    pub fn distance_to_pos(&self, pos: glam::Vec3) -> f32 {
        let a = self.get_position();
        let dx = a.x - pos.x;
        let dz = a.z - pos.z;
        let center = (dx * dx + dz * dz).sqrt();
        let ra = self.thing.template.geometry_info.bounding_circle_radius();
        (center - ra).max(0.0)
    }

    /// C++ Weapon::isWithinAttackRange after FROM_BOUNDINGSPHERE_2D distance.
    pub fn is_within_attack_range_at_distance(&self, slot: u8, dist: f32) -> bool {
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

    /// C++ Weapon::isWithinAttackRange for a world position (no victim radius).
    pub fn is_within_attack_range_pos_for_slot(&self, slot: u8, pos: glam::Vec3) -> bool {
        self.is_within_attack_range_at_distance(slot, self.distance_to_pos(pos))
    }

    /// C++ Weapon::isWithinAttackRange residual for one concrete WeaponSet slot.
    ///
    /// Unknown or unbound slots fail closed.  Tertiary deliberately has no
    /// inherited leech-range state: the host only persists the retail A/B
    /// leech flags, and aliasing C to either one would change another
    /// weapon's targeting behaviour.
    ///
    /// `KINDOF_BRIDGE`: nearer abutment (C++ `getBridgeAttackPoints`).
    /// Contact vs `KINDOF_STRUCTURE`: numeric range then geometry overlap
    /// (`iteratePotentialCollisions`).
    pub fn is_within_attack_range_for_slot(&self, slot: u8, other: &Object) -> bool {
        if other.is_kind_of(KindOf::Bridge) {
            return self.is_within_attack_range_for_slot_bridge(slot, other);
        }
        if !self.is_within_attack_range_at_distance(slot, self.distance_to_object(other)) {
            return false;
        }
        if other.is_kind_of(KindOf::Structure) && self.slot_is_contact_weapon(slot) {
            return self.contact_weapon_touches_structure(other);
        }
        true
    }

    /// C++ `isSourceObjectWithGoalPositionWithinAttackRange` (Weapon.cpp:2110).
    /// Distance from a fake source pose (garrison FIREPOINT) using this
    /// object's bounding-circle radius, not the container origin.
    pub fn is_within_attack_range_for_slot_from_goal(
        &self,
        slot: u8,
        goal: glam::Vec3,
        other: &Object,
    ) -> bool {
        let ra = self.thing.template.geometry_info.bounding_circle_radius();
        let rb = other.thing.template.geometry_info.bounding_circle_radius();
        let b = other.get_position();
        let dx = goal.x - b.x;
        let dz = goal.z - b.z;
        let dist = ((dx * dx + dz * dz).sqrt() - ra - rb).max(0.0);
        self.is_within_attack_range_at_distance(slot, dist)
    }

    /// Goal-pose range to a world position (no victim radius).
    pub fn is_within_attack_range_pos_for_slot_from_goal(
        &self,
        slot: u8,
        goal: glam::Vec3,
        pos: glam::Vec3,
    ) -> bool {
        let ra = self.thing.template.geometry_info.bounding_circle_radius();
        let dx = goal.x - pos.x;
        let dz = goal.z - pos.z;
        let dist = ((dx * dx + dz * dz).sqrt() - ra).max(0.0);
        self.is_within_attack_range_at_distance(slot, dist)
    }

    fn slot_is_contact_weapon(&self, slot: u8) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_is_contact_weapon_name, is_contact_effective_range,
        };
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        self.weapon_name_for_slot(slot)
            .is_some_and(host_is_contact_weapon_name)
            || is_contact_effective_range(weapon.range)
    }

    /// C++ Weapon.cpp:2161-2171 — try abutment 1, then 2 if out of max range.
    fn is_within_attack_range_for_slot_bridge(&self, slot: u8, other: &Object) -> bool {
        let Some(weapon) = self.weapon_slot(slot) else {
            return false;
        };
        let max_range = if self.leech_range_active_for_slot(slot) {
            f32::MAX
        } else {
            self.effective_weapon_range(weapon.range) + 1e-3
        };
        let pos = other.get_position();
        let half = other.selection_radius.max(20.0);
        let a = glam::Vec3::new(pos.x - half, pos.y, pos.z);
        let b = glam::Vec3::new(pos.x + half, pos.y, pos.z);
        let d1 = self.distance_to_pos(a);
        let dist = if d1 <= max_range {
            d1
        } else {
            self.distance_to_pos(b)
        };
        if weapon.min_range > 0.0 && dist + 1e-4 < weapon.min_range {
            return false;
        }
        dist <= max_range
    }

    /// C++ `iteratePotentialCollisions` after the contact numeric range test.
    fn contact_weapon_touches_structure(&self, other: &Object) -> bool {
        use crate::game_logic::host_squish_collide::authored_crusher_geometry;
        use gamelogic::object::collide::{
            CollideInfo, CollideLocAndNormal, Coord3D, collide_test_dispatch,
        };
        let src_info = &self.thing.template.geometry_info;
        let tgt_info = &other.thing.template.geometry_info;
        let g1 = authored_crusher_geometry(
            src_info,
            src_info.bounding_circle_radius().max(1.0),
            src_info.height.max(1.0),
        );
        let g2 = authored_crusher_geometry(
            tgt_info,
            tgt_info.bounding_circle_radius().max(1.0),
            tgt_info.height.max(1.0),
        );
        let p1 = self.get_position();
        let p2 = other.get_position();
        let info_a = CollideInfo::new(Coord3D::new(p1.x, p1.z, p1.y), g1, self.get_orientation());
        let info_b = CollideInfo::new(Coord3D::new(p2.x, p2.z, p2.y), g2, other.get_orientation());
        if info_a.position.z + info_a.geom.get_max_height_above_position() < info_b.position.z
            || info_a.position.z > info_b.position.z + info_b.geom.get_max_height_above_position()
        {
            return false;
        }
        let mut cinfo =
            CollideLocAndNormal::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0));
        collide_test_dispatch(
            g1.get_geom_type(),
            g2.get_geom_type(),
            &info_a,
            &info_b,
            Some(&mut cinfo),
        )
    }

    /// C++ `WeaponSet::isAnyWithinTargetPitch` (WeaponSet.cpp:406-419).
    /// Unlimited peels pass; if any candidate is loft-limited, one must fit.
    pub fn is_any_within_target_pitch_for_slots(&self, victim: &Object, slots: &[u8]) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_target_pitch_limits_for_weapon_name, is_pitch_within_limits_geom,
        };
        let src = self.get_position();
        let tgt = victim.get_position();
        let src_half = self.thing.template.geometry_info.height.max(0.0) * 0.5;
        let tgt_above = victim
            .thing
            .template
            .geometry_info
            .max_height_above_position();
        let tgt_below = victim.thing.template.geometry_info.height.max(0.0) * 0.5;
        let mut any_limited = false;
        for &slot in slots {
            if self.weapon_slot(slot).is_none() {
                continue;
            }
            let name = self.weapon_name_for_slot(slot).unwrap_or("");
            let limits = host_target_pitch_limits_for_weapon_name(name);
            if limits.is_unlimited() {
                continue;
            }
            any_limited = true;
            if is_pitch_within_limits_geom(src, tgt, &limits, src_half, tgt_above, tgt_below) {
                return true;
            }
        }
        !any_limited
    }

    /// C++ `GarrisonContain::calcBestGarrisonPosition` for an enclosing window.
    /// `None` when the container is not an enclosing garrison or has no FIREPOINTs.
    pub fn enclosing_garrison_fire_goal(
        container: &Object,
        occupant_id: ObjectId,
        target: glam::Vec3,
    ) -> Option<glam::Vec3> {
        if !container.is_garrison_contain() || !container.is_enclosing_garrison_container() {
            return None;
        }
        let bd = container.building_data.as_ref()?;
        if bd.garrison_fire_points.is_empty() {
            return None;
        }
        let mut best: Option<(f32, glam::Vec3)> = None;
        for (i, p) in bd.garrison_fire_points.iter().enumerate() {
            let taken = bd.garrison_point_occupant.get(i).and_then(|id| *id);
            if taken.is_some() && taken != Some(occupant_id) {
                continue;
            }
            let d = (*p - target).length_squared();
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, *p));
            }
        }
        best.map(|(_, p)| p)
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

    /// C++ `AIUpdateInterface::setLocomotorGoalOrientation`.
    pub fn set_locomotor_goal_orientation(&mut self, angle: f32) {
        self.locomotor_goal_type = LocoGoalType::Angle;
        self.locomotor_goal_angle = angle;
    }

    /// C++ `AIUpdateInterface::setLocomotorGoalPositionExplicit`.
    pub fn set_locomotor_goal_position_explicit(&mut self, pos: glam::Vec3) {
        self.locomotor_goal_type = LocoGoalType::PositionExplicit;
        self.movement.target_position = Some(pos);
    }

    /// C++ `AIUpdateInterface::setLocomotorGoalNone`.
    pub fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = LocoGoalType::None;
    }

    /// C++ `AIFaceState::update` goal select — ANGLE vs POSITION_EXPLICIT.
    /// Does not integrate; `doLocomotor` / `tick_face_towards` leftover-march.
    pub fn arm_face_locomotor_goal(&mut self, target_pos: glam::Vec3) -> bool {
        let rel = self.relative_angle_2d_to(target_pos);
        if rel.abs() < FACE_REL_THRESH_RAD {
            self.face_active = false;
            self.set_locomotor_goal_none();
            return false;
        }
        if self.face_can_turn_in_place {
            self.set_locomotor_goal_orientation(self.get_orientation() + rel);
        } else {
            self.set_locomotor_goal_position_explicit(target_pos);
        }
        true
    }

    /// C++ `AIFaceState::update` leftover-march. Returns true while still turning.
    ///
    /// `minSpeed == 0` → ANGLE goal + `locoUpdate_moveTowardsAngle`.
    /// Else → POSITION_EXPLICIT so Wings/Thrust fly a curve at the face point.
    pub fn tick_face_towards(&mut self, target_pos: glam::Vec3, dt: f32, frame: u32) -> bool {
        if !self.arm_face_locomotor_goal(target_pos) {
            return false;
        }
        if self.face_loco_frame == frame && frame != 0 {
            return true;
        }
        self.face_loco_frame = frame;
        if self.locomotor_goal_type == LocoGoalType::Angle {
            self.loco_update_move_towards_angle(self.locomotor_goal_angle, dt.max(1.0 / 30.0));
        }
        true
    }

    /// Face toward a world position (AI_FACE_POSITION residual).
    ///
    /// One leftover-march slice via ANGLE/`locoUpdate_moveTowardsAngle`, not a
    /// one-frame `rotate_towards_position` yaw snap.
    pub fn face_position(&mut self, pos: glam::Vec3, dt: f32) -> bool {
        if !self.can_move() {
            return false;
        }
        self.face_can_turn_in_place = self.min_speed == 0.0;
        !self.tick_face_towards(pos, dt, 0)
    }

    /// Face toward another object.
    pub fn face_object(&mut self, other: &Object, dt: f32) -> bool {
        self.face_position(other.get_position(), dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::weapon_bootstrap::{HostReloadType, host_reload_type_for_weapon_name};

    #[test]
    fn empty_return_to_base_clip_is_out_of_special_reload_ammo() {
        let mut jet = Object::new(
            ThingTemplate::new("AmericaJetRaptor"),
            ObjectId(1),
            Team::USA,
        );
        jet.weapon = Some(Weapon {
            ammo: Some(0),
            clip_size: 4,
            ..Weapon::default()
        });
        jet.thing.template.primary_weapon_name = Some("RaptorMissileWeapon".to_string());
        assert_eq!(
            host_reload_type_for_weapon_name("RaptorMissileWeapon"),
            HostReloadType::ReturnToBase
        );
        assert!(jet.is_out_of_special_reload_ammo());
    }

    #[test]
    fn bridge_range_uses_abutment_not_midspan() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("BridgeAtk");
        atk_t.add_kind_of(KindOf::Infantry);
        let mut br_t = ThingTemplate::new("LongBridge");
        br_t.add_kind_of(KindOf::Bridge);
        br_t.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
        let mut bridge = Object::new(br_t, ObjectId(2), Team::Neutral);
        atk.weapon = Some(Weapon {
            range: 30.0,
            damage: 10.0,
            ..Weapon::default()
        });
        bridge.set_position(Vec3::new(100.0, 0.0, 0.0));
        bridge.selection_radius = 80.0;
        // Standing on the near abutment (x=20). Mid-span is 80 wu away (out of
        // range 30); C++ accepts the nearer end at x=20.
        atk.set_position(Vec3::new(20.0, 0.0, 0.0));
        assert!(
            atk.is_within_attack_range_for_slot(0, &bridge),
            "near abutment must be in range even when mid-span is not"
        );
        atk.set_position(Vec3::new(-40.0, 0.0, 0.0));
        assert!(
            !atk.is_within_attack_range_for_slot(0, &bridge),
            "far past both abutments stays out of range"
        );
    }

    #[test]
    fn contact_weapon_vs_structure_requires_geom_overlap() {
        use crate::game_logic::{
            HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate, Weapon,
        };
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("SuicideAtk");
        atk_t.add_kind_of(KindOf::Infantry);
        atk_t.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Cylinder,
            is_small: true,
            height: 8.0,
            major_radius: 2.0,
            minor_radius: 2.0,
            authored: true,
        };
        atk_t.primary_weapon_name = Some("TerroristSuicideWeapon".to_string());
        let mut bld_t = ThingTemplate::new("WideBunker");
        bld_t.add_kind_of(KindOf::Structure);
        bld_t.add_kind_of(KindOf::Immobile);
        bld_t.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: false,
            height: 20.0,
            major_radius: 20.0,
            minor_radius: 8.0,
            authored: true,
        };
        let mut atk = Object::new(atk_t, ObjectId(1), Team::GLA);
        let mut bld = Object::new(bld_t, ObjectId(2), Team::USA);
        atk.weapon = Some(Weapon {
            range: 5.0,
            damage: 500.0,
            ..Weapon::default()
        });
        bld.set_position(Vec3::ZERO);
        // Surfaces ~6 wu apart: numeric contact range (~5–9) passes, box does not overlap.
        atk.set_position(Vec3::new(28.0, 0.0, 0.0));
        assert!(
            !atk.is_within_attack_range_for_slot(0, &bld),
            "contact vs structure must require geometry overlap, not just surface range"
        );
        // Overlap the box face (major 20 + attacker r 2).
        atk.set_position(Vec3::new(21.0, 0.0, 0.0));
        assert!(
            atk.is_within_attack_range_for_slot(0, &bld),
            "touching the building must pass contact collide"
        );
    }

    #[test]
    fn pitch_limited_tank_gun_rejects_steep_loft() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("CrusaderGun");
        atk_t.add_kind_of(KindOf::Vehicle);
        atk_t.primary_weapon_name = Some("CrusaderTankGun".to_string());
        let mut vic_t = ThingTemplate::new("HighVic");
        vic_t.add_kind_of(KindOf::Infantry);
        let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
        let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
        atk.weapon = Some(Weapon {
            range: 200.0,
            damage: 10.0,
            ..Weapon::default()
        });
        atk.set_position(Vec3::ZERO);
        vic.set_position(Vec3::new(10.0, 50.0, 0.0));
        assert!(
            !atk.is_any_within_target_pitch_for_slots(&vic, &[0]),
            "±15° tank gun must reject a 79° loft"
        );
        vic.set_position(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            atk.is_any_within_target_pitch_for_slots(&vic, &[0]),
            "level shot stays inside the loft window"
        );
    }

    #[test]
    fn goal_position_range_uses_firepoint_not_container_origin() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("GarrRanger");
        atk_t.add_kind_of(KindOf::Infantry);
        let mut vic_t = ThingTemplate::new("GarrVic");
        vic_t.add_kind_of(KindOf::Infantry);
        let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
        let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
        atk.weapon = Some(Weapon {
            range: 40.0,
            damage: 10.0,
            ..Weapon::default()
        });
        atk.set_position(Vec3::ZERO);
        vic.set_position(Vec3::new(100.0, 0.0, 0.0));
        assert!(!atk.is_within_attack_range_for_slot(0, &vic));
        let firepoint = Vec3::new(70.0, 0.0, 0.0);
        assert!(
            atk.is_within_attack_range_for_slot_from_goal(0, firepoint, &vic),
            "FIREPOINT 30 wu from victim is inside range 40"
        );
    }

    #[test]
    fn tick_face_towards_sets_angle_goal_when_min_speed_zero() {
        use crate::game_logic::{KindOf, LocoGoalType, Team, ThingTemplate};
        use glam::Vec3;

        let mut t = ThingTemplate::new("FaceTank");
        t.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(t, ObjectId(1), Team::USA);
        o.set_orientation(0.0);
        o.set_position(Vec3::ZERO);
        o.min_speed = 0.0;
        o.face_can_turn_in_place = true;
        o.movement.turn_rate = 0.3;
        let goal = Vec3::new(0.0, 0.0, 10.0);
        assert!(o.tick_face_towards(goal, 1.0 / 30.0, 1));
        assert_eq!(o.locomotor_goal_type, LocoGoalType::Angle);
        assert!(o.get_orientation().abs() > 1e-4);
        assert!(o.relative_angle_2d_to(goal).abs() > FACE_REL_THRESH_RAD);
    }

    #[test]
    fn tick_face_towards_sets_position_explicit_when_min_speed() {
        use crate::game_logic::{KindOf, LocoGoalType, Team, ThingTemplate};
        use glam::Vec3;

        let mut t = ThingTemplate::new("FaceJet");
        t.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(t, ObjectId(2), Team::USA);
        o.set_orientation(0.0);
        o.set_position(Vec3::ZERO);
        o.min_speed = 20.0;
        o.face_can_turn_in_place = false;
        let goal = Vec3::new(0.0, 0.0, 40.0);
        let yaw0 = o.get_orientation();
        assert!(o.tick_face_towards(goal, 1.0 / 30.0, 1));
        assert_eq!(o.locomotor_goal_type, LocoGoalType::PositionExplicit);
        assert_eq!(o.movement.target_position, Some(goal));
        assert_eq!(o.get_orientation(), yaw0);
    }

    #[test]
    fn tick_face_towards_persists_until_two_degree_threshold() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut t = ThingTemplate::new("FaceSlow");
        t.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(t, ObjectId(3), Team::USA);
        o.set_orientation(0.0);
        o.set_position(Vec3::ZERO);
        o.min_speed = 0.0;
        o.face_can_turn_in_place = true;
        o.face_active = true;
        o.movement.turn_rate = 0.4;
        let goal = Vec3::new(0.0, 0.0, 10.0);
        let mut still = true;
        for frame in 1..=200 {
            still = o.tick_face_towards(goal, 1.0 / 30.0, frame);
            if !still {
                break;
            }
        }
        assert!(!still);
        assert!(!o.face_active);
        assert!(o.relative_angle_2d_to(goal).abs() < FACE_REL_THRESH_RAD);
    }
}
