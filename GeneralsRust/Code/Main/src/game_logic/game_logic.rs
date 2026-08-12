#![allow(non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Game Logic System
** Copyright 2025 Electronic Arts Inc.
**
** Main GameLogic singleton - manages all objects, simulation, and game state
** Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
*/

pub(self) use super::mission_scripts::{
    CameoFlashRequest, CameraAddShakerRequest, CameraBwModeRequest,
    CameraLookTowardWaypointRequest, CameraModFinalSpeedMultiplierRequest,
    CameraModRollingAverageRequest, CameraMotionBlurRequest, CameraMoveToRequest,
    CameraPathRequest, CameraPitchRequest, CameraRotateRequest, CameraSetDefaultRequest,
    CameraSlaveModeRequest, CameraZoomRequest, MissionScriptActionHandler, MissionScriptHooks,
    NamedTimerMutation, RadarScriptEventRequest, ScreenShakeRequest, ScriptPopupMessageRequest,
    SetFpsLimitRequest, SuperweaponObjectDisplayMutation, ViewGuardbandRequest,
    VisualSpeedMultiplierRequest,
};
pub(self) use super::partition_manager::PartitionManager;
pub(self) use super::radar_notifications::{self, RadarEntry, RadarNotifications};
pub(self) use super::script_events::{self, ScriptEvent};
pub(self) use super::victory::{PlayerOutcome, PlayerResult, VictoryCondition, VictorySummary};
pub(self) use super::victory_conditions::{
    victory_rules_for_map, AllianceNotification, VictoryConditions,
};
pub(self) use super::*;
pub(self) use crate::ai::*;
pub(self) use crate::assets::{get_asset_manager, ObjectDefinition};
pub(self) use crate::localization;
pub(self) use crate::save_load::campaign::CampaignManager;
pub(self) use crate::save_load::campaign::MissionObjective;
pub(self) use crate::save_load::game_state::global_campaign_manager;
pub(self) use crate::ui::audio::translate_audio_event;
pub(self) use crate::ui::color_for_player;
pub(self) use crate::ui::objectives::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};
pub(self) use game_engine::common::dict::Dict;
pub(self) use game_engine::common::name_key_generator::NameKeyGenerator;
pub(self) use game_engine::common::rts::player_template::get_player_template_store;
pub(self) use game_engine::common::system::build_assistant::get_build_assistant;
pub(self) use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
pub(self) use gamelogic::ai::integration::{initialize_ai_integration, with_ai_integration_mut};
pub(self) use gamelogic::ai::THE_AI;
pub(self) use gamelogic::common::CommandSourceType;
pub(self) use gamelogic::modules::AIUpdateInterfaceExt;
pub(self) use gamelogic::player::{
    GameDifficulty as LogicGameDifficulty, Player as LogicPlayer, PlayerList as LogicPlayerList,
    PlayerTemplate as LogicPlayerTemplate, PlayerType as LogicPlayerType, ThePlayerList,
};
pub(self) use gamelogic::scripting::core::ScriptList;
pub(self) use gamelogic::scripting::engine::ScriptActionHandler;
pub(self) use gamelogic::scripting::{
    ScriptEvent as MissionScriptEvent, ScriptPriority, ScriptValue, ScriptingEngine,
};
pub(self) use gamelogic::sides_list::get_sides_list;
pub(self) use gamelogic::special_power_module::update as update_special_powers;
pub(self) use gamelogic::system::beacon_manager::snapshot_beacons;
pub(self) use gamelogic::system::game_logic::RadarEventType;
pub(self) use gamelogic::system::map_loader::MapLoader as LogicMapLoader;
pub(self) use gamelogic::system::radar_notifier;
pub(self) use gamelogic::system::shroud_manager::get_shroud_manager;
pub(self) use gamelogic::team::get_team_factory;
pub(self) use gamelogic::update_game_logic;
pub(self) use gamelogic::weapon::with_weapon_store_mut;
pub(self) use glam::{Vec2, Vec3};
pub(self) use std::collections::{HashMap, HashSet, VecDeque};
pub(self) use std::path::{Path, PathBuf};
pub(self) use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
pub(self) use std::sync::{Arc, Mutex, OnceLock, RwLock};
pub(self) use std::time::{Duration, Instant, SystemTime};
pub(self) use ww3d_engine::FrameTiming;

pub(self) const SCRIPT_BROADCAST_DURATION: f32 = 6.0;
pub(self) const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
pub(self) const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / LOGIC_FRAMES_PER_SECOND;
pub(self) const SHELL_MISSION_SCRIPT_BUDGET: usize = 8;
/// Cap per-frame mission script evaluation on dense campaign maps so a single
/// frame cannot stall on hundreds of always-true / CALL_SUBROUTINE scripts.
/// (Shell mode already budgets; SP/skirmish previously ran the full list.)
pub(self) const DENSE_MISSION_SCRIPT_BUDGET: usize = 24;
pub(self) const DENSE_MISSION_SCRIPT_THRESHOLD: usize = 48;

/// Host count of crate ticks that were empty-world no-ops (not C++ phase order).
static CRATE_EMPTY_NOOP_TICKS: AtomicU32 = AtomicU32::new(0);

/// Tick the gamelogic crate's full C++-parity update pipeline.
/// This runs AI players, production/build assistant, weapon store (delayed damage),
/// partition manager, death cleanup, locomotor store, victory conditions, and
/// disabled-status checks — all phases from C++ GameLogic::update().
///
/// Empty crate worlds still return `Ok(())` so the host frame loop continues.
/// That is **not** a C++ `GameLogic.cpp` phase-order tick: this helper logs at
/// debug and increments [`crate_empty_noop_tick_count`]. Do not treat `Ok(())`
/// as proof a dual-world crate simulation step ran.
pub fn tick_gamelogic_crate() -> Result<(), String> {
    update_game_logic()?;
    note_crate_empty_noop_if_any();
    Ok(())
}

fn note_crate_empty_noop_if_any() {
    let (is_noop, crate_count) = match gamelogic::get_game_logic().lock() {
        Ok(logic) => (
            logic.last_update_was_empty_noop(),
            logic.empty_world_tick_count(),
        ),
        Err(_) => return,
    };
    if !is_noop {
        return;
    }
    let host_count = CRATE_EMPTY_NOOP_TICKS
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    log::debug!(
        "tick_gamelogic_crate: empty-world no-op (not a C++ GameLogic.cpp phase-order tick); crate_count={crate_count} host_count={host_count}"
    );
}

/// How many dual-tick crate calls reported an empty-world no-op this process.
pub fn crate_empty_noop_tick_count() -> u32 {
    CRATE_EMPTY_NOOP_TICKS.load(Ordering::Relaxed)
}

/// AI command structure for parallel processing
#[derive(Debug)]
pub enum AICommand {
    AttackTarget {
        object_id: ObjectId,
        target_id: ObjectId,
    },
    StopAttack {
        object_id: ObjectId,
    },
    MoveTo {
        object_id: ObjectId,
        position: Vec3,
    },
    SetAIState {
        object_id: ObjectId,
        state: AIState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSpecialAbility {
    /// GLA Hijacker residual: transfer vehicle team + HIJACKED; hijacker consumed.
    Hijack {
        target_id: ObjectId,
    },
    Sabotage {
        target_id: ObjectId,
    },
    /// GLA Terrorist ConvertToCarBomb residual: vehicle → IS_CARBOMB (not instant kill).
    CarBomb {
        target_id: ObjectId,
    },
    /// Jarmen Kell residual: DAMAGE_KILLPILOT → unmanned Neutral vehicle.
    SnipeVehicle {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant timed demo charge on structure/vehicle.
    PlantTimedDemoCharge {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant remote demo charge on structure/vehicle
    /// (SPECIAL_REMOTE_CHARGES — no auto-timer).
    PlantRemoteDemoCharge {
        target_id: ObjectId,
    },
    /// Black Lotus residual: steal cash from enemy supply/cash building.
    StealCashHack {
        target_id: ObjectId,
    },
    /// Black Lotus residual: DISABLED_HACKED on enemy ground vehicle for EffectDuration.
    DisableVehicleHack {
        target_id: ObjectId,
    },
    /// China Hacker residual: DISABLED_HACKED on enemy structure for EffectDuration.
    /// SpecialAbilityHackerDisableBuilding.
    HackerDisableBuilding {
        target_id: ObjectId,
    },
    /// GLA Bomb Truck residual: disguise as target vehicle template/team
    /// (SpecialAbilityDisguiseAsVehicle / StealthUpdate::disguiseAsTemplate).
    DisguiseAsVehicle {
        target_id: ObjectId,
    },
    /// GLA Rebel residual: plant BoobyTrap on structure (SpecialAbilityBoobyTrap).
    PlantBoobyTrap {
        target_id: ObjectId,
    },
}

impl PendingSpecialAbility {
    fn target_id(self) -> ObjectId {
        match self {
            PendingSpecialAbility::Hijack { target_id }
            | PendingSpecialAbility::Sabotage { target_id }
            | PendingSpecialAbility::CarBomb { target_id }
            | PendingSpecialAbility::SnipeVehicle { target_id }
            | PendingSpecialAbility::PlantTimedDemoCharge { target_id }
            | PendingSpecialAbility::PlantRemoteDemoCharge { target_id }
            | PendingSpecialAbility::StealCashHack { target_id }
            | PendingSpecialAbility::DisableVehicleHack { target_id }
            | PendingSpecialAbility::HackerDisableBuilding { target_id }
            | PendingSpecialAbility::DisguiseAsVehicle { target_id }
            | PendingSpecialAbility::PlantBoobyTrap { target_id } => target_id,
        }
    }
}

/// Bridge Main's lightweight Team enum to GameEngine's Arc<RwLock<Team>>.
/// Uses the global TeamFactory to look up teams by player/faction name.
/// Global GameLogic singleton instance
pub(self) static GAME_LOGIC: OnceLock<Arc<Mutex<GameLogic>>> = OnceLock::new();

/// Audio event request (mirrors C++ AudioEventRTS pattern)
/// These events are queued each frame and processed by the audio system
#[derive(Debug, Clone)]
pub struct AudioEventRequest {
    pub event_type: String,          // e.g., "WeaponFire", "UnitDie", "Explosion"
    pub object_id: Option<ObjectId>, // Source object
    pub position: Option<Vec3>,      // 3D world position
    pub priority: u8,                // 0-255 (higher = more important)
    pub is_looping: bool,            // false = fire-and-forget, true = continuous
}

impl AudioEventRequest {
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            object_id: None,
            position: None,
            priority: 128,
            is_looping: false,
        }
    }

    pub fn with_object(mut self, object_id: ObjectId) -> Self {
        self.object_id = Some(object_id);
        self
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = Some(position);
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn looping(mut self) -> Self {
        self.is_looping = true;
        self
    }
}

/// Game mode types
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameMode {
    SinglePlayer,
    Skirmish,
    Multiplayer,
    Replay,
    Internet,
    Lan,
    Shell,
    None,
}

/// Fixed-step loop diagnostics used for shell/menu stall investigations.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedStepDiagnostics {
    pub steps_run: usize,
    pub budget_hit: bool,
    pub accumulated_time_seconds: f32,
}

/// Wave 908: post-tick host residual stamp payload (frame + fixed-step diagnostics).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SimTimingSnapshot {
    pub frame: u32,
    pub steps_run: usize,
    pub budget_hit: bool,
    pub accumulated_time_seconds: f32,
}

/// Aggregate player statistics for victory screen reporting.
#[derive(Debug, Clone, Default)]
pub struct PlayerStatistics {
    pub units_destroyed: u32,
    pub units_lost: u32,
    pub units_built: u32,
    pub structures_destroyed: u32,
    pub structures_lost: u32,
    pub structures_built: u32,
    pub resources_collected: u32,
    pub resources_spent: u32,
    /// C++ ScoreKeeper::m_totalMoneyEarned residual.
    pub money_earned: u32,
    /// C++ AcademyStats::m_structuresCaptured residual.
    pub structures_captured: u32,
    /// Alias honesty counter for academy capture residual.
    pub academy_building_captures: u32,
    /// C++ ScoreKeeper::addObjectCaptured residual count.
    pub objects_captured: u32,
    /// C++ EVA UnitLost residual fires attributed to this player.
    pub eva_unit_lost: u32,
    /// C++ EVA BuildingLost residual fires attributed to this player.
    pub eva_building_lost: u32,
}

/// Player structure
#[derive(Debug, Clone)]
pub struct Player {
    pub id: u32,
    pub team: Team,
    pub name: String,
    pub resources: Resources,
    /// In-flight supply delta under GameWorld economy authority (cleared on writeback).
    pub pending_supply_delta: i64,
    pub power_available: i32,
    /// Total power produced by this player's power plants (for energy ratio).
    pub power_produced: i32,
    /// Total power consumed by this player's buildings (for energy ratio).
    pub power_consumed: i32,
    pub income_accumulator: f32,
    pub selected_objects: Vec<ObjectId>,
    pub unlocked_sciences: HashSet<String>,
    pub queued_upgrades: HashSet<String>,
    pub is_local: bool,
    pub is_alive: bool,
    /// C++ Player::didPlayerPreorder residual (shell/skirmish preorder bonus).
    pub did_preorder: bool,
    pub statistics: PlayerStatistics,
    /// Frame at which power sabotage expires (0 = not sabotaged).
    /// Matches C++ Player::m_powerSabotagedUntilFrame.
    pub power_sabotaged_till_frame: u32,
    /// Skirmish UI color (RGB) applied from match config.
    pub color_rgb: (u8, u8, u8),
    /// Skirmish start position index from match config.
    pub start_position: i32,
    /// Skirmish alliance team index from match config (not faction Team).
    pub alliance_team: i32,
    /// Cash bounty percent residual (GLA SCIENCE_CashBounty).
    /// C++ Player::m_cashBountyPercent — fraction of victim build cost awarded on kill.
    /// 0.0 = disabled; retail tiers 0.05 / 0.10 / 0.20.
    pub cash_bounty_percent: f32,
    /// C++ Player::m_kindOfPercentProductionChangeList residual (CostModifierUpgrade).
    pub kind_of_production_cost_changes:
        Vec<crate::game_logic::host_upgrade_module_residuals::KindOfProductionCostChange>,
    /// Radar residual count (C++ Player::m_radarCount).
    /// Providers: CommandCenter / RadarVan residual ownership path.
    pub radar_count: i32,
    /// True when radar is disabled by script/power residual (C++ m_radarDisabled).
    pub radar_disabled: bool,
    /// C++ Player::m_logicalRetaliationModeEnabled residual (options Auto-Retaliate).
    pub logical_retaliation_mode_enabled: bool,
    /// C++ Player::m_rankLevel residual (1-based retail ranks).
    pub rank_level: u32,
    /// C++ Player::m_skillPoints residual.
    pub skill_points: i32,
    /// C++ Player::m_sciencePurchasePoints residual.
    pub science_purchase_points: i32,
    /// C++ Player::m_specialPowerReadyTimerList residual (seconds remaining).
    /// SharedSyncedTimer superweapons sync across a player's command centers.
    pub shared_special_power_cooldowns: HashMap<crate::command_system::SpecialPowerType, f32>,
}

impl Player {
    /// C&C Generals default starting money is $10,000 (Normal difficulty).
    /// Matches the `StartingMoney::Normal` variant from the LAN API game-info crate.
    pub const DEFAULT_STARTING_MONEY: u32 = 10_000;

    pub fn new(id: u32, team: Team, name: &str, is_local: bool) -> Self {
        Self {
            id,
            team,
            name: name.to_string(),
            resources: Resources {
                supplies: Self::DEFAULT_STARTING_MONEY,
                power: 0,
            },
            pending_supply_delta: 0,
            power_available: 0,
            power_produced: 0,
            power_consumed: 0,
            income_accumulator: 0.0,
            selected_objects: Vec::new(),
            unlocked_sciences: HashSet::new(),
            queued_upgrades: HashSet::new(),
            is_local,
            did_preorder: false,
            is_alive: true,
            statistics: PlayerStatistics::default(),
            power_sabotaged_till_frame: 0,
            color_rgb: (200, 200, 200),
            start_position: -1,
            alliance_team: -1,
            cash_bounty_percent: 0.0,
            kind_of_production_cost_changes: Vec::new(),
            radar_count: 0,
            radar_disabled: false,
            logical_retaliation_mode_enabled: false,
            rank_level: 1,
            skill_points: 0,
            science_purchase_points: 0,
            shared_special_power_cooldowns: HashMap::new(),
        }
    }

