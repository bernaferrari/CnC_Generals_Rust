// Team IDs, attitude, pending scripts, CreateUnitsInfo, relations
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team identifier type (matching C++ TeamID)
pub type TeamID = UnsignedInt;
pub const TEAM_ID_INVALID: TeamID = 0;

/// Team prototype identifier (matching C++ TeamPrototypeID)
pub type TeamPrototypeID = UnsignedInt;
pub const TEAM_PROTOTYPE_ID_INVALID: TeamPrototypeID = 0;

/// Attitude type for AI teams (matching C++ AttitudeType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttitudeType {
    Sleep = -2,
    Passive = -1,
    Normal = 0,
    Alert = 1,
    Aggressive = 2,
    Invalid = 3,
}

impl AttitudeType {
    fn from_ini(value: Int) -> Self {
        match value {
            -2 => Self::Sleep,
            -1 => Self::Passive,
            1 => Self::Alert,
            2 => Self::Aggressive,
            3 => Self::Invalid,
            _ => Self::Normal,
        }
    }
}

/// Team behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamBehavior {
    Normal = 0,
    IgnoreDistractions = 1,
    DealAggressively = 2,
}

/// Maximum number of unit types in a team template
pub const MAX_UNIT_TYPES: usize = 7;

/// Maximum generic scripts
pub const MAX_GENERIC_SCRIPTS: usize = 16;

#[derive(Debug, Clone)]
struct PendingTeamScriptEvent {
    team_name: String,
    script_name: String,
}

#[derive(Debug, Clone)]
struct PendingTeamGenericScriptEval {
    team: Arc<RwLock<Team>>,
    prototype: Arc<TeamPrototype>,
    team_name: String,
    script_name: String,
    script_index: usize,
    current_player_name: Option<String>,
}

static PENDING_TEAM_SCRIPT_EVENTS: OnceLock<Mutex<Vec<PendingTeamScriptEvent>>> = OnceLock::new();

fn pending_team_script_events() -> &'static Mutex<Vec<PendingTeamScriptEvent>> {
    PENDING_TEAM_SCRIPT_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn queue_team_script_event(team_name: &str, script_name: &str) {
    if team_name.is_empty() || script_name.is_empty() {
        return;
    }

    if let Ok(mut pending) = pending_team_script_events().lock() {
        pending.push(PendingTeamScriptEvent {
            team_name: team_name.to_string(),
            script_name: script_name.to_string(),
        });
    }
}

fn drain_pending_team_script_events() -> Vec<PendingTeamScriptEvent> {
    pending_team_script_events()
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

/// Opaque pending `Team::updateState` script events moved by the whole-world
/// save-restore boundary.  The process-global queue must not leak an old
/// world's pending callbacks into a staged map (or vice versa).
pub(crate) struct TeamScriptEventQueue {
    events: Vec<PendingTeamScriptEvent>,
}

pub(crate) fn take_pending_team_script_events_for_world_boundary() -> TeamScriptEventQueue {
    let mut pending = match pending_team_script_events().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    TeamScriptEventQueue {
        events: std::mem::take(&mut *pending),
    }
}

pub(crate) fn replace_pending_team_script_events_for_world_boundary(
    next: TeamScriptEventQueue,
) -> TeamScriptEventQueue {
    let mut pending = match pending_team_script_events().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    TeamScriptEventQueue {
        events: std::mem::replace(&mut *pending, next.events),
    }
}

pub fn flush_pending_team_script_events() {
    // Stage bootstrap must not execute callbacks before the host has committed
    // both its GameLogic and the staged singleton bundle.  The transaction
    // owns this queue while active and restores/discards it atomically.
    if crate::runtime_world_transaction::world_runtime_staging_active() {
        return;
    }
    let pending = drain_pending_team_script_events();
    if pending.is_empty() {
        return;
    }

    // C++ Team::updateState: TheScriptEngine->runScript(scriptName, this).
    let script_engine = get_script_engine();
    let Ok(mut engine_guard) = script_engine.write() else {
        return;
    };
    let Some(engine) = engine_guard.as_mut() else {
        return;
    };

    for event in pending {
        engine.run_script(&event.script_name, Some(event.team_name.as_str()));
    }
}

/// Unit creation info (matching C++ TCreateUnitsInfo)
#[derive(Debug, Clone, Copy)]
pub struct CreateUnitsInfo {
    pub min_units: Int,
    pub max_units: Int,
    pub unit_thing_name: &'static str, // Simplified for now
}

impl CreateUnitsInfo {
    pub const fn new() -> Self {
        Self {
            min_units: 0,
            max_units: 0,
            unit_thing_name: "",
        }
    }
}

impl Default for CreateUnitsInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Team relation map type (matching C++ TeamRelationMapType)
pub type TeamRelationMapType = HashMap<TeamID, Relationship>;

/// Team relation map (matching C++ TeamRelationMap)
#[derive(Debug, Clone)]
pub struct TeamRelationMap {
    pub map: TeamRelationMapType,
}

impl TeamRelationMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/// Wave 256: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}
