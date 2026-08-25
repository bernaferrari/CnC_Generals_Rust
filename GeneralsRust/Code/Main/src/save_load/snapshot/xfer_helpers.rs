//! Dyn-Xfer helpers and game_logic primitive XferData impls.

use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Helper functions for Vec/HashMap/Option xfer (dyn Xfer safe)
// ---------------------------------------------------------------------------

pub(super) fn xfer_vec_default<T: Clone + XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Vec<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut item = default.clone();
            item.xfer(xfer)?;
            data.push(item);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_option<T: XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Option<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut is_some = data.is_some();
    xfer.xfer_bool(&mut is_some)?;
    if is_some {
        if data.is_none() {
            *data = Some(default);
        }
        if let Some(val) = data.as_mut() {
            val.xfer(xfer)?;
        }
    } else {
        *data = None;
    }
    Ok(())
}

pub(super) fn xfer_hashmap_default<K, V>(
    xfer: &mut dyn Xfer,
    data: &mut HashMap<K, V>,
    key_default: K,
    val_default: V,
) -> SaveLoadResult<()>
where
    K: Clone + std::hash::Hash + Eq + XferData,
    V: Clone + XferData,
{
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut k = key_default.clone();
            let mut v = val_default.clone();
            k.xfer(xfer)?;
            v.xfer(xfer)?;
            data.insert(k, v);
        }
    } else {
        for (k, v) in data.iter_mut() {
            let mut kc = k.clone();
            kc.xfer(xfer)?;
            v.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_serde_blob<T>(xfer: &mut dyn Xfer, value: &mut T) -> SaveLoadResult<()>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    let mut bytes = if xfer.get_mode() == XferMode::Load {
        Vec::new()
    } else {
        bincode::serialize(value).map_err(|e| SaveLoadError::Serialization(e.to_string()))?
    };
    let mut len = bytes.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        bytes.resize(len as usize, 0);
    }
    if !bytes.is_empty() {
        xfer.xfer_raw(&mut bytes)?;
    }
    if xfer.get_mode() == XferMode::Load {
        *value = if bytes.is_empty() {
            T::default()
        } else {
            bincode::deserialize(&bytes).map_err(|e| SaveLoadError::Serialization(e.to_string()))?
        };
    }
    Ok(())
}