    /// C++ Player::getOrStartSpecialPowerReadyFrame residual (seconds remaining).
    /// Missing entry means ready (C++ starts timer at "now" on first query).
    pub fn shared_special_power_remaining(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> f32 {
        self.shared_special_power_cooldowns
            .get(power)
            .copied()
            .unwrap_or(0.0)
            .max(0.0)
    }

    pub fn is_shared_special_power_ready(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        self.shared_special_power_remaining(power) <= 0.0
    }

    /// C++ Player::resetOrStartSpecialPowerReadyFrame residual.
    pub fn reset_shared_special_power_timer(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        reload_seconds: f32,
    ) {
        let cd = reload_seconds.max(0.0);
        if cd <= 0.0 {
            self.shared_special_power_cooldowns.remove(power);
        } else {
            self.shared_special_power_cooldowns
                .insert(power.clone(), cd);
        }
        self.record_host_cooldowns();
    }

    /// C++ Player::expressSpecialPowerReadyFrame(now) residual — ready immediately.
    pub fn express_shared_special_power_ready_now(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
    ) {
        self.shared_special_power_cooldowns.remove(power);
    }

    /// Tick SharedSyncedTimer residual cooldowns.
    ///
    /// Returns powers that just became ready this tick (C++ PublicTimer ready edge).
    pub fn tick_shared_special_power_timers(
        &mut self,
        dt: f32,
    ) -> Vec<crate::command_system::SpecialPowerType> {
        let mut became_ready = Vec::new();
        if dt <= 0.0 || self.shared_special_power_cooldowns.is_empty() {
            return became_ready;
        }
        let keys: Vec<_> = self
            .shared_special_power_cooldowns
            .keys()
            .cloned()
            .collect();
        for power in keys {
            let Some(rem) = self.shared_special_power_cooldowns.get_mut(&power) else {
                continue;
            };
            let was = *rem;
            *rem = (*rem - dt).max(0.0);
            if was > 0.0 && *rem <= 0.0 {
                became_ready.push(power.clone());
            }
        }
        self.shared_special_power_cooldowns
            .retain(|_, rem| *rem > 0.0);
        self.record_host_cooldowns();
        became_ready
    }

    /// C++ Player::hasRadar residual: radar_count > 0 && !radar_disabled.
    pub fn has_radar(&self) -> bool {
        self.radar_count > 0 && !self.radar_disabled
    }

    /// C++ Player::addRadar residual (disable_proof ignored fail-closed).
    pub fn add_radar(&mut self, _disable_proof: bool) {
        self.radar_count = self.radar_count.saturating_add(1);
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::removeRadar residual.
    pub fn remove_radar(&mut self, _disable_proof: bool) {
        self.radar_count = (self.radar_count - 1).max(0);
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    pub fn set_radar_state(&mut self, radar_count: i32, radar_disabled: bool) {
        self.radar_count = radar_count;
        self.radar_disabled = radar_disabled;
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::getCashBounty().
    pub fn get_cash_bounty(&self) -> f32 {
        self.cash_bounty_percent
    }

    /// C++ Player::setCashBounty — only raises if new percent is higher (CashBountyPower).
    pub fn set_cash_bounty(&mut self, percentage: f32) {
        if percentage > self.cash_bounty_percent {
            self.cash_bounty_percent = percentage;
            self.record_host_progress();
        }
    }

    /// Force-set cash bounty percent (tests / load restore).
    pub fn force_set_cash_bounty(&mut self, percentage: f32) {
        self.cash_bounty_percent = percentage.max(0.0);
        self.record_host_progress();
    }

    /// C++ Player::getProductionCostChangeBasedOnKindOf residual.
    pub fn production_cost_factor(&self, kind_tokens: &[&str]) -> f32 {
        crate::game_logic::host_upgrade_module_residuals::production_cost_factor_for_kindof(
            &self.kind_of_production_cost_changes,
            kind_tokens,
        )
    }

    /// C++ Player::addKindOfProductionCostChange residual.
    pub fn add_kind_of_production_cost_change(&mut self, kind_of: &str, percent: f32) {
        crate::game_logic::host_upgrade_module_residuals::add_kind_of_production_cost_change(
            &mut self.kind_of_production_cost_changes,
            kind_of,
            percent,
        );
    }

    pub fn record_host_progress(&self) {
        crate::game_logic::host_player_progress_log::record(
            self.id,
            self.rank_level,
            self.skill_points,
            self.science_purchase_points,
            self.cash_bounty_percent,
        );
    }

    pub fn record_host_sciences(&self) {
        crate::game_logic::host_player_meta_log::record_sciences(
            self.id,
            self.unlocked_sciences.iter().cloned(),
        );
    }

    pub fn record_host_alive(&self) {
        crate::game_logic::host_player_meta_log::record_alive(self.id, self.is_alive);
    }

    pub fn record_host_cooldowns(&self) {
        let mut cds: Vec<(String, f32)> = self
            .shared_special_power_cooldowns
            .iter()
            .map(|(k, v)| (format!("{k:?}"), *v))
            .collect();
        cds.sort_by(|a, b| a.0.cmp(&b.0));
        crate::game_logic::host_player_cooldown_log::record(self.id, cds);
    }

    /// Award cash for a kill: `ceil(victim_build_cost * cash_bounty_percent)`.
    /// C++ Player::doBountyForKill residual (no floating text).
    /// Returns cash awarded (0 when disabled or zero cost).
    pub fn do_bounty_for_kill(&mut self, victim_build_cost: u32) -> u32 {
        let bounty = crate::game_logic::host_cash_bounty::compute_bounty_award(
            victim_build_cost,
            self.cash_bounty_percent,
        );
        if bounty > 0 {
            self.statistics.resources_collected =
                self.statistics.resources_collected.saturating_add(bounty);
            if crate::gameworld_shadow::gameworld_economy_authority_live() {
                self.pending_supply_delta += bounty as i64;
                crate::game_logic::host_economy_log::record(
                    self.id,
                    self.effective_supplies(),
                    self.power_available,
                );
            } else {
                self.resources.supplies = self.resources.supplies.saturating_add(bounty);
                crate::game_logic::host_economy_log::record(
                    self.id,
                    self.resources.supplies,
                    self.power_available,
                );
            }
        }
        bounty
    }

    /// C++ Player::addSkillPoints residual — returns true if rank increased.
    pub fn add_skill_points(&mut self, points: i32) -> bool {
        use crate::game_logic::host_science_rank::{
            retail_cumulative_science_points_through, retail_rank_for_level,
            retail_rank_level_for_skill_points,
        };
        if points <= 0 {
            return false;
        }
        self.skill_points = self.skill_points.saturating_add(points);
        let new_level = retail_rank_level_for_skill_points(self.skill_points).max(1);
        if new_level <= self.rank_level {
            return false;
        }
        let old = self.rank_level;
        self.rank_level = new_level;
        // Grant cumulative science points delta residual.
        let old_spp = retail_cumulative_science_points_through(old);
        let new_spp = retail_cumulative_science_points_through(new_level);
        let delta = (new_spp - old_spp).max(0);
        self.science_purchase_points = self.science_purchase_points.saturating_add(delta);
        // Unlock rank sciences residual.
        for lvl in (old + 1)..=new_level {
            if let Some(row) = retail_rank_for_level(lvl) {
                self.unlocked_sciences
                    .insert(row.science_granted.to_string());
            }
        }
        self.record_host_progress();
        self.record_host_sciences();
        true
    }

    /// Supplies visible to purchase gates (includes in-flight economy-authority delta).
    pub fn effective_supplies(&self) -> u32 {
        let v = self.resources.supplies as i64 + self.pending_supply_delta;
        if v <= 0 {
            0
        } else if v >= u32::MAX as i64 {
            u32::MAX
        } else {
            v as u32
        }
    }

    /// Clear in-flight economy delta after GameWorld writeback.
    pub fn clear_pending_supply_delta(&mut self) {
        self.pending_supply_delta = 0;
    }

    pub fn can_afford(&self, cost: &Resources) -> bool {
        // Money is the hard construction / purchase gate. Power is separate (slows
        // production / disables powered buildings). Do not block structure starts when
        // the grid is already negative — GLA has no power plants, and USA/China must
        // still place a PowerPlant after the first Command Center finishes.
        //
        // Template `build_cost.power` is the post-build power draw residual (often
        // negative). It is applied in spend_resources, not as an affordability gate.
        self.effective_supplies() >= cost.supplies
    }

    pub fn spend_resources(&mut self, cost: &Resources) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        let power_after = self.power_available + cost.power; // Negative for consumption
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta -= cost.supplies as i64;
            self.power_available = power_after;
            if cost.supplies > 0 {
                self.record_resources_spent(cost.supplies);
            }
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies -= cost.supplies;
            self.power_available = power_after;
            if cost.supplies > 0 {
                self.record_resources_spent(cost.supplies);
            }
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
        true
    }

    pub fn add_resources(&mut self, amount: &Resources) {
        if amount.supplies == 0 {
            return;
        }
        if amount.supplies > 0 {
            self.statistics.resources_collected = self
                .statistics
                .resources_collected
                .saturating_add(amount.supplies);
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta += amount.supplies as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_add(amount.supplies);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    /// Queue an upgrade for this player when not already queued/completed and affordable.
    /// Credit absolute supplies (income residual) and log economy channel.

    /// C++ ScoreKeeper::addMoneyEarned residual.

    /// C++ AcademyStats::recordBuildingCapture residual.
    pub fn record_building_capture(&mut self) {
        self.statistics.structures_captured = self.statistics.structures_captured.saturating_add(1);
        self.statistics.academy_building_captures =
            self.statistics.academy_building_captures.saturating_add(1);
    }
    /// C++ ScoreKeeper::addObjectCaptured residual.
    pub fn record_object_captured(&mut self) {
        self.statistics.objects_captured = self.statistics.objects_captured.saturating_add(1);
    }

    pub fn add_money_earned(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.statistics.money_earned = self.statistics.money_earned.saturating_add(amount);
    }

    /// Gain supplies under economy authority (pending delta) or direct mutate.
    pub fn apply_supply_gain(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta += amount as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_add(amount);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    /// Spend supplies already validated via can_afford / effective_supplies.
    pub fn apply_supply_spend_unchecked(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta -= amount as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_sub(amount);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    pub fn credit_supplies(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.statistics.resources_collected =
            self.statistics.resources_collected.saturating_add(amount);
        self.apply_supply_gain(amount);
    }

    pub fn queue_upgrade(&mut self, upgrade_name: &str, cost: &Resources) -> bool {
        if self.has_unlocked_upgrade(upgrade_name) || self.has_queued_upgrade(upgrade_name) {
            return false;
        }
        if !self.spend_resources(cost) {
            return false;
        }
        self.queued_upgrades.insert(upgrade_name.to_string());
        true
    }

    /// Cancel a queued upgrade and refund the requested resources.
    pub fn cancel_queued_upgrade(&mut self, upgrade_name: &str, refund: &Resources) -> bool {
        let Some(queued_name) = self.find_queued_upgrade_name(upgrade_name) else {
            return false;
        };
        self.queued_upgrades.remove(&queued_name);
        self.apply_supply_gain(refund.supplies);
        self.power_available -= refund.power;
        crate::game_logic::host_economy_log::record(
            self.id,
            self.effective_supplies(),
            self.power_available,
        );
        true
    }

    /// Complete all queued player upgrades into the unlocked upgrade/science set.
    pub fn complete_queued_upgrades(&mut self) -> Vec<String> {
        let mut completed: Vec<String> = self.queued_upgrades.drain().collect();
        completed.sort();
        for upgrade in &completed {
            self.unlocked_sciences.insert(upgrade.clone());
        }
        completed
    }

    pub fn has_unlocked_upgrade(&self, upgrade_name: &str) -> bool {
        let expected = normalize_upgrade_name(upgrade_name);
        self.unlocked_sciences
            .iter()
            .any(|unlocked| normalize_upgrade_name(unlocked) == expected)
    }

    pub fn has_unlocked_science(&self, science_name: &str) -> bool {
        self.has_unlocked_upgrade(science_name)
    }

    pub fn unlock_science(&mut self, science_name: &str) -> bool {
        if self.has_unlocked_science(science_name) {
            return false;
        }
        let inserted = self.unlocked_sciences.insert(science_name.to_string());
        // Cash bounty residual: SCIENCE_CashBounty1/2/3 raise player bounty percent.
        if let Some(pct) =
            crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(science_name)
        {
            self.set_cash_bounty(pct);
        }
        if inserted {
            self.record_host_sciences();
        }
        inserted
    }

    /// C++ Player::resetSciences / IntrinsicSciences + Rank1 residual at match start.
    ///
    /// Grants faction SCIENCE_AMERICA/CHINA/GLA, SCIENCE_Rank1, and Rank1
    /// SciencePurchasePointsGranted (**1**). Fail-closed: not full PlayerTemplate
    /// multi-science vector / multiplayer override matrix.
    pub fn apply_faction_intrinsic_sciences(&mut self) {
        use crate::game_logic::host_faction_skirmish_residual::intrinsic_science_for_team;
        use crate::game_logic::host_science_rank::{
            retail_rank_for_level, RANK_SCIENCE_POINTS_DEFAULT, SCIENCE_RANK1,
        };
        if let Some(sci) = intrinsic_science_for_team(self.team) {
            self.unlocked_sciences.insert(sci.to_string());
        }
        self.unlocked_sciences.insert(SCIENCE_RANK1.to_string());
        // Rank level starts at 1 residual.
        if self.rank_level < 1 {
            self.rank_level = 1;
        }
        // Ensure at least Rank1 science purchase points residual if still zero.
        if self.science_purchase_points <= 0 {
            let grant = retail_rank_for_level(1)
                .map(|r| r.science_purchase_points_granted)
                .unwrap_or(RANK_SCIENCE_POINTS_DEFAULT);
            self.science_purchase_points = grant;
        }
    }

    /// C++ Player::isCapableOfPurchasingScience residual.
    pub fn is_capable_of_purchasing_science(&self, science_name: &str) -> bool {
        crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::is_capable_of_purchasing_science_residual(
            &self.unlocked_sciences,
            self.science_purchase_points,
            science_name,
        )
    }

    /// C++ Player::attemptToPurchaseScience residual.
    ///
    /// Spends **science purchase points** (not supplies). Cost 0 = not purchasable.
    pub fn attempt_to_purchase_science(&mut self, science_name: &str) -> bool {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
            normalize_science_name_residual, science_purchase_point_cost_residual,
        };
        let canonical = normalize_science_name_residual(science_name);
        if !self.is_capable_of_purchasing_science(&canonical) {
            return false;
        }
        let cost = science_purchase_point_cost_residual(&canonical).unwrap_or(1);
        if cost > self.science_purchase_points {
            return false;
        }
        self.science_purchase_points -= cost;
        // Wave 202: SPP spend must last-write SetPlayerProgress (sciences meta already
        // records via unlock_science → record_host_sciences).
        let unlocked = self.unlock_science(&canonical);
        if unlocked {
            self.record_host_progress();
        }
        unlocked
    }

    pub fn has_queued_upgrade(&self, upgrade_name: &str) -> bool {
        self.find_queued_upgrade_name(upgrade_name).is_some()
    }

    pub fn find_queued_upgrade_name(&self, upgrade_name: &str) -> Option<String> {
        let expected = normalize_upgrade_name(upgrade_name);
        self.queued_upgrades
            .iter()
            .find(|queued| normalize_upgrade_name(queued) == expected)
            .cloned()
    }

    pub fn record_unit_destroyed(&mut self) {
        self.statistics.units_destroyed = self.statistics.units_destroyed.saturating_add(1);
    }

    pub fn record_unit_lost(&mut self) {
        self.statistics.units_lost = self.statistics.units_lost.saturating_add(1);
    }

    pub fn record_unit_produced(&mut self) {
        self.statistics.units_built = self.statistics.units_built.saturating_add(1);
    }

    pub fn record_structure_built(&mut self) {
        self.statistics.structures_built = self.statistics.structures_built.saturating_add(1);
    }

    pub fn record_structure_destroyed(&mut self) {
        self.statistics.structures_destroyed =
            self.statistics.structures_destroyed.saturating_add(1);
    }

    pub fn record_structure_lost(&mut self) {
        self.statistics.structures_lost = self.statistics.structures_lost.saturating_add(1);
    }

    pub fn record_resources_spent(&mut self, amount: u32) {
        self.statistics.resources_spent = self.statistics.resources_spent.saturating_add(amount);
    }
}

pub(self) fn normalize_upgrade_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub(self) fn capture_upgrade_names_for_team(team: Team) -> &'static [&'static str] {
    match team {
        Team::USA => &[
            "Upgrade_AmericaRangerCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::China => &[
            "Upgrade_ChinaRedguardCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::GLA => &[
            "Upgrade_GLARebelCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::Neutral => &[],
    }
}

/// Skirmish/match rules applied from UI configuration (FOW, crates, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct SkirmishRulesState {
    pub fog_of_war: bool,
    pub crates_enabled: bool,
    pub limit_superweapons: bool,
    pub allow_tech_buildings: bool,
    pub game_speed: f32,
}

impl Default for SkirmishRulesState {
    fn default() -> Self {
        Self {
            fog_of_war: true,
            crates_enabled: true,
            limit_superweapons: false,
            allow_tech_buildings: true,
            game_speed: 1.0,
        }
    }
}

/// Main GameLogic system

/// C++ BuildAssistant FRAMES_TO_ALLOW_SCAFFOLD residual (LOGICFRAMES_PER_SECOND * 1.5 = 45).

/// C++ RebuildHoleBehavior WorkerRespawnDelay residual sample (fail-closed 10s → 300f).
pub(self) const REBUILD_HOLE_WORKER_RESPAWN_FRAMES: u32 = 300;
/// C++ HoleMaxHealth residual default for GLA holes.
pub(self) const REBUILD_HOLE_MAX_HEALTH_RESIDUAL: f32 = 500.0;
/// C++ HoleHealthRegen%PerSecond residual default (0.1 = 10%/sec).
pub(self) const REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC: f32 = 0.10;
/// C++ WorkerObjectName residual sample for GLA holes.
pub(self) const REBUILD_HOLE_WORKER_TEMPLATE: &str = "GLAWorker";
pub(self) const FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL: u32 = 45;
/// C++ TOTAL_FRAMES_TO_SELL_OBJECT residual (LOGICFRAMES_PER_SECOND * 3.0 = 90).
pub(self) const TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL: u32 = 90;
/// C++ construction percent is 0..100; host uses 0..1. Decrement per frame after scaffold.
pub(self) const SELL_CONSTRUCTION_DECREMENT_RESIDUAL: f32 =
    1.0 / (TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL as f32);
/// C++ finish threshold constructionPercent <= -50.0 (host -0.5).
pub(self) const SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL: f32 = -0.5;

/// C++ ObjectSellInfo residual.
#[derive(Debug, Clone)]
pub(self) struct ObjectSellInfo {
    id: ObjectId,
    sell_frame: u32,
}

/// Fat-object ID store as its **own field** so a tick can mut-borrow objects
/// without `&mut self` on the whole [`GameLogic`] (`self.objects.get_mut` +
/// `self.frame` split-borrow).
///
/// Deref to the inner `HashMap` so existing `self.objects.get_mut` call sites
/// keep compiling. When a GameWorld session is coupled the map is a roster /
/// write-through view — `host_authoritative_*` is truth.
#[derive(Debug, Default)]
pub struct HostObjectStore {
    map: HashMap<ObjectId, Object>,
}

impl HostObjectStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    #[inline]
    pub fn map(&self) -> &HashMap<ObjectId, Object> {
        &self.map
    }

    #[inline]
    pub fn map_mut(&mut self) -> &mut HashMap<ObjectId, Object> {
        &mut self.map
    }
}

impl std::ops::Deref for HostObjectStore {
    type Target = HashMap<ObjectId, Object>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl std::ops::DerefMut for HostObjectStore {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl<'a> IntoIterator for &'a HostObjectStore {
    type Item = (&'a ObjectId, &'a Object);
    type IntoIter = std::collections::hash_map::Iter<'a, ObjectId, Object>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a> IntoIterator for &'a mut HostObjectStore {
    type Item = (&'a ObjectId, &'a mut Object);
    type IntoIter = std::collections::hash_map::IterMut<'a, ObjectId, Object>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter_mut()
    }
}

pub struct GameLogic {
    /// Named AttackPriorityInfo residual map (script sets).
    pub attack_priority_sets: std::collections::HashMap<String, AttackPriorityInfo>,
    /// C++ TAiData::m_enableRepulsors residual (AI.ini EnableRepulsors).
    pub enable_repulsors: bool,
    /// C++ TAiData::m_retaliateFriendsRadius residual (default 120).
    pub retaliate_friends_radius: f32,
    /// C++ TAiData::m_maxRetaliateDistance residual (default 210).
    pub max_retaliate_distance: f32,
    /// Objects in the world.
    ///
    /// Own field (not a method on `&mut GameLogic`) so ticks can
    /// `self.objects.get_mut` while still reading `self.frame`.
    /// When a GameWorld shadow session is coupled this map is an ID roster /
    /// read-through **view**. HP / pose / attack-target live in GameWorld;
    /// [`Self::host_object_mut`] overlays those fields from GameWorld, marks the
    /// id dirty, and [`Self::commit_dirty_host_objects_to_gameworld`] pushes
    /// them back. Fail-open host fields only when shadow is off.
    pub objects: HostObjectStore,
    /// Host ids mutated this tick that must write through to GameWorld.
    host_view_dirty: HashSet<ObjectId>,

    /// Players in the game
    players: HashMap<u32, Player>,

    /// Object ID counter
    next_object_id: ObjectId,
    /// C++ TheAI next formation id residual (starts at 1; 0 = none).
    next_formation_id: u32,

    /// Simulation frame counter
    pub(crate) frame: u32,

    /// Game mode
    game_mode: GameMode,

    /// Active skirmish/match rules (from skirmish UI config).
    skirmish_rules: SkirmishRulesState,

    /// Game world dimensions
    world_width: f32,
    world_height: f32,
    world_min: Vec3,
    world_max: Vec3,

    /// Victory conditions subsystem (mirrors SAGE VictoryConditions)
    victory_conditions: VictoryConditions,

    /// Objects to destroy at end of frame
    objects_to_destroy: VecDeque<DestructionEvent>,

    /// Host combat particle registry (kill/fire → observably registered systems).
    /// Residual hq-gq7n: not full W3D GPU parity; PresentationFrame can observe entries.
    combat_particles: CombatParticleRegistry,

    /// Host superweapon strike residual (DaisyCutter / A10 / ScudStorm / ParticleCannon /
    /// NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb / ArtilleryBarrage /
    /// CruiseMissile). Queues on DoSpecialPower and completes with area damage —
    /// NuclearMissile also spawns residual radiation; AnthraxBomb also spawns residual
    /// toxin; SpectreGunship spawns residual orbit damage ticks; CarpetBomb applies
    /// delayed multi-point line damage; ArtilleryBarrage applies delayed multi-shell
    /// scatter damage; CruiseMissile applies delayed loft + MOAB area damage.
    /// Fail-closed vs full retail.
    pub(crate) special_power_strikes:
        crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry,

    /// Host America Paradrop / Airborne residual.
    /// Queues on DoSpecialPower and spawns infantry after approach delay — fail-closed vs full OCL plane.
    pub(crate) host_paradrops: crate::game_logic::host_paradrop::HostParadropRegistry,

    /// Host GLA Rebel Ambush residual.
    /// Queues on DoSpecialPower and spawns infantry near target after fade delay —
    /// fail-closed vs full OCL CreateObject / science upgrade tiers.
    host_ambushes: crate::game_logic::host_ambush::HostAmbushRegistry,
    /// Residual: last SuperweaponCashHack requested science-tier amount.
    last_cash_hack_request_amount: u32,
    /// Residual: last SuperweaponCashHack stolen amount.
    last_cash_hack_stolen_amount: u32,
    /// Residual: last SuperweaponCrateDrop spawned crate count.
    last_crate_drop_spawned: u32,

    /// Host USA Leaflet Drop residual.
    /// Queues on DoSpecialPower; after Delay disables enemy infantry/vehicles
    /// (DISABLED_EMP residual) — fail-closed vs full OCL B52 / LeafletContainer path.
    pub(crate) host_leaflet_drops: crate::game_logic::host_leaflet_drop::HostLeafletDropRegistry,

    /// Host GLA Sneak Attack residual.
    /// Queues on DoSpecialPower; after Lifetime delay spawns tunnel structure +
    /// residual shockwave damage — fail-closed vs full OCL Start animation / TunnelContain.
    host_sneak_attacks: crate::game_logic::host_sneak_attack::HostSneakAttackRegistry,

    /// Host upgrade queue/complete residual (Capture / FlashBang / TOW / SupplyLines).
    /// Completes research into unlocked_sciences and applies observable unit unlocks.
    host_upgrades: crate::game_logic::host_upgrades::HostUpgradeRegistry,

    /// Supply Lines economy residual: total bonus cash credited on drop-off deposits.
    /// Matches C++ SupplyCenterDockUpdate + Chinook `getUpgradedSupplyBoost` path.
    /// Fail-closed: not per-template INI boost matrix / WorkerShoes / multiplayer.
    supply_lines_bonus_cash_total: u32,

    /// Host cash bounty residual (GLA SCIENCE_CashBounty → kill awards cash).
    /// Fail-closed: not full CashBountyPower palace module / floating text.
    cash_bounty: crate::game_logic::host_cash_bounty::HostCashBountyRegistry,

    /// Host garrison residual honesty counters (enter / exit / fire-from-garrison).
    /// Fail-closed: not C++ GarrisonContain fire-point bones or full weapon matrix.
    garrison_residual_enters: u32,
    garrison_residual_exits: u32,
    garrison_residual_fires: u32,

    /// Host transport residual honesty counters (load / unload-all / evacuate).
    /// Fail-closed: not multi-door or Chinook air-transport path parity.
    transport_residual_loads: u32,
    transport_residual_unloads: u32,

    /// Host China Overlord BattleBunker residual honesty counters (enter / exit).
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn.
    overlord_bunker_residual_enters: u32,
    overlord_bunker_residual_exits: u32,

    /// Host GLA Battle Bus residual honesty counters
    /// (load / unload / passenger fire / armed-riders weapon-set).
    /// Fail-closed: not SlowDeath undeath SECOND_LIFE / multi-door exit matrix.
    battle_bus: crate::game_logic::host_battle_bus::HostBattleBusRegistry,
    /// C++ HighlanderBody residual clamps.
    highlander_body_reg: crate::game_logic::host_highlander_body::HostHighlanderBodyRegistry,
    /// C++ DeployStyleAIUpdate residual counters.
    deploy_style_reg: crate::game_logic::host_deploy_style::HostDeployStyleRegistry,
    /// C++ TensileFormationUpdate residual counters.
    tensile_formation_reg: crate::game_logic::host_tensile_formation::HostTensileFormationRegistry,
    /// C++ StatusBitsUpgrade residual counters.
    status_bits_upgrade_reg:
        crate::game_logic::host_status_bits_upgrade::HostStatusBitsUpgradeRegistry,
    /// C++ FireSpreadUpdate residual counters.
    fire_spread_reg: crate::game_logic::host_fire_spread::HostFireSpreadRegistry,
    /// C++ BaseRegenerateUpdate residual counters.
    base_regenerate_reg: crate::game_logic::host_base_regenerate::HostBaseRegenerateRegistry,
    /// C++ EnemyNearUpdate residual counters.
    enemy_near_reg: crate::game_logic::host_enemy_near::HostEnemyNearRegistry,
    /// C++ PassengersFireUpgrade residual counters.
    passengers_fire_upgrade_reg:
        crate::game_logic::host_passengers_fire_upgrade::HostPassengersFireUpgradeRegistry,
    /// C++ AnimationSteeringUpdate residual counters.
    animation_steering_reg:
        crate::game_logic::host_animation_steering::HostAnimationSteeringRegistry,
    /// C++ ActiveShroudUpgrade residual counters.
    active_shroud_upgrade_reg:
        crate::game_logic::host_active_shroud_upgrade::HostActiveShroudUpgradeRegistry,
    /// C++ FloatUpdate residual counters.
    float_update_reg: crate::game_logic::host_float_update::HostFloatUpdateRegistry,
    /// C++ ProneUpdate residual counters.
    prone_update_reg: crate::game_logic::host_prone_update::HostProneUpdateRegistry,
    /// C++ RadiusDecalUpdate residual counters.
    radius_decal_update_reg:
        crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateRegistry,
    /// C++ CheckpointUpdate residual counters.
    checkpoint_update_reg: crate::game_logic::host_checkpoint_update::HostCheckpointUpdateRegistry,
    /// C++ SpectreGunshipDeploymentUpdate residual counters.
    spectre_gunship_deployment_reg:
        crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentRegistry,
    /// C++ SmartBombTargetHomingUpdate residual counters.
    smart_bomb_target_homing_reg:
        crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingRegistry,
    /// C++ OCLSpecialPower residual counters.
    ocl_special_power_reg: crate::game_logic::host_ocl_special_power::HostOclSpecialPowerRegistry,
    /// C++ ObjectCreationList CreateDebris disposition residual.
    ocl_create_debris_reg: crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisRegistry,
    /// C++ OCL FireWeaponNugget + AttackNugget residual.
    ocl_fire_weapon_attack_reg:
        crate::game_logic::host_ocl_fire_weapon_attack::HostOclFireWeaponAttackRegistry,
    /// C++ FuelAir gas SlowDeathBehavior residual.
    fuel_air_gas_reg: crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasRegistry,
    /// C++ OCL ApplyRandomForceNugget residual.
    ocl_apply_random_force_reg:
        crate::game_logic::host_ocl_apply_random_force::HostOclApplyRandomForceRegistry,
    /// C++ NeutronMissileUpdate residual counters.
    neutron_missile_update_reg:
        crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateRegistry,
    /// C++ ScudStormMissile ballistic flight residual counters.
    scud_storm_missile_flight_reg:
        crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightRegistry,
    /// C++ CarpetBomb DeliverPayload residual counters.
    pub(crate) carpet_bomb_flight_reg:
        crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightRegistry,
    /// C++ ArtilleryBarrage DeliverPayload residual counters.
    pub(crate) artillery_barrage_flight_reg:
        crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightRegistry,
    /// C++ A10Thunderbolt DeliverPayload residual counters.
    pub(crate) a10_strike_flight_reg:
        crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightRegistry,
    /// C++ DaisyCutter DeliverPayload residual counters.
    pub(crate) daisy_cutter_flight_reg:
        crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightRegistry,
    /// C++ AnthraxBomb DeliverPayload residual counters.
    pub(crate) anthrax_bomb_flight_reg:
        crate::game_logic::host_anthrax_bomb_flight::HostAnthraxBombFlightRegistry,
    /// C++ ClusterMines DeliverPayload residual counters.
    pub(crate) cluster_mines_flight_reg:
        crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightRegistry,
    /// C++ EMPPulse DeliverPayload residual counters.
    pub(crate) emp_pulse_flight_reg:
        crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightRegistry,
    /// C++ CommandButtonHuntUpdate residual counters.
    command_button_hunt_reg:
        crate::game_logic::host_command_button_hunt::HostCommandButtonHuntRegistry,
    /// C++ PreorderCreate residual counters.
    preorder_create_reg: crate::game_logic::host_preorder_create::HostPreorderCreateRegistry,
    /// C++ UpgradeDie residual removals.
    upgrade_die_reg: crate::game_logic::host_upgrade_die::HostUpgradeDieRegistry,

    /// Host GLA Tunnel Network residual (TunnelContain shared MaxTunnelCapacity=10).
    /// Enter any allied tunnel; exit/evacuate at any allied tunnel (cross-tunnel).
    /// Fail-closed: not GuardTunnelNetwork AI / TimeForFullHeal / CaveSystem cave-in.
    tunnel_network: crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry,

    /// Host AirF Combat Chinook residual honesty counters
    /// (load / unload / passenger fire / armed-riders weapon-set).
    /// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
    combat_chinook: crate::game_logic::host_combat_chinook::HostCombatChinookRegistry,

    /// Host China Listening Outpost residual honesty counters
    /// (detect / load / unload / passenger fire / armed-riders / InitialPayload).
    /// Fail-closed: not IR FX / multi-door exit / RIDERS_ATTACKING uncloak matrix.
    listening_outpost: crate::game_logic::host_listening_outpost::HostListeningOutpostRegistry,

    /// Host China Troop Crawler residual honesty counters
    /// (load / unload / initial payload / assault deploy / detect).
    /// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
    troop_crawler: crate::game_logic::host_troop_crawler::HostTroopCrawlerRegistry,

    /// Host mine / demo-trap / timed demo-charge residual honesty counters.
    /// Fail-closed: not full MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate.
    mine_residual_places: u32,
    mine_residual_proximity_detonations: u32,
    mine_residual_timed_detonations: u32,
    mine_residual_manual_detonations: u32,
    /// Dozer/Worker safe mine-clear residual (DAMAGE_DISARM destroy without detonation).
    mine_residual_clears: u32,

    /// Host structure/vehicle repair residual honesty counters.
    /// Fail-closed: not full DozerAIUpdate percent heal / RepairDockUpdate TimeForFullHeal.
    /// structure: dozer Repair command accepted / structure HP heal ticks applied.
    /// vehicle: SeekingRepair heal ticks at RepairPad / WarFactory / Airfield.
    repair_residual_structure_commands: u32,
    repair_residual_structure_heals: u32,
    repair_residual_vehicle_heals: u32,

    /// Host infantry heal residual honesty counters.
    /// Fail-closed: not full AutoHealBehavior sole-benefactor / vehicle radius matrix.
    /// ambulance: radius AutoHeal infantry HP ticks (AmericaVehicleMedic residual).
    /// heal_pad: SeekingHealing HP ticks at HealPad.
    heal_residual_ambulance_heals: u32,
    heal_residual_heal_pad_heals: u32,

    /// Host China Propaganda / Speaker Tower residual honesty counters.
    /// Fail-closed: not full PropagandaTowerBehavior sole-benefactor / upgrade FX matrix.
    /// heals: radius %max-health heal ticks applied to same-team non-structures.
    /// buffs: ENTHUSIASTIC / SUBLIMINAL weapon-bonus flag grants.
    propaganda_residual_heals: u32,
    propaganda_residual_buffs: u32,

    /// Host China ECM Tank / jammer residual honesty counters.
    /// Fail-closed: not full subdual damage / laser stream / missile scatter matrix.
    /// jams: weapons_jammed grants applied to enemy/neutral units in radius.
    ecm_residual_jams: u32,

    /// Host America Microwave Tank residual (DISABLED_SUBDUED on structures).
    /// Fail-closed: not full subdual accumulate/heal / laser stream / emitter field.
    microwaves: crate::game_logic::host_microwave::HostMicrowaveRegistry,
    /// C++ ParkingPlaceBehavior runway in-use residual (airfield → runway slots → jet).
    runway_reservations: std::collections::HashMap<ObjectId, Vec<Option<ObjectId>>>,

    /// Host China EMP Pulse residual (DISABLED_EMP on vehicles/structures).
    /// Fail-closed: not full OCL EMPPulseBomb / EMPPulseEffectSpheroid drawable path.
    emp_pulses: crate::game_logic::host_emp_pulse::HostEmpPulseRegistry,
    /// Host BaikonurLaunchPower residual (door open + detonation multi-blast).
    baikonur_launches: crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry,
    /// Host DefectorSpecialPower residual.
    defector_special:
        crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry,
    /// Host CostModifier/Unpause/WeaponBonus upgrade module residuals.
    upgrade_module_residuals:
        crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog,
    /// Host ReplaceObject / GrantScience / CommandSet upgrade residuals.
    replace_grant_command_upgrades:
        crate::game_logic::host_replace_object_upgrade::HostReplaceGrantCommandUpgradeLog,
    /// Host SubObjectsUpgrade residual log.
    sub_objects_upgrades: crate::game_logic::host_sub_objects_upgrade::HostSubObjectsUpgradeLog,

    /// Host China Frenzy ("Rage") residual — temporary ally attack buff in radius.
    /// Frenzy_InvisibleMarker + DeletionUpdate residual closed; fail-closed vs FrenzyCloud GPU.
    frenzies: crate::game_logic::host_frenzy::HostFrenzyRegistry,

    /// Host USA Strategy Center battle-plan residual (Bombardment / HoldTheLine / S&D).
    /// Fail-closed: not full BattlePlanUpdate pack/unpack / paralyze / turret matrix.
    battle_plans: crate::game_logic::host_strategy_center::HostBattlePlanRegistry,
    /// Honesty: StrategyCenterGun ScatterRadius peels applied.
    strategy_center_gun_scatter_applied: u32,
    /// Honesty: StrategyCenterGun scatter residual misses.
    strategy_center_gun_scatter_misses: u32,

    /// Host Emergency Repair residual — SingleBurst ally vehicle heal in radius.
    /// Fail-closed: not full OCL RepairVehicles invisible marker / RepairCloud path.
    emergency_repairs: crate::game_logic::host_emergency_repair::HostEmergencyRepairRegistry,

    /// Host Cleanup Area residual — clear toxin/radiation fields + mines at location.
    /// Fail-closed: not full CleanupHazardUpdate projectile stream / scan loop.
    cleanup_areas: crate::game_logic::host_cleanup_area::HostCleanupAreaRegistry,

    /// Host GLA GPS Scrambler residual — GrantStealth ally vehicles/infantry in radius.
    /// Fail-closed: not full OCL GPSScrambler_InvisibleMarker grow-radius pulse path.
    gps_scramblers: crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry,

    /// Host base-defense residual honesty (Patriot / Gattling auto-fire).
    /// Fail-closed: not full AutoAcquire / WeaponSet / continuous-fire matrix.
    base_defense_residual_fires: u32,

    /// Host PointDefenseLaser residual honesty (Paladin / Avenger intercept).
    /// Fail-closed: not full PointDefenseLaserUpdate velocity prediction matrix.
    point_defense_residual_intercepts: u32,
    /// Honesty: ECMTankMissileJammer missiles jammed/scattered residual.
    ecm_missiles_jammed: u32,
    /// Honesty: ECMDisableStream laser beams spawned residual.
    ecm_laser_beams_spawned: u32,
    /// Honesty: PointDefenseLaserBeam objects spawned on intercept residual.
    point_defense_laser_beams_spawned: u32,
    /// Per-carrier next ready frame for residual PDL shot delay.
    point_defense_next_ready_frame: HashMap<ObjectId, u32>,

    /// Host America Avenger residual honesty (FAERIE_FIRE paint / air laser / ROF).
    /// Fail-closed: not full portable laser turret / dual AirLaser stream matrix.
    avenger: crate::game_logic::host_avenger::HostAvengerRegistry,

    /// Host Neutron Shell residual honesty (Nuke Cannon secondary blast).
    /// Fail-closed: not full DumbProjectileBehavior / NeutronBlastBehavior modules.
    neutron_shell_residual_blasts: u32,
    neutron_shell_residual_infantry_kills: u32,
    neutron_shell_residual_vehicles_unmanned: u32,

    /// Host Bunker Buster residual (Stealth Fighter + Upgrade_AmericaBunkerBusters).
    /// Fail-closed: not full BunkerBusterBehavior crash FX / seismic / shockwave path.
    bunker_buster: crate::game_logic::host_bunker_buster::HostBunkerBusterRegistry,

    /// Host Comanche Rocket Pods residual honesty (area attack when secondary fires).
    /// ScatterTarget projectile residual closed; tertiary WeaponSet fail-closed.
    comanche_rocket_pod_residual_area_attacks: u32,
    comanche_rocket_pod_residual_units_hit: u32,
    comanche_rocket_pod_shot_index: std::collections::HashMap<ObjectId, u32>,
    comanche_rocket_pod_projectiles_spawned: u32,

    /// Host Sentry Drone residual honesty (auto-detect spawn + gun auto-fire).
    /// Fail-closed: not full DeployStyleAIUpdate pack/unpack matrix.
    sentry_drone_residual_auto_fires: u32,
    sentry_drone_residual_detects: u32,

    /// Host Pathfinder residual honesty (innate stealth detector + sniper).
    /// Fail-closed: not full StealthUpdate pulse / SCIENCE_Pathfinder gate.
    pathfinder_residual_detects: u32,
    pathfinder_residual_sniper_fires: u32,

    /// Host Scout / Hellfire slave-drone residual honesty.
    /// Fail-closed: not full SlavedUpdate wander / ObjectCreationUpgrade matrix.
    scout_drone_residual_detects: u32,
    scout_drone_residual_attaches: u32,
    hellfire_drone_residual_auto_fires: u32,
    hellfire_drone_residual_attaches: u32,
    /// Honesty: Hellfire ScatterRadiusVsInfantry peels applied.
    hellfire_scatter_applied: u32,
    /// Honesty: Hellfire ScatterRadiusVsInfantry residual misses vs infantry.
    hellfire_scatter_misses: u32,

    /// Host RadarScan / RadarVanScan FOW temporary-reveal residual.
    /// RadarVanPing object residual closed; fail-closed vs grid decal GPU path.
    radar_scans: crate::game_logic::host_radar_scan::HostRadarScanRegistry,

    /// Host SpySatellite FOW temporary-reveal residual.
    /// SpySatellitePing object residual closed; fail-closed vs GridDecal GPU path.
    spy_drones: crate::game_logic::host_spy_drone::HostSpyDroneRegistry,
    spy_satellites: crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry,
    /// Host America Countermeasures residual (aircraft flare diversion).
    /// CountermeasureFlare SpecialObject spawn residual closed.
    pub(crate) countermeasures:
        crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,

    /// Host CIA Intelligence / SpyVision residual (setUnitsVisionSpied).
    /// Fail-closed: not full SpyVisionUpdate module / kindof filter / sabotage path.
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,

    /// Host hero special-ability residual (snipe / timed C4 / cash hack).
    /// Fail-closed: not full SpecialAbilityUpdate preparation / flee / upgrade matrix.
    hero_abilities: crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry,

    /// Host GLA Black Market residual cash (AutoDepositUpdate residual).
    /// Fail-closed: not full floating text / InitialCaptureBonus / upgrade boost matrix.
    pub(crate) black_markets: crate::game_logic::host_black_market::HostBlackMarketRegistry,

    /// Host Tech Oil Derrick residual cash (AutoDepositUpdate residual).
    /// AutoDeposit residual (SupplyLines boost + floating text host residual closed).
    pub(crate) oil_derricks: crate::game_logic::host_oil_derrick::HostOilDerrickRegistry,

    /// Host China Hacker / Internet Center residual cash (HackInternetAIUpdate).
    /// Fail-closed: not full unpack/pack state machine / variation / floating text.
    pub(crate) hacker_income: crate::game_logic::host_hacker_income::HostHackerIncomeRegistry,

    /// Host America Supply Drop Zone residual cash (OCLUpdate residual).
    /// Fail-closed: not full CreateAtEdge cargo plane / parachute crate fall path
    /// (delayed DeliverPayload spawn residual via host_deliver_payloads).
    supply_drop_zones: crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry,

    /// Host DeliverPayload cargo residual (delayed payload spawn at location).
    /// Fail-closed: not full AmericaJetCargoPlane Object / DeliverPayloadAIUpdate
    /// flight state machine / parachute container physics (DropDelay stagger +
    /// DropOffset / MaxAttempts / PreOpenDistance constants residual closed).
    host_deliver_payloads: crate::game_logic::host_deliver_payload::HostDeliverPayloadRegistry,

    /// Host MoneyCrateCollide residual (unit + BuildingPickup).
    /// Fail-closed: not full CollideModule partition pair / Anim2D MoneyPickUp.
    pub(crate) host_money_crates: crate::game_logic::host_money_crate::HostMoneyCrateRegistry,

    /// Host CommandCenter / RadarVan radar-online residual (Player::hasRadar).
    /// Fail-closed: not full RadarUpgrade/RadarUpdate grant matrix / power-disable proof.
    pub(crate) host_radar: crate::game_logic::host_radar::HostRadarRegistry,

    /// Host GLA Hijack / ConvertToCarBomb residual.
    /// Fail-closed: not full HijackerUpdate hide-in-vehicle / WeaponSet chooser matrix.
    car_bomb: crate::game_logic::host_car_bomb::HostCarBombRegistry,
    /// Host GLA Saboteur structure sabotage residual (power/cash/factory/superweapon).
    /// Fail-closed: not full BuildingPickup CrateCollide / EVA floating-text matrix.
    saboteur: crate::game_logic::host_saboteur::HostSaboteurRegistry,
    /// USA Pilot recrew residual honesty.
    usa_pilot: crate::game_logic::host_usa_pilot::HostUsaPilotRegistry,
    /// GLA Worker / WorkerShoes residual honesty.
    gla_worker: crate::game_logic::host_gla_worker::HostGlaWorkerRegistry,

    /// Host GLA Bomb Truck disguise residual (SpecialAbilityDisguiseAsVehicle).
    /// Fail-closed: not full StealthUpdate transition opacity / model swap matrix.
    bomb_truck_disguise: crate::game_logic::host_bomb_truck_disguise::HostBombTruckDisguiseRegistry,

    /// Host GLA Bomb Truck HE/Bio FireWeaponWhenDead detonation residual.
    /// Fail-closed: not full exclusive FireWeaponWhenDead module / SubObjects matrix.
    bomb_truck_detonate: crate::game_logic::host_bomb_truck_detonate::HostBombTruckDetonateRegistry,
    /// Host China Nuclear Tanks residual (death blast + speed + radiation).
    /// Fail-closed: not full FireWeaponWhenDead exclusive / locomotor visual matrix.
    nuclear_tanks: crate::game_logic::host_nuclear_tanks::HostNuclearTanksRegistry,
    /// Host GLA Rebel BoobyTrap residual (plant + capture/death detonate).
    /// Fail-closed: not full StickyBombUpdate SpecialObject / MaxSpecialObjects matrix.
    booby_trap: crate::game_logic::host_booby_trap::HostBoobyTrapRegistry,
    /// Honesty: BoobyTrap SpecialObject Things spawned.
    booby_trap_objects_spawned: u32,

    /// Host China Helix NapalmBomb special ability residual (blast + FirestormSmall).
    /// Fail-closed: not full SpecialObject NapalmBomb fall / expand animation.
    helix_napalm: crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry,

    /// Host China FireWall / Firestorm residual (Dragon Tank line of fire zones).
    /// FireWallSegment OCL spawn + InchForwardLocomotor crawl residual closed.
    fire_walls: crate::game_logic::host_firewall::HostFireWallRegistry,

    /// Host China Inferno Cannon residual fire zones (FireFieldSmall DoT).
    /// Fail-closed: not full InfernoTankShell projectile / OCL_FireFieldSmall object spawn.
    inferno_fire_zones: crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry,

    /// Host America Aurora dive bomb residual (delayed FuelAir / AuroraBomb area damage).
    /// FuelAir CreateObjectDie gas SpecialObject residual closed.
    pub(crate) aurora_bombs: crate::game_logic::host_aurora_bomb::HostAuroraBombRegistry,
    /// Honesty: AirF/SupW Aurora FuelAir gas objects spawned on dive impact.
    aurora_fuel_air_gas_spawned: u32,

    /// Host GLA Angry Mob residual (nexus damages nearby enemies / expands members).
    /// SpawnBehavior member SpecialObject residual closed.
    angry_mobs: crate::game_logic::host_angry_mob::HostAngryMobRegistry,

    /// Host SCIENCE_StealthFighter production gate residual honesty.
    /// Fail-closed: not full PrerequisiteSciences rank tree / control-bar science UI.
    stealth_fighter_science: crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry,

    /// Host SCIENCE unit-training residual (VeterancyGainCreate StartingLevel).
    /// Fail-closed: not full PrerequisiteSciences rank tree / IsTrainable matrix.
    unit_training: crate::game_logic::host_unit_training::HostUnitTrainingRegistry,

    /// Host Demo SuicideBomb residual (Demo_Upgrade_SuicideBomb death blast).
    /// Fail-closed: not full FireWeaponWhenDead exclusive / SlowDeath fling matrix.
    demo_suicide_bomb: crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry,

    /// Host GLA Rocket Buggy residual honesty (long-range rocket + scatter splash).
    /// Fail-closed: not full projectile flight / AP rocket mult matrix.
    rocket_buggy_residual_fires: u32,
    rocket_buggy_residual_units_hit: u32,
    rocket_buggy_residual_scatter_misses: u32,

    /// Host GLA Quad Cannon residual honesty (ground gun + AA secondary + multi-barrel).
    /// Fail-closed: not full salvage W3D turret subobject matrix.
    quad_cannon_residual_ground_fires: u32,
    quad_cannon_residual_aa_fires: u32,
    quad_cannon_residual_barrel_upgrades: u32,

    /// Host GLA SCUD launcher residual (area blast + MediumPoisonField toxin DoT).
    /// Fail-closed: not full SCUDMissile projectile lob / salvage PlusOne matrix.
    scud_poison_zones: crate::game_logic::host_scud_launcher::HostScudPoisonRegistry,
    /// Host Overlord/Helix/Emperor portable addon residual honesty registry.
    overlord_addons: crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry,
    /// Host Nuke Cannon primary residual (area + medium radiation).
    nuke_cannon_residual: crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry,
    /// Host residual: GLA Technical transport + salvage weapon honesty.
    /// Fail-closed: not full SalvageCrate W3D gunner subobject matrix.
    technical_residual_fires: u32,
    technical_residual_units_hit: u32,
    technical_residual_weapon_upgrades: u32,
    technical_residual_loads: u32,
    technical_residual_unloads: u32,
    /// Host residual: GLA Toxin Tractor stream/spray/death poison fields.
    toxin_tractor: crate::game_logic::host_toxin_tractor::HostToxinTractorRegistry,

    /// Host residual: GLA Marauder salvage fire-rate tiers honesty.
    /// Fail-closed: not full SalvageCrate W3D turret subobject matrix.
    marauder_residual_fires: u32,
    marauder_residual_units_hit: u32,
    marauder_residual_weapon_upgrades: u32,

    /// Host residual: GLA Scorpion gun + salvage + rocket secondary honesty.
    /// Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
    scorpion_residual_fires: u32,
    scorpion_residual_units_hit: u32,
    scorpion_residual_rocket_upgrades: u32,
    scorpion_residual_salvage_upgrades: u32,
    scorpion_residual_missile_fires: u32,

    /// Host residual: USA Tomahawk dual-radius missile honesty.
    /// TomahawkMissile projectile lob residual closed (MissileAI peels + impact).
    tomahawk_residual_fires: u32,
    tomahawk_residual_units_hit: u32,

    /// Host residual: USA Raptor jet missiles + Laser Missiles honesty.
    /// RETURN_TO_BASE ClipReload airfield rearm residual (dock then ClipReload frames).
    raptor_residual_fires: u32,
    raptor_residual_units_hit: u32,
    raptor_residual_laser_missiles_upgrades: u32,
    /// Host China MiG residual napalm / BlackNapalm / Nuke missile honesty.
    mig_residual_fires: u32,
    mig_residual_units_hit: u32,
    mig_residual_black_napalm_upgrades: u32,
    mig_residual_tactical_nuke_upgrades: u32,
    mig_residual_fire_fields: u32,
    mig_residual_radiation_fields: u32,
    /// Honesty: MiG NapalmMissile ScatterRadiusVsInfantry peels applied.
    mig_scatter_applied: u32,
    /// Honesty: MiG ScatterRadiusVsInfantry residual misses vs infantry.
    mig_scatter_misses: u32,
    /// Host America Fire Base howitzer residual honesty.
    fire_base_residual_fires: u32,
    fire_base_residual_units_hit: u32,

    /// Host Stealth Fighter residual honesty (missile fire + splash).
    /// Fail-closed: not full RETURN_TO_BASE ClipReload / science production matrix.
    stealth_fighter_residual_fires: u32,
    stealth_fighter_residual_units_hit: u32,
    /// Honesty: StealthJetMissile projectiles spawned residual.
    stealth_jet_missiles_spawned: u32,
    /// Honesty: Stealth Jet ScatterRadiusVsInfantry aim offsets applied.
    stealth_jet_scatter_applied: u32,
    /// Honesty: Stealth Jet ScatterRadiusVsInfantry residual misses vs infantry.
    stealth_jet_scatter_misses: u32,
    /// Honesty: SCUDMissile projectiles spawned residual.
    scud_missiles_spawned: u32,
    /// Honesty: TomahawkMissile projectiles spawned residual.
    tomahawk_missiles_spawned: u32,
    /// Honesty: Tomahawk ScatterRadiusVsInfantry aim offsets applied.
    tomahawk_scatter_applied: u32,
    /// Honesty: Tomahawk ScatterRadiusVsInfantry residual misses vs infantry.
    tomahawk_scatter_misses: u32,
    /// Honesty: RocketBuggyMissile projectiles spawned residual.
    rocket_buggy_missiles_spawned: u32,
    /// Honesty: Rocket Buggy ScatterRadiusVsInfantry aim offsets applied.
    rocket_buggy_scatter_applied: u32,
    /// Honesty: SCUD Launcher ScatterRadiusVsInfantry aim offsets applied.
    scud_launcher_scatter_applied: u32,
    /// Honesty: SCUD launcher ScatterRadiusVsInfantry residual misses vs infantry.
    scud_launcher_scatter_misses: u32,
    /// Honesty: NeutronCannonShell projectiles spawned residual.
    neutron_shells_spawned: u32,
    /// Honesty: Neutron shell ScatterRadiusVsInfantry aim offsets applied.
    neutron_shell_scatter_applied: u32,
    /// Honesty: Neutron shell ScatterRadiusVsInfantry residual misses vs infantry.
    neutron_shell_scatter_misses: u32,
    /// Honesty: TunnelDefenderMissile / RPG projectiles spawned residual.
    rpg_trooper_missiles_spawned: u32,
    /// Honesty: RPG Trooper ScatterRadiusVsInfantry aim offsets applied.
    rpg_trooper_scatter_applied: u32,
    /// Honesty: RPG Trooper ScatterRadiusVsInfantry residual misses vs infantry.
    rpg_trooper_scatter_misses: u32,
    /// Honesty: TankHunterMissile projectiles spawned residual.
    tank_hunter_missiles_spawned: u32,
    /// Honesty: Tank Hunter ScatterRadiusVsInfantry aim offsets applied.
    tank_hunter_scatter_applied: u32,
    /// Honesty: Tank Hunter ScatterRadiusVsInfantry residual misses vs infantry.
    tank_hunter_scatter_misses: u32,
    /// Honesty: MissileDefenderMissile projectiles spawned residual.
    missile_defender_missiles_spawned: u32,
    /// Honesty: Missile Defender ScatterRadiusVsInfantry aim offsets applied.
    missile_defender_scatter_applied: u32,
    /// Honesty: Missile Defender ScatterRadiusVsInfantry residual misses vs infantry.
    missile_defender_scatter_misses: u32,
    /// Honesty: ScorpionTankShell projectiles spawned residual.
    scorpion_shells_spawned: u32,
    /// Honesty: Scorpion ScatterRadiusVsInfantry aim offsets applied.
    scorpion_scatter_applied: u32,
    /// Honesty: Scorpion ScatterRadiusVsInfantry residual misses vs infantry.
    scorpion_scatter_misses: u32,
    /// Honesty: ScorpionMissile projectiles spawned residual.
    scorpion_missiles_spawned: u32,
    /// Honesty: NukeCannonShell projectiles spawned residual.
    nuke_cannon_shells_spawned: u32,
    /// Honesty: NukeCannon ScatterRadiusVsInfantry aim offsets applied.
    nuke_cannon_scatter_applied: u32,
    /// Honesty: Nuke Cannon ScatterRadiusVsInfantry residual misses vs infantry.
    nuke_cannon_scatter_misses: u32,
    /// Honesty: GenericTankShell (USA Crusader/Paladin) projectiles spawned residual.
    usa_tank_shells_spawned: u32,
    /// Honesty: USA tank ScatterRadiusVsInfantry aim offsets applied.
    usa_tank_scatter_applied: u32,
    /// Honesty: USA tank ScatterRadiusVsInfantry residual misses vs infantry.
    usa_tank_scatter_misses: u32,
    /// Honesty: BattleMasterTankShell projectiles spawned residual.
    battlemaster_shells_spawned: u32,
    /// Honesty: Battlemaster ScatterRadiusVsInfantry aim offsets applied.
    battlemaster_scatter_applied: u32,
    /// Honesty: Battlemaster ScatterRadiusVsInfantry residual misses vs infantry.
    battlemaster_scatter_misses: u32,
    /// Honesty: OverlordTankShell projectiles spawned residual.
    overlord_shells_spawned: u32,
    /// Honesty: Overlord ScatterRadiusVsInfantry aim offsets applied.
    overlord_scatter_applied: u32,
    /// Honesty: Overlord ScatterRadiusVsInfantry residual misses vs infantry.
    overlord_scatter_misses: u32,
    /// Honesty: InfernoTankShell projectiles spawned residual.
    inferno_shells_spawned: u32,
    /// Honesty: Inferno Cannon ScatterRadiusVsInfantry aim offsets applied.
    inferno_scatter_applied: u32,
    /// Honesty: Inferno Cannon ScatterRadiusVsInfantry residual misses vs infantry.
    inferno_scatter_misses: u32,
    /// Honesty: MarauderTankShell projectiles spawned residual.
    marauder_shells_spawned: u32,
    /// Honesty: Marauder ScatterRadiusVsInfantry aim offsets applied.
    marauder_scatter_applied: u32,
    /// Honesty: Marauder ScatterRadiusVsInfantry residual misses vs infantry.
    marauder_scatter_misses: u32,
    /// Honesty: Fire Base GenericTankShell lob projectiles spawned residual.
    fire_base_shells_spawned: u32,
    /// Honesty: FireBaseHowitzer ScatterRadiusVsInfantry aim offsets applied.
    fire_base_scatter_applied: u32,
    /// Honesty: Fire Base ScatterRadiusVsInfantry residual misses vs infantry.
    fire_base_scatter_misses: u32,
    /// Honesty: RaptorJetMissile projectiles spawned residual.
    raptor_missiles_spawned: u32,
    /// Honesty: Raptor ScatterRadiusVsInfantry aim offsets applied.
    raptor_scatter_applied: u32,
    /// Honesty: Raptor ScatterRadiusVsInfantry residual misses vs infantry.
    raptor_scatter_misses: u32,
    /// Honesty: NapalmMissile / MiG projectiles spawned residual.
    mig_missiles_spawned: u32,
    /// Honesty: RangerFlashBangGrenade projectiles spawned residual.
    flashbang_grenades_spawned: u32,
    /// Honesty: Flashbang ScatterRadius aim offsets applied.
    flashbang_scatter_applied: u32,
    /// Honesty: Flashbang ScatterRadius residual misses intended target.
    flashbang_scatter_misses: u32,
    /// Honesty: HumveeMissile / PatriotMissile TOW projectiles spawned residual.
    humvee_tow_missiles_spawned: u32,
    /// Honesty: Humvee ground TOW ScatterRadiusVsInfantry aim offsets applied.
    humvee_tow_scatter_applied: u32,
    /// Honesty: Humvee ground TOW ScatterRadiusVsInfantry residual misses vs infantry.
    humvee_tow_scatter_misses: u32,
    /// Honesty: Humvee TOW residual fires (spawn or instant fallback).
    humvee_tow_residual_fires: u32,
    /// Honesty: DragonTankFlameProjectile spawned residual.
    dragon_flame_missiles_spawned: u32,
    /// Honesty: ToxinTruckStreamProjectile spawned residual.
    toxin_stream_missiles_spawned: u32,
    /// Honesty: TechnicalRPGMissile spawned residual.
    technical_rpg_missiles_spawned: u32,
    /// Honesty: Technical cannon GenericTankShell spawned residual.
    technical_cannon_shells_spawned: u32,
    /// Honesty: Technical cannon ScatterRadiusVsInfantry aim offsets applied.
    technical_cannon_scatter_applied: u32,
    /// Honesty: TechnicalCannon ScatterRadiusVsInfantry residual misses vs infantry.
    technical_cannon_scatter_misses: u32,
    /// Honesty: CleanupStreamProjectile spawned residual.
    cleanup_stream_missiles_spawned: u32,
    /// Honesty: Angry Mob rock/molotov projectiles spawned residual.
    angry_mob_projectiles_spawned: u32,
    /// Honesty: USA tank gun residual units hit.
    usa_tank_residual_units_hit: u32,

    /// Host Comanche combat residual honesty (20mm + anti-tank dual-radius).
    /// Rocket pods residual counters remain separate below.
    comanche_cannon_residual_fires: u32,
    comanche_cannon_residual_units_hit: u32,
    comanche_antitank_residual_fires: u32,
    comanche_antitank_residual_units_hit: u32,
    /// Honesty: Comanche AT ScatterRadiusVsInfantry peels applied.
    comanche_at_scatter_applied: u32,
    /// Honesty: Comanche AT ScatterRadiusVsInfantry residual misses vs infantry.
    comanche_at_scatter_misses: u32,

    /// Host Helix PRIMARY minigun residual honesty.
    /// Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
    helix_minigun_residual_fires: u32,
    helix_minigun_residual_units_hit: u32,

    /// Host Inferno BlackNapalm FireFieldUpgraded residual honesty.
    /// Fail-closed: not HistoricBonus Firestorm multi-shell matrix.
    inferno_black_napalm_residual_upgrades: u32,
    inferno_black_napalm_residual_zones: u32,

    /// Host residual: USA Battle Drone attach / gun / repair honesty.
    /// Fail-closed: not full SlavedUpdate arm weld FX / ConflictsWith matrix.
    battle_drone_residual_attaches: u32,
    battle_drone_residual_fires: u32,
    battle_drone_residual_units_hit: u32,
    battle_drone_residual_repairs: u32,
    battle_drone_residual_repair_amount: f32,

    /// Host residual: China Overlord / Emperor main gun dual-radius + Uranium honesty.
    /// Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
    overlord_gun_residual_fires: u32,
    overlord_gun_residual_units_hit: u32,
    overlord_gun_residual_uranium_upgrades: u32,

    /// Host residual: GLA Jarmen Kell sniper + AP Bullets honesty.
    /// Fail-closed: not full secondary pilot-sniper AutoChoose matrix.
    jarmen_kell_residual_fires: u32,
    jarmen_kell_residual_units_hit: u32,
    jarmen_kell_residual_ap_upgrades: u32,

    /// Host residual: China Battlemaster tank gun + Uranium / horde / nationalism honesty.
    /// Fail-closed: not full HordeUpdate RubOff / Nuclear Tanks death residual.
    battlemaster_residual_fires: u32,
    battlemaster_residual_units_hit: u32,
    battlemaster_residual_uranium_upgrades: u32,
    battlemaster_residual_nationalism_upgrades: u32,
    pub(crate) battlemaster_residual_horde_grants: u32,

    /// Host residual: China Red Guard gun + bayonet + horde / nationalism honesty.
    /// Fail-closed: not full WeaponSet tertiary auto-choose / RubOff matrix.
    red_guard_residual_fires: u32,
    red_guard_residual_bayonet_kills: u32,
    red_guard_residual_nationalism_upgrades: u32,
    pub(crate) red_guard_residual_horde_grants: u32,

    /// Host residual: China Tank Hunter RPG + TNT special + horde / nationalism honesty.
    /// Fail-closed: not full SpecialAbilityUpdate flee / MaxSpecialObjects matrix.
    tank_hunter_residual_fires: u32,
    tank_hunter_residual_units_hit: u32,
    tank_hunter_residual_tnt_plants: u32,
    tank_hunter_residual_nationalism_upgrades: u32,
    pub(crate) tank_hunter_residual_horde_grants: u32,
    /// Per-unit TNT special residual last plant frame (ReloadTime 7500ms).
    tank_hunter_tnt_last_frame: HashMap<ObjectId, u32>,

    /// Host residual: GLA Rebel machine gun + AP Bullets honesty.
    /// Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
    rebel_residual_fires: u32,
    rebel_residual_ap_upgrades: u32,

    /// Host residual: USA Ranger rifle + FlashBang splash honesty.
    /// Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
    ranger_residual_rifle_fires: u32,
    ranger_residual_flashbang_fires: u32,
    ranger_residual_units_hit: u32,

    /// Host residual: China Hacker DisableBuilding honesty.
    /// Fail-closed: not full unpack/prep/persistent stream matrix.
    hacker_disable_building_count: u32,

    /// Host residual: China MiniGunner ground/AA + continuous fire + chain guns + horde.
    /// Fail-closed: not full FiringTracker CONTINUOUS_FIRE_* anim / bayonet tertiary matrix.
    minigunner_residual_ground_fires: u32,
    minigunner_residual_aa_fires: u32,
    minigunner_residual_ramp_mean: u32,
    minigunner_residual_ramp_fast: u32,
    minigunner_residual_chain_gun_upgrades: u32,
    minigunner_residual_nationalism_upgrades: u32,
    pub(crate) minigunner_residual_horde_grants: u32,

    /// Host residual: Colonel Burton sniper + knife melee honesty.
    /// Fail-closed: not full clip volley / pre-attack knife anim lock matrix.
    burton_residual_sniper_fires: u32,
    burton_residual_knife_kills: u32,

    /// Host residual: GLA RPG Trooper / Tunnel Defender rocket + AP Rockets honesty.
    /// Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
    rpg_trooper_residual_fires: u32,
    rpg_trooper_residual_units_hit: u32,
    rpg_trooper_residual_ap_upgrades: u32,

    /// Host residual: GLA Terrorist SuicideDynamitePack detonation honesty.
    /// Fail-closed: not ConvertToCarBomb full matrix / Chem anthrax death weapons.
    terrorist_residual_detonations: u32,
    terrorist_residual_units_hit: u32,
    terrorist_residual_damage_dealt: f32,

    /// Host residual: USA Missile Defender missile + laser guided special honesty.
    /// Fail-closed: not full SpecialAbilityUpdate prep / LaserBeam object matrix.
    missile_defender_residual_fires: u32,
    missile_defender_residual_units_hit: u32,
    missile_defender_residual_laser_specials: u32,
    missile_defender_residual_laser_fires: u32,
    /// Honesty: LaserBeam SpecialObject spawned on MD laser-guided activate.
    missile_defender_laser_beams_spawned: u32,

    /// Host residual: GLA Combat Cycle rider weapon switch honesty.
    /// Fail-closed: not full RiderChangeContain STATUS_RIDER death OCL matrix.
    combat_cycle_residual_fires: u32,
    combat_cycle_residual_units_hit: u32,
    combat_cycle_residual_rider_switches: u32,
    combat_cycle_residual_loads: u32,
    combat_cycle_residual_suicides: u32,

    /// Host residual: China Dragon Tank primary flame honesty.
    /// Fail-closed: not full projectile stream / garrison-clear matrix.
    dragon_tank_residual_fires: u32,
    dragon_tank_residual_units_hit: u32,
    dragon_tank_residual_black_napalm_upgrades: u32,

    /// Host residual: China Gattling Tank continuous-fire ramp honesty.
    /// Fail-closed: not full FiringTracker model-condition animation matrix.
    gattling_tank_residual_ground_fires: u32,
    gattling_tank_residual_aa_fires: u32,
    gattling_tank_residual_ramp_mean: u32,
    gattling_tank_residual_ramp_fast: u32,
    gattling_tank_residual_chain_gun_upgrades: u32,

    /// Host residual: China Gattling Cannon structure continuous-fire ramp honesty.
    /// Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
    gattling_building_residual_ground_fires: u32,
    gattling_building_residual_aa_fires: u32,
    gattling_building_residual_ramp_mean: u32,
    gattling_building_residual_ramp_fast: u32,
    gattling_building_residual_chain_gun_upgrades: u32,

    /// Host residual: GLA Stinger Site SPAWNS_ARE_THE_WEAPONS dual ground/AA honesty.
    stinger_site_residual_ground_fires: u32,
    stinger_site_residual_aa_fires: u32,
    stinger_site_residual_ap_rockets_upgrades: u32,
    /// HiveStructureBody residual: damage hits applied to residual slaves.
    stinger_hive_residual_slave_hits: u32,
    /// HiveStructureBody residual: residual slaves killed.
    stinger_hive_residual_slave_kills: u32,
    /// HiveStructureBody residual: swallowed damage when no slaves.
    stinger_hive_residual_swallows: u32,
    /// SpawnBehavior residual: slave respawns completed.
    pub(crate) stinger_hive_residual_respawns: u32,
    /// getClosestSlave residual: propagate hits that used shooter world position.
    stinger_hive_residual_closest_slave_hits: u32,
    /// CamoNetting StealthLook / heat-vision residual applications.
    camo_netting_heat_vision_count: u32,
    /// CamoNetting structure StealthUpdate residual: attack/damage reveals.
    camo_netting_structure_residual_reveals: u32,
    /// OrderIdleEnemiesToAttackMeUponReveal residual wake count (CamoNetting).
    camo_netting_order_idle_enemies_count: u32,
    /// CamoNetting structure StealthUpdate residual: StealthDelay re-cloaks.
    camo_netting_structure_residual_recloaks: u32,
    /// CamoNetting FriendlyOpacity residual: cloaked (min opacity) applications.
    camo_netting_opacity_cloak_count: u32,
    /// CamoNetting FriendlyOpacity residual: revealed (max opacity) applications.
    camo_netting_opacity_reveal_count: u32,
    /// CamoNetting sub-object net mesh residual show applications.
    camo_netting_sub_object_show_count: u32,
    /// Stinger physical soldier orderSlavesToAttackTarget residual orders.
    stinger_slave_order_attack_count: u32,

    /// Host residual: USA Patriot ground/AA dual-slot honesty.
    patriot_residual_ground_fires: u32,
    patriot_residual_aa_fires: u32,
    /// Honesty: Patriot ScatterRadiusVsInfantry offsets / miss peels applied.
    patriot_scatter_applied: u32,
    /// Honesty: Patriot ScatterRadiusVsInfantry residual misses vs infantry.
    patriot_scatter_misses: u32,
    /// Honesty: Stinger ScatterRadiusVsInfantry peels applied.
    stinger_scatter_applied: u32,
    /// Honesty: Stinger ScatterRadiusVsInfantry residual misses vs infantry.
    stinger_scatter_misses: u32,
    /// Superweapon General EMP Patriot residual: DISABLED_EMP grants applied.
    supw_patriot_emp_residual_grants: u32,
    /// Honesty: SupW EMPBlast ScatterRadiusVsInfantry peels applied.
    supw_emp_scatter_applied: u32,
    /// Honesty: SupW EMPBlast scatter residual misses vs infantry.
    supw_emp_scatter_misses: u32,
    /// AssistedTargetingUpdate residual: RequestAssistRange requests issued.
    patriot_assist_residual_requests: u32,
    /// AssistedTargetingUpdate residual: assist weapon shots fired.
    patriot_assist_residual_fires: u32,
    /// AssistedTargetingUpdate residual: assistants that accepted a request.
    patriot_assist_residual_accepts: u32,
    /// BinaryDataStream residual: LaserFromAssisted beams spawned.
    patriot_assist_laser_from_assisted: u32,
    /// BinaryDataStream residual: LaserToTarget beams spawned.
    patriot_assist_laser_to_target: u32,
    /// Active residual BinaryDataStream assist lasers (DeletionUpdate lifetime).
    pub(crate) patriot_assist_lasers:
        Vec<crate::game_logic::host_base_defense::ResidualPatriotAssistLaser>,
    /// Weapon.ini LaserName residual beams (combat fire → presentation freeze).
    weapon_lasers: Vec<crate::game_logic::host_weapon_laser::ResidualWeaponLaser>,
    /// Honesty: Weapon.ini LaserName SpecialObject Things spawned.
    weapon_laser_beams_spawned: u32,
    /// C++ ProjectileStreamUpdate residual registry.
    pub(crate) projectile_streams:
        crate::game_logic::host_projectile_stream::ProjectileStreamRegistry,
    /// Pending AssistingClipSize residual clips (DelayBetweenShots cadence).
    pending_patriot_assists: Vec<crate::game_logic::host_base_defense::PendingPatriotAssist>,
    /// StealthDetectorUpdate DetectionRate residual scans performed.
    stealth_detector_rate_scans: u32,

    /// Game paused state
    is_paused: bool,

    /// Time tracking
    sim_time_seconds: f32,
    accumulated_time: f32,
    last_fixed_step_diagnostics: FixedStepDiagnostics,

    /// Thing templates registry
    pub templates: HashMap<String, ThingTemplate>,

    /// Map data
    map_name: String,
    map_loaded: bool,

    /// Combat system for parallel projectile processing
    pub(crate) combat_system: CombatSystem,

    /// Pathfinding system for parallel path computation
    pathfinding_system: PathfindingSystem,

    /// AI Management System
    ai_manager: AIManager,

    /// Script execution tracking
    pub scripts_loaded: bool,
    pub mission_script_counter: u32,

    /// Audio events queued this frame (mirrors C++ TheAudio pattern)
    /// In production, these would be sent to the audio engine
    pub queued_audio_events: Vec<AudioEventRequest>,

    /// Command queue for UI-generated commands
    pub command_queue: VecDeque<crate::command_system::GameCommand>,
    /// Narrow command-acceptance observation edge used to bind an actual
    /// physical right-click Gather input to the executor-confirmed carriers.
    accepted_gather_commands: VecDeque<AcceptedGatherCommand>,
    /// Narrow economy observation edge emitted only by ReturningResources
    /// after crediting carried supplies to a concrete player.
    supply_dropoff_events: VecDeque<SupplyDropoffEvent>,
    pending_special_abilities: HashMap<ObjectId, PendingSpecialAbility>,

    /// Currently selected objects (used by UI)
    pub selected_objects: Vec<ObjectId>,

    partition_manager: PartitionManager,
    radar_notifications: &'static RadarNotifications,
    last_radar_kind_time: [f32; 3],
    last_radar_audio_time: f32,
    last_radar_event: Option<RadarEntry>,
    /// C++ Radar tryUnderAttackEvent throttle residual (frame, xz pos).
    under_attack_event_history: Vec<(u32, f32, f32)>,
    /// tryUnderAttackEvent residual honesty fires.
    under_attack_events: u32,
    /// EVA BaseUnderAttack residual honesty fires.
    eva_base_under_attack: u32,
    /// EVA AllyUnderAttack residual honesty fires.
    eva_ally_under_attack: u32,
    /// EVA LowPower residual honesty fires.
    eva_low_power: u32,
    /// Next frame LowPower may re-fire (C++ framesBetweenChecks residual).
    eva_low_power_next_frame: u32,
    /// Tracks previous low-power state for edge residual.
    eva_low_power_active: bool,
    /// EVA InsufficientFunds residual honesty fires.
    eva_insufficient_funds: u32,
    /// Next frame InsufficientFunds may re-fire.
    eva_insufficient_funds_next_frame: u32,
    /// EVA UpgradeComplete residual honesty fires.
    eva_upgrade_complete: u32,
    /// EVA GeneralLevelUp residual honesty fires.
    eva_general_level_up: u32,
    /// EVA SuperweaponReady residual honesty fires.
    eva_superweapon_ready: u32,
    /// EVA SuperweaponDetected residual honesty fires.
    eva_superweapon_detected: u32,
    /// EVA SuperweaponLaunched residual honesty fires.
    eva_superweapon_launched: u32,
    /// EVA BeaconDetected residual honesty fires.
    eva_beacon_detected: u32,
    /// EVA hero Own/Enemy *Detected residual honesty fires.
    eva_hero_detected: u32,
    /// EVA SuperweaponLaunched GPS/Sneak residual honesty fires.
    eva_special_launched_misc: u32,
    /// RADAR_EVENT_UPGRADE residual honesty fires.
    radar_upgrade_events: u32,
    /// Structure construction-complete residual honesty fires.
    structure_complete_events: u32,
    /// Unit production-complete residual honesty fires.
    unit_ready_events: u32,
    /// RadarUpdate extendRadar residual starts.
    radar_extend_starts: u32,
    /// RadarUpdate extend completion residual fires.
    radar_extend_completes: u32,
    /// RADAR_EVENT_CONSTRUCTION residual honesty fires.
    radar_construction_events: u32,
    /// Production door cycle residual honesty starts.
    production_door_cycles: u32,
    /// Under-construction model condition residual updates.
    construction_model_condition_updates: u32,
    /// ACTIVELY_CONSTRUCTING residual bit updates.
    pub(crate) actively_constructing_updates: u32,
    /// C++ BuildAssistant m_sellList residual.
    sell_list: Vec<ObjectSellInfo>,
    /// Sell residual process starts / finishes.
    sell_process_starts: u32,
    sell_process_finishes: u32,
    /// C++ sellObject destroy owned mines residual.
    sell_owned_mines_destroyed: u32,
    /// C++ OpenContain::onSelling passenger eject residual.
    sell_passengers_ejected: u32,
    /// C++ ParkingPlaceBehavior::killAllParkedUnits residual.
    sell_parked_units_killed: u32,
    /// C++ TunnelContain::onSelling last-tunnel eject residual.
    sell_tunnel_last_ejects: u32,
    /// C++ ContainModule::onCapture kick residual events.
    capture_kick_outs: u32,
    /// C++ Object::onCapture skirmish AI auto-sell residual events.
    capture_ai_auto_sells: u32,
    /// C++ deselectObject on capture residual events.
    capture_deselections: u32,
    /// C++ TunnelContain::onCapture entrance transfer residual events.
    capture_tunnel_transfers: u32,
    /// C++ TunnelContain last-entrance capture eject residual events.
    capture_tunnel_last_ejects: u32,
    /// C++ TechBuildingBehavior CAPTURED model residual events.
    capture_tech_model_updates: u32,
    /// C++ infantry→unmanned vehicle recrew residual events.
    unmanned_reclaims: u32,
    /// C++ car-bomb dead-man on DISABLED_UNMANNED residual events.
    carbomb_unmanned_detonations: u32,
    /// C++ OverchargeBehavior toggle residual events.
    overcharge_toggles: u32,
    /// C++ OverchargeBehavior drain residual ticks.
    overcharge_drain_ticks: u32,
    /// C++ OverchargeBehavior exhausted auto-disable residual events.
    overcharge_exhaustions: u32,
    /// C++ PowerPlantUpgrade Advanced Control Rods residual completions.
    control_rods_upgrades: u32,
    /// Plants that received EnergyBonus from control rods residual.
    control_rods_plants_affected: u32,
    /// C++ SubliminalMessaging upgrade residual completions.
    subliminal_messaging_upgrades: u32,
    /// Propaganda towers tagged by subliminal residual.
    subliminal_towers_affected: u32,
    /// CONSTRUCTION_COMPLETE duration clears residual.
    construction_complete_clears: u32,
    /// C++ DozerAIUpdate::cancelTask residual events.
    dozer_cancel_task_events: u32,
    /// C++ MSG_RESUME_CONSTRUCTION residual assigns.
    resume_construction_events: u32,
    /// C++ DOZER:RepairComplete residual events.
    repair_complete_events: u32,
    /// C++ attemptHealingFromSoleBenefactor reject residual (dozer repair).
    sole_benefactor_repair_rejects: u32,
    /// C++ DozerPrimaryIdleState bored auto-repair residual events.
    dozer_bored_repair_events: u32,
    /// C++ DozerPrimaryIdleState bored mine-clear residual events.
    dozer_bored_mine_clear_events: u32,
    /// C++ RebuildHoleExposeDie spawn residual events.
    rebuild_hole_spawns: u32,
    /// C++ SupplyWarehouseCreate::onCreate residual registers.
    supply_create_warehouse_registers: u32,
    /// C++ SupplyCenterCreate::onBuildComplete residual registers.
    supply_create_center_registers: u32,
    /// C++ GenerateMinefieldBehavior structure mine placements.
    structure_minefield_placements: u32,
    /// C++ SpecialPowerCompletionDie + PowerPlantUpdate residual log.
    pub(crate) special_power_completion_log:
        crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog,
    /// C++ StickyBombUpdate follow-position residual ticks.
    pub(crate) sticky_bomb_follow_ticks: u32,
    /// C++ StickyBombUpdate target-dead charge destroy residual.
    pub(crate) sticky_bomb_target_deaths: u32,
    /// C++ RebuildHoleBehavior reconstruct residual events.
    rebuild_hole_reconstructs: u32,
    rebuild_hole_workers: u32,
    rebuild_hole_heals: u32,
    rebuild_hole_completes: u32,
    /// C++ transferAttack residual events around rebuild holes.
    rebuild_hole_attack_transfers: u32,
    /// C++ RebuildHoleBehavior::transferBombs residual events.
    rebuild_hole_bomb_transfers: u32,
    /// C++ RECONSTRUCTING death → hole restart residual events.
    rebuild_hole_recon_deaths: u32,
    /// C++ newWorkerRespawnProcess residual events.
    rebuild_hole_worker_restarts: u32,
    pending_camera_focus: Option<Vec3>,
    script_camera_focus_estimate: Vec3,
    script_camera_move_to: Option<ScriptCameraMoveTo>,
    script_camera_path: Option<ScriptCameraPathMove>,
    camera_follow_target: Option<ObjectId>,
    script_default_camera_pitch: f32,
    script_default_camera_angle: f32,
    script_default_camera_max_height: f32,
    script_camera_freeze_time_armed: bool,
    script_camera_freeze_angle_armed: bool,
    script_camera_pending_final_speed_multiplier: Option<f32>,
    script_camera_pending_rolling_average_frames: Option<i32>,
    visual_speed_multiplier: f32,
    script_time_frozen_by_script: bool,
    pending_script_fps_limit: Option<i32>,
    pending_camera_zoom_reset: bool,
    pending_camera_zoom: Option<CameraZoomRequest>,
    pending_camera_pitch: Option<CameraPitchRequest>,
    pending_camera_rotate: Option<CameraRotateRequest>,
    pending_camera_look_toward: Option<CameraLookTowardWaypointRequest>,
    pending_camera_slave_mode_enable: Option<CameraSlaveModeRequest>,
    pending_camera_slave_mode_disable: bool,
    pending_screen_shakes: Vec<ScreenShakeRequest>,
    pending_camera_add_shakers: Vec<CameraAddShakerRequest>,
    pending_popup_messages: Vec<ScriptPopupMessageRequest>,
    pending_view_guardband: Option<ViewGuardbandRequest>,
    pending_camera_bw_mode: Option<CameraBwModeRequest>,
    pending_camera_motion_blur: Vec<CameraMotionBlurRequest>,
    script_skybox_enabled: bool,
    script_cameo_flash_count: HashMap<String, i32>,
    script_named_timers: HashMap<String, (String, bool)>,
    script_named_timer_display_shown: bool,
    script_superweapon_display_enabled: bool,
    script_superweapon_hidden_objects: HashSet<ObjectId>,
    /// Host-owned active beacon world positions (presentation freeze; Wave 211).
    /// Mirrors beacon_manager place/remove without mid-frame Mutex dual-read.
    host_beacons: Vec<Vec3>,
    /// Beacon locations created this frame for HUD highlighting/bloom.
    recent_beacons: Vec<Vec3>,
    script_engine: Option<Arc<ScriptingEngine>>,
    script_event_pump_in_flight: Arc<AtomicBool>,
    script_event_pump_busy_frames: u32,
    loaded_script_lists: Vec<ScriptList>,
    script_source_path: Option<PathBuf>,
    mission_scripts: Arc<MissionScriptHooks>,
    script_broadcasts: Vec<ScriptBroadcast>,
    new_script_messages: Vec<String>,
    cinematic_letterbox: bool,
    cinematic_text: Option<(String, f32)>,
    military_caption: Option<(String, f32)>,
    radar_enabled: bool,
    radar_forced: bool,
    pending_music_stop: bool,
    pending_movie: Option<String>,
    pending_radar_movie: Option<String>,
    mission_objectives: Vec<ObjectiveDisplay>,
    objective_lookup: HashMap<String, usize>,
    campaign_manager: Option<Arc<Mutex<CampaignManager>>>,
    last_map_settings: Option<super::script_loader::MapMetadata>,
    spawned_map_object_ids: Vec<(ObjectId, usize)>,
    terrain: Option<super::terrain::TerrainData>,
    runtime_road_segments: Vec<super::script_loader::RuntimeRoadSegment>,
    runtime_terrain_texture_classes: Vec<super::script_loader::BlendTileTextureClass>,
    pathfinding_height_samples: Option<PathfindingHeightSamples>,
    weather_state: RuntimeWeatherState,
}

#[derive(Debug, Clone)]
pub(self) struct PathfindingHeightSamples {
    width: u32,
    height: u32,
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWeatherState {
    pub current_weather: String,
    pub intensity: f32,
    pub duration_remaining: f32,
    pub next_change_time: f32,
    pub visible: bool,
}

impl Default for RuntimeWeatherState {
    fn default() -> Self {
        Self {
            current_weather: "clear".to_string(),
            intensity: 0.0,
            duration_remaining: 0.0,
            next_change_time: 0.0,
            visible: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(self) struct ParabolicEase {
    in_t: f32,
    out_t: f32,
}

impl ParabolicEase {
    fn new(ease_in_time: f32, ease_out_time: f32) -> Self {
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        Self { in_t, out_t }
    }

    fn eval(self, param: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let v0 = 1.0 + self.out_t - self.in_t;
        if param < self.in_t {
            if self.in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * self.in_t)
            }
        } else if param <= self.out_t {
            (self.in_t + 2.0 * (param - self.in_t)) / v0
        } else {
            let denom = (1.0 - self.out_t).max(f32::EPSILON);
            (self.in_t
                + 2.0 * (self.out_t - self.in_t)
                + (2.0 * (param - self.out_t) + self.out_t * self.out_t - param * param) / denom)
                / v0
        }
    }
}

#[derive(Debug, Clone)]
pub(self) struct ScriptCameraMoveTo {
    start: Vec3,
    target: Vec3,
    ease: ParabolicEase,
    total_time_seconds: f32,
    elapsed_seconds: f32,
    shutter_frames: u32,
    cur_shutter: u32,
    last_ease: f32,
    freeze_time: bool,
    freeze_angle: bool,
    speed_ramp_start_t: f32,
    speed_ramp_start_multiplier: f32,
    speed_ramp_final_multiplier: f32,
}

impl ScriptCameraMoveTo {
    fn new(start: Vec3, request: &CameraMoveToRequest) -> Self {
        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);
        let shutter_frames =
            (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);
        Self {
            start,
            target: request.position,
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
        }
    }

    fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    fn final_focus(&self) -> Vec3 {
        self.target
    }

    fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 && next_ease > prev_ease {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        Some(self.start.lerp(self.target, next_ease))
    }
}

#[derive(Debug, Clone)]
pub(self) struct ScriptCameraPathMove {
    points: Vec<Vec3>,
    segment_length: Vec<f32>,
    total_distance: f32,
    ease: ParabolicEase,
    total_time_seconds: f32,
    elapsed_seconds: f32,
    cur_segment: usize,
    cur_seg_distance: f32,
    shutter_frames: u32,
    cur_shutter: u32,
    last_ease: f32,
    freeze_time: bool,
    freeze_angle: bool,
    rolling_average_frames: i32,
    smoothed_focus: Option<Vec3>,
    speed_ramp_start_t: f32,
    speed_ramp_start_multiplier: f32,
    speed_ramp_final_multiplier: f32,
}

impl ScriptCameraPathMove {
    fn new(start_focus: Vec3, request: &CameraPathRequest) -> Option<Self> {
        let waypoint_name = gamelogic::common::AsciiString::from(&request.waypoint);
        let chain: Vec<Vec3> = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                let mut points = Vec::new();
                let mut current = terrain.get_waypoint_by_name(&waypoint_name)?;
                points.push(Vec3::new(
                    current.get_location().x,
                    0.0,
                    current.get_location().y,
                ));
                while let Some(next_id) = current.get_link(0) {
                    let next = terrain.get_waypoint_by_id(next_id)?;
                    points.push(Vec3::new(next.get_location().x, 0.0, next.get_location().y));
                    current = next;
                }
                Some(points)
            })
            .unwrap_or_default();

        if chain.is_empty() {
            return None;
        }

        let min_delta = gamelogic::common::MAP_XY_FACTOR;
        let mut points: Vec<Vec3> = Vec::with_capacity(chain.len() + 4);
        points.push(start_focus);
        points.push(start_focus);

        for p in chain {
            if let Some(last) = points.last().copied() {
                if Vec2::new(p.x - last.x, p.z - last.z).length() < min_delta {
                    continue;
                }
            }
            points.push(p);
        }

        if points.len() < 3 {
            return None;
        }

        // Pad start to allow spline interpolation like the original W3D view.
        let first = points[1];
        let second = points[2];
        points[0] = Vec3::new(
            first.x - (second.x - first.x),
            0.0,
            first.z - (second.z - first.z),
        );

        // Pad end one segment beyond last to keep interpolation stable.
        let last = *points.last().unwrap();
        let prev = points[points.len() - 2];
        points.push(Vec3::new(
            last.x + (last.x - prev.x),
            0.0,
            last.z + (last.z - prev.z),
        ));

        let last_meaningful = points.len() - 2;
        let mut segment_length = vec![0.0f32; points.len()];
        let mut total_distance = 0.0f32;

        for i in 1..last_meaningful {
            let a = points[i];
            let b = points[i + 1];
            let len = Vec2::new(b.x - a.x, b.z - a.z).length();
            segment_length[i] = len;
            total_distance += len;
        }

        if total_distance < 1.0 && last_meaningful >= 2 {
            let idx = last_meaningful - 1;
            segment_length[idx] += 1.0 - total_distance;
            total_distance = 1.0;
        }

        if last_meaningful >= 2 {
            segment_length[last_meaningful] = segment_length[last_meaningful - 1];
        }

        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);

        let shutter_frames =
            (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);

        Some(Self {
            points,
            segment_length,
            total_distance,
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            cur_segment: 1,
            cur_seg_distance: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            rolling_average_frames: 1,
            smoothed_focus: None,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
        })
    }

    fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    fn final_focus(&self) -> Vec3 {
        let idx = self.points.len().saturating_sub(2);
        self.points.get(idx).copied().unwrap_or(Vec3::ZERO)
    }

    fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    fn set_rolling_average_frames(&mut self, frames: i32) {
        self.rolling_average_frames = frames.max(1);
    }

    fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let last_meaningful = self.points.len().saturating_sub(2);
        if last_meaningful <= 1 {
            return None;
        }

        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        let delta = next_ease - prev_ease;
        self.cur_seg_distance += delta * self.total_distance;

        while self.cur_segment < last_meaningful
            && self.cur_seg_distance >= self.segment_length[self.cur_segment]
        {
            self.cur_seg_distance -= self.segment_length[self.cur_segment];
            self.cur_segment += 1;
            if self.cur_segment >= last_meaningful {
                return None;
            }
        }

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        let seg_len = self.segment_length[self.cur_segment].max(f32::EPSILON);
        let mut factor = (self.cur_seg_distance / seg_len).clamp(0.0, 1.0);

        let (start, mid, end) = if factor < 0.5 {
            let start = (self.points[self.cur_segment - 1] + self.points[self.cur_segment]) * 0.5;
            let mid = self.points[self.cur_segment];
            let end = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            factor += 0.5;
            (start, mid, end)
        } else {
            let start = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            let mid = self.points[self.cur_segment + 1];
            let end = (self.points[self.cur_segment + 1] + self.points[self.cur_segment + 2]) * 0.5;
            factor -= 0.5;
            (start, mid, end)
        };

        let p =
            start + (end - start) * factor + (mid - end + mid - start) * (1.0 - factor) * factor;
        let focus = Vec3::new(p.x, 0.0, p.z);
        let average_factor = 1.0 / self.rolling_average_frames.max(1) as f32;
        let smoothed = if let Some(previous) = self.smoothed_focus {
            previous + (focus - previous) * average_factor
        } else {
            focus
        };
        self.smoothed_focus = Some(smoothed);
        Some(smoothed)
    }
}

pub(self) struct ScriptBroadcast {
    text: String,
    expires_at: f32,
}

pub(self) fn localized_objective_string(id: &str, suffix: &str, fallback: &str) -> String {
    if id.is_empty() {
        return fallback.to_string();
    }
    let normalized = id.replace(' ', "_").to_ascii_lowercase();
    let key = format!("mission.objective.{normalized}.{suffix}");
    localization::localize(&key, fallback)
}

pub(self) fn derive_objective_status(
    obj: &MissionObjective,
) -> (ObjectiveStatus, Option<(u32, u32)>) {
    if let Some(total) = obj.required_count {
        let current = obj.current_count.min(total);
        let status = if current >= total {
            ObjectiveStatus::Completed
        } else {
            ObjectiveStatus::Active
        };
        (status, Some((current, total)))
    } else {
        (ObjectiveStatus::Active, None)
    }
}

pub(self) fn mission_objective_to_display(
    obj: &MissionObjective,
    category: ObjectiveCategory,
) -> ObjectiveDisplay {
    let id = obj.id.clone();
    let fallback_title = if obj.description.is_empty() {
        id.clone()
    } else {
        obj.description.clone()
    };
    let title = localized_objective_string(&id, "title", &fallback_title);
    let description = localized_objective_string(&id, "desc", "");
    let (status, progress) = derive_objective_status(obj);
    ObjectiveDisplay {
        id: if id.is_empty() { None } else { Some(id) },
        title,
        description,
        status,
        progress,
        category,
    }
}

/// C++ AI::findClosestEnemy qualifier flags residual.
/// C++ AttackPriorityInfo residual (ScriptEngine).
#[derive(Debug, Clone)]
pub struct AttackPriorityInfo {
    pub name: String,
    pub default_priority: i32,
    /// Template name → priority (case-insensitive keys stored lowercased).
    pub priorities: std::collections::HashMap<String, i32>,
    /// KindOf name token → priority residual (SetAttackPriorityKindOf).
    pub kind_priorities: std::collections::HashMap<String, i32>,
}

impl Default for AttackPriorityInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            default_priority: 1, // ATTACK_PRIORITY_DEFAULT
            priorities: std::collections::HashMap::new(),
            kind_priorities: std::collections::HashMap::new(),
        }
    }
}

impl AttackPriorityInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn set_priority_template(&mut self, template_name: &str, priority: i32) {
        self.priorities
            .insert(template_name.to_ascii_lowercase(), priority);
    }

