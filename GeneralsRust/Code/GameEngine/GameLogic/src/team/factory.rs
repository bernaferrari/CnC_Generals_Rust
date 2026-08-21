// TeamFactory prototype/instance management
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team factory for managing team prototypes and instances (matching C++ TeamFactory)
#[derive(Debug)]
pub struct TeamFactory {
    prototypes: HashMap<String, Arc<TeamPrototype>>,
    teams: HashMap<TeamID, Arc<RwLock<Team>>>,
    unique_team_prototype_id: TeamPrototypeID,
    unique_team_id: TeamID,
    pending_create_action_scripts: Vec<String>,
    pending_generic_script_evals: Vec<PendingTeamGenericScriptEval>,
}

impl TeamFactory {
    /// Create new team factory
    pub fn new() -> Self {
        game_engine::common::rts::team::set_team_home_waypoint_resolver(
            leftover_resolve_team_home_waypoint,
        );
        Self {
            prototypes: HashMap::new(),
            teams: HashMap::new(),
            unique_team_prototype_id: 1,
            unique_team_id: 1,
            pending_create_action_scripts: Vec::new(),
            pending_generic_script_evals: Vec::new(),
        }
    }

    /// Initialize team factory
    pub fn init(&mut self) {
        self.unlink_prototypes_from_owning_players();
        self.prototypes.clear();
        self.teams.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
    }

    /// Reset team factory
    pub fn reset(&mut self) {
        self.unlink_prototypes_from_owning_players();
        self.prototypes.clear();
        self.teams.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
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

    /// Initialize a team from configuration
    pub fn init_team(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
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
        self.prototypes.insert(name.to_string(), prototype.clone());
        // C++ TeamPrototype ctor: owningPlayer->addTeamToList(this).
        self.bind_prototype_to_owning_player(&prototype);
        if is_singleton {
            let _ = self.create_inactive_team(name.as_str());
        }
        Some(prototype)
    }

    /// Find team prototype by name
    pub fn find_team_prototype(&self, name: &str) -> Option<Arc<TeamPrototype>> {
        self.prototypes.get(name).cloned()
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
        self.prototypes.values().cloned().collect()
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
        let owner_name = prototype.get_owner_name().to_string();
        let owner_player = player_list().read().ok().and_then(|list| {
            if owner_name.is_empty() {
                None
            } else {
                list.find_player_by_name(&owner_name)
            }
            .or_else(|| list.get_neutral_player())
        });
        if let Some(owner_player) = owner_player {
            if let (Ok(owner_guard), Ok(mut team_guard)) = (owner_player.read(), team.write()) {
                team_guard.set_controlling_player_id(Some(owner_guard.get_player_index() as u32));
            }
        }
        self.teams.insert(team_id, team.clone());
        if team_id >= self.unique_team_id {
            self.unique_team_id = team_id.saturating_add(1);
        }
        Some(team)
    }

    /// Replace a prototype after xfer mutates template fields stored by value.
    pub fn replace_team_prototype(&mut self, prototype: TeamPrototype) {
        let name = prototype.get_name().to_string();
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
        for team in self.teams.values() {
            if let Ok(team_ref) = team.read() {
                if team_ref.get_name() == name {
                    return Some(team.clone());
                }
            }
        }
        None
    }

    fn queue_create_actions_for_prototype(&mut self, prototype: &TeamPrototype) {
        if !prototype.get_execute_actions_on_create() {
            return;
        }
        let production_condition = prototype.get_production_condition();
        if production_condition.is_empty() {
            return;
        }
        self.pending_create_action_scripts
            .push(production_condition.to_string());
    }

    /// Create team from prototype name
    pub fn create_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let team = self.create_inactive_team(name)?;
        team.write().ok()?.set_active();
        Some(team)
    }

    /// Create inactive team
    pub fn create_inactive_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let prototype = self.find_team_prototype(name);
        if prototype.is_none() {
            return None;
        }
        if prototype.as_ref().is_some_and(|p| p.is_singleton()) {
            if let Some(existing) = self.find_existing_team_by_name(name) {
                if let Some(prototype) = prototype.as_deref() {
                    self.queue_create_actions_for_prototype(prototype);
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
            let owner_name = prototype.get_owner_name().to_string();
            let owner_player = player_list().read().ok().and_then(|list| {
                if owner_name.is_empty() {
                    None
                } else {
                    list.find_player_by_name(&owner_name)
                }
                .or_else(|| list.get_neutral_player())
            });
            if let Some(owner_player) = owner_player {
                if let (Ok(owner_guard), Ok(mut team_guard)) = (owner_player.read(), team.write()) {
                    team_guard
                        .set_controlling_player_id(Some(owner_guard.get_player_index() as u32));
                }
            }
        }

        self.teams.insert(team_id, team.clone());
        if let Some(prototype) = prototype.as_deref() {
            self.queue_create_actions_for_prototype(prototype);
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

    /// Find all team instances that were created from the same prototype name.
    ///
    /// C++ Reference: `TeamPrototype::iterate_TeamInstanceList()` used by
    /// `ScriptEngine::executeScript()` when `conditionTeamName` is set.
    pub fn find_team_instances(&self, prototype_name: &str) -> Vec<Arc<RwLock<Team>>> {
        self.teams
            .values()
            .filter_map(|team| {
                let guard = team.read().ok()?;
                (guard.get_name() == prototype_name).then_some(team.clone())
            })
            .collect()
    }

    /// Return all live team instances.
    pub fn get_all_teams(&self) -> Vec<Arc<RwLock<Team>>> {
        self.teams.values().cloned().collect()
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
                    engine.notify_of_team_destruction(name);
                }
            }
        }
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

        self.teams.remove(&team_id);
    }

