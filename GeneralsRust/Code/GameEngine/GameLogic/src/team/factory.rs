// TeamFactory prototype/instance management
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team factory for managing team prototypes and instances (matching C++ TeamFactory)
#[derive(Debug)]
pub struct TeamFactory {
    prototypes: HashMap<String, Arc<TeamPrototype>>,
    /// CPP Player::addTeamToList appends definitions; map keys and IDs are lookup only.
    prototype_order: Vec<String>,
    /// Presence admits an exact session controller, including explicit absence.
    prototype_controllers: HashMap<String, Option<u32>>,
    teams: HashMap<TeamID, Arc<RwLock<Team>>>,
    /// CPP TeamInstanceList admission order; IDs do not encode list position.
    instance_order: Vec<TeamID>,
    unique_team_prototype_id: TeamPrototypeID,
    unique_team_id: TeamID,
    pending_create_action_scripts: Vec<String>,
    pending_generic_script_evals: Vec<PendingTeamGenericScriptEval>,
    /// Host `Player::preTeamDestroy` notes for this factory instance.
    /// Swapped with the world; not a process-wide queue.
    host_pre_team_destroy: Vec<(TeamID, String)>,
}

/// One still-visible team whose deletion is driven by an explicit Main owner.
/// The identity pin prevents callbacks from finalizing a replacement with the same ID.
#[derive(Debug)]
pub struct HostTeamDeletion {
    id: TeamID,
    name: String,
    members: Vec<ObjectID>,
    identity: Arc<RwLock<Team>>,
}

