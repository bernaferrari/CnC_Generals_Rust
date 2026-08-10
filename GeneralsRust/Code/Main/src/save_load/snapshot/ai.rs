//! AI player / strategy / global-AI snapshot residual.

use super::xfer_helpers::{
    xfer_hashmap_default, xfer_option, xfer_vec_default, xfer_vec_vec3,
};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// AI player snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIPlayerSnapshot {
    pub player_id: u32,
    pub difficulty: String,
    pub personality: String,
    pub current_strategy: String,
    pub strategic_state: AIStrategicStateSnapshot,
    pub tactical_state: AITacticalStateSnapshot,
    pub economic_state: AIEconomicStateSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIStrategicStateSnapshot {
    pub current_phase: String,
    pub objectives: Vec<AIObjective>,
    pub threat_assessment: ThreatAssessmentSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIObjective {
    pub objective_type: String,
    pub priority: f32,
    pub target_position: Option<glam::Vec3>,
    pub assigned_units: Vec<ObjectId>,
    pub completion_percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatAssessmentSnapshot {
    pub enemy_strengths: HashMap<Team, f32>,
    pub vulnerable_areas: Vec<glam::Vec3>,
    pub threat_level: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITacticalStateSnapshot {
    pub unit_groups: Vec<AIUnitGroupSnapshot>,
    pub active_attacks: Vec<AIAttackSnapshot>,
    pub defensive_positions: Vec<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIUnitGroupSnapshot {
    pub group_id: u32,
    pub units: Vec<ObjectId>,
    pub role: String,
    pub current_task: String,
    pub formation: String,
    pub target_position: Option<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAttackSnapshot {
    pub attack_id: u32,
    pub target_position: glam::Vec3,
    pub assigned_groups: Vec<u32>,
    pub attack_phase: String,
    pub start_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIEconomicStateSnapshot {
    pub build_priorities: Vec<BuildPriority>,
    pub economic_focus: String,
    pub resource_allocation: ResourceAllocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPriority {
    pub template_name: String,
    pub priority: f32,
    pub desired_count: u32,
    pub current_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub military_percentage: f32,
    pub economic_percentage: f32,
    pub defensive_percentage: f32,
}

/// Global AI state snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalAIStateSnapshot {
    pub global_timers: HashMap<String, f32>,
    pub global_flags: HashMap<String, bool>,
    pub difficulty_modifiers: DifficultyModifiers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultyModifiers {
    pub ai_resource_bonus: f32,
    pub ai_damage_bonus: f32,
    pub ai_health_bonus: f32,
    pub ai_build_speed_bonus: f32,
}

impl Default for DifficultyModifiers {
    fn default() -> Self {
        Self {
            ai_resource_bonus: 1.0,
            ai_damage_bonus: 1.0,
            ai_health_bonus: 1.0,
            ai_build_speed_bonus: 1.0,
        }
    }
}

impl XferData for AIObjective {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIObjective")?;
        xfer.xfer_marker_label("ObjectiveType")?;
        self.objective_type.xfer(xfer)?;
        xfer.xfer_marker_label("Priority")?;
        xfer.xfer_f32(&mut self.priority)?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        xfer.xfer_marker_label("AssignedUnits")?;
        xfer_vec_default(xfer, &mut self.assigned_units, ObjectId(0))?;
        xfer.xfer_marker_label("CompletionPercentage")?;
        xfer.xfer_f32(&mut self.completion_percentage)?;
        Ok(())
    }
}

impl XferData for ThreatAssessmentSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ThreatAssessmentSnapshot")?;
        xfer.xfer_marker_label("EnemyStrengths")?;
        xfer_hashmap_default(xfer, &mut self.enemy_strengths, Team::Neutral, 0.0f32)?;
        xfer.xfer_marker_label("VulnerableAreas")?;
        xfer_vec_vec3(xfer, &mut self.vulnerable_areas)?;
        xfer.xfer_marker_label("ThreatLevel")?;
        xfer.xfer_f32(&mut self.threat_level)?;
        Ok(())
    }
}

impl XferData for AIStrategicStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIStrategicStateSnapshot")?;
        xfer.xfer_marker_label("CurrentPhase")?;
        self.current_phase.xfer(xfer)?;
        xfer.xfer_marker_label("Objectives")?;
        xfer_vec_default(
            xfer,
            &mut self.objectives,
            AIObjective {
                objective_type: String::new(),
                priority: 0.0,
                target_position: None,
                assigned_units: Vec::new(),
                completion_percentage: 0.0,
            },
        )?;
        xfer.xfer_marker_label("ThreatAssessment")?;
        self.threat_assessment.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for AIUnitGroupSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIUnitGroupSnapshot")?;
        xfer.xfer_marker_label("GroupId")?;
        xfer.xfer_u32(&mut self.group_id)?;
        xfer.xfer_marker_label("Units")?;
        xfer_vec_default(xfer, &mut self.units, ObjectId(0))?;
        xfer.xfer_marker_label("Role")?;
        self.role.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentTask")?;
        self.current_task.xfer(xfer)?;
        xfer.xfer_marker_label("Formation")?;
        self.formation.xfer(xfer)?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        Ok(())
    }
}

impl XferData for AIAttackSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIAttackSnapshot")?;
        xfer.xfer_marker_label("AttackId")?;
        xfer.xfer_u32(&mut self.attack_id)?;
        xfer.xfer_marker_label("TargetPosition")?;
        self.target_position.xfer(xfer)?;
        xfer.xfer_marker_label("AssignedGroups")?;
        xfer.xfer_vec_u32(&mut self.assigned_groups)?;
        xfer.xfer_marker_label("AttackPhase")?;
        self.attack_phase.xfer(xfer)?;
        xfer.xfer_marker_label("StartTime")?;
        xfer.xfer_f32(&mut self.start_time)?;
        Ok(())
    }
}

