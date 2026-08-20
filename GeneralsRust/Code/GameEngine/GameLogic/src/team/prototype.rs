// TeamPrototype and production-condition runtime
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team prototype (matching C++ TeamPrototype functionality)
/// Runtime state for C++ `TeamPrototype::evaluateProductionCondition`.
#[derive(Debug, Default)]
struct ProductionConditionRuntime {
    always_false: bool,
    script: Option<Script>,
}

#[derive(Debug, Clone)]
pub struct TeamPrototype {
    // Identity
    id: TeamPrototypeID,
    name: AsciiString,
    owner_name: AsciiString,
    is_singleton: Bool,

    // Base settings
    is_ai_recruitable: Bool,
    is_base_defense: Bool,
    is_perimeter_defense: Bool,
    automatically_reinforce: Bool,
    initial_team_attitude: AttitudeType,
    transports_return: Bool,
    avoid_threats: Bool,
    attack_common_target: Bool,
    max_instances: Int,
    script_on_create: AsciiString,
    script_on_idle: AsciiString,
    initial_idle_frames: Int,
    script_on_enemy_sighted: AsciiString,
    script_on_all_clear: AsciiString,
    script_on_destroyed: AsciiString,
    destroyed_threshold: Real,
    script_on_unit_destroyed: AsciiString,
    production_priority: Int,
    production_priority_success_increase: Int,
    production_priority_failure_decrease: Int,
    production_condition: AsciiString,
    /// C++ m_productionConditionAlwaysFalse + m_productionConditionScript (shared runtime).
    production_condition_runtime: Arc<Mutex<ProductionConditionRuntime>>,
    execute_actions_on_create: Bool,
    team_generic_scripts: [AsciiString; MAX_GENERIC_SCRIPTS],
    generic_script_runtime: Arc<Mutex<Vec<Option<Script>>>>,

    // Attack priority
    attack_priority_name: AsciiString,

    // Unit creation info (matches TeamTemplateInfo::m_unitsInfo)
    units_info: [CreateUnitsInfo; MAX_UNIT_TYPES],
    num_units_info: usize,

    // Reinforcement-specific settings (matches TeamTemplateInfo reinforcement fields)
    transport_unit_type: AsciiString,
    start_reinforce_waypoint: AsciiString,
    team_starts_full: Bool,
    transports_exit: Bool,

    // C++ TeamTemplateInfo::m_homeLocation / m_hasHomeLocation
    home_location: Coord3D,
    has_home_location: Bool,
}

impl TeamPrototype {
    /// Create new team prototype
    pub fn new(name: AsciiString) -> Self {
        Self {
            id: TEAM_PROTOTYPE_ID_INVALID,
            name,
            owner_name: String::new().into(),
            is_singleton: false,
            is_ai_recruitable: false,
            is_base_defense: false,
            is_perimeter_defense: false,
            automatically_reinforce: false,
            initial_team_attitude: AttitudeType::Normal,
            transports_return: false,
            avoid_threats: false,
            attack_common_target: false,
            max_instances: 1,
            script_on_create: String::new().into(),
            script_on_idle: String::new().into(),
            initial_idle_frames: 0,
            script_on_enemy_sighted: String::new().into(),
            script_on_all_clear: String::new().into(),
            script_on_destroyed: String::new().into(),
            destroyed_threshold: 0.0,
            script_on_unit_destroyed: String::new().into(),
            production_priority: 0,
            production_priority_success_increase: 0,
            production_priority_failure_decrease: 0,
            production_condition: String::new().into(),
            production_condition_runtime: Arc::new(Mutex::new(
                ProductionConditionRuntime::default(),
            )),
            execute_actions_on_create: false,
            team_generic_scripts: std::array::from_fn(|_| String::new().into()),
            generic_script_runtime: Arc::new(Mutex::new(vec![None; MAX_GENERIC_SCRIPTS])),
            attack_priority_name: String::new().into(),
            units_info: [CreateUnitsInfo::new(); MAX_UNIT_TYPES],
            num_units_info: 0,
            transport_unit_type: String::new().into(),
            start_reinforce_waypoint: String::new().into(),
            team_starts_full: false,
            transports_exit: false,
            home_location: Coord3D::new(0.0, 0.0, 0.0),
            has_home_location: false,
        }
    }

    /// C++ TeamTemplateInfo::m_hasHomeLocation
    pub fn has_home_location(&self) -> bool {
        self.has_home_location
    }

    /// C++ TeamTemplateInfo::m_homeLocation
    pub fn home_location(&self) -> Coord3D {
        self.home_location
    }

    pub fn set_home_location(&mut self, loc: Coord3D) {
        self.home_location = loc;
        self.has_home_location = true;
    }