impl HostTeamDeletion {
    pub fn id(&self) -> TeamID {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn members(&self) -> &[ObjectID] {
        &self.members
    }
}

impl TeamFactory {
    /// Create new team factory
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
            prototype_order: Vec::new(),
            prototype_controllers: HashMap::new(),
            teams: HashMap::new(),
            instance_order: Vec::new(),
            unique_team_prototype_id: 1,
            unique_team_id: 1,
            pending_create_action_scripts: Vec::new(),
            pending_generic_script_evals: Vec::new(),
            host_pre_team_destroy: Vec::new(),
        }
    }

    /// Definition admission for the existing mapless logical save API.
    /// Preserve authored identity/order/controller values without importing any
    /// live Team, callback queue, or mutable cached Script into the candidate.
    fn detached_definition_catalog(&self, names: Option<&[String]>) -> Result<Self, String> {
        let mut candidate = Self::new();
        candidate.prototype_order = self
            .prototype_order
            .iter()
            .filter(|name| names.is_none_or(|names| names.contains(name)))
            .cloned()
            .collect();
        candidate.prototype_controllers = self
            .prototype_controllers
            .iter()
            .filter(|(name, _)| names.is_none_or(|names| names.contains(name)))
            .map(|(name, owner)| (name.clone(), *owner))
            .collect();
        candidate.unique_team_prototype_id = self.unique_team_prototype_id;
        for (name, definition) in &self.prototypes {
            if names.is_some_and(|names| !names.contains(name)) {
                continue;
            }
            // Deliberate field-by-field definition copy: the only two
            // shared mutable cache fields get fresh owners, never Arc::clone.
            let always_false = definition
                .production_condition_runtime
                .lock()
                .map_err(|_| "team production-condition cache poisoned")?
                .always_false;
            let detached = TeamPrototype {
                id: definition.id,
                name: definition.name.clone(),
                owner_name: definition.owner_name.clone(),
                is_singleton: definition.is_singleton,
                is_ai_recruitable: definition.is_ai_recruitable,
                is_base_defense: definition.is_base_defense,
                is_perimeter_defense: definition.is_perimeter_defense,
                automatically_reinforce: definition.automatically_reinforce,
                initial_team_attitude: definition.initial_team_attitude,
                transports_return: definition.transports_return,
                avoid_threats: definition.avoid_threats,
                attack_common_target: definition.attack_common_target,
                max_instances: definition.max_instances,
                script_on_create: definition.script_on_create.clone(),
                script_on_idle: definition.script_on_idle.clone(),
                initial_idle_frames: definition.initial_idle_frames,
                script_on_enemy_sighted: definition.script_on_enemy_sighted.clone(),
                script_on_all_clear: definition.script_on_all_clear.clone(),
                script_on_destroyed: definition.script_on_destroyed.clone(),
                destroyed_threshold: definition.destroyed_threshold,
                script_on_unit_destroyed: definition.script_on_unit_destroyed.clone(),
                production_priority: definition.production_priority,
                production_priority_success_increase: definition
                    .production_priority_success_increase,
                production_priority_failure_decrease: definition
                    .production_priority_failure_decrease,
                production_condition: definition.production_condition.clone(),
                production_condition_runtime: Arc::new(Mutex::new(ProductionConditionRuntime {
                    always_false,
                    script: None,
                })),
                execute_actions_on_create: definition.execute_actions_on_create,
                team_generic_scripts: definition.team_generic_scripts.clone(),
                generic_script_runtime: Arc::new(Mutex::new(
                    (0..MAX_GENERIC_SCRIPTS)
                        .map(|_| GenericScriptRuntimeSlot::Empty)
                        .collect(),
                )),
                attack_priority_name: definition.attack_priority_name.clone(),
                units_info: definition.units_info,
                num_units_info: definition.num_units_info,
                transport_unit_type: definition.transport_unit_type.clone(),
                start_reinforce_waypoint: definition.start_reinforce_waypoint.clone(),
                team_starts_full: definition.team_starts_full,
                transports_exit: definition.transports_exit,
                home_location: definition.home_location,
                has_home_location: definition.has_home_location,
            };
            candidate
                .prototypes
                .insert(name.clone(), Arc::new(detached));
        }
        Ok(candidate)
    }

    /// Initialize team factory
    pub fn init(&mut self) {
        self.unlink_prototypes_from_owning_players();
        self.prototypes.clear();
        self.prototype_order.clear();
        self.prototype_controllers.clear();
        self.teams.clear();
        self.instance_order.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
        self.host_pre_team_destroy.clear();
    }

    /// Reset team factory
    pub fn reset(&mut self) {
        self.unlink_prototypes_from_owning_players();
        self.prototypes.clear();
        self.prototype_order.clear();
        self.prototype_controllers.clear();
        self.teams.clear();
        self.instance_order.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
        self.host_pre_team_destroy.clear();
    }

    /// Update team factory (called each frame)
    pub fn update(&mut self) {
        // Update all teams
        for team_arc in self.teams.values() {
            if let Ok(mut team) = team_arc.write() {
                team.update_state();
            }
        }

        // Queue generic script evaluations (executed after factory unlock in guard drop).
        for team_arc in self.teams.values() {
            let (team_name, controlling_player_id) = match team_arc.read() {
                Ok(team_guard) => (
                    team_guard.get_name().to_string(),
                    team_guard.get_controlling_player_id(),
                ),
                Err(_) => continue,
            };

            let Some(prototype) = self.prototypes.get(&team_name).cloned() else {
                continue;
            };

            let current_player_name = controlling_player_id.and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as Int).cloned())
                    .and_then(|player_arc| {
                        player_arc
                            .read()
                            .ok()
                            .map(|player| player.get_player_name_key())
                    })
                    .and_then(NameKeyGenerator::key_to_name)
            });

            let Ok(mut team_guard) = team_arc.write() else {
                continue;
            };
            for idx in 0..MAX_GENERIC_SCRIPTS {
                if !team_guard.should_attempt_generic_script(idx) {
                    continue;
                }

                let script_name = prototype
                    .get_generic_script(idx)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if script_name.is_empty() {
                    team_guard.disable_generic_script_attempt(idx);
                    continue;
                }

                self.pending_generic_script_evals
                    .push(PendingTeamGenericScriptEval {
                        team: team_arc.clone(),
                        prototype: prototype.clone(),
                        team_name: team_name.clone(),
                        script_name,
                        script_index: idx,
                        current_player_name: current_player_name.clone(),
                    });
            }
        }

        // C++ parity: remove empty active non-singleton teams that are not default teams.
        let mut teams_to_remove = Vec::new();
        for (team_id, team_arc) in &self.teams {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            if !team_guard.get_members().is_empty() {
                continue;
            }
            if !team_guard.is_active() || team_guard.is_default_team_for_controller() {
                continue;
            }

            let team_name = team_guard.get_name().to_string();
            if self
                .prototypes
                .get(&team_name)
                .is_some_and(|prototype| prototype.is_singleton())
            {
                continue;
            }
            teams_to_remove.push(*team_id);
        }

        for team_id in teams_to_remove {
            self.team_about_to_be_deleted(team_id);
        }
    }

    /// CPP TeamPrototype1038: query the first eligible empty instance again
    /// after every synchronous deletion. Default identity is supplied by Player.
    pub fn empty_active_team_for_prototype(
        &self,
        prototype: &str,
        default_team: Option<TeamID>,
    ) -> Option<TeamID> {
        if self.find_team_prototype(prototype)?.is_singleton() {
            return None;
        }
        self.find_team_instances(prototype)
            .into_iter()
            .find_map(|team| {
                let team = team.read().ok()?;
                (team.is_active()
                    && team.get_members().is_empty()
                    && Some(team.get_id()) != default_team)
                    .then(|| team.get_id())
            })
    }

    /// Initialize a team from configuration
    pub fn init_team(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
    ) -> Option<Arc<TeamPrototype>> {
        self.init_team_with_controller(name, owner, is_singleton, dict, None, None)
    }

    /// Map/session admission never publishes to the compatibility PlayerList.
    pub fn init_team_owned(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
        controller: Option<u32>,
    ) -> Option<Arc<TeamPrototype>> {
        self.init_team_with_controller(name, owner, is_singleton, dict, Some(controller), None)
    }

    /// CPP initTeam singleton creation executes its null-team action before
    /// admitting the next definition. The caller owns that interpreter phase.
    pub fn init_team_owned_with_effects(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
        controller: Option<u32>,
    ) -> Option<(Arc<TeamPrototype>, Vec<String>)> {
        let mut effects = Vec::new();
        let prototype = self.init_team_with_controller(
            name,
            owner,
            is_singleton,
            dict,
            Some(controller),
            Some(&mut effects),
        )?;
        Some((prototype, effects))
    }

    fn init_team_with_controller(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
        controller: Option<Option<u32>>,
        owned_effects: Option<&mut Vec<String>>,
    ) -> Option<Arc<TeamPrototype>> {
        let mut prototype = TeamPrototype::new(name.clone());
        prototype.set_id(self.unique_team_prototype_id);
        prototype.set_owner_name(owner);
        prototype.set_singleton(is_singleton);

        if let Some(dict) = dict {
            let unit_type_keys = [
                key_team_unit_type1(),
                key_team_unit_type2(),
                key_team_unit_type3(),
                key_team_unit_type4(),
                key_team_unit_type5(),
                key_team_unit_type6(),
                key_team_unit_type7(),
            ];
            let unit_min_keys = [
                key_team_unit_min_count1(),
                key_team_unit_min_count2(),
                key_team_unit_min_count3(),
                key_team_unit_min_count4(),
                key_team_unit_min_count5(),
                key_team_unit_min_count6(),
                key_team_unit_min_count7(),
            ];
            let unit_max_keys = [
                key_team_unit_max_count1(),
                key_team_unit_max_count2(),
                key_team_unit_max_count3(),
                key_team_unit_max_count4(),
                key_team_unit_max_count5(),
                key_team_unit_max_count6(),
                key_team_unit_max_count7(),
            ];

            for idx in 0..MAX_UNIT_TYPES {
                let type_key = unit_type_keys[idx];
                let min_key = unit_min_keys[idx];
                let max_key = unit_max_keys[idx];

                if dict.get_type(type_key).is_none() || dict.get_type(max_key).is_none() {
                    continue;
                }

                let max_units = dict.get_int(max_key);
                if max_units <= 0 {
                    continue;
                }
                let min_units = dict.get_int(min_key);
                let template_name = dict.get_ascii_string(type_key);
                if template_name.is_empty() {
                    continue;
                }

                let leaked_name: &'static str = Box::leak(template_name.into_boxed_str());
                prototype.set_units_info(
                    idx,
                    CreateUnitsInfo {
                        min_units,
                        max_units,
                        unit_thing_name: leaked_name,
                    },
                );
            }

            // C++ Team.cpp:669-679 — TheKey_teamHome walks TheTerrainLogic
            // waypoints; last name match sets m_homeLocation / m_hasHomeLocation.
            apply_team_home_from_dict(&mut prototype, dict);

            if dict.get_type(key_team_max_instances()).is_some() {
                prototype.set_max_instances(dict.get_int(key_team_max_instances()));
            }

            if dict.get_type(key_team_production_priority()).is_some() {
                prototype.set_production_priority(dict.get_int(key_team_production_priority()));
            }

            if dict
                .get_type(key_team_production_priority_success_increase())
                .is_some()
            {
                prototype.set_production_priority_success_increase(
                    dict.get_int(key_team_production_priority_success_increase()),
                );
            }

            if dict
                .get_type(key_team_production_priority_failure_decrease())
                .is_some()
            {
                prototype.set_production_priority_failure_decrease(
                    dict.get_int(key_team_production_priority_failure_decrease()),
                );
            }

            if dict.get_type(key_team_is_ai_recruitable()).is_some() {
                prototype.set_ai_recruitable(dict.get_bool(key_team_is_ai_recruitable()));
            }

            if dict.get_type(key_team_is_base_defense()).is_some() {
                prototype.set_base_defense(dict.get_bool(key_team_is_base_defense()));
            }

            if dict.get_type(key_team_is_perimeter_defense()).is_some() {
                prototype.set_perimeter_defense(dict.get_bool(key_team_is_perimeter_defense()));
            }

            if dict.get_type(key_team_auto_reinforce()).is_some() {
                prototype.set_automatically_reinforce(dict.get_bool(key_team_auto_reinforce()));
            }

            if dict.get_type(key_team_aggressiveness()).is_some() {
                prototype.set_initial_team_attitude(AttitudeType::from_ini(
                    dict.get_int(key_team_aggressiveness()),
                ));
            }

            if dict.get_type(key_team_transports_return()).is_some() {
                prototype.set_transports_return(dict.get_bool(key_team_transports_return()));
            }

            if dict.get_type(key_team_avoid_threats()).is_some() {
                prototype.set_avoid_threats(dict.get_bool(key_team_avoid_threats()));
            }

            if dict.get_type(key_team_attack_common_target()).is_some() {
                prototype.set_attack_common_target(dict.get_bool(key_team_attack_common_target()));
            }

            if dict.get_type(key_team_on_create_script()).is_some() {
                prototype.set_script_on_create(
                    dict.get_ascii_string(key_team_on_create_script()).into(),
                );
            }

            if dict.get_type(key_team_on_idle_script()).is_some() {
                prototype
                    .set_script_on_idle(dict.get_ascii_string(key_team_on_idle_script()).into());
            }

            if dict.get_type(key_team_initial_idle_frames()).is_some() {
                prototype.set_initial_idle_frames(dict.get_int(key_team_initial_idle_frames()));
            }

            if dict.get_type(key_team_enemy_sighted_script()).is_some() {
                prototype.set_script_on_enemy_sighted(
                    dict.get_ascii_string(key_team_enemy_sighted_script())
                        .into(),
                );
            }

            if dict.get_type(key_team_all_clear_script()).is_some() {
                prototype.set_script_on_all_clear(
                    dict.get_ascii_string(key_team_all_clear_script()).into(),
                );
            }

            if dict.get_type(key_team_on_destroyed_script()).is_some() {
                prototype.set_script_on_destroyed(
                    dict.get_ascii_string(key_team_on_destroyed_script()).into(),
                );
            }

            if dict.get_type(key_team_destroyed_threshold()).is_some() {
                prototype.set_destroyed_threshold(dict.get_real(key_team_destroyed_threshold()));
            }

            if dict.get_type(key_team_on_unit_destroyed_script()).is_some() {
                prototype.set_script_on_unit_destroyed(
                    dict.get_ascii_string(key_team_on_unit_destroyed_script())
                        .into(),
                );
            }

            if dict.get_type(key_team_production_condition()).is_some() {
                prototype.set_production_condition(
                    dict.get_ascii_string(key_team_production_condition())
                        .into(),
                );
            }

            if dict
                .get_type(key_team_executes_actions_on_create())
                .is_some()
            {
                prototype.set_execute_actions_on_create(
                    dict.get_bool(key_team_executes_actions_on_create()),
                );
            }

            let generic_base =
                NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap_or_default();
            for idx in 0..MAX_GENERIC_SCRIPTS {
                let key_name = format!("{}{}", generic_base, idx);
                let key = NameKeyGenerator::name_to_key(&key_name);
                if dict.get_type(key).is_some() {
                    prototype.set_generic_script(idx, dict.get_ascii_string(key).into());
                } else {
                    prototype.set_generic_script(idx, String::new().into());
                }
            }

            if dict.get_type(key_team_transport()).is_some() {
                prototype
                    .set_transport_unit_type(dict.get_ascii_string(key_team_transport()).into());
            }

            if dict.get_type(key_team_reinforcement_origin()).is_some() {
                prototype.set_start_reinforce_waypoint(
                    dict.get_ascii_string(key_team_reinforcement_origin())
                        .into(),
                );
            }

            if dict.get_type(key_team_starts_full()).is_some() {
                prototype.set_team_starts_full(dict.get_bool(key_team_starts_full()));
            }

            if dict.get_type(key_team_transports_exit()).is_some() {
                prototype.set_transports_exit(dict.get_bool(key_team_transports_exit()));
            }
        }

        self.unique_team_prototype_id += 1;

        let prototype = Arc::new(prototype);
        if !self.prototypes.contains_key(name.as_str()) {
            self.prototype_order.push(name.to_string());
        }
        self.prototypes.insert(name.to_string(), prototype.clone());
        if let Some(controller) = controller {
            self.prototype_controllers
                .insert(name.to_string(), controller);
        } else {
            self.prototype_controllers.remove(name.as_str());
            // Standalone C++ compatibility adapter only.
            self.bind_prototype_to_owning_player(&prototype);
        }
        if is_singleton {
            if let Some(effects) = owned_effects {
                let _ = self.create_inactive_team_collecting_effects(name.as_str(), effects);
            } else {
                let _ = self.create_inactive_team(name.as_str());
            }
        }
        Some(prototype)
    }

    /// Find team prototype by name
    pub fn find_team_prototype(&self, name: &str) -> Option<Arc<TeamPrototype>> {
        self.prototypes.get(name).cloned()
    }

    /// CPP Team.cpp:1315 prepends newly constructed instances. Restoring an
    /// existing saved ID does not relink it (Team.cpp:1249-1258).
    fn admit_team_instance(&mut self, id: TeamID, team: Arc<RwLock<Team>>) {
        if !self.teams.contains_key(&id) {
            self.instance_order.insert(0, id);
        }
        self.teams.insert(id, team);
    }

    fn remove_team_instance(&mut self, id: TeamID) {
        self.teams.remove(&id);
        self.instance_order.retain(|admitted| *admitted != id);
    }

    /// CPP ScriptEngine.cpp:5933 resolves an existing team without creating it.
    /// Ordinary lookup uses the head of the prototype's admitted instance list,
    /// independently of numeric IDs. A uniquely named calling/condition team
    /// bypasses prototype and singleton activity checks; ambiguous context
    /// requires an instance ID.
    pub fn script_team_member_ids(
        &self,
        name: &str,
        contextual: bool,
    ) -> crate::GameLogicResult<Option<Vec<ObjectID>>> {
        let prototype = self.prototypes.get(name);
        if !contextual && prototype.is_none() {
            return Ok(None);
        }
        let mut selected: Option<(bool, Vec<ObjectID>)> = None;
        for id in &self.instance_order {
            let team = self.teams.get(id).ok_or_else(|| {
                crate::GameLogicError::Configuration(format!(
                    "Script team instance order references missing instance {id}"
                ))
            })?;
            let team = team.read().map_err(|error| {
                crate::GameLogicError::Threading(format!(
                    "Cannot read script team instance {id}: {error}"
                ))
            })?;
            if team.get_name().as_str() != name {
                continue;
            }
            if contextual && selected.is_some() {
                return Err(crate::GameLogicError::Configuration(format!(
                    "Script team context '{name}' matches multiple instances; a contextual team ID is required"
                )));
            }
            if selected.is_none() {
                selected = Some((team.is_active(), team.get_members().to_vec()));
            }
            if !contextual {
                break;
            }
        }
        let Some((active, members)) = selected else {
            return Ok(None);
        };
        if !contextual && prototype.is_some_and(|prototype| prototype.is_singleton()) && !active {
            return Ok(None);
        }
        Ok(Some(members))
    }

    /// Script team prototype names declared on the map (C++ TeamFactory prototypes).
    pub fn prototype_names(&self) -> Vec<String> {
        self.prototype_order.clone()
    }

    /// Find team prototype by ID
    pub fn find_team_prototype_by_id(&self, id: TeamPrototypeID) -> Option<Arc<TeamPrototype>> {
        for prototype in self.prototypes.values() {
            if prototype.get_id() == id {
                return Some(prototype.clone());
            }
        }
        None
    }

    pub fn list_team_prototypes(&self) -> Vec<Arc<TeamPrototype>> {
        self.list_team_prototypes_in_admission_order()
    }

    pub fn list_team_prototypes_in_admission_order(&self) -> Vec<Arc<TeamPrototype>> {
        self.prototype_order
            .iter()
            .filter_map(|name| self.prototypes.get(name).cloned())
            .collect()
    }

    /// Exact admitted controller identity, in original player-list visitation order.
    pub fn owned_team_prototypes_for_controller(&self, controller: u32) -> Vec<Arc<TeamPrototype>> {
        self.list_team_prototypes_in_admission_order()
            .into_iter()
            .filter(|prototype| {
                self.prototype_controllers
                    .get(prototype.get_name().as_str())
                    == Some(&Some(controller))
            })
            .collect()
    }

    /// CPP Player::iterateObjects: player prototype admission order, then each
    /// prototype's actual instance-list order. Member traversal remains live
    /// in the caller, which resolves each object at its callback point.
    pub fn owned_team_instances_for_controller(&self, controller: u32) -> Vec<Arc<RwLock<Team>>> {
        self.owned_team_prototypes_for_controller(controller)
            .into_iter()
            .flat_map(|prototype| self.find_team_instances(prototype.get_name().as_str()))
            .collect()
    }

    /// Return the receiving controller's prototype and live-instance pairs in
    /// C++ Player::iterateObjects order. The caller executes callbacks only
    /// after this detached list has been collected and the factory unlocked.
    pub(crate) fn owned_team_generic_script_entries(
        &self,
        controller: u32,
    ) -> Vec<(Arc<TeamPrototype>, Arc<RwLock<Team>>)> {
        self.owned_team_prototypes_for_controller(controller)
            .into_iter()
            .flat_map(|prototype| {
                self.find_team_instances(prototype.get_name().as_str())
                    .into_iter()
                    .map(move |team| (Arc::clone(&prototype), team))
            })
            .collect()
    }

    /// Snapshot one prototype's TeamInstanceList at the moment C++ Player::update
    /// reaches that prototype. A previous prototype's synchronous generic hook
    /// can create a team instance that the later prototype's live traversal sees.
    pub(crate) fn generic_script_entries_for_prototype(
        &self,
        prototype: &Arc<TeamPrototype>,
    ) -> Vec<(Arc<TeamPrototype>, Arc<RwLock<Team>>)> {
        let name = prototype.get_name().as_str();
        if !self
            .prototypes
            .get(name)
            .is_some_and(|current| Arc::ptr_eq(current, prototype))
        {
            return Vec::new();
        }
        self.find_team_instances(name)
            .into_iter()
            .map(|team| (Arc::clone(prototype), team))
            .collect()
    }

    /// Restore the saved visitation sequence after validating every identity.
    /// Definitions not included in an older partial capsule retain their order.
    pub fn restore_prototype_admission_order(&mut self, names: &[String]) -> GameLogicResult<()> {
        let mut seen = HashSet::new();
        for name in names {
            if !self.prototypes.contains_key(name) || !seen.insert(name.clone()) {
                return Err(GameLogicError::Configuration(format!(
                    "Invalid saved team prototype order entry {name}"
                )));
            }
        }
        let mut order = names.to_vec();
        order.extend(
            self.prototype_order
                .iter()
                .filter(|name| !seen.contains(*name))
                .cloned(),
        );
        self.prototype_order = order;
        Ok(())
    }

    /// Snapshot hook: next team instance ID allocator value.
    pub fn get_next_team_id(&self) -> TeamID {
        self.unique_team_id
    }

    /// Snapshot hook: next team prototype ID allocator value.
    pub fn get_next_team_prototype_id(&self) -> TeamPrototypeID {
        self.unique_team_prototype_id
    }

    /// Snapshot hook: restore allocator state from save data.
    pub fn set_next_team_ids(
        &mut self,
        next_team_id: TeamID,
        next_team_prototype_id: TeamPrototypeID,
    ) {
        self.unique_team_id = next_team_id.max(1);
        self.unique_team_prototype_id = next_team_prototype_id.max(1);
    }

    /// C++ `TeamFactory::xfer` only persists `m_uniqueTeamID`.
    pub fn set_next_team_id(&mut self, next_team_id: TeamID) {
        self.unique_team_id = next_team_id;
    }

    /// C++ `TheTeamFactory->createTeamOnPrototype` + `Team::setID`.
    pub fn create_team_on_prototype_with_id(
        &mut self,
        prototype: &TeamPrototype,
        team_id: TeamID,
    ) -> Option<Arc<RwLock<Team>>> {
        if let Some(existing) = self.find_team_by_id(team_id) {
            return Some(existing);
        }

        let name = prototype.get_name().to_string();
        let team = Arc::new(RwLock::new(Team::new(name.clone().into(), team_id)));
        if let Ok(mut team_guard) = team.write() {
            team_guard.set_prototype_recruitable(prototype.is_ai_recruitable());
            team_guard.apply_template_script_hooks(prototype);
        }
        let controller = self.controller_for_prototype(prototype);
        team.write().ok()?.set_controlling_player_id(controller);
        self.admit_team_instance(team_id, team.clone());
        if team_id >= self.unique_team_id {
            self.unique_team_id = team_id.saturating_add(1);
        }
        Some(team)
    }

    /// Replace a prototype after xfer mutates template fields stored by value.
    pub fn replace_team_prototype(&mut self, prototype: TeamPrototype) {
        let name = prototype.get_name().to_string();
        if !self.prototypes.contains_key(&name) {
            self.prototype_order.push(name.clone());
        }
        self.prototypes.insert(name, Arc::new(prototype));
    }

    /// C++ `TeamFactory::loadPostProcess` — next IDs just over the highest in use.
    pub fn restore_unique_ids_after_load(&mut self) {
        self.unique_team_id = 0;
        self.unique_team_prototype_id = 0;
        for prototype in self.prototypes.values() {
            if prototype.get_id() >= self.unique_team_prototype_id {
                self.unique_team_prototype_id = prototype.get_id().saturating_add(1);
            }
        }
        for team_id in self.teams.keys() {
            if *team_id >= self.unique_team_id {
                self.unique_team_id = team_id.saturating_add(1);
            }
        }
    }

    /// Find team by ID
    pub fn find_team_by_id(&self, team_id: TeamID) -> Option<Arc<RwLock<Team>>> {
        self.teams.get(&team_id).cloned()
    }

    fn find_existing_team_by_name(&self, name: &str) -> Option<Arc<RwLock<Team>>> {
        for id in &self.instance_order {
            let Some(team) = self.teams.get(id) else {
                continue;
            };
            if let Ok(team_ref) = team.read() {
                if team_ref.get_name() == name {
                    return Some(team.clone());
                }
            }
        }
        None
    }

    /// Create team from prototype name
    pub fn create_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let team = self.create_inactive_team(name)?;
        team.write().ok()?.set_active();
        Some(team)
    }

    /// Create inactive team
    pub fn create_inactive_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let mut on_create = Vec::new();
        let team = self.create_inactive_team_collecting_effects(name, &mut on_create);
        self.pending_create_action_scripts.extend(on_create);
        team
    }

    /// The driving interpreter executes these original null-team callbacks
    /// after releasing the factory borrow, at the synchronous creation phase.
    pub fn create_inactive_team_with_owned_effects(
        &mut self,
        name: &str,
    ) -> Option<(Arc<RwLock<Team>>, Vec<String>)> {
        let mut on_create = Vec::new();
        let team = self.create_inactive_team_collecting_effects(name, &mut on_create)?;
        Some((team, on_create))
    }

    fn collect_create_effects(prototype: &TeamPrototype, on_create: &mut Vec<String>) {
        if prototype.get_execute_actions_on_create()
            && !prototype.get_production_condition().is_empty()
        {
            on_create.push(prototype.get_production_condition().to_string());
        }
    }

    fn create_inactive_team_collecting_effects(
        &mut self,
        name: &str,
        on_create: &mut Vec<String>,
    ) -> Option<Arc<RwLock<Team>>> {
        let prototype = self.find_team_prototype(name);
        if prototype.is_none() {
            return None;
        }
        if prototype.as_ref().is_some_and(|p| p.is_singleton()) {
            if let Some(existing) = self.find_existing_team_by_name(name) {
                if let Some(prototype) = prototype.as_deref() {
                    Self::collect_create_effects(prototype, on_create);
                }
                return Some(existing);
            }
        }

        let team_id = self.unique_team_id;
        self.unique_team_id += 1;

        let team = Arc::new(RwLock::new(Team::new(name.to_string().into(), team_id)));
        if let Some(ref prototype) = prototype {
            if let Ok(mut team_guard) = team.write() {
                team_guard.set_prototype_recruitable(prototype.is_ai_recruitable());
                team_guard.apply_template_script_hooks(prototype);
            }
            let controller = self.controller_for_prototype(prototype);
            team.write().ok()?.set_controlling_player_id(controller);
        }

        self.admit_team_instance(team_id, team.clone());
        if let Some(prototype) = prototype.as_deref() {
            Self::collect_create_effects(prototype, on_create);
        }
        Some(team)
    }

    /// Find team by name
    pub fn find_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let prototype = self.find_team_prototype(name)?;
        if let Some(team) = self.find_existing_team_by_name(name) {
            return Some(team);
        }
        if !prototype.is_singleton() {
            return self.create_inactive_team(name);
        }
        None
    }

    /// C++ findTeam: only an implicit non-singleton creation runs callbacks.
    /// The caller delivers them synchronously after releasing the factory.
    pub fn find_team_with_owned_effects(
        &mut self,
        name: &str,
    ) -> Option<(Arc<RwLock<Team>>, Vec<String>)> {
        let prototype = self.find_team_prototype(name)?;
        if let Some(team) = self.find_existing_team_by_name(name) {
            return Some((team, Vec::new()));
        }
        if prototype.is_singleton() {
            return None;
        }
        self.create_inactive_team_with_owned_effects(name)
    }

    /// Find all team instances that were created from the same prototype name.
    ///
    /// C++ Reference: `TeamPrototype::iterate_TeamInstanceList()` used by
    /// `ScriptEngine::executeScript()` when `conditionTeamName` is set.
    pub fn find_team_instances(&self, prototype_name: &str) -> Vec<Arc<RwLock<Team>>> {
        self.instance_order
            .iter()
            .filter_map(|id| self.teams.get(id))
            .filter_map(|team| {
                let guard = team.read().ok()?;
                (guard.get_name() == prototype_name).then_some(team.clone())
            })
            .collect()
    }

    /// C++ DLINK_ITERATOR rereads the current instance's next link after a
    /// synchronous script callback. Use the live admission order, without
    /// retaining a roster or team guard across that callback. A removed cursor
    /// ends the walk; it can never be rebound to another same-name instance.
    pub fn condition_team_after(
        &self,
        prototype_name: &str,
        after: Option<TeamID>,
    ) -> Option<TeamID> {
        let start = match after {
            Some(id) => {
                if self
                    .teams
                    .get(&id)?
                    .read()
                    .expect("condition-team cursor")
                    .get_name()
                    != prototype_name
                {
                    return None;
                }
                self.instance_order.iter().position(|live| *live == id)? + 1
            }
            None => 0,
        };
        self.instance_order[start..].iter().find_map(|id| {
            let team = self.teams.get(id)?.read().expect("condition-team identity");
            (team.get_name() == prototype_name).then_some(*id)
        })
    }

    /// Return all live team instances.
    pub fn get_all_teams(&self) -> Vec<Arc<RwLock<Team>>> {
        self.instance_order
            .iter()
            .filter_map(|id| self.teams.get(id).cloned())
            .collect()
    }

    /// Adjust production priority for a team prototype at runtime.
    ///
    /// C++ parity: `TeamPrototype::increaseAIPriorityForSuccess` /
    /// `TeamPrototype::decreaseAIPriorityForFailure` mutate template runtime state.
    pub fn adjust_team_prototype_priority(
        &mut self,
        prototype_name: &str,
        delta: Int,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let next = updated.get_production_priority().saturating_add(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    pub fn increase_team_prototype_priority_for_success(
        &mut self,
        prototype_name: &str,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let delta = updated.get_production_priority_success_increase();
        let next = updated.get_production_priority().saturating_add(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    pub fn decrease_team_prototype_priority_for_failure(
        &mut self,
        prototype_name: &str,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let delta = updated.get_production_priority_failure_decrease();
        let next = updated.get_production_priority().saturating_sub(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    /// Set runtime attack-priority set name for a team prototype.
    pub fn set_team_prototype_attack_priority_name(
        &mut self,
        prototype_name: &str,
        attack_priority_name: &str,
    ) -> bool {
        let Some(prototype) = self.prototypes.get(prototype_name).cloned() else {
            return false;
        };
        let mut updated = (*prototype).clone();
        updated.set_attack_priority_name(AsciiString::from(attack_priority_name));
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        true
    }

    /// Notify that team is about to be deleted
    pub fn team_about_to_be_deleted(&mut self, team_id: TeamID) {
        let team_arc = self.teams.get(&team_id).cloned();
        let team_name = team_arc
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|team| team.get_name().to_string()));

        // C++ TeamFactory::teamAboutToBeDeleted — drop override relationships.
        for other in self.teams.values() {
            if let Ok(mut other_guard) = other.try_write() {
                let _ = other_guard.remove_override_team_relationship(team_id);
            }
        }
        if let (Some(team_arc), Ok(list)) = (&team_arc, player_list().read()) {
            if let Ok(team_guard) = team_arc.read() {
                for player_arc in list.iter() {
                    if let Ok(mut player) = player_arc.write() {
                        let _ = player.remove_team_relationship(&team_guard);
                    }
                }
            }
        }

        // C++ Team::~Team — notify scripts, then every Player::preTeamDestroy.
        if let Some(name) = &team_name {
            if let Ok(mut engine) = get_script_engine().write() {
                if let Some(engine) = engine.as_mut() {
                    engine.notify_of_team_destruction(team_id);
                }
            }
        }
        // Live host AIPlayer queues are not leftover IntegratedAiPlayer.
        // C++ Player::preTeamDestroy walks every player; drain on the host tick.
        self.host_pre_team_destroy
            .push((team_id, team_name.as_deref().unwrap_or("").to_string()));

        if let Some(team_arc) = &team_arc {
            if let Ok(list) = player_list().read() {
                for player_arc in list.iter() {
                    let player_id = player_arc
                        .read()
                        .ok()
                        .map(|player| player.get_player_index() as u32);
                    let Some(player_id) = player_id else {
                        continue;
                    };
                    let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
                        manager.with_ai_player_mut(player_id, |ai| match ai {
                            crate::ai::integration::IntegratedAiPlayer::Standard(player) => {
                                player.ai_pre_team_destroy(team_arc);
                            }
                            crate::ai::integration::IntegratedAiPlayer::Skirmish(player) => {
                                player.ai_pre_team_destroy(team_arc);
                            }
                        })
                    });
                }
            }

            let members = team_arc
                .read()
                .ok()
                .map(|team| team.get_members().to_vec())
                .unwrap_or_default();
            for object_id in members {
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object| {
                    let _ = object.set_team(None);
                });
            }
        }

        self.remove_team_instance(team_id);
    }

    /// C++ TeamFactory::teamAboutToBeDeleted, restricted to this factory's metadata.
    /// Main supplies the player/script/member callbacks before finalization; this
    /// path never discovers a Core Player, AI controller, or Object registry.
    pub fn prepare_host_team_deletion(&mut self, team_id: TeamID) -> Option<HostTeamDeletion> {
        let identity = self.teams.get(&team_id)?.clone();
        let (name, members) = {
            let team = identity.read().unwrap_or_else(|e| e.into_inner());
            (team.get_name().to_string(), team.get_members().to_vec())
        };
        for other in self.teams.values() {
            other
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove_override_team_relationship(team_id);
        }
        Some(HostTeamDeletion {
            id: team_id,
            name,
            members,
            identity,
        })
    }

    /// Finish only the team prepared above, after synchronous owner callbacks.
    pub fn finalize_host_team_deletion(&mut self, deletion: HostTeamDeletion) -> bool {
        if !self
            .teams
            .get(&deletion.id)
            .is_some_and(|team| Arc::ptr_eq(team, &deletion.identity))
        {
            return false;
        }
        self.remove_team_instance(deletion.id);
        true
    }

    /// C++ TeamPrototype ctor/dtor + initTeam owner lookup (Team.cpp:216-223, 799-800).
    fn controller_for_prototype(&self, prototype: &TeamPrototype) -> Option<u32> {
        if let Some(controller) = self
            .prototype_controllers
            .get(prototype.get_name().as_str())
        {
            return *controller;
        }
        self.resolve_owning_player(prototype.get_owner_name().as_str())
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|player| player.get_player_index() as u32)
            })
    }

    fn bind_prototype_to_owning_player(&self, prototype: &Arc<TeamPrototype>) {
        let Some(player) = self.resolve_owning_player(prototype.get_owner_name().as_str()) else {
            return;
        };
        if let Ok(mut player_guard) = player.write() {
            player_guard.add_team_to_list(Arc::clone(prototype));
        }
    }

    fn unlink_prototypes_from_owning_players(&self) {
        for prototype in self.prototypes.values().filter(|prototype| {
            !self
                .prototype_controllers
                .contains_key(prototype.get_name().as_str())
        }) {
            let Some(player) = self.resolve_owning_player(prototype.get_owner_name().as_str())
            else {
                continue;
            };
            if let Ok(mut player_guard) = player.write() {
                player_guard.remove_team_from_list(prototype);
            }
        }
    }

    fn resolve_owning_player(
        &self,
        owner_name: &str,
    ) -> Option<Arc<RwLock<crate::player::Player>>> {
        let list = player_list().read().ok()?;
        if owner_name.is_empty() {
            list.get_neutral_player()
        } else {
            list.find_player_by_name(owner_name)
                .or_else(|| list.get_neutral_player())
        }
    }

    fn drain_pending_create_action_scripts(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_create_action_scripts)
    }

    fn drain_pending_generic_script_evals(&mut self) -> Vec<PendingTeamGenericScriptEval> {
        std::mem::take(&mut self.pending_generic_script_evals)
    }

    /// Drain host `Player::preTeamDestroy` notifications recorded on this factory.
    pub fn take_host_pre_team_destroy_requests(&mut self) -> Vec<(TeamID, String)> {
        std::mem::take(&mut self.host_pre_team_destroy)
    }
}