    pub fn set_priority_kind(&mut self, kind_name: &str, priority: i32) {
        self.kind_priorities
            .insert(kind_name.to_ascii_lowercase(), priority);
    }

    /// C++ AttackPriorityInfo::getPriority residual.
    pub fn get_priority_for_template(&self, template_name: &str) -> i32 {
        let key = template_name.to_ascii_lowercase();
        self.priorities
            .get(&key)
            .copied()
            .unwrap_or(self.default_priority)
    }
}

/// C++ AIData::m_attackPriorityDistanceModifier residual (world units per priority step).
pub const ATTACK_PRIORITY_DISTANCE_MODIFIER: f32 = 50.0;

pub mod find_enemy_flags {
    pub const CAN_SEE: u32 = 1 << 0;
    pub const CAN_ATTACK: u32 = 1 << 1;
    pub const IGNORE_INSIGNIFICANT_BUILDINGS: u32 = 1 << 2;
    pub const ATTACK_BUILDINGS: u32 = 1 << 3;
    pub const WITHIN_ATTACK_RANGE: u32 = 1 << 4;
    pub const UNFOGGED: u32 = 1 << 5;
}

/// C++ MoodMatrixAction residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoodMatrixAction {
    Idle,
    Move,
    Attack,
    AttackMove,
}

