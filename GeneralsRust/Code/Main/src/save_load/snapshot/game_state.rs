//! Resource, combat, experience, and pathfinding snapshot residual.

use super::xfer_helpers::{xfer_hashmap_default, xfer_option, xfer_vec_default};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Resource manager snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceManagerSnapshot {
    pub supply_deposits: Vec<SupplyDepositSnapshot>,
    pub resource_zones: Vec<ResourceZoneSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyDepositSnapshot {
    pub position: glam::Vec3,
    pub amount: u32,
    pub depletion_rate: f32,
    pub harvesters: Vec<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceZoneSnapshot {
    pub bounds: GeometryInfo,
    pub resource_type: String,
    pub total_amount: u32,
    pub remaining_amount: u32,
}

/// Combat tracking snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CombatTrackerSnapshot {
    pub active_combats: Vec<ActiveCombatSnapshot>,
    pub recent_deaths: Vec<DeathEventSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCombatSnapshot {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub start_time: f32,
    pub damage_dealt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeathEventSnapshot {
    pub object_id: ObjectId,
    pub killer_id: Option<ObjectId>,
    pub death_time: f32,
    pub death_position: glam::Vec3,
}

/// Experience tracking snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExperienceTrackerSnapshot {
    pub experience_events: Vec<ExperienceEventSnapshot>,
    pub veterancy_bonuses: HashMap<ObjectId, VeterancyBonuses>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEventSnapshot {
    pub object_id: ObjectId,
    pub experience_gained: f32,
    pub source: String,
    pub timestamp: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeterancyBonuses {
    pub health_bonus: f32,
    pub damage_bonus: f32,
    pub accuracy_bonus: f32,
    pub range_bonus: f32,
}

/// Pathfinding cache snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathfindingCacheSnapshot {
    pub cached_paths: HashMap<(SerializableVec3, SerializableVec3), Vec<SerializableVec3>>,
    pub cache_timestamps: HashMap<(SerializableVec3, SerializableVec3), f32>,
}

impl XferData for SupplyDepositSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("SupplyDepositSnapshot")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Amount")?;
        xfer.xfer_u32(&mut self.amount)?;
        xfer.xfer_marker_label("DepletionRate")?;
        xfer.xfer_f32(&mut self.depletion_rate)?;
        xfer.xfer_marker_label("Harvesters")?;
        xfer_vec_default(xfer, &mut self.harvesters, ObjectId(0))?;
        Ok(())
    }
}

impl XferData for ResourceZoneSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceZoneSnapshot")?;
        xfer.xfer_marker_label("Bounds")?;
        self.bounds.xfer(xfer)?;
        xfer.xfer_marker_label("ResourceType")?;
        self.resource_type.xfer(xfer)?;
        xfer.xfer_marker_label("TotalAmount")?;
        xfer.xfer_u32(&mut self.total_amount)?;
        xfer.xfer_marker_label("RemainingAmount")?;
        xfer.xfer_u32(&mut self.remaining_amount)?;
        Ok(())
    }
}

impl XferData for ResourceManagerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceManagerSnapshot")?;
        xfer.xfer_marker_label("SupplyDeposits")?;
        xfer_vec_default(
            xfer,
            &mut self.supply_deposits,
            SupplyDepositSnapshot {
                position: glam::Vec3::ZERO,
                amount: 0,
                depletion_rate: 0.0,
                harvesters: Vec::new(),
            },
        )?;
        xfer.xfer_marker_label("ResourceZones")?;
        xfer_vec_default(
            xfer,
            &mut self.resource_zones,
            ResourceZoneSnapshot {
                bounds: GeometryInfo::default(),
                resource_type: String::new(),
                total_amount: 0,
                remaining_amount: 0,
            },
        )?;
        Ok(())
    }
}

impl XferData for ActiveCombatSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ActiveCombatSnapshot")?;
        xfer.xfer_marker_label("Attacker")?;
        self.attacker.xfer(xfer)?;
        xfer.xfer_marker_label("Target")?;
        self.target.xfer(xfer)?;
        xfer.xfer_marker_label("StartTime")?;
        xfer.xfer_f32(&mut self.start_time)?;
        xfer.xfer_marker_label("DamageDealt")?;
        xfer.xfer_f32(&mut self.damage_dealt)?;
        Ok(())
    }
}