    pub fn clear_home_location(&mut self) {
        self.has_home_location = false;
        self.home_location = Coord3D::new(0.0, 0.0, 0.0);
    }

    /// Get prototype ID
    pub fn get_id(&self) -> TeamPrototypeID {
        self.id
    }

    /// Set prototype ID
    pub fn set_id(&mut self, id: TeamPrototypeID) {
        self.id = id;
    }

    /// Get prototype name
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Get controlling owner/player name from team definition.
    pub fn get_owner_name(&self) -> &AsciiString {
        &self.owner_name
    }

    /// Set controlling owner/player name from team definition.
    pub fn set_owner_name(&mut self, owner_name: AsciiString) {
        self.owner_name = owner_name;
    }

    /// Check if prototype is singleton
    pub fn is_singleton(&self) -> Bool {
        self.is_singleton
    }

    /// Set singleton flag
    pub fn set_singleton(&mut self, singleton: Bool) {
        self.is_singleton = singleton;
    }

    /// Check if AI recruitable
    pub fn is_ai_recruitable(&self) -> Bool {
        self.is_ai_recruitable
    }

    /// Set AI recruitable
    pub fn set_ai_recruitable(&mut self, recruitable: Bool) {
        self.is_ai_recruitable = recruitable;
    }

    /// Check if base defense
    pub fn is_base_defense(&self) -> Bool {
        self.is_base_defense
    }

    /// Set base defense
    pub fn set_base_defense(&mut self, base_defense: Bool) {
        self.is_base_defense = base_defense;
    }

    pub fn is_perimeter_defense(&self) -> Bool {
        self.is_perimeter_defense
    }

    pub fn set_perimeter_defense(&mut self, perimeter_defense: Bool) {
        self.is_perimeter_defense = perimeter_defense;
    }

    pub fn automatically_reinforce(&self) -> Bool {
        self.automatically_reinforce
    }

    pub fn set_automatically_reinforce(&mut self, automatically_reinforce: Bool) {
        self.automatically_reinforce = automatically_reinforce;
    }

    pub fn get_initial_team_attitude(&self) -> AttitudeType {
        self.initial_team_attitude
    }

    pub fn set_initial_team_attitude(&mut self, attitude: AttitudeType) {
        self.initial_team_attitude = attitude;
    }

    pub fn transports_return(&self) -> Bool {
        self.transports_return
    }

    pub fn set_transports_return(&mut self, transports_return: Bool) {
        self.transports_return = transports_return;
    }

    pub fn avoid_threats(&self) -> Bool {
        self.avoid_threats
    }

    pub fn set_avoid_threats(&mut self, avoid_threats: Bool) {
        self.avoid_threats = avoid_threats;
    }

    pub fn attack_common_target(&self) -> Bool {
        self.attack_common_target
    }

    pub fn set_attack_common_target(&mut self, attack_common_target: Bool) {
        self.attack_common_target = attack_common_target;
    }

    /// Get max instances
    pub fn get_max_instances(&self) -> Int {
        self.max_instances
    }

    /// Set max instances
    pub fn set_max_instances(&mut self, max_instances: Int) {
        self.max_instances = max_instances;
    }

    pub fn get_script_on_create(&self) -> &AsciiString {
        &self.script_on_create
    }

    pub fn set_script_on_create(&mut self, script_name: AsciiString) {
        self.script_on_create = script_name;
    }

    pub fn get_script_on_idle(&self) -> &AsciiString {
        &self.script_on_idle
    }

    pub fn set_script_on_idle(&mut self, script_name: AsciiString) {
        self.script_on_idle = script_name;
    }

    pub fn get_initial_idle_frames(&self) -> Int {
        self.initial_idle_frames
    }

    pub fn set_initial_idle_frames(&mut self, frames: Int) {
        self.initial_idle_frames = frames;
    }

    pub fn get_script_on_enemy_sighted(&self) -> &AsciiString {
        &self.script_on_enemy_sighted
    }

    pub fn set_script_on_enemy_sighted(&mut self, script_name: AsciiString) {
        self.script_on_enemy_sighted = script_name;
    }

    pub fn get_script_on_all_clear(&self) -> &AsciiString {
        &self.script_on_all_clear
    }

    pub fn set_script_on_all_clear(&mut self, script_name: AsciiString) {
        self.script_on_all_clear = script_name;
    }

    pub fn get_script_on_destroyed(&self) -> &AsciiString {
        &self.script_on_destroyed
    }

    pub fn set_script_on_destroyed(&mut self, script_name: AsciiString) {
        self.script_on_destroyed = script_name;
    }