/// C++ MAA_* residual flags (host simplified).
pub mod mood_action_adjust {
    pub const ACTION_OK: u32 = 0x01;
    pub const ACTION_TO_IDLE: u32 = 0x02;
    pub const ACTION_TO_ATTACK_MOVE: u32 = 0x04;
    pub const AFFECT_RANGE_IGNORE_ALL: u32 = 0x10;
    pub const AFFECT_RANGE_WAIT_FOR_ATTACK: u32 = 0x20;
    pub const AFFECT_RANGE_ALERT: u32 = 0x40;
    pub const AFFECT_RANGE_AGGRESSIVE: u32 = 0x80;
}

/// C++ CanAttackResult residual (WeaponSet.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanAttackResult {
    /// C++ ATTACKRESULT_NOT_POSSIBLE
    NotPossible,
    /// C++ ATTACKRESULT_POSSIBLE
    Possible,
    /// C++ ATTACKRESULT_POSSIBLE_AFTER_MOVING
    PossibleAfterMoving,
    /// C++ ATTACKRESULT_INVALID_SHOT
    InvalidShot,
}

/// C++ AbleToAttackType residual (GameCommon.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbleToAttackType {
    /// ATTACK_NEW_TARGET
    NewTarget,
    /// ATTACK_NEW_TARGET_FORCED
    NewTargetForced,
    /// ATTACK_CONTINUED_TARGET
    ContinuedTarget,
    /// ATTACK_CONTINUED_TARGET_FORCED
    ContinuedTargetForced,
}