impl XferData for DeathEventSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("DeathEventSnapshot")?;
        xfer.xfer_marker_label("ObjectId")?;
        self.object_id.xfer(xfer)?;
        xfer.xfer_marker_label("KillerId")?;
        xfer_option(xfer, &mut self.killer_id, ObjectId(0))?;
        xfer.xfer_marker_label("DeathTime")?;
        xfer.xfer_f32(&mut self.death_time)?;
        xfer.xfer_marker_label("DeathPosition")?;
        self.death_position.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for CombatTrackerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatTrackerSnapshot")?;
        xfer.xfer_marker_label("ActiveCombats")?;
        xfer_vec_default(
            xfer,
            &mut self.active_combats,
            ActiveCombatSnapshot {
                attacker: ObjectId(0),
                target: ObjectId(0),
                start_time: 0.0,
                damage_dealt: 0.0,
            },
        )?;
        xfer.xfer_marker_label("RecentDeaths")?;
        xfer_vec_default(
            xfer,
            &mut self.recent_deaths,
            DeathEventSnapshot {
                object_id: ObjectId(0),
                killer_id: None,
                death_time: 0.0,
                death_position: glam::Vec3::ZERO,
            },
        )?;
        Ok(())
    }
}

impl XferData for ExperienceEventSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ExperienceEventSnapshot")?;
        xfer.xfer_marker_label("ObjectId")?;
        self.object_id.xfer(xfer)?;
        xfer.xfer_marker_label("ExperienceGained")?;
        xfer.xfer_f32(&mut self.experience_gained)?;
        xfer.xfer_marker_label("Source")?;
        self.source.xfer(xfer)?;
        xfer.xfer_marker_label("Timestamp")?;
        xfer.xfer_f32(&mut self.timestamp)?;
        Ok(())
    }
}

impl XferData for VeterancyBonuses {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("VeterancyBonuses")?;
        xfer.xfer_marker_label("HealthBonus")?;
        xfer.xfer_f32(&mut self.health_bonus)?;
        xfer.xfer_marker_label("DamageBonus")?;
        xfer.xfer_f32(&mut self.damage_bonus)?;
        xfer.xfer_marker_label("AccuracyBonus")?;
        xfer.xfer_f32(&mut self.accuracy_bonus)?;
        xfer.xfer_marker_label("RangeBonus")?;
        xfer.xfer_f32(&mut self.range_bonus)?;
        Ok(())
    }
}

impl XferData for ExperienceTrackerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ExperienceTrackerSnapshot")?;
        xfer.xfer_marker_label("ExperienceEvents")?;
        xfer_vec_default(
            xfer,
            &mut self.experience_events,
            ExperienceEventSnapshot {
                object_id: ObjectId(0),
                experience_gained: 0.0,
                source: String::new(),
                timestamp: 0.0,
            },
        )?;
        xfer.xfer_marker_label("VeterancyBonuses")?;
        xfer_hashmap_default(
            xfer,
            &mut self.veterancy_bonuses,
            ObjectId(0),
            VeterancyBonuses {
                health_bonus: 0.0,
                damage_bonus: 0.0,
                accuracy_bonus: 0.0,
                range_bonus: 0.0,
            },
        )?;
        Ok(())
    }
}

impl XferData for PathfindingCacheSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PathfindingCacheSnapshot")?;
        xfer.xfer_marker_label("CachedPaths")?;
        {
            let mut len = self.cached_paths.len() as u32;
            xfer.xfer_u32(&mut len)?;
            if xfer.get_mode() == XferMode::Load {
                self.cached_paths.clear();
                for _ in 0..len {
                    let mut k = (
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                    );
                    let mut v = Vec::new();
                    k.0.xfer(xfer)?;
                    k.1.xfer(xfer)?;
                    let mut path_len = 0u32;
                    xfer.xfer_u32(&mut path_len)?;
                    for _ in 0..path_len {
                        let mut sv = SerializableVec3 { x: 0, y: 0, z: 0 };
                        sv.xfer(xfer)?;
                        v.push(sv);
                    }
                    self.cached_paths.insert(k, v);
                }
            } else {
                for (k, v) in &mut self.cached_paths {
                    let mut k0 = k.0;
                    let mut k1 = k.1;
                    k0.xfer(xfer)?;
                    k1.xfer(xfer)?;
                    let mut path_len = v.len() as u32;
                    xfer.xfer_u32(&mut path_len)?;
                    for sv in v.iter_mut() {
                        sv.xfer(xfer)?;
                    }
                }
            }
        }
        xfer.xfer_marker_label("CacheTimestamps")?;
        {
            let mut len = self.cache_timestamps.len() as u32;
            xfer.xfer_u32(&mut len)?;
            if xfer.get_mode() == XferMode::Load {
                self.cache_timestamps.clear();
                for _ in 0..len {
                    let mut k = (
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                    );
                    let mut ts = 0.0f32;
                    k.0.xfer(xfer)?;
                    k.1.xfer(xfer)?;
                    ts.xfer(xfer)?;
                    self.cache_timestamps.insert(k, ts);
                }
            } else {
                for (k, ts) in &mut self.cache_timestamps {
                    let mut k0 = k.0;
                    let mut k1 = k.1;
                    k0.xfer(xfer)?;
                    k1.xfer(xfer)?;
                    ts.xfer(xfer)?;
                }
            }
        }
        Ok(())
    }
}

