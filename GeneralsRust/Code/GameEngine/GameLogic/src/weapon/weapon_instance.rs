//! Canonical leftover Weapon instance extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{INVALID_ID, ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion};
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
    get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::audio_event::AudioEventRts;
use super::helpers::{
    INVALID_OBJECT_ID, NO_MAX_SHOTS_LIMIT, ObjectId, ammo_count_for_clip_size,
    dual_world_registry_unavailable, map_common_bonus_flags, map_weapon_slot_to_common,
};
use super::masks_enums::*;
use super::template::WeaponTemplate;

/// Values borrowed from C++'s source Object during one synchronous weapon call.
/// This is transient input, never independently owned gameplay or Xfer state.
#[derive(Debug)]
pub(crate) struct WeaponSource {
    pub(crate) id: ObjectId,
    pub(crate) position: Coord3D,
    pub(crate) geometry: crate::common::GeometryInfo,
    pub(crate) is_bridge: bool,
    pub(crate) is_structure: bool,
    pub(crate) bonus_flags: WeaponBonusConditionFlags,
}

#[derive(Debug)]
pub struct Weapon {
    /// Template defining weapon properties
    pub(crate) template: Arc<WeaponTemplate>,

    /// Weapon slot type
    pub(crate) weapon_slot: WeaponSlotType,

    /// Current weapon status
    pub(crate) status: AtomicU8,

    /// Ammunition in current clip
    pub(crate) ammo_in_clip: u32,

    /// Frame when weapon can fire again
    pub(crate) when_we_can_fire_again: u32,

    /// Frame when pre-attack will finish
    pub(crate) when_pre_attack_finished: u32,

    /// Frame when last reload started
    pub(crate) when_last_reload_started: u32,

    /// Frame when weapon was last fired
    pub(crate) last_fire_frame: u32,

    /// Frame when FX will be unsuspended
    pub(crate) suspend_fx_frame: u32,

    /// Projectile stream object ID
    pub(crate) projectile_stream_id: ObjectId,

    /// Maximum shot count limit
    pub(crate) max_shot_count: i32,

    /// Current barrel being used for firing
    pub(crate) current_barrel: i32,

    /// Number of shots fired from current barrel
    pub(crate) num_shots_for_current_barrel: i32,

    /// Unused scatter targets tracking
    pub(crate) scatter_targets_unused: Vec<i32>,

    /// Whether weapon is pitch limited
    pub(crate) pitch_limited: bool,

    /// Whether leech range is currently active
    pub(crate) leech_weapon_range_active: bool,

    /// Drawable barrel count (C++ sourceObj->getDrawable()->getBarrelCount(m_wslot)).
    /// Default 1 until the drawable reports a multi-barrel count.
    pub(crate) barrel_count: i32,
    /// Last projectile spawned by private_fire (C++ projectileID out-param).
    pub(crate) last_projectile_id: ObjectId,
    /// Set by post-fire or reload. The weapon set applies it after this borrow ends.
    pub(crate) shared_fire_sync: Option<(u32, WeaponStatus)>,
    /// Copied from the set when statistics rebuild. Not read from the object.
    pub(crate) shares_reload_time: bool,
    /// Exact source already borrowed by the caller. Cleared after the call.
    pub(crate) caller_held_source: Option<WeaponSource>,
    pub(crate) caller_veterancy: Option<crate::common::VeterancyLevel>,
    pending_assault: Option<Option<ObjectId>>,
    pending_mine_cleared: bool,
    pub(crate) caller_team: Option<std::sync::Arc<std::sync::RwLock<crate::team::Team>>>,
    caller_barrel_count: Option<u32>,
    caller_player_mask: Option<crate::common::PlayerMaskType>,
    caller_player: Option<std::sync::Arc<std::sync::RwLock<crate::player::Player>>>,
    pending_self_damage: Option<crate::damage::DamageInfo>,
}
fn goal_boundary_dist_sqr(
    goal: &Coord3D,
    source_radius: f32,
    target: &Coord3D,
    target_radius: f32,
) -> f32 {
    let dx = goal.x - target.x;
    let dy = goal.y - target.y;
    let center = (dx * dx + dy * dy).sqrt();
    let boundary = (center - source_radius - target_radius).max(0.0);
    boundary * boundary
}

impl Weapon {
    pub fn new(template: Arc<WeaponTemplate>, weapon_slot: WeaponSlotType) -> Self {
        let min_pitch = template.min_target_pitch;
        let max_pitch = template.max_target_pitch;
        let shots_per_barrel = template.shots_per_barrel;
        let pitch_limited = min_pitch > -std::f32::consts::PI || max_pitch < std::f32::consts::PI;
        let suspend_fx_frame =
            TheGameLogic::get_frame().saturating_add(template.suspend_fx_delay as UnsignedInt);

        Self {
            template,
            weapon_slot,
            status: AtomicU8::new(WeaponStatus::OutOfAmmo as u8),
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            suspend_fx_frame,
            projectile_stream_id: INVALID_OBJECT_ID,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            current_barrel: 0,
            num_shots_for_current_barrel: shots_per_barrel,
            scatter_targets_unused: Vec::new(),
            pitch_limited,
            leech_weapon_range_active: false,
            barrel_count: 1,
            last_projectile_id: INVALID_OBJECT_ID,
            shared_fire_sync: None,
            shares_reload_time: false,
            caller_held_source: None,
            caller_veterancy: None,
            pending_assault: None,
            pending_mine_cleared: false,
            caller_team: None,
            caller_barrel_count: None,
            caller_player_mask: None,
            caller_player: None,
            pending_self_damage: None,
        }
    }

    pub fn set_caller_held_source(
        &mut self,
        source: &crate::object::Object,
        bonus_flags: WeaponBonusConditionFlags,
    ) {
        self.caller_held_source = Some(WeaponSource {
            id: source.get_id(),
            position: *source.get_position(),
            geometry: *source.get_geometry_info(),
            is_bridge: source.is_kind_of(KindOf::Bridge),
            is_structure: source.is_kind_of(KindOf::Structure),
            bonus_flags,
        });
    }

    pub(crate) fn caller_source(&self, id: ObjectId) -> Option<&WeaponSource> {
        self.caller_held_source
            .as_ref()
            .filter(|source| source.id == id)
    }

    pub fn clear_caller_held_source(&mut self) {
        self.caller_held_source = None;
        self.caller_veterancy = None;
        self.caller_team = None;
        self.caller_barrel_count = None;
        self.caller_player_mask = None;
        self.caller_player = None;
    }

    pub fn set_caller_veterancy(&mut self, level: crate::common::VeterancyLevel) {
        self.caller_veterancy = Some(level);
    }

    pub fn set_caller_team(
        &mut self,
        team: Option<std::sync::Arc<std::sync::RwLock<crate::team::Team>>>,
    ) {
        self.caller_team = team;
    }

    pub fn set_caller_barrel_count(&mut self, count: i32) {
        self.caller_barrel_count = Some(count.max(1) as u32);
    }

    pub fn set_caller_player_mask(&mut self, mask: crate::common::PlayerMaskType) {
        self.caller_player_mask = Some(mask);
    }

    pub fn set_caller_player(
        &mut self,
        player: Option<std::sync::Arc<std::sync::RwLock<crate::player::Player>>>,
    ) {
        self.caller_player = player;
    }

    pub fn caller_player(
        &self,
    ) -> Option<std::sync::Arc<std::sync::RwLock<crate::player::Player>>> {
        self.caller_player.clone()
    }

    pub fn caller_player_mask(&self) -> Option<crate::common::PlayerMaskType> {
        self.caller_player_mask
    }

    pub fn take_pending_self_damage(&mut self) -> Option<crate::damage::DamageInfo> {
        self.pending_self_damage.take()
    }

    pub(crate) fn queue_self_damage(&mut self, info: crate::damage::DamageInfo) {
        self.pending_self_damage = Some(info);
    }

    pub fn take_pending_assault(&mut self) -> Option<Option<ObjectId>> {
        self.pending_assault.take()
    }

    pub fn take_pending_mine_cleared(&mut self) -> bool {
        let pending = self.pending_mine_cleared;
        self.pending_mine_cleared = false;
        pending
    }

