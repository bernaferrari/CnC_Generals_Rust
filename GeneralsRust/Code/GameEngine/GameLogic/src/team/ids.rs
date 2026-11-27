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
pub(crate) struct PendingTeamScriptEvent {
    team_id: TeamID,
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

fn queue_team_script_event(
    world: &crate::system::engine_stores::WorldServices,
    team_id: TeamID,
    script_name: &str,
) {
    if team_id == TEAM_ID_INVALID || script_name.is_empty() {
        return;
    }
    if let Ok(mut pending) = world.pending_team_script_events().lock() {
        pending.push(PendingTeamScriptEvent {
            team_id,
            script_name: script_name.to_owned(),
        });
    }
}

#[cfg(test)]
fn drain_pending_team_script_events() -> Vec<PendingTeamScriptEvent> {
    take_team_script_events(&crate::system::engine_stores::active_services())
}

fn take_team_script_events(
    world: &crate::system::engine_stores::WorldServices,
) -> Vec<PendingTeamScriptEvent> {
    world.pending_team_script_events().lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

/// The end-of-frame adapter can deliver only when this world's slot is free.
pub(crate) fn flush_pending_team_script_events(
    world: &Arc<crate::system::engine_stores::WorldServices>,
    frame: u32,
) {
    let Ok(engine) = world.scripts().engine().try_write() else {
        return;
    };
    if let Some(engine) = engine.as_ref() {
        dispatch_pending_team_script_events(world, engine, frame);
    }
}

/// The script-phase adapter already has its interpreter, even during a slot loan.
pub(crate) fn dispatch_pending_team_script_events(
    world: &Arc<crate::system::engine_stores::WorldServices>,
    engine: &crate::scripting::engine::ScriptEngine,
    frame: u32,
) {
    // Detached save candidates hold callbacks until admission.
    if crate::runtime_world_transaction::world_runtime_staging_active_in(world) {
        return;
    }
    let pending = take_team_script_events(world);
    if pending.is_empty() {
        return;
    }
    crate::system::engine_stores::with_world_services(world, || {
        for event in pending {
            engine.run_standalone_team_script_at_frame(&event.script_name, Some(event.team_id), frame);
        }
    });
}

/// Unit creation info (matching C++ TCreateUnitsInfo)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateUnitsInfo {
    pub min_units: Int,
    pub max_units: Int,
    /// Authored unit template name owned by this team prototype.
    pub unit_thing_name: String,
}

impl CreateUnitsInfo {
    pub const fn new() -> Self {
        Self {
            min_units: 0,
            max_units: 0,
            unit_thing_name: String::new(),
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