    /// C++ TeamPrototype ctor/dtor + initTeam owner lookup (Team.cpp:216-223, 799-800).
    fn bind_prototype_to_owning_player(&self, prototype: &Arc<TeamPrototype>) {
        let Some(player) = self.resolve_owning_player(prototype.get_owner_name().as_str()) else {
            return;
        };
        if let Ok(mut player_guard) = player.write() {
            player_guard.add_team_to_list(Arc::clone(prototype));
        }
    }

    fn unlink_prototypes_from_owning_players(&self) {
        for prototype in self.prototypes.values() {
            let Some(player) = self.resolve_owning_player(prototype.get_owner_name().as_str())
            else {
                continue;
            };
            if let Ok(mut player_guard) = player.write() {
                player_guard.remove_team_from_list(prototype);
            }
        }
    }

    fn resolve_owning_player(&self, owner_name: &str) -> Option<Arc<RwLock<crate::player::Player>>> {
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
}

/// C++ TeamTemplateInfo (Team.cpp:669-679): walk `getFirstWaypoint` / `getNext`.
/// Last matching name wins. Empty names are still searched if the key exists.
pub(super) fn resolve_team_home_waypoint_location(name: &str) -> Option<Coord3D> {
    let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
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
    resolve_team_home_waypoint_location(name).map(|loc| {
        game_engine::common::system::geometry::Coord3D::new(loc.x, loc.y, loc.z)
    })
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
    current_player_name: Option<&str>,
) -> Result<bool, String> {
    if !script.is_active() {
        return Ok(false);
    }

    let difficulty = current_player_name
        .and_then(|player_name| {
            player_list()
                .read()
                .ok()
                .and_then(|list| list.find_player_by_name(player_name))
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|player| player.get_player_difficulty())
                })
        })
        .unwrap_or(crate::player::GameDifficulty::Normal);

    match difficulty {
        crate::player::GameDifficulty::Easy if !script.easy => return Ok(false),
        crate::player::GameDifficulty::Normal if !script.normal => return Ok(false),
        crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal
            if !script.hard =>
        {
            return Ok(false);
        }
        _ => {}
    }

    let current_frame = crate::helpers::TheGameLogic::get_frame();
    if current_frame < script.frame_to_evaluate_at {
        return Ok(false);
    }

    if script.delay_evaluation_seconds > 0 {
        script.frame_to_evaluate_at = current_frame
            + (script.delay_evaluation_seconds as u32) * (LOGICFRAMES_PER_SECOND as u32);
    }

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

    let script_engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(script_engine.clone());

    for pending in script_evals {
        let Some(mut script) = pending
            .prototype
            .take_or_load_generic_script_runtime(pending.script_index)
        else {
            if let Ok(mut team_guard) = pending.team.write() {
                team_guard.disable_generic_script_attempt(pending.script_index);
            }
            continue;
        };

        let saved_context = match script_engine.write() {
            Ok(mut engine_guard) => engine_guard.as_mut().map(|engine| {
                engine.set_external_eval_context(
                    pending.current_player_name.clone(),
                    Some(pending.team_name.clone()),
                )
            }),
            Err(_) => None,
        };

        let Some(saved_context) = saved_context else {
            pending
                .prototype
                .store_generic_script_runtime(pending.script_index, Some(script));
            continue;
        };

        let eval_result = evaluate_generic_script_conditions(
            &mut script,
            &evaluator,
            pending.current_player_name.as_deref(),
        );

        if let Ok(mut engine_guard) = script_engine.write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.restore_external_eval_context(saved_context);
            }
        }

        match eval_result {
            Ok(condition_true) => {
                if condition_true {
                    if let Some(action) = script.get_action().cloned() {
                        // C++ friend_executeAction(action, this) — team-scoped.
                        if let Ok(mut eng) = script_engine.write() {
                            if let Some(e) = eng.as_mut() {
                                e.friend_execute_action(&action, Some(pending.team_name.as_str()));
                            }
                        }
                    }

                    if script.is_one_shot() {
                        if let Ok(mut team_guard) = pending.team.write() {
                            team_guard.disable_generic_script_attempt(pending.script_index);
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

        pending
            .prototype
            .store_generic_script_runtime(pending.script_index, Some(script));
    }
}