impl AbleToAttackType {
    pub fn is_forced(self) -> bool {
        matches!(
            self,
            AbleToAttackType::NewTargetForced | AbleToAttackType::ContinuedTargetForced
        )
    }

    pub fn is_continued(self) -> bool {
        matches!(
            self,
            AbleToAttackType::ContinuedTarget | AbleToAttackType::ContinuedTargetForced
        )
    }
}

/// C++ AIAttackState outer residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackMachineResult {
    /// Keep running nested AttackStateMachine.
    Continue,
    /// Victim dead / exit success.
    Success,
    /// Cannot attack (no weapon, max shots, under construction).
    Failure,
}

/// C++ AIAttackFireWeaponState residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackFireResult {
    /// C++ STATE_CONTINUE (PRE_ATTACK wind-up).
    Continue,
    /// C++ STATE_SUCCESS (shot discharged).
    Success,
    /// C++ STATE_FAILURE (dead target / not ready / out of range).
    Failure,
}

/// C++ AIAttackAimAtTargetState residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackAimResult {
    /// Still turning / held out-of-range wait.
    Continue,
    /// Within AcceptableAimDelta.
    Success,
    /// Dead victim / no weapon / held out of range.
    Failure,
}

// Wave 960: chained .find_object/.get_object → host_object idiom.
// Wave 959: internal host_object idiom (legacy get_object/find_object aliases only).
impl GameLogic {
    fn seed_sample_objectives() -> Vec<ObjectiveDisplay> {
        vec![
            ObjectiveDisplay {
                id: Some("sample_primary".to_string()),
                title: localization::localize("objectives.primary.sample.title", "Secure the Area"),
                description: localization::localize(
                    "objectives.primary.sample.desc",
                    "Capture all nearby resource points.",
                ),
                status: ObjectiveStatus::Active,
                progress: Some((0, 3)),
                category: ObjectiveCategory::Primary,
            },
            ObjectiveDisplay {
                id: Some("sample_secondary".to_string()),
                title: localization::localize(
                    "objectives.secondary.sample.title",
                    "Bonus: Destroy Radar",
                ),
                description: localization::localize(
                    "objectives.secondary.sample.desc",
                    "Take out the enemy radar installation.",
                ),
                status: ObjectiveStatus::Completed,
                progress: None,
                category: ObjectiveCategory::Secondary,
            },
        ]
    }
}

#[derive(Debug)]
pub(self) struct DestructionEvent {
    id: ObjectId,
    killer: Option<Team>,
}

/// An authoritative Gather command that the command executor accepted.
///
/// This is intentionally a narrow simulation event rather than an input
/// claim: Main attaches physical mouse provenance only while matching the
/// exact command fingerprint after it has succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedGatherCommand {
    pub(crate) command_id: u32,
    pub(crate) issued_at: SystemTime,
    pub(crate) player_id: u32,
    pub(crate) target_id: ObjectId,
    /// Only carriers whose Gather path assignment succeeded.
    pub(crate) carrier_ids: Vec<ObjectId>,
}

/// A real carried-supply deposit performed by `AIState::ReturningResources`.
///
/// It does not represent passive income, resource-total deltas, or an order
/// request.  Consumers can therefore match the carrier and credited player
/// directly without inferring an economy source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SupplyDropoffEvent {
    pub(crate) carrier_id: ObjectId,
    pub(crate) player_id: u32,
    pub(crate) carried_amount: u32,
}

impl Default for GameLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLogic {
    const MAX_PENDING_GATHER_EVENTS: usize = 64;
    const MAX_PENDING_SUPPLY_DROPOFF_EVENTS: usize = 128;

    /// Record the exact carrier subset accepted by `CommandExecutor::execute_gather`.
    /// The bounded queue is an observation edge only; it never affects command
    /// execution or resource simulation.
    fn record_accepted_gather_command(&mut self, event: AcceptedGatherCommand) {
        if event.carrier_ids.is_empty() {
            return;
        }
        if self.accepted_gather_commands.len() >= Self::MAX_PENDING_GATHER_EVENTS {
            self.accepted_gather_commands.pop_front();
        }
        self.accepted_gather_commands.push_back(event);
    }

    /// Consume Gather acceptances observed since the previous Main input edge.
    pub(crate) fn take_accepted_gather_commands(&mut self) -> Vec<AcceptedGatherCommand> {
        self.accepted_gather_commands.drain(..).collect()
    }

    /// Record a positive carried-resource deposit after the owning player's
    /// `credit_supplies` call has succeeded.
    fn record_supply_dropoff_event(&mut self, event: SupplyDropoffEvent) {
        if event.carried_amount == 0 {
            return;
        }
        if self.supply_dropoff_events.len() >= Self::MAX_PENDING_SUPPLY_DROPOFF_EVENTS {
            self.supply_dropoff_events.pop_front();
        }
        self.supply_dropoff_events.push_back(event);
    }

    /// Consume real ReturningResources deposits for host-side presentation and
    /// physical-input evidence.  No economy totals are synthesized here.
    pub(crate) fn take_supply_dropoff_events(&mut self) -> Vec<SupplyDropoffEvent> {
        self.supply_dropoff_events.drain(..).collect()
    }
}