/// C++ TeamTemplateInfo (Team.cpp:669-679): walk `getFirstWaypoint` / `getNext`.
/// Last matching name wins. Empty names are still searched if the key exists.
pub(super) fn resolve_team_home_waypoint_location(name: &str) -> Option<Coord3D> {
    let terrain_owner_handle = crate::terrain::get_terrain_logic();
    let Ok(terrain) = terrain_owner_handle.read() else {
        return None;
    };
    let mut current = terrain.get_first_waypoint();
    let mut found = None;
    while let Some(way) = current {
        if way.get_name().as_str() == name {
            found = Some(*way.get_location());
        }
        current = way.get_next();
    }
    found
}

fn leftover_resolve_team_home_waypoint(
    name: &str,
) -> Option<game_engine::common::system::geometry::Coord3D> {
    resolve_team_home_waypoint_location(name)
        .map(|loc| game_engine::common::system::geometry::Coord3D::new(loc.x, loc.y, loc.z))
}

fn apply_team_home_from_dict(prototype: &mut TeamPrototype, dict: &Dict) {
    if dict.get_type(key_team_home()).is_none() {
        return;
    }
    let waypoint = dict.get_ascii_string(key_team_home());
    if let Some(loc) = resolve_team_home_waypoint_location(&waypoint) {
        prototype.set_home_location(loc);
    }
}