pub(super) fn xfer_vec_f32(xfer: &mut dyn Xfer, data: &mut Vec<f32>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = 0.0f32;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_bool(xfer: &mut dyn Xfer, data: &mut Vec<bool>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = false;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_u8(xfer: &mut dyn Xfer, data: &mut Vec<u8>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        data.reserve(len as usize);
        for _ in 0..len {
            let mut val = 0u8;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

pub(super) fn xfer_vec_vec3(xfer: &mut dyn Xfer, data: &mut Vec<glam::Vec3>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = glam::Vec3::ZERO;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Default constructors for complex snapshot types (used during load)
// ---------------------------------------------------------------------------

pub(super) fn default_object_snapshot() -> ObjectSnapshot {
    ObjectSnapshot {
        id: ObjectId(0),
        template_name: String::new(),
        team: Team::Neutral,
        player_id: 0,
        geometry: GeometryInfo::default(),
        status: ObjectStatusSnapshot::default(),
        health: Health {
            current: 0.0,
            maximum: 0.0,
        },
        movement: Movement::default(),
        experience: Experience::default(),
        weapons: Vec::new(),
        contained_objects: Vec::new(),
        container_object: None,
        modules: HashMap::new(),
        object_type: ObjectTypeSnapshot::Unit(UnitSnapshot {
            unit_type: String::new(),
            formation_position: None,
            formation_id: None,
            group_id: None,
            waypoints: Vec::new(),
        }),
        hacker_disable_channel: None,
        weapon_barrel_states: default_weapon_barrel_state_snapshots(),
        last_weapon_discharge_sequence: 0,
        last_weapon_discharge_slot: 0,
        last_weapon_discharge_barrel: 0,
        last_weapon_discharge_frame: 0,
        collector_runtime: None,
        weapon_suspend_fx_frames: Vec::new(),
        temporary_weapon_runtime: None,
        weapon_bonus_frenzy: false,
        weapon_bonus_frenzy_level: 0,
        weapon_bonus_frenzy_until_frame: 0,
    }
}

pub(super) fn default_player_snapshot() -> PlayerSnapshot {
    PlayerSnapshot {
        id: 0,
        name: String::new(),
        team: Team::Neutral,
        is_human: false,
        is_active: false,
        resources: Resources::default(),
        population: PopulationInfo {
            current: 0,
            maximum: 0,
        },
        tech_tree: TechTreeSnapshot {
            unlocked_units: Vec::new(),
            unlocked_buildings: Vec::new(),
            unlocked_upgrades: Vec::new(),
            research_progress: HashMap::new(),
        },
        upgrades: Vec::new(),
        build_queue: Vec::new(),
        research_queue: Vec::new(),
        statistics: PlayerStatisticsSnapshot {
            units_built: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_lost: 0,
            damage_dealt: 0.0,
            damage_received: 0.0,
            resources_gathered: 0,
            experience_gained: 0.0,
        },
    }
}

pub(super) fn default_ai_strategic_state() -> AIStrategicStateSnapshot {
    AIStrategicStateSnapshot {
        current_phase: String::new(),
        objectives: Vec::new(),
        threat_assessment: ThreatAssessmentSnapshot {
            enemy_strengths: HashMap::new(),
            vulnerable_areas: Vec::new(),
            threat_level: 0.0,
        },
    }
}

pub(super) fn default_ai_tactical_state() -> AITacticalStateSnapshot {
    AITacticalStateSnapshot {
        unit_groups: Vec::new(),
        active_attacks: Vec::new(),
        defensive_positions: Vec::new(),
    }
}

pub(super) fn default_ai_economic_state() -> AIEconomicStateSnapshot {
    AIEconomicStateSnapshot {
        build_priorities: Vec::new(),
        economic_focus: String::new(),
        resource_allocation: ResourceAllocation {
            military_percentage: 0.0,
            economic_percentage: 0.0,
            defensive_percentage: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for game_logic types
// ---------------------------------------------------------------------------

impl XferData for GeometryInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("GeometryInfo")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Rotation")?;
        xfer.xfer_f32(&mut self.rotation)?;
        xfer.xfer_marker_label("BoundsMin")?;
        self.bounds_min.xfer(xfer)?;
        xfer.xfer_marker_label("BoundsMax")?;
        self.bounds_max.xfer(xfer)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        Ok(())
    }
}

impl XferData for Health {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Health")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Maximum")?;
        xfer.xfer_f32(&mut self.maximum)?;
        Ok(())
    }
}

impl XferData for Resources {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Resources")?;
        xfer.xfer_marker_label("Supplies")?;
        xfer.xfer_u32(&mut self.supplies)?;
        xfer.xfer_marker_label("Power")?;
        xfer.xfer_i32(&mut self.power)?;
        Ok(())
    }
}

impl XferData for Movement {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Movement")?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        xfer.xfer_marker_label("Velocity")?;
        self.velocity.xfer(xfer)?;
        xfer.xfer_marker_label("MaxSpeed")?;
        xfer.xfer_f32(&mut self.max_speed)?;
        xfer.xfer_marker_label("Acceleration")?;
        xfer.xfer_f32(&mut self.acceleration)?;
        xfer.xfer_marker_label("TurnRate")?;
        xfer.xfer_f32(&mut self.turn_rate)?;
        xfer.xfer_marker_label("MaxSpeedDamaged")?;
        xfer.xfer_f32(&mut self.max_speed_damaged)?;
        xfer.xfer_marker_label("AccelerationDamaged")?;
        xfer.xfer_f32(&mut self.acceleration_damaged)?;
        xfer.xfer_marker_label("TurnRateDamaged")?;
        xfer.xfer_f32(&mut self.turn_rate_damaged)?;
        xfer.xfer_marker_label("Path")?;
        xfer_vec_vec3(xfer, &mut self.path)?;
        xfer.xfer_marker_label("CurrentPathIndex")?;
        let mut idx = self.current_path_index as u32;
        xfer.xfer_u32(&mut idx)?;
        self.current_path_index = idx as usize;
        Ok(())
    }
}

impl XferData for VeterancyLevel {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut disc: u32 = match self {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        *self = match disc {
            0 => VeterancyLevel::Rookie,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid VeterancyLevel: {disc}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for Experience {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Experience")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Level")?;
        self.level.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for Weapon {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Weapon")?;
        xfer.xfer_marker_label("Damage")?;
        xfer.xfer_f32(&mut self.damage)?;
        xfer.xfer_marker_label("Range")?;
        xfer.xfer_f32(&mut self.range)?;
        xfer.xfer_marker_label("MinRange")?;
        xfer.xfer_f32(&mut self.min_range)?;
        xfer.xfer_marker_label("ReloadTime")?;
        xfer.xfer_f32(&mut self.reload_time)?;
        xfer.xfer_marker_label("LastFireTime")?;
        xfer.xfer_f32(&mut self.last_fire_time)?;
        xfer.xfer_marker_label("Ammo")?;
        xfer_option(xfer, &mut self.ammo, 0u32)?;
        xfer.xfer_marker_label("CanTargetAir")?;
        xfer.xfer_bool(&mut self.can_target_air)?;
        xfer.xfer_marker_label("CanTargetGround")?;
        xfer.xfer_bool(&mut self.can_target_ground)?;
        xfer.xfer_marker_label("ProjectileSpeed")?;
        xfer.xfer_f32(&mut self.projectile_speed)?;
        xfer.xfer_marker_label("PreAttackDelay")?;
        xfer.xfer_f32(&mut self.pre_attack_delay)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for snapshot types
// ---------------------------------------------------------------------------
