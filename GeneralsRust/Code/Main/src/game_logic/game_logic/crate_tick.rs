//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::authority::*;
use super::construct::*;
use super::host::*;
use super::player::*;
use super::prelude::*;
use super::script_camera::*;
use super::*;

/// Host count of crate ticks that were empty-world no-ops (not C++ phase order).
pub(super) static CRATE_EMPTY_NOOP_TICKS: AtomicU32 = AtomicU32::new(0);

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

pub(super) fn note_crate_empty_noop_if_any() {
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
    /// GLA Hijacker: transfer team + HIJACKED; ride-hide (drawable+partition)
    /// when the vehicle can eject, else consume the hijacker.
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
    /// China Helix Napalm/Nuke bomb: fly to StartAbilityRange 3, then drop.
    /// Location target stored as world*1000 so the enum stays `Eq`.
    HelixNapalmBomb {
        target_x_milli: i32,
        target_y_milli: i32,
        target_z_milli: i32,
    },
}

impl PendingSpecialAbility {
    pub(crate) fn target_id(self) -> ObjectId {
        match self {
            PendingSpecialAbility::HelixNapalmBomb { .. } => ObjectId(u32::MAX),
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

    pub(crate) fn helix_napalm_at(pos: Vec3) -> Self {
        Self::HelixNapalmBomb {
            target_x_milli: (pos.x * 1000.0).round() as i32,
            target_y_milli: (pos.y * 1000.0).round() as i32,
            target_z_milli: (pos.z * 1000.0).round() as i32,
        }
    }

    pub(crate) fn helix_napalm_target(self) -> Option<Vec3> {
        match self {
            PendingSpecialAbility::HelixNapalmBomb {
                target_x_milli,
                target_y_milli,
                target_z_milli,
            } => Some(Vec3::new(
                target_x_milli as f32 / 1000.0,
                target_y_milli as f32 / 1000.0,
                target_z_milli as f32 / 1000.0,
            )),
            _ => None,
        }
    }
}

/// Bridge Main's lightweight Team enum to GameEngine's Arc<RwLock<Team>>.
/// Uses the global TeamFactory to look up teams by player/faction name.
/// Global GameLogic singleton instance
pub(super) static GAME_LOGIC: OnceLock<Arc<Mutex<GameLogic>>> = OnceLock::new();

/// Audio event request (mirrors C++ AudioEventRTS pattern)
/// These events are queued each frame and processed by the audio system
#[derive(Debug, Clone)]
pub struct AudioEventRequest {
    pub event_type: String,          // e.g., "WeaponFire", "UnitDie", "Explosion"
    pub object_id: Option<ObjectId>, // Source object
    pub position: Option<Vec3>,      // 3D world position
    pub priority: u8,                // 0-255 (higher = more important)
    pub is_looping: bool,            // false = fire-and-forget, true = continuous
    /// C++ `TheAudio->removeAudioEvent` — stop a previously queued looping event.
    pub stop: bool,
    /// C++ `AudioEventRTS::setPlayerIndex` — VoiceEject and other owner-local voices.
    pub player_index: Option<i32>,
}

impl AudioEventRequest {
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            object_id: None,
            position: None,
            priority: 128,
            is_looping: false,
            stop: false,
            player_index: None,
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

    pub fn with_player_index(mut self, player_index: i32) -> Self {
        self.player_index = Some(player_index);
        self
    }

    pub fn looping(mut self) -> Self {
        self.is_looping = true;
        self
    }

    pub fn stopping(mut self) -> Self {
        self.stop = true;
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
    /// C++ `m_totalUnitsDestroyed[m_myPlayerIdx]` — display includes, score skips.
    pub units_destroyed_self: u32,
    /// C++ `m_totalBuildingsDestroyed[m_myPlayerIdx]`.
    pub structures_destroyed_self: u32,
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
    /// C++ AcademyStats::m_structuresGarrisoned residual.
    pub structures_garrisoned: u32,
    /// Alias honesty counter for academy garrison residual.
    pub academy_buildings_garrisoned: u32,
    /// C++ EVA UnitLost residual fires attributed to this player.
    pub eva_unit_lost: u32,
    /// C++ EVA BuildingLost residual fires attributed to this player.
    pub eva_building_lost: u32,
}