impl GameLogic {
    fn load_campaign_objectives(&self, map_name: &str) -> Vec<ObjectiveDisplay> {
        let Some(manager) = &self.campaign_manager else {
            return Self::seed_sample_objectives();
        };

        let Ok(guard) = manager.lock() else {
            log::warn!(
                "Campaign manager unavailable while loading objectives for '{}'",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        // Path-stem + short-name match (MD_USA01 ↔ .../MD_USA01.map); prefer
        // missions that actually define objectives (Campaign.ini residual table).
        let Some(mission) = guard.find_mission_for_map(map_name) else {
            log::info!(
                "No campaign mission metadata found for map '{}'; using sample objectives",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        let mut displays = Vec::new();
        for (category, list) in [
            (ObjectiveCategory::Primary, &mission.primary_objectives),
            (ObjectiveCategory::Secondary, &mission.secondary_objectives),
            (ObjectiveCategory::Bonus, &mission.bonus_objectives),
        ] {
            for obj in list.iter() {
                displays.push(mission_objective_to_display(obj, category));
            }
        }

        if displays.is_empty() {
            log::warn!(
                "Mission '{}' ({}) does not define objectives; falling back to samples",
                mission.name,
                mission.id
            );
            Self::seed_sample_objectives()
        } else {
            log::info!(
                "Loaded {} mission objectives for '{}' ({})",
                displays.len(),
                mission.name,
                mission.id
            );
            displays
        }
    }

    fn rebuild_objective_lookup(&mut self) {
        self.objective_lookup.clear();
        for (idx, objective) in self.mission_objectives.iter().enumerate() {
            if let Some(id) = &objective.id {
                self.objective_lookup.insert(id.to_ascii_lowercase(), idx);
            }
        }
    }

    fn with_objective_mut<F>(&mut self, objective_id: &str, mut f: F) -> bool
    where
        F: FnMut(&mut ObjectiveDisplay),
    {
        let key = objective_id.to_ascii_lowercase();
        if let Some(&index) = self.objective_lookup.get(&key) {
            if let Some(objective) = self.mission_objectives.get_mut(index) {
                f(objective);
                return true;
            }
        } else {
            log::debug!("Objective '{}' not found in current mission", objective_id);
        }
        false
    }

    pub fn set_objective_status(&mut self, objective_id: &str, status: ObjectiveStatus) -> bool {
        self.with_objective_mut(objective_id, |objective| objective.status = status)
    }

    pub fn set_objective_progress(
        &mut self,
        objective_id: &str,
        current: u32,
        total: Option<u32>,
    ) -> bool {
        self.with_objective_mut(objective_id, |objective| {
            objective.progress = total.map(|goal| (current.min(goal), goal));
        })
    }

    pub fn mark_objective_completed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Completed)
    }

    pub fn mark_objective_failed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Failed)
    }

    /// Current mission objective displays (campaign residual / UI snapshot source).

    /// Upsert a mission objective residual for presentation/UI freeze.
    pub fn upsert_mission_objective(&mut self, objective: crate::ui::objectives::ObjectiveDisplay) {
        if let Some(id) = objective.id.as_ref() {
            let key = id.to_ascii_lowercase();
            if let Some(&idx) = self.objective_lookup.get(&key) {
                if let Some(slot) = self.mission_objectives.get_mut(idx) {
                    *slot = objective;
                    return;
                }
            }
            self.mission_objectives.push(objective);
            let idx = self.mission_objectives.len().saturating_sub(1);
            self.objective_lookup.insert(key, idx);
        } else {
            self.mission_objectives.push(objective);
        }
    }

    pub fn mission_objectives(&self) -> &[ObjectiveDisplay] {
        &self.mission_objectives
    }

    /// Number of mission scripts currently installed from the last map load.
    pub fn installed_mission_script_count(&self) -> usize {
        self.mission_script_count()
    }
}

/// Wave 930: host direct player-order authority payload.
#[derive(Debug, Clone, Copy)]
pub enum DirectPlayerOrder {
    Attack { player_id: u32, target: ObjectId },
    Stop { player_id: u32 },
    Move { player_id: u32, dest: glam::Vec3 },
    AttackMove { player_id: u32, dest: glam::Vec3 },
}

/// Wave 931: object lifecycle authority payload (create/destroy/prod/path/guard).
#[derive(Debug, Clone)]
pub enum ObjectLifecycleOp {
    Create {
        name: String,
        team: Team,
        spawn_at: glam::Vec3,
    },
    Destroy {
        id: ObjectId,
    },
    ForceCompleteConstruction {
        id: ObjectId,
    },
    ClearMovementPath {
        id: ObjectId,
    },
    AdjustGuardRadius {
        id: ObjectId,
        delta: f32,
    },
    EnqueueProduction {
        producer: ObjectId,
        template_name: String,
    },
    CancelProduction {
        id: ObjectId,
        template_name: String,
    },
    /// Cancel the exact queue slot selected by the Control Bar.
    ///
    /// The UI identifies queue entries by index, so collapsing this to a
    /// template-name lookup can cancel an earlier duplicate instead of the
    /// clicked item.
    CancelProductionAtIndex {
        id: ObjectId,
        queue_index: usize,
    },
}

/// Wave 931: heterogeneous result for [`ObjectLifecycleOp`].
#[derive(Debug, Clone, Copy)]
pub enum ObjectLifecycleResult {
    Created(Option<ObjectId>),
    Bool(bool),
    Radius(Option<f32>),
    Destroyed,
}

/// Wave 932: command-pipeline authority payload (queue/process).
#[derive(Debug, Clone)]
pub enum CommandPipelineOp {
    Queue {
        command: crate::command_system::GameCommand,
    },
    QueueAndProcess {
        command: crate::command_system::GameCommand,
    },
    ProcessIfNeeded,
}

/// Wave 933: session-control authority payload (select/pause/start/reset/camera/world).
#[derive(Debug, Clone)]
pub enum SessionControlOp {
    SelectObjects {
        player_id: u32,
        ids: Vec<ObjectId>,
    },
    SetPaused {
        paused: bool,
    },
    SetCameraFollow {
        id: Option<ObjectId>,
    },
    StartNewGameWithFaction {
        mode: GameMode,
        player_id: u32,
        faction_team: Team,
        setup_skirmish_ai: bool,
    },
    Reset,
    OverrideWorldSize {
        width: f32,
        height: f32,
    },
}

/// Wave 934: host-support residual authority payload (barracks/supplies/shell/destroy/template).
#[derive(Debug, Clone)]
pub enum HostSupportOp {
    EnsureBarracksBuildingData {
        id: ObjectId,
    },
    ForceEnsureBarracksBuildingData {
        id: ObjectId,
    },
    EnsurePlayerMinSupplies {
        player_id: u32,
        floor: u32,
    },
    UpdateShellWithBudget {
        dt: f32,
        budget: usize,
    },
    ProcessDestroyListIfNeeded,
    InsertThingTemplate {
        name: String,
        template: ThingTemplate,
    },
}

/// Wave 934: heterogeneous result for [`HostSupportOp`].
#[derive(Debug, Clone, Copy)]
pub enum HostSupportResult {
    Bool(bool),
    Snapshot(SimTimingSnapshot),
    Unit,
}

/// Wave 937: production complete/spawn authority payload.
#[derive(Debug, Clone)]
pub enum ProductionAuthorityOp {
    /// Sole-tick path: collect ready producers after GW writeback, then apply.
    ApplyCompletionsAfterReadyWriteback { dt: f32 },
    /// Spawn one completed unit (ObjectId host bind residual).
    SpawnUnit {
        template: String,
        team: Team,
        spawn_pos: glam::Vec3,
    },
    /// Drain production spawn-ready log into host door/notify residuals.
    ApplySpawnReadyCompletions,
    /// Drain production door-ready log into host door model residuals.
    ApplyDoorReadyCompletions,
}

/// Wave 937: result for [`ProductionAuthorityOp`].
#[derive(Debug, Clone, Copy)]
pub enum ProductionAuthorityResult {
    Unit,
    Spawned(Option<ObjectId>),
}

/// Wave 938: post-writeback sole-tick complete authority (construction/sell/SP).
#[derive(Debug, Clone, Copy)]
pub enum PostWritebackCompleteOp {
    /// Wave 715: construction complete after GW construction writeback.
    ConstructionCompletionsAfterReadyWriteback,
    /// Wave 716: sell finish after GW construction/sell writeback.
    SellCompletionsAfterReadyWriteback,
    /// Wave 717: special-power ready EVA after GW SP writeback.
    SpecialPowerReadyAfterWriteback,
}

/// Wave 939: post-writeback ready-log drain authority payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyLogDrainOp {
    Contain,
    Projectiles,
    AttackTarget,
    AiState,
    Movement,
    FireIntent,
    MoveTarget,
    Transform,
    Locomotor,
    AiRequest,
    Hijacker,
    PhysicsMotive,
    BounceLand,
    CombatStatus,
    BodyDamage,
    DeathType,
    RadarExtend,
    ShockStun,
    ConstructionCompleteClear,
    SoleHealing,
    AiMood,
    Owner,
    Veterancy,
    WeaponBonus,
    FaerieFire,
    Repulsor,
    DisableTimers,
    WeaponSlot,
    EntityPower,
    Turret,
    StealthDelay,
    CombatAttack,
    TargetLocation,
    Detector,
    ContinuousFire,
    Guard,
    AiAttitude,
    WeaponSet,
    Overcharge,
    Hive,
    StealthFlags,
    Overlord,
    CommandSet,
    Disguise,
    VisionCamo,
    WeaponStats,
    SelectionRadius,
    ModelCondition,
    DemoMineCheer,
    CrushVision,
    BuildingType,
    Identity,
    GroundHeight,
    Economy,
    Upgrade,
    StoredSupplies,
}

/// Wave 940: post-writeback sole-tick batch is a single authority call (no enum).
/// Host ObjectId mutations used by shadow residual drains.
#[derive(Debug, Clone)]
pub enum HostObjectIdOp {
    MarkForDestruction {
        id: ObjectId,
        team: Option<Team>,
    },
    Create {
        template: String,
        team: Team,
        spawn_at: glam::Vec3,
    },
}

/// Wave 940: result for [`HostObjectIdOp`].
#[derive(Debug, Clone, Copy)]
pub enum HostObjectIdResult {
    Unit,
    Created(Option<ObjectId>),
}

/// Wave 941/942: host residual mutation payload (poison/kill/pending fire/expire/field).
#[derive(Debug, Clone)]
pub enum HostResidualMutationOp {
    /// PoisonedBehavior DoT — UNRESISTABLE typed death.
    PoisonDot {
        object: ObjectId,
        amount: f32,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    },
    /// Force HP to 0 / destroyed (+ optional death_type / model refresh).
    ForceKill {
        id: ObjectId,
        death_type: Option<crate::game_logic::host_usa_pilot::HostDeathType>,
        refresh_model_condition: bool,
        mark_destroy: bool,
    },
    /// FireWeaponWhenDamaged pending weapon residual.
    SetPendingFireWhenDamaged {
        id: ObjectId,
        weapon: String,
        /// When false, only set if pending is currently None (continuous).
        overwrite: bool,
    },
    /// Projectile/field expire: optional damage-log lethal, flags, position, mark-destroy.
    LethalExpire {
        id: ObjectId,
        position: Option<glam::Vec3>,
        effectively_dead: bool,
        clear: ObjectIdentityClear,
        /// None => do not mark; Some(team_opt) => MarkForDestruction with team.
        mark_destroy_team: Option<Option<Team>>,
    },
    /// Bomb/mine payload residual destroy (no damage-log path).
    DestroyBomb { id: ObjectId, mark_destroy: bool },
    /// Model condition bits residual (actively constructing).
    SetModelConditionBits {
        id: ObjectId,
        bits: u128,
        count_update: bool,
    },
    /// Power plant control rods completion residual.
    PowerPlantRodsComplete {
        id: ObjectId,
        model_condition_bits: u128,
    },
    /// Horde weapon-bonus residual (Battlemaster / China infantry).
    SetWeaponBonusHorde {
        id: ObjectId,
        now_horde: bool,
        was_horde: bool,
        grant: HordeGrantCounter,
    },
    /// Stinger hive slave residual snapshot.
    ApplyStingerHiveState {
        id: ObjectId,
        hive_slave_count: u8,
        hive_slave_hp: f32,
        hive_slave_respawn_frame: u32,
        slaves_alive: [bool; 3],
        slaves_hp: [f32; 3],
    },
    /// Sticky/booby follow position residual.
    SetPosition {
        id: ObjectId,
        position: glam::Vec3,
        sticky_follow_tick: bool,
    },
    /// Post-create flight payload config (producer, identity flag, velocity, target).
    ConfigureSpawnedPayload {
        id: ObjectId,
        producer: ObjectId,
        target: glam::Vec3,
        kind: SpawnedPayloadKind,
    },
    /// Host-only raw HP damage for combat events with no shadow entity mapping.
    /// `amount` is already post-armor — do not re-run armor/log.
    ApplyRawHpDamage { id: ObjectId, amount: f32 },
}

/// Wave 942: which projectile/identity flag to clear on lethal expire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectIdentityClear {
    None,
    FlashbangGrenadeProjectile,
    ScorpionMissileProjectile,
    SpySatellitePing,
    AngryMobMember,
    AuroraBombProjectile,
    InfernoShellProjectile,
    ToxinStreamProjectile,
    AngryMobProjectile,
    /// Clears scud/neutron/nuke cannon shell projectile flags.
    CannonShellProjectile,
    LeafletContainer,
    ParadropCargo,
    ComancheRocketPodProjectile,
    EmpPulseSpheroid,
    FieldObject(crate::game_logic::host_field_object_expire_log::FieldObjectKind),
}

/// Wave 942: which residual counter to bump on horde grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HordeGrantCounter {
    #[default]
    None,
    Battlemaster,
    RedGuard,
    TankHunter,
    Minigunner,
}

/// Wave 942: post-create flight payload identity/config residual.
#[derive(Debug, Clone)]
pub enum SpawnedPayloadKind {
    DaisyCutter { moab_template: Option<String> },
    AnthraxBomb,
    ClusterMinesBomb,
    EmpPulseBomb,
    A10StrikeMissile,
    ArtilleryBarrageShell,
    CarpetBomb,
    LeafletContainer,
    ParadropParachute,
}

/// Wave 944: shadow→host writeback payload (core combat/pose channels).
#[derive(Debug, Clone)]
pub enum HostWritebackOp {
    /// Health/max + optional destroy bookkeeping fields.
    Health {
        id: ObjectId,
        current: f32,
        maximum: f32,
        /// When true, mark destroyed + clear AI target.
        destroy: bool,
    },
    /// Experience points and/or veterancy level.
    Experience {
        id: ObjectId,
        points: Option<f32>,
        level: Option<crate::game_logic::VeterancyLevel>,
    },
    /// World pose last-write.
    Transform {
        id: ObjectId,
        position: glam::Vec3,
        orientation: f32,
    },
    /// Attack target last-write (direct assign; ready-log handles residuals).
    AttackTarget {
        id: ObjectId,
        target: Option<ObjectId>,
        clear_target_location: bool,
    },
    /// Move destination last-write.
    MoveTarget {
        id: ObjectId,
        destination: Option<glam::Vec3>,
    },
    /// Wave 945: AI state ordinal last-write.
    AiState { id: ObjectId, ordinal: u8 },
    /// Wave 945: AI attitude last-write.
    AiAttitude { id: ObjectId, attitude: i8 },
    /// Wave 945: owner team last-write.
    Owner {
        id: ObjectId,
        team: Team,
        team_color: [f32; 4],
    },
    /// Wave 945: special-power ready/cooldown last-write.
    SpecialPower {
        id: ObjectId,
        ready: bool,
        cooldown_remaining: f32,
        cooldown: f32,
    },
    /// Wave 945: overcharge flag last-write.
    Overcharge { id: ObjectId, enabled: bool },
    /// Wave 945: active weapon slot last-write.
    WeaponSlot { id: ObjectId, slot: u8 },
    /// Wave 945: selection radius last-write.
    SelectionRadius { id: ObjectId, radius: f32 },
    /// Wave 945: entity power provided/consumed last-write.
    EntityPower {
        id: ObjectId,
        provided: i32,
        consumed: i32,
    },
    /// Wave 945: target location last-write.
    TargetLocation {
        id: ObjectId,
        location: Option<glam::Vec3>,
    },
    /// Wave 945: command-set override last-write.
    CommandSet {
        id: ObjectId,
        override_name: Option<String>,
    },
    /// Wave 945: ground height last-write.
    GroundHeight {
        id: ObjectId,
        height: f32,
        from_terrain: bool,
    },
    /// Wave 945: body damage state last-write.
    BodyDamage {
        id: ObjectId,
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    },
    /// Wave 945: death type last-write.
    DeathType {
        id: ObjectId,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    },
    /// Wave 945: stored supplies last-write.
    StoredSupplies { id: ObjectId, supplies: u32 },
    /// Wave 945: faerie-fire status last-write.
    FaerieFire {
        id: ObjectId,
        active: bool,
        until_frame: u32,
    },
    /// Wave 945: repulsor status last-write.
    Repulsor {
        id: ObjectId,
        active: bool,
        until_frame: u32,
    },
    /// Wave 945: detector residual last-write.
    Detector {
        id: ObjectId,
        is_detector: bool,
        range: f32,
        rate_frames: u32,
    },
    /// Wave 945: guard residual last-write.
    Guard {
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        radius: f32,
    },
}

impl GameLogic {
    fn script_engine_handle(&self) -> Option<Arc<ScriptingEngine>> {
        self.script_engine.as_ref().map(Arc::clone)
    }

    fn forward_event_to_scripts(&self, event: &ScriptEvent) {
        let engine = match self.script_engine_handle() {
            Some(engine) => engine,
            None => return,
        };

        let mission_event = match self.convert_script_event(event) {
            Some(evt) => evt,
            None => return,
        };

        if let Err(err) = engine.fire_event_sync(mission_event) {
            log::error!("Scripting engine failed to accept event: {}", err);
        }
    }