    pub fn get_destroyed_threshold(&self) -> Real {
        self.destroyed_threshold
    }

    pub fn set_destroyed_threshold(&mut self, destroyed_threshold: Real) {
        self.destroyed_threshold = destroyed_threshold;
    }

    pub fn get_script_on_unit_destroyed(&self) -> &AsciiString {
        &self.script_on_unit_destroyed
    }

    pub fn set_script_on_unit_destroyed(&mut self, script_name: AsciiString) {
        self.script_on_unit_destroyed = script_name;
    }

    /// Get production priority
    pub fn get_production_priority(&self) -> Int {
        self.production_priority
    }

    /// Set production priority
    pub fn set_production_priority(&mut self, priority: Int) {
        self.production_priority = priority;
    }

    pub fn get_production_priority_success_increase(&self) -> Int {
        self.production_priority_success_increase
    }

    pub fn set_production_priority_success_increase(&mut self, increase: Int) {
        self.production_priority_success_increase = increase;
    }

    pub fn get_production_priority_failure_decrease(&self) -> Int {
        self.production_priority_failure_decrease
    }

    pub fn set_production_priority_failure_decrease(&mut self, decrease: Int) {
        self.production_priority_failure_decrease = decrease;
    }

    pub fn get_production_condition(&self) -> &AsciiString {
        &self.production_condition
    }

    pub fn set_production_condition(&mut self, production_condition: AsciiString) {
        self.production_condition = production_condition;
        // Changing the named condition invalidates any cached always-false / script copy.
        if let Ok(mut rt) = self.production_condition_runtime.lock() {
            rt.always_false = false;
            rt.script = None;
        }
    }

    /// C++ `TeamPrototype::evaluateProductionCondition` (Team.cpp).
    ///
    /// Loads/caches the productionCondition script, gates on player difficulty flags,
    /// honors delay-eval frame, and evaluates conditions for the controlling player.
    pub fn evaluate_production_condition(&self) -> Bool {
        let Ok(mut rt) = self.production_condition_runtime.lock() else {
            return false;
        };
        if rt.always_false {
            return false;
        }

        // Already have a local script copy — periodic / immediate eval.
        if rt.script.is_some() {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            if let Some(script) = rt.script.as_mut() {
                if current_frame < script.frame_to_evaluate_at {
                    return false;
                }
                let delay_seconds = script.delay_evaluation_seconds;
                if delay_seconds > 0 {
                    script.frame_to_evaluate_at = current_frame.saturating_add(
                        (delay_seconds as u32).saturating_mul(LOGICFRAMES_PER_SECOND as u32),
                    );
                }
            }
            let player_name = self.controlling_player_name();
            let script_engine = get_script_engine();
            let Ok(mut eng) = script_engine.write() else {
                return false;
            };
            let Some(engine) = eng.as_mut() else {
                return false;
            };
            if let Some(script) = rt.script.as_mut() {
                return engine.evaluate_conditions(script, None, player_name.as_deref());
            }
            return false;
        }

        // No script yet — resolve from name.
        let cond_name = self.production_condition.to_string();
        if cond_name.is_empty() {
            rt.always_false = true;
            return false;
        }

        let script_engine = get_script_engine();
        let Ok(mut eng) = script_engine.write() else {
            return false;
        };
        let Some(engine) = eng.as_mut() else {
            return false;
        };
        let Some(mut script) = engine.find_script_clone_by_name(&cond_name) else {
            rt.always_false = true;
            return false;
        };

        // Difficulty gate (C++ isEasy/isNormal/isHard on script).
        let difficulty = self.controlling_player_difficulty();
        let ok_for_diff = match difficulty {
            crate::player::GameDifficulty::Easy => script.easy,
            crate::player::GameDifficulty::Normal => script.normal,
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal => {
                script.hard
            }
        };
        if !ok_for_diff {
            rt.always_false = true;
            return false;
        }

        let player_name = self.controlling_player_name();
        let result = engine.evaluate_conditions(&mut script, None, player_name.as_deref());
        rt.script = Some(script);
        result
    }

    fn controlling_player_name(&self) -> Option<String> {
        let owner = self.owner_name.to_string();
        if owner.is_empty() {
            return None;
        }
        Some(owner)
    }

