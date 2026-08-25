//! Shared leftover helpers extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
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

use super::masks_enums::{
    WeaponBonusConditionFlags, WeaponBonusConditionType, WeaponSlotType, WeaponStatus,
};

/// Wave 265: host-only path has no dual-world factory objects.
#[inline]
pub(crate) fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Maximum shots limit constant
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7fffffff;
pub(crate) const EFFECTIVELY_UNLIMITED_CLIP_AMMO: u32 = 0x7fffffff;

/// Object ID type
pub type ObjectId = u32;
pub const INVALID_OBJECT_ID: ObjectId = 0;

#[allow(dead_code)]
pub(crate) fn get_player_index_for_object(object_id: ObjectId) -> Option<usize> {
    let source_arc = TheGameLogic::find_object_by_id(object_id)?;
    let source_guard = source_arc.read().ok()?;
    let player_arc = source_guard.get_controlling_player()?;
    let player_guard = player_arc.read().ok()?;
    Some(player_guard.get_player_index() as usize)
}

#[allow(dead_code)]
pub(crate) fn notify_special_power_completion_on_source(object_id: ObjectId) -> bool {
    let Some(source_arc) = TheGameLogic::find_object_by_id(object_id) else {
        return false;
    };
    let Ok(source_guard) = source_arc.read() else {
        return false;
    };
    source_guard.notify_special_power_completion_die()
}

pub(crate) fn ammo_count_for_clip_size(clip_size: i32) -> u32 {
    if clip_size <= 0 {
        EFFECTIVELY_UNLIMITED_CLIP_AMMO
    } else {
        clip_size as u32
    }
}

pub(crate) fn weapon_slot_to_u32(slot: WeaponSlotType) -> u32 {
    match slot {
        WeaponSlotType::Primary => 0,
        WeaponSlotType::Secondary => 1,
        WeaponSlotType::Tertiary => 2,
    }
}

pub(crate) fn map_weapon_slot_to_common(slot: WeaponSlotType) -> crate::common::WeaponSlotType {
    slot.into()
}

pub(crate) fn weapon_slot_from_u32(value: u32) -> WeaponSlotType {
    match value {
        0 => WeaponSlotType::Primary,
        1 => WeaponSlotType::Secondary,
        2 => WeaponSlotType::Tertiary,
        _ => WeaponSlotType::Primary,
    }
}

pub(crate) fn weapon_status_to_u32(status: WeaponStatus) -> u32 {
    match status {
        WeaponStatus::ReadyToFire => 0,
        WeaponStatus::OutOfAmmo => 1,
        WeaponStatus::BetweenFiringShots => 2,
        WeaponStatus::ReloadingClip => 3,
        WeaponStatus::PreAttack => 4,
    }
}

pub(crate) fn weapon_status_from_u32(value: u32) -> WeaponStatus {
    match value {
        0 => WeaponStatus::ReadyToFire,
        1 => WeaponStatus::OutOfAmmo,
        2 => WeaponStatus::BetweenFiringShots,
        3 => WeaponStatus::ReloadingClip,
        4 => WeaponStatus::PreAttack,
        _ => WeaponStatus::OutOfAmmo,
    }
}

pub(crate) fn map_common_bonus_flags(
    flags: crate::common::types::WeaponBonusConditionFlags,
) -> WeaponBonusConditionFlags {
    let mut mapped = WeaponBonusConditionFlags::new();

    if flags.contains(crate::common::types::WeaponBonusConditionFlags::GARRISONED) {
        mapped.set(WeaponBonusConditionType::Garrisoned);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::HORDE) {
        mapped.set(WeaponBonusConditionType::Horde);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN) {
        mapped.set(WeaponBonusConditionType::ContinuousFireMean);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST) {
        mapped.set(WeaponBonusConditionType::ContinuousFireFast);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::NATIONALISM) {
        mapped.set(WeaponBonusConditionType::Nationalism);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::PLAYER_UPGRADE) {
        mapped.set(WeaponBonusConditionType::PlayerUpgrade);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::DRONE_SPOTTING) {
        mapped.set(WeaponBonusConditionType::DroneSpotting);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::DEMORALIZED) {
        mapped.set(WeaponBonusConditionType::Demoralized);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::ENTHUSIASTIC) {
        mapped.set(WeaponBonusConditionType::Enthusiastic);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::VETERAN) {
        mapped.set(WeaponBonusConditionType::Veteran);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::ELITE) {
        mapped.set(WeaponBonusConditionType::Elite);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::HERO) {
        mapped.set(WeaponBonusConditionType::Hero);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::BATTLEPLAN_BOMBARDMENT) {
        mapped.set(WeaponBonusConditionType::BattleplanBombardment);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::BATTLEPLAN_HOLDTHELINE) {
        mapped.set(WeaponBonusConditionType::BattleplanHoldtheLine);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::BATTLEPLAN_SEARCHANDDESTROY)
    {
        mapped.set(WeaponBonusConditionType::BattleplanSearchAndDestroy);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SUBLIMINAL) {
        mapped.set(WeaponBonusConditionType::Subliminal);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_HUMAN_EASY) {
        mapped.set(WeaponBonusConditionType::SoloHumanEasy);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_HUMAN_NORMAL) {
        mapped.set(WeaponBonusConditionType::SoloHumanNormal);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_HUMAN_HARD) {
        mapped.set(WeaponBonusConditionType::SoloHumanHard);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_AI_EASY) {
        mapped.set(WeaponBonusConditionType::SoloAiEasy);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_AI_NORMAL) {
        mapped.set(WeaponBonusConditionType::SoloAiNormal);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::SOLO_AI_HARD) {
        mapped.set(WeaponBonusConditionType::SoloAiHard);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::TARGET_FAERIE_FIRE) {
        mapped.set(WeaponBonusConditionType::TargetFaerieFire);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::FANATICISM) {
        mapped.set(WeaponBonusConditionType::Fanaticism);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::FRENZY_ONE) {
        mapped.set(WeaponBonusConditionType::FrenzyOne);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::FRENZY_TWO) {
        mapped.set(WeaponBonusConditionType::FrenzyTwo);
    }
    if flags.contains(crate::common::types::WeaponBonusConditionFlags::FRENZY_THREE) {
        mapped.set(WeaponBonusConditionType::FrenzyThree);
    }

    mapped
}