    pub fn new() -> Self {
        log::debug!("GameLogic::new() - creating new GameLogic instance");
        let world_width = 512.0;
        let world_height = 512.0;
        let world_min = Vec3::new(-world_width * 0.5, 0.0, -world_height * 0.5);
        let world_max = Vec3::new(world_width * 0.5, 0.0, world_height * 0.5);

        let mission_hooks = MissionScriptHooks::new().expect("Mission script runtime init failed");

        let mut instance = Self {
            attack_priority_sets: std::collections::HashMap::new(),
            enable_repulsors: false,
            retaliate_friends_radius: 120.0,
            max_retaliate_distance: 210.0,
            objects: HostObjectStore::new(),
            host_view_dirty: HashSet::new(),
            players: HashMap::new(),
            next_object_id: ObjectId(1), // Start at 1, 0 is invalid
            next_formation_id: 1,
            frame: 0,
            game_mode: GameMode::None,
            skirmish_rules: SkirmishRulesState::default(),
            world_width,
            world_height,
            world_min,
            world_max,
            victory_conditions: VictoryConditions::new(),
            objects_to_destroy: VecDeque::new(),
            combat_particles: CombatParticleRegistry::new(),
            special_power_strikes:
                crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry::new(),
            host_paradrops: crate::game_logic::host_paradrop::HostParadropRegistry::new(),
            host_ambushes: crate::game_logic::host_ambush::HostAmbushRegistry::new(),
            last_cash_hack_request_amount: 0,
            last_cash_hack_stolen_amount: 0,
            last_crate_drop_spawned: 0,
            host_leaflet_drops: crate::game_logic::host_leaflet_drop::HostLeafletDropRegistry::new(
            ),
            host_sneak_attacks: crate::game_logic::host_sneak_attack::HostSneakAttackRegistry::new(
            ),
            host_upgrades: crate::game_logic::host_upgrades::HostUpgradeRegistry::new(),
            supply_lines_bonus_cash_total: 0,
            cash_bounty: crate::game_logic::host_cash_bounty::HostCashBountyRegistry::new(),
            garrison_residual_enters: 0,
            garrison_residual_exits: 0,
            garrison_residual_fires: 0,
            transport_residual_loads: 0,
            transport_residual_unloads: 0,
            overlord_bunker_residual_enters: 0,
            overlord_bunker_residual_exits: 0,
            battle_bus: crate::game_logic::host_battle_bus::HostBattleBusRegistry::new(),
            highlander_body_reg:
                crate::game_logic::host_highlander_body::HostHighlanderBodyRegistry::new(),
            deploy_style_reg: crate::game_logic::host_deploy_style::HostDeployStyleRegistry::new(),
            tensile_formation_reg: crate::game_logic::host_tensile_formation::HostTensileFormationRegistry::new(),
            status_bits_upgrade_reg: crate::game_logic::host_status_bits_upgrade::HostStatusBitsUpgradeRegistry::new(),
            fire_spread_reg: crate::game_logic::host_fire_spread::HostFireSpreadRegistry::new(),
            base_regenerate_reg: crate::game_logic::host_base_regenerate::HostBaseRegenerateRegistry::new(),
            enemy_near_reg: crate::game_logic::host_enemy_near::HostEnemyNearRegistry::new(),
            passengers_fire_upgrade_reg: crate::game_logic::host_passengers_fire_upgrade::HostPassengersFireUpgradeRegistry::new(),
            animation_steering_reg: crate::game_logic::host_animation_steering::HostAnimationSteeringRegistry::new(),
            active_shroud_upgrade_reg: crate::game_logic::host_active_shroud_upgrade::HostActiveShroudUpgradeRegistry::new(),
            float_update_reg: crate::game_logic::host_float_update::HostFloatUpdateRegistry::new(),
            prone_update_reg: crate::game_logic::host_prone_update::HostProneUpdateRegistry::new(),
            radius_decal_update_reg: crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateRegistry::new(),
            checkpoint_update_reg: crate::game_logic::host_checkpoint_update::HostCheckpointUpdateRegistry::new(),
            spectre_gunship_deployment_reg: crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentRegistry::new(),
            smart_bomb_target_homing_reg: crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingRegistry::new(),
            ocl_special_power_reg: crate::game_logic::host_ocl_special_power::HostOclSpecialPowerRegistry::new(),
            ocl_create_debris_reg: crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisRegistry::new(),
            ocl_fire_weapon_attack_reg:
                crate::game_logic::host_ocl_fire_weapon_attack::HostOclFireWeaponAttackRegistry::new(),
            fuel_air_gas_reg: crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasRegistry::new(),
            ocl_apply_random_force_reg:
                crate::game_logic::host_ocl_apply_random_force::HostOclApplyRandomForceRegistry::new(),
            neutron_missile_update_reg:
                crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateRegistry::new(),
            scud_storm_missile_flight_reg:
                crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightRegistry::new(),
            carpet_bomb_flight_reg:
                crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightRegistry::new(),
            artillery_barrage_flight_reg:
                crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightRegistry::new(),
            a10_strike_flight_reg:
                crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightRegistry::new(),
            daisy_cutter_flight_reg:
                crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightRegistry::new(),
            anthrax_bomb_flight_reg:
                crate::game_logic::host_anthrax_bomb_flight::HostAnthraxBombFlightRegistry::new(),
            cluster_mines_flight_reg:
                crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightRegistry::new(),
            emp_pulse_flight_reg:
                crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightRegistry::new(),
            command_button_hunt_reg: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntRegistry::new(),
            preorder_create_reg: crate::game_logic::host_preorder_create::HostPreorderCreateRegistry::new(),
            upgrade_die_reg: crate::game_logic::host_upgrade_die::HostUpgradeDieRegistry::new(),
            tunnel_network: crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry::new(
            ),
            combat_chinook: crate::game_logic::host_combat_chinook::HostCombatChinookRegistry::new(
            ),
            listening_outpost:
                crate::game_logic::host_listening_outpost::HostListeningOutpostRegistry::new(),
            troop_crawler: crate::game_logic::host_troop_crawler::HostTroopCrawlerRegistry::new(),
            mine_residual_places: 0,
            mine_residual_proximity_detonations: 0,
            mine_residual_timed_detonations: 0,
            mine_residual_manual_detonations: 0,
            mine_residual_clears: 0,
            repair_residual_structure_commands: 0,
            repair_residual_structure_heals: 0,
            repair_residual_vehicle_heals: 0,
            heal_residual_ambulance_heals: 0,
            heal_residual_heal_pad_heals: 0,
            propaganda_residual_heals: 0,
            propaganda_residual_buffs: 0,
            ecm_residual_jams: 0,
            microwaves: crate::game_logic::host_microwave::HostMicrowaveRegistry::new(),
            runway_reservations: std::collections::HashMap::new(),
            emp_pulses: crate::game_logic::host_emp_pulse::HostEmpPulseRegistry::new(),
            baikonur_launches:
                crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry::new(),
            defector_special: crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry::new(),
            upgrade_module_residuals: crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog::default(),
            replace_grant_command_upgrades: crate::game_logic::host_replace_object_upgrade::HostReplaceGrantCommandUpgradeLog::default(),
            sub_objects_upgrades: crate::game_logic::host_sub_objects_upgrade::HostSubObjectsUpgradeLog::default(),
            frenzies: crate::game_logic::host_frenzy::HostFrenzyRegistry::new(),
            battle_plans: crate::game_logic::host_strategy_center::HostBattlePlanRegistry::new(),
            strategy_center_gun_scatter_applied: 0,
            strategy_center_gun_scatter_misses: 0,
            emergency_repairs:
                crate::game_logic::host_emergency_repair::HostEmergencyRepairRegistry::new(),
            cleanup_areas: crate::game_logic::host_cleanup_area::HostCleanupAreaRegistry::new(),
            gps_scramblers: crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry::new(),
            base_defense_residual_fires: 0,
            point_defense_residual_intercepts: 0,
            ecm_missiles_jammed: 0,
            ecm_laser_beams_spawned: 0,
            point_defense_laser_beams_spawned: 0,
            point_defense_next_ready_frame: HashMap::new(),
            avenger: crate::game_logic::host_avenger::HostAvengerRegistry::new(),
            neutron_shell_residual_blasts: 0,
            neutron_shell_residual_infantry_kills: 0,
            neutron_shell_residual_vehicles_unmanned: 0,
            bunker_buster: crate::game_logic::host_bunker_buster::HostBunkerBusterRegistry::new(),
            comanche_rocket_pod_residual_area_attacks: 0,
            comanche_rocket_pod_residual_units_hit: 0,
            comanche_rocket_pod_shot_index: std::collections::HashMap::new(),
            comanche_rocket_pod_projectiles_spawned: 0,
            sentry_drone_residual_auto_fires: 0,
            sentry_drone_residual_detects: 0,
            pathfinder_residual_detects: 0,
            pathfinder_residual_sniper_fires: 0,
            scout_drone_residual_detects: 0,
            scout_drone_residual_attaches: 0,
            hellfire_drone_residual_auto_fires: 0,
            hellfire_drone_residual_attaches: 0,
            hellfire_scatter_applied: 0,
            hellfire_scatter_misses: 0,
            radar_scans: crate::game_logic::host_radar_scan::HostRadarScanRegistry::new(),
            spy_satellites: crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry::new(),
            spy_drones: crate::game_logic::host_spy_drone::HostSpyDroneRegistry::new(),
            countermeasures:
                crate::game_logic::host_countermeasures::HostCountermeasuresRegistry::new(),
            cia_intelligence:
                crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry::new(),
            hero_abilities: crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry::new(),
            black_markets: crate::game_logic::host_black_market::HostBlackMarketRegistry::new(),
            oil_derricks: crate::game_logic::host_oil_derrick::HostOilDerrickRegistry::new(),
            hacker_income: crate::game_logic::host_hacker_income::HostHackerIncomeRegistry::new(),
            supply_drop_zones:
                crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry::new(),
            host_deliver_payloads:
                crate::game_logic::host_deliver_payload::HostDeliverPayloadRegistry::new(),
            host_money_crates: crate::game_logic::host_money_crate::HostMoneyCrateRegistry::new(),
            host_radar: crate::game_logic::host_radar::HostRadarRegistry::new(),
            car_bomb: crate::game_logic::host_car_bomb::HostCarBombRegistry::new(),
            saboteur: crate::game_logic::host_saboteur::HostSaboteurRegistry::new(),
            usa_pilot: crate::game_logic::host_usa_pilot::HostUsaPilotRegistry::new(),
            gla_worker: crate::game_logic::host_gla_worker::HostGlaWorkerRegistry::new(),
            bomb_truck_disguise:
                crate::game_logic::host_bomb_truck_disguise::HostBombTruckDisguiseRegistry::new(),
            bomb_truck_detonate:
                crate::game_logic::host_bomb_truck_detonate::HostBombTruckDetonateRegistry::new(),
            nuclear_tanks: crate::game_logic::host_nuclear_tanks::HostNuclearTanksRegistry::new(),
            booby_trap: crate::game_logic::host_booby_trap::HostBoobyTrapRegistry::new(),
            booby_trap_objects_spawned: 0,
            helix_napalm: crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry::new(),
            fire_walls: crate::game_logic::host_firewall::HostFireWallRegistry::new(),
            inferno_fire_zones:
                crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry::new(),
            aurora_bombs: crate::game_logic::host_aurora_bomb::HostAuroraBombRegistry::new(),
            aurora_fuel_air_gas_spawned: 0,
            angry_mobs: crate::game_logic::host_angry_mob::HostAngryMobRegistry::new(),
            stealth_fighter_science:
                crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry::new(),
            unit_training: crate::game_logic::host_unit_training::HostUnitTrainingRegistry::new(),
            demo_suicide_bomb:
                crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry::new(),
            rocket_buggy_residual_fires: 0,
            rocket_buggy_residual_units_hit: 0,
            rocket_buggy_residual_scatter_misses: 0,
            quad_cannon_residual_ground_fires: 0,
            quad_cannon_residual_aa_fires: 0,
            quad_cannon_residual_barrel_upgrades: 0,
            scud_poison_zones: crate::game_logic::host_scud_launcher::HostScudPoisonRegistry::new(),
            overlord_addons:
                crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry::new(),
            nuke_cannon_residual: crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry::new(
            ),
            technical_residual_fires: 0,
            technical_residual_units_hit: 0,
            technical_residual_weapon_upgrades: 0,
            technical_residual_loads: 0,
            technical_residual_unloads: 0,
            toxin_tractor: crate::game_logic::host_toxin_tractor::HostToxinTractorRegistry::new(),
            marauder_residual_fires: 0,
            marauder_residual_units_hit: 0,
            marauder_residual_weapon_upgrades: 0,
            scorpion_residual_fires: 0,
            scorpion_residual_units_hit: 0,
            scorpion_residual_rocket_upgrades: 0,
            scorpion_residual_salvage_upgrades: 0,
            scorpion_residual_missile_fires: 0,
            tomahawk_residual_fires: 0,
            tomahawk_residual_units_hit: 0,
            raptor_residual_fires: 0,
            raptor_residual_units_hit: 0,
            raptor_residual_laser_missiles_upgrades: 0,
            mig_residual_fires: 0,
            mig_residual_units_hit: 0,
            mig_residual_black_napalm_upgrades: 0,
            mig_residual_tactical_nuke_upgrades: 0,
            mig_residual_fire_fields: 0,
            mig_residual_radiation_fields: 0,
            mig_scatter_applied: 0,
            mig_scatter_misses: 0,
            fire_base_residual_fires: 0,
            fire_base_residual_units_hit: 0,
            stealth_fighter_residual_fires: 0,
            stealth_fighter_residual_units_hit: 0,
            stealth_jet_missiles_spawned: 0,
            stealth_jet_scatter_applied: 0,
            stealth_jet_scatter_misses: 0,
            scud_missiles_spawned: 0,
            tomahawk_missiles_spawned: 0,
            tomahawk_scatter_applied: 0,
            tomahawk_scatter_misses: 0,
            rocket_buggy_missiles_spawned: 0,
            rocket_buggy_scatter_applied: 0,
            scud_launcher_scatter_applied: 0,
            scud_launcher_scatter_misses: 0,
            neutron_shells_spawned: 0,
            neutron_shell_scatter_applied: 0,
            neutron_shell_scatter_misses: 0,
            rpg_trooper_missiles_spawned: 0,
            rpg_trooper_scatter_applied: 0,
            rpg_trooper_scatter_misses: 0,
            tank_hunter_missiles_spawned: 0,
            tank_hunter_scatter_applied: 0,
            tank_hunter_scatter_misses: 0,
            missile_defender_missiles_spawned: 0,
            missile_defender_scatter_applied: 0,
            missile_defender_scatter_misses: 0,
            scorpion_shells_spawned: 0,
            scorpion_scatter_applied: 0,
            scorpion_scatter_misses: 0,
            scorpion_missiles_spawned: 0,
            nuke_cannon_shells_spawned: 0,
            nuke_cannon_scatter_applied: 0,
            nuke_cannon_scatter_misses: 0,
            usa_tank_shells_spawned: 0,
            usa_tank_scatter_applied: 0,
            usa_tank_scatter_misses: 0,
            battlemaster_shells_spawned: 0,
            battlemaster_scatter_applied: 0,
            battlemaster_scatter_misses: 0,
            overlord_shells_spawned: 0,
            overlord_scatter_applied: 0,
            overlord_scatter_misses: 0,
            inferno_shells_spawned: 0,
            inferno_scatter_applied: 0,
            inferno_scatter_misses: 0,
            marauder_shells_spawned: 0,
            marauder_scatter_applied: 0,
            marauder_scatter_misses: 0,
            fire_base_shells_spawned: 0,
            fire_base_scatter_applied: 0,
            fire_base_scatter_misses: 0,
            raptor_missiles_spawned: 0,
            raptor_scatter_applied: 0,
            raptor_scatter_misses: 0,
            mig_missiles_spawned: 0,
            flashbang_grenades_spawned: 0,
            flashbang_scatter_applied: 0,
            flashbang_scatter_misses: 0,
            humvee_tow_missiles_spawned: 0,
            humvee_tow_scatter_applied: 0,
            humvee_tow_scatter_misses: 0,
            humvee_tow_residual_fires: 0,
            dragon_flame_missiles_spawned: 0,
            toxin_stream_missiles_spawned: 0,
            technical_rpg_missiles_spawned: 0,
            technical_cannon_shells_spawned: 0,
            technical_cannon_scatter_applied: 0,
            technical_cannon_scatter_misses: 0,
            cleanup_stream_missiles_spawned: 0,
            angry_mob_projectiles_spawned: 0,
            usa_tank_residual_units_hit: 0,
            comanche_cannon_residual_fires: 0,
            comanche_cannon_residual_units_hit: 0,
            comanche_antitank_residual_fires: 0,
            comanche_antitank_residual_units_hit: 0,
            comanche_at_scatter_applied: 0,
            comanche_at_scatter_misses: 0,
            helix_minigun_residual_fires: 0,
            helix_minigun_residual_units_hit: 0,
            inferno_black_napalm_residual_upgrades: 0,
            inferno_black_napalm_residual_zones: 0,
            battle_drone_residual_attaches: 0,
            battle_drone_residual_fires: 0,
            battle_drone_residual_units_hit: 0,
            battle_drone_residual_repairs: 0,
            battle_drone_residual_repair_amount: 0.0,
            overlord_gun_residual_fires: 0,
            overlord_gun_residual_units_hit: 0,
            overlord_gun_residual_uranium_upgrades: 0,
            jarmen_kell_residual_fires: 0,
            jarmen_kell_residual_units_hit: 0,
            jarmen_kell_residual_ap_upgrades: 0,
            battlemaster_residual_fires: 0,
            battlemaster_residual_units_hit: 0,
            battlemaster_residual_uranium_upgrades: 0,
            battlemaster_residual_nationalism_upgrades: 0,
            battlemaster_residual_horde_grants: 0,
            red_guard_residual_fires: 0,
            red_guard_residual_bayonet_kills: 0,
            red_guard_residual_nationalism_upgrades: 0,
            red_guard_residual_horde_grants: 0,
            tank_hunter_residual_fires: 0,
            tank_hunter_residual_units_hit: 0,
            tank_hunter_residual_tnt_plants: 0,
            tank_hunter_residual_nationalism_upgrades: 0,
            tank_hunter_residual_horde_grants: 0,
            tank_hunter_tnt_last_frame: HashMap::new(),
            rebel_residual_fires: 0,
            rebel_residual_ap_upgrades: 0,
            ranger_residual_rifle_fires: 0,
            ranger_residual_flashbang_fires: 0,
            ranger_residual_units_hit: 0,
            hacker_disable_building_count: 0,
            minigunner_residual_ground_fires: 0,
            minigunner_residual_aa_fires: 0,
            minigunner_residual_ramp_mean: 0,
            minigunner_residual_ramp_fast: 0,
            minigunner_residual_chain_gun_upgrades: 0,
            minigunner_residual_nationalism_upgrades: 0,
            minigunner_residual_horde_grants: 0,
            burton_residual_sniper_fires: 0,
            burton_residual_knife_kills: 0,
            rpg_trooper_residual_fires: 0,
            rpg_trooper_residual_units_hit: 0,
            rpg_trooper_residual_ap_upgrades: 0,
            terrorist_residual_detonations: 0,
            terrorist_residual_units_hit: 0,
            terrorist_residual_damage_dealt: 0.0,
            missile_defender_residual_fires: 0,
            missile_defender_residual_units_hit: 0,
            missile_defender_residual_laser_specials: 0,
            missile_defender_residual_laser_fires: 0,
            missile_defender_laser_beams_spawned: 0,
            combat_cycle_residual_fires: 0,
            combat_cycle_residual_units_hit: 0,
            combat_cycle_residual_rider_switches: 0,
            combat_cycle_residual_loads: 0,
            combat_cycle_residual_suicides: 0,
            dragon_tank_residual_fires: 0,
            dragon_tank_residual_units_hit: 0,
            dragon_tank_residual_black_napalm_upgrades: 0,
            gattling_tank_residual_ground_fires: 0,
            gattling_tank_residual_aa_fires: 0,
            gattling_tank_residual_ramp_mean: 0,
            gattling_tank_residual_ramp_fast: 0,
            gattling_tank_residual_chain_gun_upgrades: 0,
            gattling_building_residual_ground_fires: 0,
            gattling_building_residual_aa_fires: 0,
            gattling_building_residual_ramp_mean: 0,
            gattling_building_residual_ramp_fast: 0,
            gattling_building_residual_chain_gun_upgrades: 0,
            stinger_site_residual_ground_fires: 0,
            stinger_site_residual_aa_fires: 0,
            stinger_site_residual_ap_rockets_upgrades: 0,
            stinger_hive_residual_slave_hits: 0,
            stinger_hive_residual_slave_kills: 0,
            stinger_hive_residual_swallows: 0,
            stinger_hive_residual_respawns: 0,
            stinger_hive_residual_closest_slave_hits: 0,
            camo_netting_heat_vision_count: 0,
            camo_netting_structure_residual_reveals: 0,
            camo_netting_order_idle_enemies_count: 0,
            camo_netting_structure_residual_recloaks: 0,
            camo_netting_opacity_cloak_count: 0,
            camo_netting_opacity_reveal_count: 0,
            camo_netting_sub_object_show_count: 0,
            stinger_slave_order_attack_count: 0,
            patriot_residual_ground_fires: 0,
            patriot_residual_aa_fires: 0,
            patriot_scatter_applied: 0,
            patriot_scatter_misses: 0,
            stinger_scatter_applied: 0,
            stinger_scatter_misses: 0,
            supw_patriot_emp_residual_grants: 0,
            supw_emp_scatter_applied: 0,
            supw_emp_scatter_misses: 0,
            patriot_assist_residual_requests: 0,
            patriot_assist_residual_fires: 0,
            patriot_assist_residual_accepts: 0,
            patriot_assist_laser_from_assisted: 0,
            patriot_assist_laser_to_target: 0,
            patriot_assist_lasers: Vec::new(),
            weapon_lasers: Vec::new(),
            weapon_laser_beams_spawned: 0,
            projectile_streams:
                crate::game_logic::host_projectile_stream::ProjectileStreamRegistry::new(),
            pending_patriot_assists: Vec::new(),
            stealth_detector_rate_scans: 0,
            is_paused: false,
            sim_time_seconds: 0.0,
            accumulated_time: 0.0,
            last_fixed_step_diagnostics: FixedStepDiagnostics::default(),
            templates: HashMap::new(),
            map_name: String::new(),
            map_loaded: false,
            combat_system: CombatSystem::new(),
            pathfinding_system: PathfindingSystem::new_with_origin(
                world_min,
                world_width,
                world_height,
            ),
            ai_manager: AIManager::new(),
            scripts_loaded: false,
            mission_script_counter: 0,
            queued_audio_events: Vec::new(),
            command_queue: VecDeque::new(),
            accepted_gather_commands: VecDeque::new(),
            supply_dropoff_events: VecDeque::new(),
            pending_special_abilities: HashMap::new(),
            selected_objects: Vec::new(),
            partition_manager: PartitionManager::new(),
            radar_notifications: radar_notifications::global_radar_notifications(),
            last_radar_kind_time: [-10.0; 3],
            last_radar_audio_time: -10.0,
            last_radar_event: None,
            under_attack_event_history: Vec::new(),
            under_attack_events: 0,
            eva_base_under_attack: 0,
            eva_ally_under_attack: 0,
            eva_low_power: 0,
            eva_low_power_next_frame: 0,
            eva_low_power_active: false,
            eva_insufficient_funds: 0,
            eva_insufficient_funds_next_frame: 0,
            eva_upgrade_complete: 0,
            eva_general_level_up: 0,
            eva_superweapon_ready: 0,
            eva_superweapon_detected: 0,
            eva_superweapon_launched: 0,
            eva_beacon_detected: 0,
            eva_hero_detected: 0,
            eva_special_launched_misc: 0,
            radar_upgrade_events: 0,
            structure_complete_events: 0,
            unit_ready_events: 0,
            radar_extend_starts: 0,
            radar_extend_completes: 0,
            radar_construction_events: 0,
            production_door_cycles: 0,
            construction_model_condition_updates: 0,
            actively_constructing_updates: 0,
            sell_list: Vec::new(),
            sell_process_starts: 0,
            sell_process_finishes: 0,
            sell_owned_mines_destroyed: 0,
            sell_passengers_ejected: 0,
            sell_parked_units_killed: 0,
            sell_tunnel_last_ejects: 0,
            capture_kick_outs: 0,
            capture_ai_auto_sells: 0,
            capture_deselections: 0,
            capture_tunnel_transfers: 0,
            capture_tunnel_last_ejects: 0,
            capture_tech_model_updates: 0,
            unmanned_reclaims: 0,
            carbomb_unmanned_detonations: 0,
            overcharge_toggles: 0,
            overcharge_drain_ticks: 0,
            overcharge_exhaustions: 0,
            control_rods_upgrades: 0,
            control_rods_plants_affected: 0,
            subliminal_messaging_upgrades: 0,
            subliminal_towers_affected: 0,
            construction_complete_clears: 0,
            dozer_cancel_task_events: 0,
            resume_construction_events: 0,
            repair_complete_events: 0,
            sole_benefactor_repair_rejects: 0,
            dozer_bored_repair_events: 0,
            dozer_bored_mine_clear_events: 0,
            rebuild_hole_spawns: 0,
            supply_create_warehouse_registers: 0,
            supply_create_center_registers: 0,
            structure_minefield_placements: 0,
            special_power_completion_log: crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog::default(),
            sticky_bomb_follow_ticks: 0,
            sticky_bomb_target_deaths: 0,
            rebuild_hole_reconstructs: 0,
            rebuild_hole_workers: 0,
            rebuild_hole_heals: 0,
            rebuild_hole_completes: 0,
            rebuild_hole_attack_transfers: 0,
            rebuild_hole_bomb_transfers: 0,
            rebuild_hole_recon_deaths: 0,
            rebuild_hole_worker_restarts: 0,
            pending_camera_focus: None,
            script_camera_focus_estimate: Vec3::ZERO,
            script_camera_move_to: None,
            script_camera_path: None,
            camera_follow_target: None,
            script_default_camera_pitch: 1.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time_armed: false,
            script_camera_freeze_angle_armed: false,
            script_camera_pending_final_speed_multiplier: None,
            script_camera_pending_rolling_average_frames: None,
            visual_speed_multiplier: 1.0,
            script_time_frozen_by_script: false,
            pending_script_fps_limit: None,
            pending_camera_zoom_reset: false,
            pending_camera_zoom: None,
            pending_camera_pitch: None,
            pending_camera_rotate: None,
            pending_camera_look_toward: None,
            pending_camera_slave_mode_enable: None,
            pending_camera_slave_mode_disable: false,
            pending_screen_shakes: Vec::new(),
            pending_camera_add_shakers: Vec::new(),
            pending_popup_messages: Vec::new(),
            pending_view_guardband: None,
            pending_camera_bw_mode: None,
            pending_camera_motion_blur: Vec::new(),
            script_skybox_enabled: true,
            script_cameo_flash_count: HashMap::new(),
            script_named_timers: HashMap::new(),
            script_named_timer_display_shown: true,
            script_superweapon_display_enabled: true,
            script_superweapon_hidden_objects: HashSet::new(),
            host_beacons: Vec::new(),
            recent_beacons: Vec::new(),
            script_engine: None,
            script_event_pump_in_flight: Arc::new(AtomicBool::new(false)),
            script_event_pump_busy_frames: 0,
            loaded_script_lists: Vec::new(),
            script_source_path: None,
            mission_scripts: mission_hooks,
            script_broadcasts: Vec::new(),
            new_script_messages: Vec::new(),
            cinematic_letterbox: false,
            cinematic_text: None,
            military_caption: None,
            radar_enabled: true,
            radar_forced: false,
            pending_music_stop: false,
            pending_movie: None,
            pending_radar_movie: None,
            mission_objectives: Self::seed_sample_objectives(),
            objective_lookup: HashMap::new(),
            campaign_manager: global_campaign_manager().ok(),
            last_map_settings: None,
            spawned_map_object_ids: Vec::new(),
            terrain: None,
            runtime_road_segments: Vec::new(),
            runtime_terrain_texture_classes: Vec::new(),
            pathfinding_height_samples: None,
            weather_state: RuntimeWeatherState::default(),
        };
        instance.rebuild_objective_lookup();
        instance
    }