impl XferData for AITacticalStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AITacticalStateSnapshot")?;
        xfer.xfer_marker_label("UnitGroups")?;
        xfer_vec_default(
            xfer,
            &mut self.unit_groups,
            AIUnitGroupSnapshot {
                group_id: 0,
                units: Vec::new(),
                role: String::new(),
                current_task: String::new(),
                formation: String::new(),
                target_position: None,
            },
        )?;
        xfer.xfer_marker_label("ActiveAttacks")?;
        xfer_vec_default(
            xfer,
            &mut self.active_attacks,
            AIAttackSnapshot {
                attack_id: 0,
                target_position: glam::Vec3::ZERO,
                assigned_groups: Vec::new(),
                attack_phase: String::new(),
                start_time: 0.0,
            },
        )?;
        xfer.xfer_marker_label("DefensivePositions")?;
        xfer_vec_vec3(xfer, &mut self.defensive_positions)?;
        Ok(())
    }
}

impl XferData for BuildPriority {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("BuildPriority")?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Priority")?;
        xfer.xfer_f32(&mut self.priority)?;
        xfer.xfer_marker_label("DesiredCount")?;
        xfer.xfer_u32(&mut self.desired_count)?;
        xfer.xfer_marker_label("CurrentCount")?;
        xfer.xfer_u32(&mut self.current_count)?;
        Ok(())
    }
}

impl XferData for ResourceAllocation {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceAllocation")?;
        xfer.xfer_marker_label("MilitaryPercentage")?;
        xfer.xfer_f32(&mut self.military_percentage)?;
        xfer.xfer_marker_label("EconomicPercentage")?;
        xfer.xfer_f32(&mut self.economic_percentage)?;
        xfer.xfer_marker_label("DefensivePercentage")?;
        xfer.xfer_f32(&mut self.defensive_percentage)?;
        Ok(())
    }
}

impl XferData for AIEconomicStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIEconomicStateSnapshot")?;
        xfer.xfer_marker_label("BuildPriorities")?;
        xfer_vec_default(
            xfer,
            &mut self.build_priorities,
            BuildPriority {
                template_name: String::new(),
                priority: 0.0,
                desired_count: 0,
                current_count: 0,
            },
        )?;
        xfer.xfer_marker_label("EconomicFocus")?;
        self.economic_focus.xfer(xfer)?;
        xfer.xfer_marker_label("ResourceAllocation")?;
        self.resource_allocation.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for AIPlayerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIPlayerSnapshot")?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("Difficulty")?;
        self.difficulty.xfer(xfer)?;
        xfer.xfer_marker_label("Personality")?;
        self.personality.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentStrategy")?;
        self.current_strategy.xfer(xfer)?;
        xfer.xfer_marker_label("StrategicState")?;
        self.strategic_state.xfer(xfer)?;
        xfer.xfer_marker_label("TacticalState")?;
        self.tactical_state.xfer(xfer)?;
        xfer.xfer_marker_label("EconomicState")?;
        self.economic_state.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for DifficultyModifiers {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("DifficultyModifiers")?;
        xfer.xfer_marker_label("AIResourceBonus")?;
        xfer.xfer_f32(&mut self.ai_resource_bonus)?;
        xfer.xfer_marker_label("AIDamageBonus")?;
        xfer.xfer_f32(&mut self.ai_damage_bonus)?;
        xfer.xfer_marker_label("AIHealthBonus")?;
        xfer.xfer_f32(&mut self.ai_health_bonus)?;
        xfer.xfer_marker_label("AIBuildSpeedBonus")?;
        xfer.xfer_f32(&mut self.ai_build_speed_bonus)?;
        Ok(())
    }
}

impl XferData for GlobalAIStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("GlobalAIStateSnapshot")?;
        xfer.xfer_marker_label("GlobalTimers")?;
        xfer_hashmap_default(xfer, &mut self.global_timers, String::new(), 0.0f32)?;
        xfer.xfer_marker_label("GlobalFlags")?;
        xfer_hashmap_default(xfer, &mut self.global_flags, String::new(), false)?;
        xfer.xfer_marker_label("DifficultyModifiers")?;
        self.difficulty_modifiers.xfer(xfer)?;
        Ok(())
    }
}