    fn controlling_player_difficulty(&self) -> crate::player::GameDifficulty {
        let owner = self.owner_name.to_string();
        if owner.is_empty() {
            return crate::player::GameDifficulty::Normal;
        }
        player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&owner))
            .and_then(|p| p.read().ok().map(|g| g.get_player_difficulty()))
            .unwrap_or(crate::player::GameDifficulty::Normal)
    }

    pub fn get_execute_actions_on_create(&self) -> Bool {
        self.execute_actions_on_create
    }

    pub fn set_execute_actions_on_create(&mut self, execute_actions_on_create: Bool) {
        self.execute_actions_on_create = execute_actions_on_create;
    }

    pub fn get_generic_script(&self, index: usize) -> Option<&AsciiString> {
        self.team_generic_scripts.get(index)
    }

    pub fn set_generic_script(&mut self, index: usize, script_name: AsciiString) {
        if let Some(slot) = self.team_generic_scripts.get_mut(index) {
            *slot = script_name;
        }
    }

    fn take_or_load_generic_script_runtime(&self, index: usize) -> Option<Script> {
        let script_name = self.get_generic_script(index)?.to_string();
        if script_name.is_empty() {
            return None;
        }

        let mut runtime = self.generic_script_runtime.lock().ok()?;
        if index >= runtime.len() {
            return None;
        }

        if runtime[index].is_none() {
            let script_engine = get_script_engine();
            let script = script_engine.read().ok().and_then(|engine_guard| {
                engine_guard
                    .as_ref()
                    .and_then(|engine| engine.find_script_clone_by_name(&script_name))
            })?;
            runtime[index] = Some(script);
        }

        runtime[index].take()
    }

    fn store_generic_script_runtime(&self, index: usize, script: Option<Script>) {
        let Ok(mut runtime) = self.generic_script_runtime.lock() else {
            return;
        };
        if let Some(slot) = runtime.get_mut(index) {
            *slot = script;
        }
    }

    pub fn set_units_info(&mut self, index: usize, info: CreateUnitsInfo) {
        if index >= MAX_UNIT_TYPES {
            return;
        }
        self.units_info[index] = info;
        if self.num_units_info <= index {
            self.num_units_info = index + 1;
        }
    }

    pub fn units_info(&self) -> &[CreateUnitsInfo] {
        &self.units_info[..self.num_units_info]
    }

    pub fn get_transport_unit_type(&self) -> &AsciiString {
        &self.transport_unit_type
    }

    pub fn set_transport_unit_type(&mut self, unit_type: AsciiString) {
        self.transport_unit_type = unit_type;
    }

    pub fn get_start_reinforce_waypoint(&self) -> &AsciiString {
        &self.start_reinforce_waypoint
    }

    pub fn set_start_reinforce_waypoint(&mut self, waypoint_name: AsciiString) {
        self.start_reinforce_waypoint = waypoint_name;
    }

    pub fn get_team_starts_full(&self) -> Bool {
        self.team_starts_full
    }

    pub fn set_team_starts_full(&mut self, starts_full: Bool) {
        self.team_starts_full = starts_full;
    }

    pub fn get_transports_exit(&self) -> Bool {
        self.transports_exit
    }

    pub fn set_transports_exit(&mut self, transports_exit: Bool) {
        self.transports_exit = transports_exit;
    }

    /// Set attack priority name
    pub fn set_attack_priority_name(&mut self, name: AsciiString) {
        self.attack_priority_name = name;
    }

    /// Get attack priority name
    pub fn get_attack_priority_name(&self) -> &AsciiString {
        &self.attack_priority_name
    }

    /// C++ `TeamPrototype::m_productionConditionAlwaysFalse`.
    pub fn production_condition_always_false(&self) -> Bool {
        self.production_condition_runtime
            .lock()
            .map(|rt| rt.always_false)
            .unwrap_or(false)
    }

    /// C++ `TeamPrototype::xfer` writes/restores `m_productionConditionAlwaysFalse`.
    pub fn set_production_condition_always_false(&self, always_false: Bool) {
        if let Ok(mut rt) = self.production_condition_runtime.lock() {
            rt.always_false = always_false;
        }
    }

    /// Resolve the controlling player index for `TeamPrototype::xfer`.
    pub fn owning_player_index(&self) -> Int {
        let owner_name = self.owner_name.as_str();
        player_list()
            .read()
            .ok()
            .and_then(|list| {
                if owner_name.is_empty() {
                    list.get_neutral_player()
                } else {
                    list.find_player_by_name(owner_name)
                        .or_else(|| list.get_neutral_player())
                }
            })
            .and_then(|player| player.read().ok().map(|p| p.get_player_index()))
            .unwrap_or(-1)
    }

    /// C++ load sets `m_owningPlayer = ThePlayerList->getNthPlayer(index)`.
    pub fn set_owning_player_index(&mut self, index: Int) {
        if index < 0 {
            self.owner_name = String::new().into();
            return;
        }
        let owner_name = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(index).cloned())
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
            })
            .unwrap_or_default();
        self.owner_name = owner_name.into();
    }


}