    /// World bounds used for minimap/FOW projections.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        (self.world_min, self.world_max)
    }

    pub fn fixed_step_diagnostics(&self) -> FixedStepDiagnostics {
        self.last_fixed_step_diagnostics
    }

    /// Override world dimensions when terrain provides authoritative size.
    pub fn override_world_size(&mut self, width: f32, height: f32) {
        self.world_width = width;
        self.world_height = height;
        self.world_min = Vec3::new(-width * 0.5, 0.0, -height * 0.5);
        self.world_max = Vec3::new(width * 0.5, 0.0, height * 0.5);
        self.pathfinding_system = PathfindingSystem::new_with_origin(self.world_min, width, height);
    }

    /// Reset method - matching C++ GameLogic interface
    pub fn reset(&mut self) {
        log::debug!("GameLogic::reset() - resetting game state");
        self.objects.clear();
        self.host_view_dirty.clear();
        self.players.clear();
        self.next_object_id = ObjectId(1);
        self.next_formation_id = 1;
        self.frame = 0;
        self.objects_to_destroy.clear();
        self.accepted_gather_commands.clear();
        self.supply_dropoff_events.clear();
        self.combat_particles.clear();
        self.special_power_strikes.clear();
        self.host_paradrops.clear();
        self.host_ambushes.clear();
        self.host_leaflet_drops.clear();
        self.host_sneak_attacks.clear();
        self.host_upgrades.clear();
        self.supply_lines_bonus_cash_total = 0;
        self.cash_bounty.clear();
        self.garrison_residual_enters = 0;
        self.garrison_residual_exits = 0;
        self.garrison_residual_fires = 0;
        self.transport_residual_loads = 0;
        self.transport_residual_unloads = 0;
        self.overlord_bunker_residual_enters = 0;
        self.overlord_bunker_residual_exits = 0;
        self.battle_bus.clear();
        self.highlander_body_reg.clear();
        self.deploy_style_reg.clear();
        self.tensile_formation_reg.clear();
        self.status_bits_upgrade_reg.clear();
        self.fire_spread_reg.clear();
        self.base_regenerate_reg.clear();
        self.enemy_near_reg.clear();
        self.passengers_fire_upgrade_reg.clear();
        self.animation_steering_reg.clear();
        self.active_shroud_upgrade_reg.clear();
        self.float_update_reg.clear();
        self.prone_update_reg.clear();
        self.radius_decal_update_reg.clear();
        self.checkpoint_update_reg.clear();
        self.spectre_gunship_deployment_reg.clear();
        self.smart_bomb_target_homing_reg.clear();
        self.ocl_special_power_reg.clear();
        self.ocl_create_debris_reg.clear();
        self.ocl_fire_weapon_attack_reg.clear();
        self.fuel_air_gas_reg.clear();
        self.ocl_apply_random_force_reg.clear();
        self.neutron_missile_update_reg.clear();
        self.scud_storm_missile_flight_reg.clear();
        self.carpet_bomb_flight_reg.clear();
        self.artillery_barrage_flight_reg.clear();
        self.a10_strike_flight_reg.clear();
        self.daisy_cutter_flight_reg.clear();
        self.anthrax_bomb_flight_reg.clear();
        self.cluster_mines_flight_reg.clear();
        self.emp_pulse_flight_reg.clear();
        self.command_button_hunt_reg.clear();
        self.preorder_create_reg.clear();
        self.upgrade_die_reg.clear();
        self.tunnel_network.clear();
        self.combat_chinook.clear();
        self.listening_outpost.clear();
        self.troop_crawler.clear();
        self.mine_residual_places = 0;
        self.mine_residual_proximity_detonations = 0;
        self.mine_residual_timed_detonations = 0;
        self.mine_residual_manual_detonations = 0;
        self.mine_residual_clears = 0;
        self.repair_residual_structure_commands = 0;
        self.repair_residual_structure_heals = 0;
        self.repair_residual_vehicle_heals = 0;
        self.heal_residual_ambulance_heals = 0;
        self.heal_residual_heal_pad_heals = 0;
        self.propaganda_residual_heals = 0;
        self.propaganda_residual_buffs = 0;
        self.ecm_residual_jams = 0;
        self.microwaves.clear();
        self.runway_reservations.clear();
        self.emp_pulses.clear();
        self.baikonur_launches =
            crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry::new();
        self.defector_special =
            crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry::new();
        self.upgrade_module_residuals =
            crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog::default(
            );
        self.frenzies.clear();
        self.cleanup_areas.clear();
        self.base_defense_residual_fires = 0;
        self.point_defense_residual_intercepts = 0;
        self.ecm_missiles_jammed = 0;
        self.ecm_laser_beams_spawned = 0;
        self.point_defense_laser_beams_spawned = 0;
        self.point_defense_next_ready_frame.clear();
        self.avenger.clear();
        self.neutron_shell_residual_blasts = 0;
        self.neutron_shell_residual_infantry_kills = 0;
        self.neutron_shell_residual_vehicles_unmanned = 0;
        self.bunker_buster.clear();
        self.comanche_rocket_pod_residual_area_attacks = 0;
        self.comanche_rocket_pod_residual_units_hit = 0;
        self.comanche_rocket_pod_shot_index.clear();
        self.comanche_rocket_pod_projectiles_spawned = 0;
        self.sentry_drone_residual_auto_fires = 0;
        self.sentry_drone_residual_detects = 0;
        self.pathfinder_residual_detects = 0;
        self.pathfinder_residual_sniper_fires = 0;
        self.scout_drone_residual_detects = 0;
        self.scout_drone_residual_attaches = 0;
        self.hellfire_drone_residual_auto_fires = 0;
        self.hellfire_drone_residual_attaches = 0;
        self.hellfire_scatter_applied = 0;
        self.hellfire_scatter_misses = 0;
        self.radar_scans.clear();
        self.spy_satellites.clear();
        self.spy_drones.clear();
        self.countermeasures.clear();
        self.hero_abilities.clear();
        self.black_markets.clear();
        self.oil_derricks.clear();
        self.hacker_income.clear();
        self.supply_drop_zones.clear();
        self.host_deliver_payloads.clear();
        self.host_money_crates.clear();
        self.host_radar.clear();
        self.car_bomb.clear();
        self.saboteur.clear();
        self.usa_pilot.clear();
        self.gla_worker.clear();
        self.bomb_truck_disguise.clear();
        self.bomb_truck_detonate.clear();
        self.nuclear_tanks.clear();
        self.booby_trap.clear();
        self.booby_trap_objects_spawned = 0;
        self.helix_napalm.clear();
        self.fire_walls.clear();
        self.inferno_fire_zones.clear();
        self.aurora_bombs.clear();
        self.aurora_fuel_air_gas_spawned = 0;
        self.angry_mobs.clear();
        self.stealth_fighter_science.clear();
        self.unit_training.clear();
        self.demo_suicide_bomb.clear();
        self.rocket_buggy_residual_fires = 0;
        self.rocket_buggy_residual_units_hit = 0;
        self.rocket_buggy_residual_scatter_misses = 0;
        self.quad_cannon_residual_ground_fires = 0;
        self.quad_cannon_residual_aa_fires = 0;
        self.quad_cannon_residual_barrel_upgrades = 0;
        self.scud_poison_zones.clear();
        self.overlord_addons.clear();
        self.nuke_cannon_residual.clear();
        self.technical_residual_fires = 0;
        self.technical_residual_units_hit = 0;
        self.technical_residual_weapon_upgrades = 0;
        self.technical_residual_loads = 0;
        self.technical_residual_unloads = 0;
        self.toxin_tractor.clear();
        self.marauder_residual_fires = 0;
        self.marauder_residual_units_hit = 0;
        self.marauder_residual_weapon_upgrades = 0;
        self.scorpion_residual_fires = 0;
        self.scorpion_residual_units_hit = 0;
        self.scorpion_residual_rocket_upgrades = 0;
        self.scorpion_residual_salvage_upgrades = 0;
        self.scorpion_residual_missile_fires = 0;
        self.tomahawk_residual_fires = 0;
        self.tomahawk_residual_units_hit = 0;
        self.raptor_residual_fires = 0;
        self.raptor_residual_units_hit = 0;
        self.raptor_residual_laser_missiles_upgrades = 0;
        self.mig_residual_fires = 0;
        self.mig_residual_units_hit = 0;
        self.mig_residual_black_napalm_upgrades = 0;
        self.mig_residual_tactical_nuke_upgrades = 0;
        self.mig_residual_fire_fields = 0;
        self.mig_residual_radiation_fields = 0;
        self.mig_scatter_applied = 0;
        self.mig_scatter_misses = 0;
        self.fire_base_residual_fires = 0;
        self.fire_base_residual_units_hit = 0;
        self.stealth_fighter_residual_fires = 0;
        self.stealth_fighter_residual_units_hit = 0;
        self.stealth_jet_missiles_spawned = 0;
        self.stealth_jet_scatter_applied = 0;
        self.stealth_jet_scatter_misses = 0;
        self.scud_missiles_spawned = 0;
        self.tomahawk_missiles_spawned = 0;
        self.tomahawk_scatter_applied = 0;
        self.tomahawk_scatter_misses = 0;
        self.rocket_buggy_missiles_spawned = 0;
        self.rocket_buggy_scatter_applied = 0;
        self.scud_launcher_scatter_applied = 0;
        self.scud_launcher_scatter_misses = 0;
        self.neutron_shells_spawned = 0;
        self.neutron_shell_scatter_applied = 0;
        self.neutron_shell_scatter_misses = 0;
        self.rpg_trooper_missiles_spawned = 0;
        self.rpg_trooper_scatter_applied = 0;
        self.rpg_trooper_scatter_misses = 0;
        self.tank_hunter_missiles_spawned = 0;
        self.tank_hunter_scatter_applied = 0;
        self.tank_hunter_scatter_misses = 0;
        self.missile_defender_missiles_spawned = 0;
        self.missile_defender_scatter_applied = 0;
        self.missile_defender_scatter_misses = 0;
        self.scorpion_shells_spawned = 0;
        self.scorpion_scatter_applied = 0;
        self.scorpion_scatter_misses = 0;
        self.scorpion_missiles_spawned = 0;
        self.nuke_cannon_shells_spawned = 0;
        self.nuke_cannon_scatter_applied = 0;
        self.nuke_cannon_scatter_misses = 0;
        self.usa_tank_shells_spawned = 0;
        self.usa_tank_scatter_applied = 0;
        self.usa_tank_scatter_misses = 0;
        self.battlemaster_shells_spawned = 0;
        self.battlemaster_scatter_applied = 0;
        self.battlemaster_scatter_misses = 0;
        self.overlord_shells_spawned = 0;
        self.overlord_scatter_applied = 0;
        self.overlord_scatter_misses = 0;
        self.inferno_shells_spawned = 0;
        self.inferno_scatter_applied = 0;
        self.inferno_scatter_misses = 0;
        self.marauder_shells_spawned = 0;
        self.marauder_scatter_applied = 0;
        self.marauder_scatter_misses = 0;
        self.fire_base_shells_spawned = 0;
        self.fire_base_scatter_applied = 0;
        self.fire_base_scatter_misses = 0;
        self.raptor_missiles_spawned = 0;
        self.raptor_scatter_applied = 0;
        self.raptor_scatter_misses = 0;
        self.mig_missiles_spawned = 0;
        self.flashbang_grenades_spawned = 0;
        self.flashbang_scatter_applied = 0;
        self.flashbang_scatter_misses = 0;
        self.usa_tank_residual_units_hit = 0;
        self.comanche_cannon_residual_fires = 0;
        self.comanche_cannon_residual_units_hit = 0;
        self.comanche_antitank_residual_fires = 0;
        self.comanche_antitank_residual_units_hit = 0;
        self.comanche_at_scatter_applied = 0;
        self.comanche_at_scatter_misses = 0;
        self.strategy_center_gun_scatter_applied = 0;
        self.strategy_center_gun_scatter_misses = 0;
        self.helix_minigun_residual_fires = 0;
        self.helix_minigun_residual_units_hit = 0;
        self.inferno_black_napalm_residual_upgrades = 0;
        self.inferno_black_napalm_residual_zones = 0;
        self.battle_drone_residual_attaches = 0;
        self.battle_drone_residual_fires = 0;
        self.battle_drone_residual_units_hit = 0;
        self.battle_drone_residual_repairs = 0;
        self.battle_drone_residual_repair_amount = 0.0;
        self.overlord_gun_residual_fires = 0;
        self.overlord_gun_residual_units_hit = 0;
        self.overlord_gun_residual_uranium_upgrades = 0;
        self.jarmen_kell_residual_fires = 0;
        self.jarmen_kell_residual_units_hit = 0;
        self.jarmen_kell_residual_ap_upgrades = 0;
        self.battlemaster_residual_fires = 0;
        self.battlemaster_residual_units_hit = 0;
        self.battlemaster_residual_uranium_upgrades = 0;
        self.battlemaster_residual_nationalism_upgrades = 0;
        self.battlemaster_residual_horde_grants = 0;
        self.red_guard_residual_fires = 0;
        self.red_guard_residual_bayonet_kills = 0;
        self.red_guard_residual_nationalism_upgrades = 0;
        self.red_guard_residual_horde_grants = 0;
        self.tank_hunter_residual_fires = 0;
        self.tank_hunter_residual_units_hit = 0;
        self.tank_hunter_residual_tnt_plants = 0;
        self.tank_hunter_residual_nationalism_upgrades = 0;
        self.tank_hunter_residual_horde_grants = 0;
        self.tank_hunter_tnt_last_frame.clear();
        self.rebel_residual_fires = 0;
        self.rebel_residual_ap_upgrades = 0;
        self.ranger_residual_rifle_fires = 0;
        self.ranger_residual_flashbang_fires = 0;
        self.ranger_residual_units_hit = 0;
        self.hacker_disable_building_count = 0;
        self.minigunner_residual_ground_fires = 0;
        self.minigunner_residual_aa_fires = 0;
        self.minigunner_residual_ramp_mean = 0;
        self.minigunner_residual_ramp_fast = 0;
        self.minigunner_residual_chain_gun_upgrades = 0;
        self.minigunner_residual_nationalism_upgrades = 0;
        self.minigunner_residual_horde_grants = 0;
        self.burton_residual_sniper_fires = 0;
        self.burton_residual_knife_kills = 0;
        self.rpg_trooper_residual_fires = 0;
        self.rpg_trooper_residual_units_hit = 0;
        self.rpg_trooper_residual_ap_upgrades = 0;
        self.terrorist_residual_detonations = 0;
        self.terrorist_residual_units_hit = 0;
        self.terrorist_residual_damage_dealt = 0.0;
        self.missile_defender_residual_fires = 0;
        self.missile_defender_residual_units_hit = 0;
        self.missile_defender_residual_laser_specials = 0;
        self.missile_defender_residual_laser_fires = 0;
        self.missile_defender_laser_beams_spawned = 0;
        self.combat_cycle_residual_fires = 0;
        self.combat_cycle_residual_units_hit = 0;
        self.combat_cycle_residual_rider_switches = 0;
        self.combat_cycle_residual_loads = 0;
        self.combat_cycle_residual_suicides = 0;
        self.dragon_tank_residual_fires = 0;
        self.dragon_tank_residual_units_hit = 0;
        self.dragon_tank_residual_black_napalm_upgrades = 0;
        self.gattling_tank_residual_ground_fires = 0;
        self.gattling_tank_residual_aa_fires = 0;
        self.gattling_tank_residual_ramp_mean = 0;
        self.gattling_tank_residual_ramp_fast = 0;
        self.gattling_tank_residual_chain_gun_upgrades = 0;
        self.gattling_building_residual_ground_fires = 0;
        self.gattling_building_residual_aa_fires = 0;
        self.gattling_building_residual_ramp_mean = 0;
        self.gattling_building_residual_ramp_fast = 0;
        self.gattling_building_residual_chain_gun_upgrades = 0;
        self.stinger_site_residual_ground_fires = 0;
        self.stinger_site_residual_aa_fires = 0;
        self.stinger_site_residual_ap_rockets_upgrades = 0;
        self.stinger_hive_residual_slave_hits = 0;
        self.stinger_hive_residual_slave_kills = 0;
        self.stinger_hive_residual_swallows = 0;
        self.stinger_hive_residual_respawns = 0;
        self.stinger_hive_residual_closest_slave_hits = 0;
        self.camo_netting_heat_vision_count = 0;
        self.camo_netting_structure_residual_reveals = 0;
        self.camo_netting_order_idle_enemies_count = 0;
        self.camo_netting_structure_residual_recloaks = 0;
        self.camo_netting_opacity_cloak_count = 0;
        self.camo_netting_opacity_reveal_count = 0;
        self.camo_netting_sub_object_show_count = 0;
        self.stinger_slave_order_attack_count = 0;
        self.patriot_residual_ground_fires = 0;
        self.patriot_residual_aa_fires = 0;
        self.patriot_scatter_applied = 0;
        self.patriot_scatter_misses = 0;
        self.stinger_scatter_applied = 0;
        self.stinger_scatter_misses = 0;
        self.supw_patriot_emp_residual_grants = 0;
        self.supw_emp_scatter_applied = 0;
        self.supw_emp_scatter_misses = 0;
        self.patriot_assist_residual_requests = 0;
        self.patriot_assist_residual_fires = 0;
        self.patriot_assist_residual_accepts = 0;
        self.patriot_assist_laser_from_assisted = 0;
        self.patriot_assist_laser_to_target = 0;
        self.patriot_assist_lasers.clear();
        self.pending_patriot_assists.clear();
        self.stealth_detector_rate_scans = 0;
        self.is_paused = false;
        self.sim_time_seconds = 0.0;
        self.accumulated_time = 0.0;
        self.last_fixed_step_diagnostics = FixedStepDiagnostics::default();
        self.map_loaded = false;
        self.victory_conditions.reset();
        self.scripts_loaded = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        self.script_source_path = None;
        self.mission_scripts.install_lists(&[]);
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.cinematic_letterbox = false;
        self.cinematic_text = None;
        self.military_caption = None;
        self.radar_enabled = true;
        self.radar_forced = false;
        self.pending_music_stop = false;
        self.pending_movie = None;
        self.pending_radar_movie = None;
        self.spawned_map_object_ids.clear();
        self.pending_special_abilities.clear();
        self.mission_objectives = Self::seed_sample_objectives();
        self.rebuild_objective_lookup();
        self.last_radar_event = None;
        self.under_attack_event_history.clear();
        self.under_attack_events = 0;
        self.eva_base_under_attack = 0;
        self.eva_ally_under_attack = 0;
        self.eva_low_power = 0;
        self.eva_low_power_next_frame = 0;
        self.eva_low_power_active = false;
        self.eva_insufficient_funds = 0;
        self.eva_insufficient_funds_next_frame = 0;
        self.eva_upgrade_complete = 0;
        self.eva_general_level_up = 0;
        self.eva_superweapon_ready = 0;
        self.eva_superweapon_detected = 0;
        self.eva_superweapon_launched = 0;
        self.eva_beacon_detected = 0;
        self.eva_hero_detected = 0;
        self.eva_special_launched_misc = 0;
        self.radar_upgrade_events = 0;
        self.structure_complete_events = 0;
        self.unit_ready_events = 0;
        self.radar_extend_starts = 0;
        self.radar_extend_completes = 0;
        self.radar_construction_events = 0;
        self.production_door_cycles = 0;
        self.construction_model_condition_updates = 0;
        self.actively_constructing_updates = 0;
        self.sell_list.clear();
        self.sell_process_starts = 0;
        self.sell_process_finishes = 0;
        self.sell_owned_mines_destroyed = 0;
        self.sell_passengers_ejected = 0;
        self.sell_parked_units_killed = 0;
        self.sell_tunnel_last_ejects = 0;
        self.capture_kick_outs = 0;
        self.capture_ai_auto_sells = 0;
        self.capture_deselections = 0;
        self.capture_tunnel_transfers = 0;
        self.capture_tunnel_last_ejects = 0;
        self.capture_tech_model_updates = 0;
        self.unmanned_reclaims = 0;
        self.carbomb_unmanned_detonations = 0;
        self.overcharge_toggles = 0;
        self.overcharge_drain_ticks = 0;
        self.overcharge_exhaustions = 0;
        self.control_rods_upgrades = 0;
        self.control_rods_plants_affected = 0;
        self.subliminal_messaging_upgrades = 0;
        self.subliminal_towers_affected = 0;
        self.construction_complete_clears = 0;
        self.dozer_cancel_task_events = 0;
        self.resume_construction_events = 0;
        self.repair_complete_events = 0;
        self.sole_benefactor_repair_rejects = 0;
        self.dozer_bored_repair_events = 0;
        self.dozer_bored_mine_clear_events = 0;
        self.rebuild_hole_spawns = 0;
        self.supply_create_warehouse_registers = 0;
        self.supply_create_center_registers = 0;
        self.structure_minefield_placements = 0;
        self.special_power_completion_log =
            crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog::default();
        self.sticky_bomb_follow_ticks = 0;
        self.sticky_bomb_target_deaths = 0;
        self.rebuild_hole_reconstructs = 0;
        self.rebuild_hole_workers = 0;
        self.rebuild_hole_heals = 0;
        self.rebuild_hole_completes = 0;
        self.rebuild_hole_attack_transfers = 0;
        self.rebuild_hole_bomb_transfers = 0;
        self.rebuild_hole_recon_deaths = 0;
        self.rebuild_hole_worker_restarts = 0;
        self.last_radar_audio_time = -10.0;
        self.last_radar_kind_time = [-10.0; 3];
        self.pending_camera_focus = None;
        self.script_camera_focus_estimate = Vec3::ZERO;
        self.script_camera_move_to = None;
        self.script_camera_path = None;
        self.camera_follow_target = None;
        self.script_default_camera_pitch = 1.0;
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = 1.0;
        self.script_camera_freeze_time_armed = false;
        self.script_camera_freeze_angle_armed = false;
        self.script_camera_pending_final_speed_multiplier = None;
        self.script_camera_pending_rolling_average_frames = None;
        self.visual_speed_multiplier = 1.0;
        self.script_time_frozen_by_script = false;
        self.pending_script_fps_limit = None;
        self.pending_camera_zoom_reset = false;
        self.pending_camera_zoom = None;
        self.pending_camera_pitch = None;
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        self.pending_camera_slave_mode_enable = None;
        self.pending_camera_slave_mode_disable = false;
        self.pending_screen_shakes.clear();
        self.pending_camera_add_shakers.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.host_beacons.clear();
        self.recent_beacons.clear();
        self.terrain = None;
        self.runtime_road_segments.clear();
        self.pathfinding_height_samples = None;
        self.weather_state = RuntimeWeatherState::default();
        // Host AI is match-scoped. Wipe so rematch / start_new_game cannot leave
        // orphan AI slots with stale object_ids while players were cleared above.
        // load_map does not call reset, so preserve_host_players still keeps AI.
        self.ai_manager = AIManager::new();
        log::debug!("GameLogic::reset() complete");
    }
}

// Split `impl GameLogic` chunks as real child modules (type-checked separately).
// Combat lives in world_combat/*.rs (#[path] because this file is game_logic.rs).
// Sibling private methods use pub(super) / pub(in super::super).
#[path = "world_combat/mod.rs"]
mod world_combat;
#[path = "world_objects/mod.rs"]
mod world_objects;
#[path = "world_save.rs"]
mod world_save;
#[path = "world_scripts/mod.rs"]
mod world_scripts;
#[path = "world_tick/mod.rs"]
mod world_tick;

impl GameLogic {
    fn update_player_alive_state(&mut self) {
        let mut events = Vec::new();
        for player in self.players.values_mut() {
            let alive = self
                .objects
                .values()
                .any(|obj| obj.team == player.team && obj.is_alive());
            if player.is_alive != alive {
                player.is_alive = alive;
                events.push((player.id, alive));
            } else {
                player.is_alive = alive;
            }
        }
        for (id, alive) in events {
            crate::game_logic::host_player_meta_log::record_alive(id, alive);
        }
    }

    pub fn evaluate_victory_condition(&mut self) -> Option<VictoryCondition> {
        // Wave 816: under coupled shadow, player is_alive owned by GW expire + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_player_alive_state();
        }
        self.victory_conditions
            .evaluate(&self.players, &self.objects, self.frame)
    }

    pub fn peek_defeat_events(&self) -> &[u32] {
        self.victory_conditions.peek_defeat_events()
    }

    pub fn take_defeat_events(&mut self) -> Vec<u32> {
        self.victory_conditions.take_defeat_events()
    }

    pub fn peek_alliance_events(&self) -> &[AllianceNotification] {
        self.victory_conditions.peek_alliance_events()
    }

    pub fn take_alliance_events(&mut self) -> Vec<AllianceNotification> {
        self.victory_conditions.take_alliance_events()
    }
}

/// Detailed object information for UI display
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub id: ObjectId,
    pub name: String,
    pub team: Team,
    pub object_type: ObjectType,
    pub health: Health,
    pub max_health: f32,
    pub position: Vec3,
    pub is_selected: bool,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub under_construction: bool,
    pub construction_percent: f32,
    pub experience_level: VeterancyLevel,
    pub ai_state: AIState,
    pub can_attack: bool,
    pub can_move: bool,
}

#[derive(Clone)]
pub(self) struct ShroudVisibilitySnapshot {
    visible_objects: HashSet<u32>,
    explored_objects: HashSet<u32>,
}

#[cfg(test)]
#[path = "world_tests/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "world_skirmish_tests.rs"]
mod skirmish_starting_unit_residual_tests;
