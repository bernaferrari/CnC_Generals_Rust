// Team struct fields
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team class (matching C++ Team structure and functionality)
#[derive(Debug)]
pub struct Team {
    // Core identity
    id: TeamID,
    name: AsciiString,

    // Team members (using ObjectID for now)
    members: Vec<ObjectID>,

    // Player control
    controlling_player_id: Option<UnsignedInt>,

    // AI state
    state: AsciiString,

    // Status flags
    entered_or_exited: Bool,
    active: Bool,
    created: Bool,
    recruitable: Bool,
    recruitability_set: Bool,

    // Enemy sighting and awareness
    check_enemy_sighted: Bool,
    see_enemy: Bool,
    prev_see_enemy: Bool,

    // Idle detection
    was_idle: Bool,

    // Destruction tracking
    destroy_threshold: Int,
    cur_units: Int,
    destroyed_threshold_ratio: Real,

    // Script hooks copied from TeamTemplateInfo.
    script_on_create: AsciiString,
    script_on_idle: AsciiString,
    script_on_enemy_sighted: AsciiString,
    script_on_all_clear: AsciiString,
    script_on_destroyed: AsciiString,
    script_on_unit_destroyed: AsciiString,

    // Common attack target
    common_attack_target: AtomicU32,

    // Current waypoint for group pathing (matches C++ Team::setCurrentWaypoint)
    current_waypoint_id: Option<WaypointId>,

    // Generic script hooks runtime state (C++ Team::m_shouldAttemptGenericScript)
    should_attempt_generic_script: [Bool; MAX_GENERIC_SCRIPTS],

    // Relationship overrides
    team_relations: Option<TeamRelationMap>,
    player_relations: Option<HashMap<Int, Relationship>>,

    // Singleton flag (from TeamPrototype at creation time)
    is_singleton: Bool,
}