    pub fn is_within_target_pitch(&self, source_obj: ObjectId, target_obj: ObjectId) -> bool {
        // C++ Weapon.cpp:2757 returns before any object lookup.
        if self.is_contact_weapon() || !self.pitch_limited {
            return true;
        }

        // Wave 265: empty dual-world → fail-closed only for a real pitch limit.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some((src_pos, src_geom)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                (
                    *source_guard.get_position(),
                    source_guard.get_geometry_info().clone(),
                )
            })
        else {
            return true;
        };
        let Some((dst_pos, dst_geom)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                (
                    *target_guard.get_position(),
                    target_guard.get_geometry_info().clone(),
                )
            })
        else {
            return true;
        };

        const ACCEPTABLE_DZ: Real = 10.0;
        if (dst_pos.z - src_pos.z).abs() < ACCEPTABLE_DZ {
            return true;
        }

        let (min_pitch, max_pitch) = src_geom.calc_pitches(&src_pos, &dst_geom, &dst_pos);

        let min_target = self.template.min_target_pitch;
        let max_target = self.template.max_target_pitch;

        (min_pitch >= min_target && min_pitch <= max_target)
            || (max_pitch >= min_target && max_pitch <= max_target)
            || (min_pitch <= min_target && max_pitch >= max_target)
    }

    pub(crate) fn is_within_target_pitch_for_objects(
        &self,
        source: &crate::object::Object,
        target: &crate::object::Object,
    ) -> bool {
        if self.is_contact_weapon() || !self.pitch_limited {
            return true;
        }
        let source_pos = source.get_position();
        let target_pos = target.get_position();
        if (target_pos.z - source_pos.z).abs() < 10.0 {
            return true;
        }
        let (min_pitch, max_pitch) = source.get_geometry_info().calc_pitches(
            source_pos,
            target.get_geometry_info(),
            target_pos,
        );
        let min_target = self.template.min_target_pitch;
        let max_target = self.template.max_target_pitch;
        (min_pitch >= min_target && min_pitch <= max_target)
            || (max_pitch >= min_target && max_pitch <= max_target)
            || (min_pitch <= min_target && max_pitch >= max_target)
    }

    /// Fire weapon at target object
    pub fn fire_weapon_at_object(
        &mut self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<(), WeaponError> {
        self.fire(source, target)
    }

    /// Fire weapon at position
    pub fn fire_weapon_at_position(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
    ) -> Result<(), WeaponError> {
        self.fire_at_position(source, position)
    }

    pub fn fire_weapon_at_position_with_bonus(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<(), WeaponError> {
        self.fire_weapon_at_position_with_bonus_and_reload_flag(
            source,
            position,
            source_bonus_flags,
            container_bonus_flags,
        )?;
        Ok(())
    }

    /// Fire weapon at position with bonus integration and report whether the clip completed.
    pub fn fire_weapon_at_position_with_bonus_and_reload_flag(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<bool, WeaponError> {
        self.fire_weapon_at_position_with_bonus_and_reload_flag_with_source(
            source,
            position,
            source_bonus_flags,
            container_bonus_flags,
            None,
        )
    }

    pub(crate) fn fire_weapon_at_position_with_bonus_and_reload_flag_with_source(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
        borrowed_source: Option<&mut crate::object::Object>,
    ) -> Result<bool, WeaponError> {
        let current_frame = TheGameLogic::get_frame();
        let disarm = self.template.get_damage_type() == DamageType::Disarm;
        let ready = self.get_status() == WeaponStatus::ReadyToFire;
        // C++ Weapon::fireWeapon(pos) does not range-check. OutOfRange must still fire.
        if ready && !disarm {
            if let Err(err) = self.check_can_fire(source, None, Some(position), current_frame) {
                if !matches!(err, WeaponError::OutOfRange { .. }) {
                    return Err(err);
                }
            }
        }
        let reload_frame_before = self.when_last_reload_started;

        let mut combined_flags = source_bonus_flags;
        if let Some(container_flags) = container_bonus_flags {
            combined_flags |= container_flags;
        }
        let internal_flags = map_common_bonus_flags(combined_flags);
        let bonus = if self.caller_source(source).is_some() {
            self.bonus_from_flags(internal_flags)
        } else {
            self.compute_bonus(source, internal_flags)
        };
        if !self.private_fire_weapon_with_source(
            source,
            None,
            Some(position),
            &bonus,
            false,
            false,
            true,
            borrowed_source,
        )? {
            return Ok(self.apply_post_fire_state(source, current_frame, &bonus));
        }
        if self.when_last_reload_started != reload_frame_before
            && self.load_status() == WeaponStatus::ReloadingClip
        {
            return Ok(true);
        }
        if self.get_status() != WeaponStatus::ReadyToFire {
            return Ok(false);
        }
        Ok(self.load_status() == WeaponStatus::ReloadingClip)
    }
    /// Linked turrets call `Weapon::fireWeapon`, which does not range-check.
    /// `check_can_fire` would return `OutOfRange` before leech/assist/shot.
    pub fn fire_weapon_at_position_no_range_check(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        held_flags: WeaponBonusConditionFlags,
    ) -> Result<bool, WeaponError> {
        self.fire_weapon_at_position_no_range_check_with_source(source, position, held_flags, None)
    }

    pub(crate) fn fire_weapon_at_position_no_range_check_with_source(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        held_flags: WeaponBonusConditionFlags,
        borrowed_source: Option<&mut crate::object::Object>,
    ) -> Result<bool, WeaponError> {
        let current_frame = TheGameLogic::get_frame();
        let reload_frame_before = self.when_last_reload_started;
        let bonus = self.bonus_from_flags(held_flags);
        let _ = source;
        if !self.private_fire_weapon_with_source(
            source,
            None,
            Some(position),
            &bonus,
            false,
            true,
            true,
            borrowed_source,
        )? {
            return Ok(self.apply_post_fire_state(source, current_frame, &bonus));
        }
        if self.when_last_reload_started != reload_frame_before
            && self.load_status() == WeaponStatus::ReloadingClip
        {
            return Ok(true);
        }
        if self.get_status() != WeaponStatus::ReadyToFire {
            return Ok(false);
        }
        Ok(self.load_status() == WeaponStatus::ReloadingClip)
    }

    /// Fire projectile detonation weapon
    pub fn fire_projectile_detonation_weapon(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        position: Option<&Coord3D>,
        extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        let internal_flags = map_common_bonus_flags(extra_bonus_flags);
        let bonus = self.compute_bonus(source, internal_flags);
        self.fire_projectile_detonation_weapon_with_bonus(
            source,
            target,
            position,
            &bonus,
            inflict_damage,
        )
    }

    /// Fire projectile detonation weapon using a precomputed bonus snapshot.
    pub fn fire_projectile_detonation_weapon_with_bonus(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        position: Option<&Coord3D>,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        let current_frame = TheGameLogic::get_frame();
        if !self.private_fire_weapon(
            source,
            target,
            position,
            bonus,
            true,
            false,
            inflict_damage,
        )? {
            let _ = self.apply_post_fire_state(source, current_frame, bonus);
        }
        Ok(())
    }

    /// Fire weapon with full bonus integration
    /// Matches C++ Object.cpp fireCurrentWeapon which passes source and container bonus flags
    pub fn fire_weapon(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<(), WeaponError> {
        self.fire_weapon_with_bonus_and_reload_flag(
            source_id,
            target_id,
            current_frame,
            source_bonus_flags,
            container_bonus_flags,
        )?;
        Ok(())
    }

    /// Fire weapon with full bonus integration and report whether the clip completed.
    pub fn fire_weapon_with_bonus_and_reload_flag(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<bool, WeaponError> {
        self.fire_weapon_with_bonus_and_reload_flag_with_source(
            source_id,
            target_id,
            current_frame,
            source_bonus_flags,
            container_bonus_flags,
            None,
        )
    }

    pub(crate) fn fire_weapon_with_bonus_and_reload_flag_with_source(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
        borrowed_source: Option<&mut crate::object::Object>,
    ) -> Result<bool, WeaponError> {
        let disarm = self.template.get_damage_type() == DamageType::Disarm;
        let ready = self.get_status() == WeaponStatus::ReadyToFire;

        if ready && !disarm {
            if let Err(err) = self.check_can_fire(source_id, Some(target_id), None, current_frame) {
                if !matches!(err, WeaponError::OutOfRange { .. }) {
                    return Err(err);
                }
            }
        }
        let reload_frame_before = self.when_last_reload_started;

        // Combine source and container bonus flags
        let mut combined_flags = source_bonus_flags;
        if let Some(container_flags) = container_bonus_flags {
            combined_flags |= container_flags;
        }

        // Convert to internal WeaponBonusConditionFlags type
        let internal_flags = map_common_bonus_flags(combined_flags);

        let bonus = if self.caller_source(source_id).is_some() {
            self.bonus_from_flags(internal_flags)
        } else {
            self.compute_bonus(source_id, internal_flags)
        };

        let fired = self.private_fire_weapon_with_source(
            source_id,
            Some(target_id),
            None,
            &bonus,
            false,
            false,
            true,
            borrowed_source,
        )?;
        if !fired {
            return Ok(self.apply_post_fire_state(source_id, current_frame, &bonus));
        }
        if self.when_last_reload_started != reload_frame_before
            && self.load_status() == WeaponStatus::ReloadingClip
        {
            return Ok(true);
        }
        if self.get_status() != WeaponStatus::ReadyToFire {
            return Ok(false);
        }
        Ok(self.load_status() == WeaponStatus::ReloadingClip)
    }

    pub(crate) fn apply_post_fire_state(
        &mut self,
        source: ObjectId,
        current_frame: u32,
        bonus: &WeaponBonus,
    ) -> bool {
        // C++ Weapon.cpp:2617-2625. The pre-shot wrap already ran in
        // private_fire_weapon. After the shot only the counters move.
        self.last_fire_frame = current_frame;
        if self.ammo_in_clip > 0 {
            self.ammo_in_clip -= 1;
        }
        self.max_shot_count -= 1;
        self.num_shots_for_current_barrel -= 1;
        if self.num_shots_for_current_barrel <= 0 {
            self.current_barrel += 1;
            self.num_shots_for_current_barrel = self.template.shots_per_barrel;
        }

        if self.ammo_in_clip == 0 {
            if self.template.get_auto_reloads_clip() {
                // C++ Weapon.cpp:2629-2632 — empty clip + auto-reload starts
                // reloadWithBonus immediately (ammo refill + RELOADING_CLIP).
                let _ = self.reload_with_bonus(source, bonus, false);
                return true;
            }
            // C++ Weapon.cpp:2634-2637, 2672 — no-auto-reload stays empty
            // and reports reloaded=false so Object.cpp:1466 does not release
            // LOCKED_TEMPORARILY.
            self.store_status(WeaponStatus::OutOfAmmo);
            self.when_we_can_fire_again = 0x7fffffff;
            return false;
        }

        let delay = self.template.get_delay_between_shots(bonus);
        self.when_last_reload_started = current_frame;
        self.when_we_can_fire_again = current_frame + (delay as u32);
        self.store_status(WeaponStatus::BetweenFiringShots);
        // C++ Weapon.cpp:2664 uses BETWEEN_FIRING_SHOTS. The fire path already
        // holds this weapon via mem::take, so try_write would hit an empty set.
        self.shared_fire_sync = Some((
            self.when_we_can_fire_again,
            WeaponStatus::BetweenFiringShots,
        ));
        false
    }

    /// Pre-fire weapon (for weapons with pre-attack delay)
    pub fn pre_fire_weapon(&mut self, _source: ObjectId, _victim: ObjectId) -> GameLogicResult<()> {
        self.pre_fire_weapon_with_consecutive(_source, _victim, None, None)
    }

    pub fn pre_fire_weapon_with_consecutive(
        &mut self,
        _source: ObjectId,
        _victim: ObjectId,
        consecutive_shots: Option<i32>,
        held_bonus_flags: Option<WeaponBonusConditionFlags>,
    ) -> GameLogicResult<()> {
        let delay = self.get_pre_attack_delay_with_consecutive(
            _source,
            _victim,
            consecutive_shots,
            held_bonus_flags,
        );
        if delay > 0 {
            self.store_status(WeaponStatus::PreAttack);
            self.when_pre_attack_finished = TheGameLogic::get_frame() + (delay as u32);
            if self.template.leech_range_weapon {
                self.leech_weapon_range_active = true;
            }
        }
        Ok(())
    }

    /// Force fire weapon and return projectile object
    pub fn force_fire_weapon(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
    ) -> GameLogicResult<Option<ObjectId>> {
        let current_frame = TheGameLogic::get_frame();
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.last_projectile_id = INVALID_OBJECT_ID;
        if !self.private_fire_weapon(source, None, Some(position), &bonus, false, true, true)? {
            let _ = self.apply_post_fire_state(source, current_frame, &bonus);
        }
        if self.last_projectile_id == INVALID_OBJECT_ID {
            Ok(None)
        } else {
            Ok(Some(self.last_projectile_id))
        }
    }

    /// Estimate weapon damage against target
    pub fn estimate_weapon_damage(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> f32 {
        // C++ Weapon.cpp:2376-2379 — reloading is fine; out of ammo with no
        // autoreload estimates as zero.
        if self.get_status() == WeaponStatus::OutOfAmmo && !self.template.get_auto_reloads_clip() {
            return 0.0;
        }
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.estimate_weapon_template_damage(
            source_obj as crate::common::ObjectID,
            target_obj.map(|id| id as crate::common::ObjectID),
            target_pos,
            &bonus,
        )
    }

    pub(crate) fn estimate_weapon_damage_for_objects(
        &self,
        source: &crate::object::Object,
        target: &crate::object::Object,
    ) -> f32 {
        if self.get_status() == WeaponStatus::OutOfAmmo && !self.template.get_auto_reloads_clip() {
            return 0.0;
        }
        let bonus = self.compute_bonus_for_object(source);
        self.template
            .estimate_weapon_template_damage_for_objects(source.get_id(), target, &bonus)
    }

    pub(crate) fn get_attack_range_for_object(&self, source: &crate::object::Object) -> f32 {
        self.template
            .get_attack_range(&self.compute_bonus_for_object(source))
    }

    pub(crate) fn compute_bonus_for_object(&self, source: &crate::object::Object) -> WeaponBonus {
        let mut flags = map_common_bonus_flags(source.get_weapon_bonus_condition());
        let inherited =
            super::weapon_bonus::container_passenger_bonus_flags(source.get_contained_by());
        flags.union(map_common_bonus_flags(inherited));
        self.bonus_from_flags(flags)
    }

    /// Get the attack distance including object bounding radii.
    ///
    /// Matches C++ Weapon::getAttackDistance() from Weapon.cpp line 2352.
    /// Returns `getAttackRange(source)` plus the bounding circle radii of both
    /// the source and victim objects (2D path via ATTACK_RANGE_IS_2D).
    pub fn get_attack_distance(&self, source_obj: ObjectId, victim_obj: Option<ObjectId>) -> f32 {
        // Wave 265: empty dual-world → zero.
        if dual_world_registry_unavailable() {
            return 0.0;
        }

        let mut range = self.get_attack_range(source_obj);

        if let Some(victim_id) = victim_obj {
            let source_radius = crate::object::registry::OBJECT_REGISTRY
                .with_object(source_obj, |guard| {
                    guard.get_geometry_info().get_bounding_circle_radius()
                })
                .unwrap_or(0.0);

            let victim_radius = crate::object::registry::OBJECT_REGISTRY
                .with_object(victim_id, |guard| {
                    guard.get_geometry_info().get_bounding_circle_radius()
                })
                .unwrap_or(0.0);

            range += source_radius + victim_radius;
        }

        range
    }

    /// Check if the source object's goal position is within attack range of the target.
    ///
    /// Matches C++ Weapon::isSourceObjectWithGoalPositionWithinAttackRange()
    /// (Weapon.cpp:2110) with RATIONALIZE_ATTACK_RANGE: contact/bounding-sphere
    /// distance and no `-0.5` min-range fudge (Weapon.cpp:2122-2124).
    pub fn is_source_object_with_goal_position_within_attack_range(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let source_radius = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |guard| {
                guard.get_geometry_info().get_bounding_circle_radius()
            })
            .unwrap_or(0.0);

        let (tgt_pos, target_radius) = if let Some(target_id) = target_obj {
            let Some(pair) =
                crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    (
                        *target_guard.get_position(),
                        target_guard
                            .get_geometry_info()
                            .get_bounding_circle_radius(),
                    )
                })
            else {
                return false;
            };
            pair
        } else if let Some(pos) = target_pos {
            (*pos, 0.0)
        } else {
            return false;
        };

        let dx = goal_pos.x - tgt_pos.x;
        let dy = goal_pos.y - tgt_pos.y;
        let center_dist = (dx * dx + dy * dy).sqrt();
        let boundary_dist = (center_dist - source_radius - target_radius).max(0.0);
        let dist_sqr = boundary_dist * boundary_dist;

        let attack_range = self.get_attack_range(source_obj);
        let min_attack_range = self.template.get_minimum_attack_range();

        // C++ Weapon.cpp:2122-2124 (RATIONALIZE_ATTACK_RANGE): no -0.5 fudge.
        if dist_sqr < min_attack_range * min_attack_range {
            return false;
        }

        dist_sqr <= attack_range * attack_range
    }

    /// Goal-position range test using the Objects already borrowed by the
    /// synchronous caller. Mirrors the ID form without reacquiring either one.
    pub(crate) fn is_source_object_with_goal_position_within_attack_range_for_objects(
        &self,
        source: &crate::object::Object,
        goal_pos: &Coord3D,
        target: Option<&crate::object::Object>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        let source_radius = source.get_geometry_info().get_bounding_circle_radius();
        let (target_position, target_radius) = if let Some(target) = target {
            (
                *target.get_position(),
                target.get_geometry_info().get_bounding_circle_radius(),
            )
        } else if let Some(pos) = target_pos {
            (*pos, 0.0)
        } else {
            return false;
        };

        let dx = goal_pos.x - target_position.x;
        let dy = goal_pos.y - target_position.y;
        let center_dist = (dx * dx + dy * dy).sqrt();
        let boundary_dist = (center_dist - source_radius - target_radius).max(0.0);
        let dist_sqr = boundary_dist * boundary_dist;
        let bonus = self.compute_bonus_for_object(source);
        let attack_range = self.template.get_attack_range(&bonus);
        let min_attack_range = self.template.get_minimum_attack_range();
        if dist_sqr < min_attack_range * min_attack_range {
            return false;
        }
        dist_sqr <= attack_range * attack_range
    }

    /// C++ Weapon.cpp:2241. Quarter pathfind cell so the goal is not on the
    /// edge of the firing range. Still no `-0.5` min fudge.
    pub fn is_goal_pos_within_attack_range(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        if dual_world_registry_unavailable() {
            return false;
        }
        const CELL_FUDGE: f32 = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F * 0.25;
        let source_radius = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |guard| {
                guard.get_geometry_info().get_bounding_circle_radius()
            })
            .unwrap_or(0.0);
        let attack_range = (self.get_attack_range(source_obj) - CELL_FUDGE).max(0.0);
        let min_range = self.template.get_minimum_attack_range() + CELL_FUDGE;
        let dist_sqr = if let Some(target_id) = target_obj {
            let Some((pos, radius, is_bridge)) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |guard| {
                    (
                        *guard.get_position(),
                        guard.get_geometry_info().get_bounding_circle_radius(),
                        guard.is_kind_of(KindOf::Bridge),
                    )
                })
            else {
                return false;
            };
            if is_bridge {
                let mut info = crate::terrain::BridgeAttackInfo::new();
                if let Ok(guard) = crate::terrain::get_terrain_logic().try_read() {
                    guard.get_bridge_attack_points(target_id, &mut info);
                }
                let d1 = goal_boundary_dist_sqr(goal_pos, source_radius, &info.attack_point1, 0.0);
                if d1 <= attack_range * attack_range {
                    d1
                } else {
                    goal_boundary_dist_sqr(goal_pos, source_radius, &info.attack_point2, 0.0)
                }
            } else {
                goal_boundary_dist_sqr(goal_pos, source_radius, &pos, radius)
            }
        } else if let Some(pos) = target_pos {
            goal_boundary_dist_sqr(goal_pos, source_radius, pos, 0.0)
        } else {
            return false;
        };
        if dist_sqr < min_range * min_range {
            return false;
        }
        dist_sqr <= attack_range * attack_range
    }

    /// Load ammo instantly (for newly created units)
    pub fn load_ammo_now(&mut self, source: ObjectId) -> GameLogicResult<()> {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.reload_with_bonus(source, &bonus, true)
    }

    /// Reload ammo with delay
    pub fn reload_ammo(&mut self, source: ObjectId) -> GameLogicResult<()> {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.reload_with_bonus(source, &bonus, false)
    }

    /// Reload with bonus and optional instant load
    pub(crate) fn reload_with_bonus(
        &mut self,
        source: ObjectId,
        bonus: &WeaponBonus,
        load_instantly: bool,
    ) -> GameLogicResult<()> {
        // C++ Weapon::reloadWithBonus (Weapon.cpp:1877-1912).
        let clip_size = self.template.clip_size;
        // Drop a stale pending so a skipped slot cannot beat the weapon that reloads.
        self.shared_fire_sync = None;
        // C++ Weapon.cpp:1879-1882. Flag copied from the set. A try_read fails
        // under with_object_mut and sees an empty set after mem::take.
        if clip_size > 0 && self.ammo_in_clip == clip_size as u32 && !self.shares_reload_time {
            return Ok(());
        }

        self.ammo_in_clip = ammo_count_for_clip_size(clip_size);
        self.store_status(WeaponStatus::ReloadingClip);
        let reload_time = if load_instantly {
            0
        } else {
            self.template.get_clip_reload_time(bonus)
        };
        self.when_last_reload_started = TheGameLogic::get_frame();
        self.when_we_can_fire_again = self.when_last_reload_started + (reload_time as u32);
        self.shared_fire_sync = Some((self.when_we_can_fire_again, WeaponStatus::ReloadingClip));

        self.rebuild_scatter_targets();
        Ok(())
    }

    /// C++ Weapon::rebuildScatterTargets (Weapon.cpp:1864).
    fn rebuild_scatter_targets(&mut self) {
        self.scatter_targets_unused.clear();
        for i in 0..self.template.get_scatter_targets_count() {
            self.scatter_targets_unused.push(i as i32);
        }
    }

    /// Get weapon status
    pub(crate) fn load_status(&self) -> WeaponStatus {
        match self.status.load(Ordering::Relaxed) {
            0 => WeaponStatus::ReadyToFire,
            1 => WeaponStatus::OutOfAmmo,
            2 => WeaponStatus::BetweenFiringShots,
            3 => WeaponStatus::ReloadingClip,
            _ => WeaponStatus::PreAttack,
        }
    }

    pub(crate) fn store_status(&self, status: WeaponStatus) {
        self.status.store(status as u8, Ordering::Relaxed);
    }

    pub fn get_status(&self) -> WeaponStatus {
        let current_frame = TheGameLogic::get_frame();

        // C++ Weapon::getStatus (Weapon.cpp:2736-2742): PRE_ATTACK is a pure
        // frame test — stored status is not consulted.
        if current_frame < self.when_pre_attack_finished {
            return WeaponStatus::PreAttack;
        }

        if current_frame >= self.when_we_can_fire_again {
            let next = if self.ammo_in_clip > 0 {
                WeaponStatus::ReadyToFire
            } else {
                WeaponStatus::OutOfAmmo
            };
            self.store_status(next);
            return next;
        }

        self.load_status()
    }

    /// Get remaining ammunition
    pub fn get_remaining_ammo(&self) -> u32 {
        match self.get_status() {
            WeaponStatus::ReloadingClip => 0,
            _ => self.ammo_in_clip,
        }
    }

    pub fn get_percent_ready_to_fire(&self) -> f32 {
        // C++ Weapon.cpp:2291 uses getStatus(), not the stored field.
        match self.get_status() {
            WeaponStatus::ReadyToFire => 1.0,
            WeaponStatus::OutOfAmmo | WeaponStatus::PreAttack => 0.0,
            WeaponStatus::BetweenFiringShots | WeaponStatus::ReloadingClip => {
                let current_frame = TheGameLogic::get_frame();
                if current_frame >= self.when_we_can_fire_again {
                    1.0
                } else {
                    let total_time = self.when_we_can_fire_again - self.when_last_reload_started;
                    if total_time == 0 {
                        return 1.0;
                    }
                    let elapsed_time = current_frame.saturating_sub(self.when_last_reload_started);
                    (elapsed_time as f32) / (total_time as f32)
                }
            }
        }
    }

    /// Get attack range for this weapon
    pub fn get_attack_range(&self, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_attack_range(&bonus)
    }

    /// Get minimum attack range for this weapon.
    pub fn get_minimum_attack_range(&self) -> f32 {
        self.template.get_minimum_attack_range()
    }

    /// Aim delta in radians (matches C++ Weapon::getAimDelta).
    pub fn get_aim_delta(&self) -> f32 {
        self.template.aim_delta
    }

    /// Get clip reload time for this weapon
    pub fn get_clip_reload_time(&self, source: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_clip_reload_time(&bonus)
    }

    /// Get primary damage radius for this weapon
    pub fn get_primary_damage_radius(&self, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_primary_damage_radius(&bonus)
    }

    pub fn get_pre_attack_delay(&self, source: ObjectId, victim: ObjectId) -> i32 {
        self.get_pre_attack_delay_with_consecutive(source, victim, None, None)
    }

    pub fn get_pre_attack_delay_with_consecutive(
        &self,
        source: ObjectId,
        victim: ObjectId,
        consecutive_shots: Option<i32>,
        held_bonus_flags: Option<WeaponBonusConditionFlags>,
    ) -> i32 {
        match self.template.prefire_type {
            WeaponPrefireType::PrefirePerClip => {
                if self.template.clip_size > 0 && self.ammo_in_clip < self.template.clip_size as u32
                {
                    return 0;
                }
            }
            WeaponPrefireType::PrefirePerAttack => {
                let consecutive = consecutive_shots.unwrap_or_else(|| {
                    TheGameLogic::find_object_by_id(source)
                        .and_then(|arc| {
                            arc.try_read()
                                .ok()
                                .map(|obj| obj.get_num_consecutive_shots_fired_at_target(victim))
                        })
                        .unwrap_or(0)
                });
                if consecutive > 0 {
                    return 0;
                }
            }
            WeaponPrefireType::PrefirePerShot => {}
        }
        let bonus = if let Some(flags) = held_bonus_flags {
            self.bonus_from_flags(flags)
        } else {
            self.compute_bonus(source, WeaponBonusConditionFlags::new())
        };
        self.template.get_pre_attack_delay(&bonus)
    }
    /// Check if this is a damage weapon.
    ///
    /// C++ Weapon::isDamageWeapon (Weapon.cpp:2789-2816): DEPLOY/DISARM are
    /// always damage weapons, HACK never is, else primary/secondary > 0.
    pub fn is_damage_weapon(&self) -> bool {
        match self.template.get_damage_type() {
            DamageType::Deploy | DamageType::Disarm => true,
            DamageType::Hack => false,
            // C++ Weapon.cpp:2810-2811: empty WeaponBonus, then primary or secondary > 0.
            _ => {
                let bonus = WeaponBonus::new();
                self.template.get_primary_damage(&bonus) > 0.0
                    || self.template.get_secondary_damage(&bonus) > 0.0
            }
        }
    }

    /// Check if this is a contact weapon (requires collision with target)
    pub fn is_contact_weapon(&self) -> bool {
        self.template.is_contact_weapon()
    }

    /// Get the damage type for this weapon
    pub fn get_damage_type(&self) -> DamageType {
        self.template.get_damage_type()
    }

    /// Check if weapon is pitch limited
    pub fn is_pitch_limited(&self) -> bool {
        self.pitch_limited
    }

    /// Set leech range active state
    pub fn set_leech_range_active(&mut self, active: bool) {
        self.leech_weapon_range_active = active;
    }

    /// Check if leech range is active
    pub fn has_leech_range(&self) -> bool {
        self.leech_weapon_range_active
    }

    /// Set the frame when the weapon can fire next (matches C++ Weapon::setPossibleNextShotFrame)
    pub fn set_possible_next_shot_frame(&mut self, frame: u32) {
        self.when_we_can_fire_again = frame;
    }

    /// Set weapon status directly (matches C++ Weapon::setStatus)
    pub fn set_status(&mut self, status: WeaponStatus) {
        self.store_status(status);
    }

    /// Set maximum shot count
    pub fn set_max_shot_count(&mut self, max_shots: i32) {
        self.max_shot_count = max_shots;
    }

    /// Get maximum shot count
    pub fn get_max_shot_count(&self) -> i32 {
        self.max_shot_count
    }

    /// C++ Weapon.cpp:2577 drawable barrel count used to wrap m_curBarrel.
    pub fn get_barrel_count(&self) -> i32 {
        self.barrel_count
    }

    /// Set leftover barrel count from leftover drawable (at least 1).
    pub fn set_barrel_count(&mut self, count: i32) {
        self.barrel_count = count.max(1);
    }

    /// C++ Weapon::getCurBarrel residual.
    /// Set clip percent full
    pub fn set_clip_percent_full(&mut self, percent: f32, allow_reduction: bool) {
        if self.template.clip_size == 0 {
            return;
        }
        let new_ammo = (self.template.clip_size as f32 * percent) as u32;
        let old_ammo = self.ammo_in_clip;
        if new_ammo > old_ammo || (allow_reduction && new_ammo < old_ammo) {
            self.ammo_in_clip = new_ammo;
            // C++ Weapon.cpp:1854 writes OUT_OF_AMMO when ammo is non-zero.
            self.store_status(if new_ammo > 0 {
                WeaponStatus::OutOfAmmo
            } else {
                WeaponStatus::ReadyToFire
            });
            let now = TheGameLogic::get_frame();
            self.when_last_reload_started = now;
            self.when_we_can_fire_again = now;
            self.rebuild_scatter_targets();
        }
    }

    /// Transfer next shot stats from another weapon
    pub fn transfer_next_shot_stats_from(&mut self, other: &Weapon) {
        self.when_we_can_fire_again = other.when_we_can_fire_again;
        self.when_last_reload_started = other.when_last_reload_started;
        self.store_status(other.get_status());
    }

    /// Update weapon on bonus change
    pub fn on_weapon_bonus_change(
        &mut self,
        flags: WeaponBonusConditionFlags,
    ) -> GameLogicResult<Option<u32>> {
        let bonus = self.bonus_from_flags(flags);
        let new_delay = match self.get_status() {
            WeaponStatus::ReloadingClip => self.template.get_clip_reload_time(&bonus),
            WeaponStatus::BetweenFiringShots => self.template.get_delay_between_shots(&bonus),
            _ => return Ok(None),
        };
        self.when_last_reload_started = TheGameLogic::get_frame();
        self.when_we_can_fire_again = self.when_last_reload_started + (new_delay as u32);
        Ok(Some(self.when_we_can_fire_again))
    }

    /// Get weapon template
    pub fn get_template(&self) -> &Arc<WeaponTemplate> {
        &self.template
    }

    /// Get weapon slot
    pub fn get_weapon_slot(&self) -> WeaponSlotType {
        self.weapon_slot
    }

    /// Notify weapon stream systems that a new projectile was fired (matches C++ Weapon::newProjectileFired).
    pub fn new_projectile_fired(
        &mut self,
        source_obj_id: ObjectId,
        projectile_id: ObjectId,
        victim_obj: Option<ObjectId>,
        victim_pos: Option<&Coord3D>,
    ) {
        let stream_name = self.template.projectile_stream_name.trim();
        if stream_name.is_empty() {
            return;
        }

        let mut stream_arc = if self.projectile_stream_id != INVALID_OBJECT_ID {
            TheGameLogic::find_object_by_id(self.projectile_stream_id)
        } else {
            None
        };

        if stream_arc.is_none() {
            self.projectile_stream_id = INVALID_OBJECT_ID;

            let team_arc = if self.caller_source(source_obj_id).is_some() {
                self.caller_team.clone()
            } else {
                let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) else {
                    return;
                };
                let Ok(source_guard) = source_arc.try_read() else {
                    return;
                };
                source_guard
                    .get_controlling_player()
                    .and_then(|player| {
                        player
                            .read()
                            .ok()
                            .and_then(|guard| guard.get_default_team())
                    })
                    .or_else(|| source_guard.get_team())
            };
            let Some(team_arc) = team_arc else {
                return;
            };
            let Ok(team_guard) = team_arc.read() else {
                return;
            };
            let Some(template) = TheThingFactory::find_template(stream_name) else {
                return;
            };
            let factory = match TheThingFactory::get() {
                Ok(factory) => factory,
                Err(_) => return,
            };
            let stream_obj = match factory.new_object(template, &team_guard) {
                Ok(obj) => obj,
                Err(_) => return,
            };

            self.projectile_stream_id = stream_obj
                .read()
                .ok()
                .map(|guard| guard.get_id())
                .unwrap_or(INVALID_OBJECT_ID);
            stream_arc = Some(stream_obj);
        }

        let Some(stream_arc) = stream_arc else {
            return;
        };
        let Ok(mut stream_guard) = stream_arc.write() else {
            return;
        };
        let pos = if let Some(source) = self.caller_source(source_obj_id) {
            Some(source.position)
        } else {
            TheGameLogic::find_object_by_id(source_obj_id)
                .and_then(|arc| arc.try_read().ok().map(|guard| *guard.get_position()))
        };
        if let Some(pos) = pos {
            let _ = stream_guard.set_position(&pos);
        }
        for mut behavior in stream_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.access() else {
                continue;
            };
            let Some(stream_update) = behavior.get_projectile_stream_update_interface() else {
                continue;
            };
            stream_update.add_projectile(
                source_obj_id,
                projectile_id,
                victim_obj.unwrap_or(INVALID_OBJECT_ID),
                victim_pos,
            );
            break;
        }
    }

    /// Get weapon name
    pub fn get_name(&self) -> &str {
        &self.template.name
    }

    /// Get last shot frame
    pub fn get_last_shot_frame(&self) -> u32 {
        self.last_fire_frame
    }

    /// Get possible next shot frame
    pub fn get_possible_next_shot_frame(&self) -> u32 {
        self.when_we_can_fire_again
    }

    /// Get pre-attack finished frame
    pub fn get_pre_attack_finished_frame(&self) -> u32 {
        self.when_pre_attack_finished
    }

    pub fn set_pre_attack_finished_frame(&mut self, frame: u32) {
        self.when_pre_attack_finished = frame;
    }

    /// Get last reload started frame
    pub fn get_last_reload_started_frame(&self) -> u32 {
        self.when_last_reload_started
    }

    /// Get suspend FX frame
    pub fn get_suspend_fx_frame(&self) -> u32 {
        self.suspend_fx_frame
    }

    pub fn get_continue_attack_range(&self) -> crate::common::Real {
        self.template.continue_attack_range
    }

    pub fn get_lock_on_range(&self) -> crate::common::Real {
        self.template.continue_attack_range
    }

    pub fn get_anti_mask(&self) -> u32 {
        self.template.get_anti_mask()
    }

    /// Get the scatter radius for this weapon.
    /// C++ Reference: Weapon.h line 670
    pub fn get_scatter_radius(&self) -> f32 {
        self.template.scatter_radius
    }

    /// Matches C++ Weapon::getScatterTargetScalar() from Weapon.cpp line 1910
    pub fn get_scatter_target_scalar(&self) -> f32 {
        self.template.get_scatter_target_scalar()
    }

    /// Get whether this weapon is capable of following waypoint paths.
    /// C++ Reference: Weapon.h line 673
    pub fn is_capable_of_following_waypoint(&self) -> bool {
        self.template.capable_of_following_waypoint
    }

    /// Get the continuous fire coast frames.
    /// C++ Reference: Weapon.h line 676
    pub fn get_continuous_fire_coast_frames(&self) -> u32 {
        self.template.continuous_fire_coast_frames
    }

    /// Get the auto-reload when idle frames.
    /// C++ Reference: Weapon.h line 677
    pub fn get_auto_reload_when_idle_frames(&self) -> u32 {
        self.template.auto_reload_when_idle_frames
    }

    /// Get the reload type for this weapon.
    /// C++ Reference: Weapon.h line 667
    pub fn get_reload_type(&self) -> WeaponReloadType {
        self.template.reload_type
    }

    /// Get the death type for this weapon.
    /// C++ Reference: Weapon.h line 681
    pub fn get_death_type(&self) -> DeathType {
        self.template.death_type
    }

    /// Set the last reload started frame.
    /// C++ Reference: Weapon.h line 642
    pub fn set_last_reload_started_frame(&mut self, frame: u32) {
        self.when_last_reload_started = frame;
    }

    /// Get the fire sound for this weapon.
    /// C++ Reference: Weapon.h line 678 (inline getFireSound)
    pub fn get_fire_sound(&self) -> &AudioEventRts {
        &self.template.fire_sound
    }

    /// Get the fire sound loop time.
    /// C++ Reference: Weapon.h line 679
    pub fn get_fire_sound_loop_time(&self) -> u32 {
        self.template.fire_sound_loop_time
    }

    /// Check if there is clear terrain line of sight from source to victim object.
    /// C++ Reference: Weapon.cpp line 3066 (isClearFiringLineOfSightTerrain)
    ///
    /// Adjusts source position upward by geometry height (eye level), then
    /// checks terrain LOS to the victim's geometry center.
    pub fn is_clear_firing_line_of_sight_terrain(
        &self,
        source_obj: ObjectId,
        target_obj: ObjectId,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_pos = source_guard.get_position();
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(source_pos.x, source_pos.y, source_pos.z + source_height)
            })
        else {
            return true;
        };

        let Some(victim_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                let center = target_guard
                    .get_geometry_info()
                    .get_center_position(target_guard.get_position());
                CollideCoord::new(center.x, center.y, center.z)
            })
        else {
            return true;
        };

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None,
            &origin,
            None,
            &victim_pos,
        )
    }

    /// Check if there is clear terrain LOS from source to a specific position.
    /// C++ Reference: Weapon.cpp line 3083
    pub fn is_clear_firing_line_of_sight_terrain_pos(
        &self,
        source_obj: ObjectId,
        victim_pos: &Coord3D,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_pos = source_guard.get_position();
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(source_pos.x, source_pos.y, source_pos.z + source_height)
            })
        else {
            return true;
        };

        let victim = CollideCoord::new(victim_pos.x, victim_pos.y, victim_pos.z);

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None, &origin, None, &victim,
        )
    }

    /// Check if source at goal position would have clear terrain LOS to victim object.
    /// C++ Reference: Weapon.cpp line 3096 (isClearGoalFiringLineOfSightTerrain)
    ///
    /// Used by AI to pre-check if moving to a goal position would give clear shot.
    pub fn is_clear_goal_firing_line_of_sight_terrain(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        target_obj: ObjectId,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(goal_pos.x, goal_pos.y, goal_pos.z + source_height)
            })
        else {
            return true;
        };

        let Some(victim_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                let center = target_guard
                    .get_geometry_info()
                    .get_center_position(target_guard.get_position());
                CollideCoord::new(center.x, center.y, center.z)
            })
        else {
            return true;
        };

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None,
            &origin,
            None,
            &victim_pos,
        )
    }

    /// Check if source at goal position would have clear terrain LOS to a position.
    /// C++ Reference: Weapon.cpp line 3110
    pub fn is_clear_goal_firing_line_of_sight_terrain_pos(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        victim_pos: &Coord3D,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(goal_pos.x, goal_pos.y, goal_pos.z + source_height)
            })
        else {
            return true;
        };

        let victim = CollideCoord::new(victim_pos.x, victim_pos.y, victim_pos.z);

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None, &origin, None, &victim,
        )
    }

    /// Get clip reload time for this weapon with source object context.
    /// C++ Reference: Weapon.h line 688
    pub fn get_clip_reload_time_obj(&self, source_obj: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.get_clip_reload_time(&bonus)
    }

    /// Get pre-attack delay for a specific target.
    pub fn get_pre_attack_delay_obj(&self, source_obj: ObjectId) -> i32 {
        self.get_pre_attack_delay(source_obj, INVALID_OBJECT_ID)
    }
    /// Get primary damage radius for this weapon with source object context.
    /// C++ Reference: Weapon.h line 690
    pub fn get_primary_damage_radius_obj(&self, source_obj: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.get_primary_damage_radius(&bonus)
    }

    // ========================================================================
    // CRITICAL WEAPON FIRING METHODS
    // ========================================================================
    // C++ Reference: Weapon.cpp lines 1400-1600

    /// Fire weapon at object target
    /// C++ Reference: Weapon.cpp lines 1400-1450 (main firing entry point)
    ///
    /// # Behavior
    /// - Checks ammunition and weapon status
    /// - Validates range and line-of-sight
    /// - Fires weapon via private_fire_weapon()
    /// - Updates cooldown and ammo counters
    /// - Returns success or specific error
    pub fn fire(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: ObjectId,
    ) -> Result<(), WeaponError> {
        // Get current frame from game logic
        let current_frame = TheGameLogic::get_frame();

        // Check if we can fire
        if let Err(err) =
            self.check_can_fire(source_obj_id, Some(target_obj_id), None, current_frame)
        {
            if !matches!(err, WeaponError::OutOfRange { .. }) {
                return Err(err);
            }
        }

        // Fire the weapon through private implementation
        let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());

        // Call private fire weapon
        if !self.private_fire_weapon(
            source_obj_id,
            Some(target_obj_id),
            None,
            &bonus,
            false,
            false,
            true,
        )? {
            let _ = self.apply_post_fire_state(source_obj_id, current_frame, &bonus);
        }

        Ok(())
    }

    /// Fire weapon at position target
    pub fn fire_at_position(
        &mut self,
        source_obj_id: ObjectId,
        target_pos: &Coord3D,
    ) -> Result<(), WeaponError> {
        let current_frame = TheGameLogic::get_frame();

        // Check if we can fire
        if let Err(err) = self.check_can_fire(source_obj_id, None, Some(target_pos), current_frame)
        {
            if !matches!(err, WeaponError::OutOfRange { .. }) {
                return Err(err);
            }
        }

        let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());

        // Call private fire weapon
        if !self.private_fire_weapon(
            source_obj_id,
            None,
            Some(target_pos),
            &bonus,
            false,
            false,
            true,
        )? {
            let _ = self.apply_post_fire_state(source_obj_id, current_frame, &bonus);
        }

        Ok(())
    }

    /// Check if weapon can fire at target.
    ///
    /// C++ fireWeapon performs no relationship/vision/shroud check — legality
    /// lives in WeaponSet::getAbleToAttackSpecificObject, not at fire time.
    pub fn check_can_fire(
        &self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        current_frame: u32,
    ) -> Result<(), WeaponError> {
        if self.ammo_in_clip == 0 {
            return Err(WeaponError::NoAmmo);
        }

        if self.get_status() != WeaponStatus::ReadyToFire {
            let frames_remaining = self.when_we_can_fire_again.saturating_sub(current_frame);
            let time_remaining = (frames_remaining as f32) / LOGICFRAMES_PER_SECOND as f32;
            return Err(WeaponError::NotReady { time_remaining });
        }

        let source_pos = self.get_object_position(source_obj_id)?;
        let target_position = if let Some(target_id) = target_obj_id {
            self.get_object_position(target_id)?
        } else if let Some(pos) = target_pos {
            *pos
        } else {
            return Err(WeaponError::InvalidTarget);
        };

        let leech = self.template.leech_range_weapon || self.has_leech_range();
        if !leech && !self.is_within_attack_range(source_obj_id, target_obj_id, target_pos) {
            let distance = source_pos.distance(target_position);
            let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());
            return Err(WeaponError::OutOfRange {
                distance,
                max_range: self.template.get_attack_range(&bonus),
            });
        }
        if leech && self.is_too_close(source_obj_id, target_obj_id, target_pos) {
            return Err(WeaponError::OutOfRange {
                distance: source_pos.distance(target_position),
                max_range: self.template.get_minimum_attack_range(),
            });
        }

        if let Some(target_id) = target_obj_id {
            if !self.is_target_valid(target_id) {
                return Err(WeaponError::InvalidTarget);
            }
        }

        Ok(())
    }

    fn begin_assault_if_present(
        &mut self,
        _source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
    ) {
        self.pending_assault = Some(target_obj_id);
    }

    fn disarm_target(&mut self, _source_obj_id: ObjectId, victim_id: ObjectId) {
        let veterancy = self
            .caller_veterancy
            .unwrap_or(crate::common::VeterancyLevel::Regular);
        let play_disarm_fx = |pos: &Coord3D| {
            let _veterancy = veterancy;
            if let Some(fx) = self.template.get_fire_fx(veterancy) {
                let matrix =
                    crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |obj| {
                        crate::common::Matrix3D::from_translation(*obj.get_position())
                            * crate::common::Matrix3D::from_rotation_z(obj.get_orientation())
                    });
                let _ = fx.do_fx_pos(pos, matrix.as_ref(), 0.0, Some(pos), 0.0);
            }
        };

        let mut found = false;
        if let Some(behaviors) = crate::object::registry::OBJECT_REGISTRY
            .with_object(victim_id, |obj| obj.get_behavior_modules())
        {
            for mut behavior in behaviors {
                if let Ok(mut guard) = behavior.access() {
                    if let Some(land_mine) = guard.get_land_mine_interface() {
                        if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                            .with_object(victim_id, |obj| *obj.get_position())
                        {
                            play_disarm_fx(&pos);
                        }
                        land_mine.disarm();
                        found = true;
                        break;
                    }
                }
            }
        }

        let kinds = crate::object::registry::OBJECT_REGISTRY
            .with_object(victim_id, |obj| {
                (
                    obj.is_kind_of(KindOf::Mine),
                    obj.is_kind_of(KindOf::BoobyTrap),
                    obj.is_kind_of(KindOf::Demotrap),
                )
            })
            .unwrap_or((false, false, false));
        // C++ Weapon.cpp:2528 — `!found && MINE || BOOBY_TRAP || DEMOTRAP`
        if (!found && kinds.0) || kinds.1 || kinds.2 {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(victim_id, |obj| *obj.get_position())
            {
                play_disarm_fx(&pos);
            }
            let _ = TheGameLogic::destroy_object_by_id(victim_id);
            found = true;
        }

        if found {
            self.pending_mine_cleared = true;
        }
    }

    /// Private weapon firing. Returns true when post-fire bookkeeping was
    /// already applied (DAMAGE_DISARM).
    pub(crate) fn private_fire_weapon(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        inflict_damage: bool,
    ) -> Result<bool, WeaponError> {
        self.private_fire_weapon_with_source(
            source_obj_id,
            target_obj_id,
            target_pos,
            bonus,
            is_projectile_detonation,
            ignore_ranges,
            inflict_damage,
            None,
        )
    }

    fn private_fire_weapon_with_source(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        inflict_damage: bool,
        mut borrowed_source: Option<&mut crate::object::Object>,
    ) -> Result<bool, WeaponError> {
        if self.template.get_request_assist_range() > 0.0 {
            if let Some(victim) = target_obj_id {
                self.process_request_assistance(source_obj_id, victim);
            }
        }

        if self.template.leech_range_weapon {
            self.leech_weapon_range_active = true;
        }

        match self.template.get_damage_type() {
            DamageType::Deploy => {
                self.begin_assault_if_present(source_obj_id, target_obj_id);
            }
            DamageType::Disarm => {
                // Spend the round here. Return true so callers do not run
                // apply_post_fire_state (that would spend again and set
                // BETWEEN_FIRING_SHOTS). The wrapper turns a still-ReadyToFire
                // weapon into a public false, and a reload into true.
                if let Some(victim) = target_obj_id {
                    self.disarm_target(source_obj_id, victim);
                }
                self.max_shot_count -= 1;
                if self.ammo_in_clip > 0 {
                    self.ammo_in_clip -= 1;
                }
                if self.ammo_in_clip == 0 && self.template.get_auto_reloads_clip() {
                    let _ = self.reload_with_bonus(source_obj_id, bonus, false);
                }
                return Ok(true);
            }
            DamageType::Hack => {}
            _ => {}
        }
        // Not ready must not reach apply_post_fire_state. Callers treat false
        // as "account the shot". Ok(true) skips that. The wrapper then returns
        // false while the status is still not ReadyToFire.
        if self.get_status() != WeaponStatus::ReadyToFire {
            return Ok(true);
        }

        let source_pos = self.get_object_position(source_obj_id)?;
        let mut victim_id = target_obj_id;
        let mut target_position = if let Some(target_id) = victim_id {
            self.get_object_position(target_id)?
        } else if let Some(pos) = target_pos {
            *pos
        } else {
            return Err(WeaponError::InvalidTarget);
        };

        if let Some(target_id) = victim_id {
            if let Some(arc) = TheGameLogic::find_object_by_id(target_id) {
                if let Ok(guard) = arc.try_read() {
                    if let Some(ai) = guard.get_ai() {
                        if let Ok(ai_guard) = ai.try_lock() {
                            let mut offset = Coord3D::new(0.0, 0.0, 0.0);
                            if ai_guard.get_sneaky_targeting_offset(&mut offset) {
                                target_position.x += offset.x;
                                target_position.y += offset.y;
                                target_position.z += offset.z;
                                victim_id = None;
                            }
                        }
                    }
                }
            }
        }

        let flight_position = target_position;
        let target_layer = victim_id
            .and_then(|id| TheGameLogic::find_object_by_id(id))
            .and_then(|arc| arc.try_read().ok().map(|guard| guard.get_layer()))
            .unwrap_or(crate::common::PathfindLayerEnum::Ground);
        let mut used_scatter_table = false;
        if let Some(scattered) = self.take_scatter_target_pos(&target_position, target_layer) {
            victim_id = None;
            target_position = scattered;
            used_scatter_table = true;
        }
        // C++ Weapon.cpp:849-886 runs inside fireWeaponTemplate, after the
        // scatter-table offset. A 0 return still spends the shot (2617-2618).
        let leech = self.template.leech_range_weapon || self.has_leech_range();
        let out_of_range = (!ignore_ranges
            && !leech
            && !self.is_within_attack_range(source_obj_id, victim_id, Some(&target_position)))
            || (!ignore_ranges
                && !is_projectile_detonation
                && self.is_too_close(source_obj_id, victim_id, Some(&target_position)));
        if out_of_range {
            return Ok(false);
        }

        // CPP Weapon.cpp:889–950: muzzle FX/recoil then FireOCL before
        // aim scatter, projectile creation, or damage. Barrel selection is
        // normalized by the caller before entering that template FX path.
        if !is_projectile_detonation {
            if let Some(count) = self.caller_barrel_count {
                self.barrel_count = count as i32;
            } else if let Some(source) = borrowed_source
                .as_deref()
                .filter(|source| source.get_id() == source_obj_id)
            {
                self.barrel_count = source
                    .get_drawable()
                    .and_then(|drawable| {
                        drawable
                            .try_read()
                            .ok()
                            .map(|draw| draw.get_barrel_count(self.weapon_slot).max(1))
                    })
                    .unwrap_or(1);
            } else {
                self.barrel_count = crate::object::registry::OBJECT_REGISTRY
                    .with_object(source_obj_id, |obj| {
                        obj.get_drawable().and_then(|drawable| {
                            drawable
                                .try_read()
                                .ok()
                                .map(|draw| draw.get_barrel_count(self.weapon_slot).max(1))
                        })
                    })
                    .flatten()
                    .unwrap_or(1);
            }
            if self.current_barrel >= self.barrel_count {
                self.current_barrel = 0;
                self.num_shots_for_current_barrel = self.template.shots_per_barrel;
            }
        }
        let target_geometry = victim_id.and_then(|id| {
            if let Some(target) = borrowed_source
                .as_deref()
                .filter(|source| source.get_id() == id)
            {
                return Some((
                    target
                        .get_geometry_info()
                        .get_center_position(target.get_position()),
                    target.is_kind_of(KindOf::Structure),
                ));
            }
            TheGameLogic::find_object_by_id(id).and_then(|arc| {
                arc.try_read().ok().map(|target| {
                    (
                        target
                            .get_geometry_info()
                            .get_center_position(target.get_position()),
                        target.is_kind_of(KindOf::Structure),
                    )
                })
            })
        });
        let fx_position = target_geometry
            .map(|(center, _)| center)
            .unwrap_or(target_position);
        self.fire_weapon_effects(
            source_obj_id,
            &source_pos,
            &fx_position,
            is_projectile_detonation,
            bonus,
            borrowed_source.as_deref(),
        )?;
        if let Some((center, true)) = target_geometry {
            target_position = center;
        }

        let target_type = victim_id
            .map(|id| self.get_object_type(id))
            .unwrap_or(ObjectType::Unknown);
        let rolled_scatter = if used_scatter_table {
            0.0
        } else {
            let (scattered_pos, rolled_scatter) =
                self.scatter_aim_point(target_position, target_type, target_layer);
            target_position = scattered_pos;
            rolled_scatter
        };

        let mut damage_victim = victim_id;
        let mut laser_victim = victim_id;
        if self.template.is_laser() {
            let primary_r = self.template.get_primary_damage_radius(bonus);
            let secondary_r = self.template.get_secondary_damage_radius(bonus);
            if rolled_scatter <= primary_r || rolled_scatter <= secondary_r {
                if let Some(tid) = victim_id {
                    if let Some(arc) = TheGameLogic::find_object_by_id(tid) {
                        if let Ok(guard) = arc.try_read() {
                            target_position = *guard.get_position();
                        }
                    }
                }
            } else {
                damage_victim = None;
                laser_victim = None;
            }
        }

        if is_projectile_detonation {
            if inflict_damage {
                self.deal_damage_internal_with_source(
                    source_obj_id,
                    damage_victim,
                    &target_position,
                    bonus,
                    true,
                    borrowed_source.as_deref_mut(),
                )?;
            }
            return Ok(false);
        }

        match self.determine_fire_mode() {
            FireMode::InstantImpact { splash_radius: _ } => {
                if inflict_damage {
                    self.deal_damage_internal_with_source(
                        source_obj_id,
                        damage_victim,
                        &target_position,
                        bonus,
                        is_projectile_detonation,
                        borrowed_source.as_deref_mut(),
                    )?;
                }
            }
            FireMode::Projectile { speed, lifetime } => {
                if self.template.projectile_name.trim().is_empty() {
                    self.handle_projectileless_flight_damage_with_source(
                        source_obj_id,
                        &source_pos,
                        damage_victim,
                        &flight_position,
                        speed,
                        bonus,
                        inflict_damage,
                        borrowed_source.as_deref_mut(),
                    )?;
                } else {
                    let projectile_id = self.create_projectile(
                        source_obj_id,
                        &source_pos,
                        &target_position,
                        victim_id,
                        speed,
                        lifetime,
                        bonus,
                    )?;
                    self.last_projectile_id = projectile_id;
                    self.new_projectile_fired(
                        source_obj_id,
                        projectile_id,
                        victim_id,
                        Some(&target_position),
                    );
                }
            }
            FireMode::ContinuousBeam {
                duration,
                damage_per_frame,
            } => {
                let _ = self.create_laser_object(
                    source_obj_id,
                    laser_victim,
                    &target_position,
                    damage_per_frame,
                    duration,
                );
                self.inflict_damage_if_requested_with_source(
                    source_obj_id,
                    damage_victim,
                    &target_position,
                    bonus,
                    is_projectile_detonation,
                    inflict_damage,
                    borrowed_source.as_deref_mut(),
                )?;
            }
        }

        Ok(false)
    }

    /// C++ Weapon.cpp:1028-1031 `if (inflictDamage) dealDamageInternal(...)`.
    pub(crate) fn inflict_damage_if_requested(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_position: &Coord3D,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        self.inflict_damage_if_requested_with_source(
            source_obj_id,
            target_obj_id,
            target_position,
            bonus,
            is_projectile_detonation,
            inflict_damage,
            None,
        )
    }

    pub(crate) fn inflict_damage_if_requested_with_source(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_position: &Coord3D,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        inflict_damage: bool,
        borrowed_source: Option<&mut crate::object::Object>,
    ) -> Result<(), WeaponError> {
        if inflict_damage {
            self.deal_damage_internal_with_source(
                source_obj_id,
                target_obj_id,
                target_position,
                bonus,
                is_projectile_detonation,
                borrowed_source,
            )?;
        }
        Ok(())
    }
}