fn execute_pending_team_create_action_scripts(script_names: Vec<String>) {
    if script_names.is_empty() {
        return;
    }

    // C++ createInactiveTeam: friend_executeAction(action) with NULL team.
    let script_engine = get_script_engine();
    for script_name in script_names {
        let action = {
            let Ok(engine_guard) = script_engine.read() else {
                continue;
            };
            engine_guard
                .as_ref()
                .and_then(|engine| engine.find_script_clone_by_name(&script_name))
                .and_then(|script| script.get_action().cloned())
        };
        let Some(action) = action else {
            continue;
        };
        if let Ok(mut eng) = script_engine.write() {
            if let Some(e) = eng.as_mut() {
                e.friend_execute_action(&action, None);
            }
        }
    }
}

fn evaluate_generic_script_conditions(
    script: &mut Script,
    evaluator: &ScriptEvaluator,
) -> Result<bool, String> {
    // CPP Team.cpp2515 calls evaluateConditions directly, not executeScript:
    // generic slots bypass root active/difficulty/periodic execution gates.
    let Some(or_condition) = script.condition.as_deref_mut() else {
        return Ok(false);
    };

    evaluator
        .evaluate_or_condition(or_condition)
        .map_err(|err| err.to_string())
}

fn execute_pending_team_generic_script_evals(script_evals: Vec<PendingTeamGenericScriptEval>) {
    if script_evals.is_empty() {
        return;
    }
    execute_pending_team_generic_script_evals_with_engine(script_evals, &get_script_engine());
}

fn execute_pending_team_generic_script_evals_with_engine(
    script_evals: Vec<PendingTeamGenericScriptEval>,
    script_engine: &crate::scripting::engine::ScriptEngineHandle,
) {
    if script_evals.is_empty() {
        return;
    }

    let evaluator = ScriptEvaluator::new(script_engine.clone());

    for pending in script_evals {
        let team_id = match pending.team.read() {
            Ok(team) if team.should_attempt_generic_script(pending.script_index) => team.get_id(),
            Ok(_) | Err(_) => continue,
        };
        let mut script = match pending
            .prototype
            .take_or_load_generic_script_runtime_checked(pending.script_index, script_engine)
        {
            Ok(Some(script)) => script,
            Err(GenericScriptRuntimeBusy::InFlight) => {
                log::warn!(
                    "Team generic script '{}' was reentered while its runtime was active",
                    pending.script_name
                );
                continue;
            }
            Ok(None) => {
                if let Ok(mut team_guard) = pending.team.write() {
                    team_guard.disable_generic_script_attempt(pending.script_index);
                }
                continue;
            }
        };

        let saved_context = match script_engine.write() {
            Ok(mut engine_guard) => engine_guard.as_mut().map(|engine| {
                engine.set_external_eval_context(pending.current_player_name.clone(), Some(team_id))
            }),
            Err(_) => None,
        };

        let Some(saved_context) = saved_context else {
            continue;
        };

        let eval_result = evaluate_generic_script_conditions(&mut script, &evaluator);

        if let Ok(mut engine_guard) = script_engine.write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.restore_external_eval_context(saved_context);
            }
        }

        match eval_result {
            Ok(condition_true) => {
                if condition_true {
                    // CPP Team.cpp:2519 consumes this Team's one-shot before
                    // the synchronous action can observe or revisit the slot.
                    if script.is_one_shot() {
                        if let Ok(mut team_guard) = pending.team.write() {
                            team_guard.disable_generic_script_attempt(pending.script_index);
                        }
                    }
                    if let Some(action) = script.get_action().cloned() {
                        // C++ friend_executeAction(action, this) — team-scoped.
                        if let Ok(mut eng) = script_engine.write() {
                            if let Some(e) = eng.as_mut() {
                                e.friend_execute_action(&action, Some(team_id));
                            }
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!(
                    "Team generic script '{}' evaluation failed for team '{}': {}",
                    pending.script_name,
                    pending.team_name,
                    err
                );
            }
        }
    }
}

#[cfg(test)]
mod canonical_find_order_tests {
    use super::*;

    fn team_id(team: &Arc<RwLock<Team>>) -> TeamID {
        team.read().unwrap().get_id()
    }

    #[test]
    fn find_team_uses_explicit_restored_prototype_head_not_hash_iteration() {
        let mut factory = TeamFactory::new();
        factory
            .init_team_owned("SharedName".into(), "Owner".into(), false, None, Some(7))
            .unwrap();
        // Restore through the session-owned API; each insertion becomes the
        // prototype-list head, matching TeamPrototype's prepend behavior.
        for id in [11, 17, 29] {
            factory
                .restore_owned_team_instance("SharedName", id, Some(7))
                .unwrap();
        }
        assert_eq!(
            factory
                .find_team_instances("SharedName")
                .iter()
                .map(team_id)
                .collect::<Vec<_>>(),
            [29, 17, 11]
        );

        // Form a valid explicit restored list whose head is guaranteed to
        // differ from this HashMap's first same-name value. This avoids a
        // probabilistic red control based on whichever random hash order occurs.
        let hash_first = factory
            .teams
            .values()
            .find(|team| team.read().unwrap().get_name() == "SharedName")
            .map(team_id)
            .unwrap();
        let expected_head = *factory
            .instance_order
            .iter()
            .find(|id| **id != hash_first)
            .unwrap();
        let mut explicit_restore_order = vec![expected_head];
        explicit_restore_order.extend(
            factory
                .instance_order
                .iter()
                .copied()
                .filter(|id| *id != expected_head),
        );
        factory.instance_order = explicit_restore_order;
        assert_eq!(factory.instance_order.len(), 3);
        assert_eq!(
            factory
                .instance_order
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            3
        );

        let count_before = factory.find_team_instances("SharedName").len();
        let found = factory.find_team("SharedName").unwrap();
        assert_eq!(team_id(&found), expected_head);
        assert_eq!(
            factory.find_team_instances("SharedName").len(),
            count_before
        );
    }

    #[test]
    fn owned_singleton_lookup_reuses_its_instance_and_preserves_create_effects() {
        let mut dict = Dict::new();
        dict.set_bool(key_team_executes_actions_on_create(), true);
        dict.set_ascii_string(key_team_production_condition(), "SingletonCreate");
        let mut factory = TeamFactory::new();
        let (_, init_effects) = factory
            .init_team_owned_with_effects(
                "SharedSingleton".into(),
                "Owner".into(),
                true,
                Some(&dict),
                Some(7),
            )
            .unwrap();
        assert_eq!(init_effects, ["SingletonCreate"]);
        let existing = factory.find_team_instances("SharedSingleton")[0].clone();
        assert_eq!(
            existing.read().unwrap().get_controlling_player_id(),
            Some(7)
        );

        let (reused, effects) = factory
            .create_inactive_team_with_owned_effects("SharedSingleton")
            .unwrap();
        assert!(Arc::ptr_eq(&existing, &reused));
        assert_eq!(effects, ["SingletonCreate"]);
        let found = factory.find_team("SharedSingleton").unwrap();
        assert!(Arc::ptr_eq(&existing, &found));
        assert_eq!(factory.find_team_instances("SharedSingleton").len(), 1);
        assert!(factory.drain_pending_create_action_scripts().is_empty());
    }
}
