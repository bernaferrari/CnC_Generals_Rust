// Team factory and script unit tests
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

// Note: rhai::Locked<T> is an alias for RwLock<T>, so the impl above covers both

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_team_parses_unit_and_reinforcement_fields_from_dict() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();

        dict.set_ascii_string(key_team_unit_type1(), "AmericaInfantryRanger");
        dict.set_int(key_team_unit_min_count1(), 1);
        dict.set_int(key_team_unit_max_count1(), 3);

        dict.set_ascii_string(key_team_unit_type2(), "AmericaVehicleHumvee");
        dict.set_int(key_team_unit_min_count2(), 2);
        dict.set_int(key_team_unit_max_count2(), 4);

        dict.set_int(key_team_max_instances(), 5);
        dict.set_ascii_string(key_team_transport(), "AmericaJetCargoPlane");
        dict.set_ascii_string(key_team_reinforcement_origin(), "ReinforceStart01");
        dict.set_bool(key_team_starts_full(), true);
        dict.set_bool(key_team_transports_exit(), false);

        let prototype = factory
            .init_team(
                AsciiString::from("TestTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let units = prototype.units_info();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].unit_thing_name, "AmericaInfantryRanger");
        assert_eq!(units[0].min_units, 1);
        assert_eq!(units[0].max_units, 3);
        assert_eq!(units[1].unit_thing_name, "AmericaVehicleHumvee");
        assert_eq!(units[1].min_units, 2);
        assert_eq!(units[1].max_units, 4);

        assert_eq!(prototype.get_max_instances(), 5);
        assert_eq!(
            prototype.get_transport_unit_type().as_str(),
            "AmericaJetCargoPlane"
        );
        assert_eq!(
            prototype.get_start_reinforce_waypoint().as_str(),
            "ReinforceStart01"
        );
        assert!(prototype.get_team_starts_full());
        assert!(!prototype.get_transports_exit());
    }

    #[test]
    fn create_inactive_team_requires_existing_prototype() {
        let mut factory = TeamFactory::new();
        assert!(factory.create_inactive_team("MissingTeam").is_none());
        assert!(factory.create_team("MissingTeam").is_none());
    }

    #[test]
    fn find_team_creates_missing_non_singleton_instance() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "AutoCreateTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("AutoCreateTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(factory.get_all_teams().is_empty());

        let team = factory
            .find_team("AutoCreateTeam")
            .expect("find_team should auto-create for non-singleton prototype");
        let team_name = team.read().expect("team read lock").get_name().to_string();

        assert_eq!(team_name, "AutoCreateTeam");
        assert_eq!(factory.get_all_teams().len(), 1);
    }

    #[test]
    fn create_team_marks_instance_active() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ActiveTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("ActiveTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let team = factory
            .create_team("ActiveTeam")
            .expect("create_team should create from prototype");
        assert!(team.read().expect("team read lock").is_active());
    }

    #[test]
    fn init_team_parses_and_defaults_ai_recruitable_flags() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "RecruitableTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_bool(key_team_is_ai_recruitable(), true);
        dict.set_bool(key_team_is_base_defense(), true);

        let prototype = factory
            .init_team(
                AsciiString::from("RecruitableTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(prototype.is_ai_recruitable());
        assert!(prototype.is_base_defense());

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "DefaultFlagsTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let default_prototype = factory
            .init_team(
                AsciiString::from("DefaultFlagsTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        assert!(!default_prototype.is_ai_recruitable());
        assert!(!default_prototype.is_base_defense());
    }

    #[test]
    fn init_team_parses_create_action_production_fields() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ActionTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_production_condition(), "ScriptCreateTeam");
        dict.set_bool(key_team_executes_actions_on_create(), true);

        let prototype = factory
            .init_team(
                AsciiString::from("ActionTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");
        assert_eq!(
            prototype.get_production_condition().as_str(),
            "ScriptCreateTeam"
        );
        assert!(prototype.get_execute_actions_on_create());

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "ActionDefaultsTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let default_prototype = factory
            .init_team(
                AsciiString::from("ActionDefaultsTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        assert!(default_prototype.get_production_condition().is_empty());
        assert!(!default_prototype.get_execute_actions_on_create());
    }

    #[test]
    fn evaluate_production_condition_empty_is_false() {
        let proto = TeamPrototype::new("NoCondTeam".into());
        // C++: empty productionCondition → always false thereafter.
        assert!(!proto.evaluate_production_condition());
        assert!(!proto.evaluate_production_condition());
    }

    #[test]
    fn evaluate_production_condition_missing_script_is_false() {
        let mut proto = TeamPrototype::new("MissingScriptTeam".into());
        proto.set_production_condition("DoesNotExist_ScriptXYZ".into());
        assert!(!proto.evaluate_production_condition());
    }

    #[test]
    fn flush_team_scripts_use_run_script_like_cpp() {
        let src = crate::team::TEAM_SRC;
        let i = src
            .find("pub fn flush_pending_team_script_events")
            .expect("flush");
        let w = &src[i..src.len().min(i + 900)];
        assert!(
            w.contains("run_script")
                && w.contains("Some(event.team_name.as_str())")
                && !w.contains("append_sequential_script"),
            "Team event flush must runScript(name, team) like C++ updateState"
        );
    }

    #[test]
    fn team_scripts_use_friend_execute_like_cpp() {
        let src = crate::team::TEAM_SRC;
        let create = src
            .find("fn execute_pending_team_create_action_scripts")
            .expect("create scripts");
        let create_w = &src[create..src.len().min(create + 1200)];
        assert!(
            create_w.contains("friend_execute_action")
                && create_w.contains("None")
                && !create_w.contains("ScriptEvaluator::new"),
            "createInactiveTeam path must friend_executeAction with NULL team"
        );
        let generic = src
            .find("fn execute_pending_team_generic_script_evals")
            .expect("generic scripts");
        let generic_w = &src[generic..src.len().min(generic + 3500)];
        assert!(
            generic_w.contains("friend_execute_action")
                && generic_w.contains("pending.team_name")
                && !generic_w.contains("evaluator.execute_action_sequence"),
            "updateGenericScripts path must friend_executeAction with team"
        );
    }

    #[test]
    fn create_inactive_team_queues_create_action_script() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "QueueActionTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_production_condition(), "ScriptQueueActionTeam");
        dict.set_bool(key_team_executes_actions_on_create(), true);

        let _ = factory
            .init_team(
                AsciiString::from("QueueActionTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let _ = factory
            .create_inactive_team("QueueActionTeam")
            .expect("team should be created");

        let queued = factory.drain_pending_create_action_scripts();
        assert_eq!(queued, vec!["ScriptQueueActionTeam".to_string()]);
    }

    #[test]
    fn team_priority_success_and_failure_use_template_deltas() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "PriorityTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_int(key_team_production_priority(), 10);
        dict.set_int(key_team_production_priority_success_increase(), 3);
        dict.set_int(key_team_production_priority_failure_decrease(), 2);

        let prototype = factory
            .init_team(
                AsciiString::from("PriorityTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");
        assert_eq!(prototype.get_production_priority(), 10);
        assert_eq!(prototype.get_production_priority_success_increase(), 3);
        assert_eq!(prototype.get_production_priority_failure_decrease(), 2);

        let increased = factory
            .increase_team_prototype_priority_for_success("PriorityTeam")
            .expect("prototype should exist");
        assert_eq!(increased, 13);

        let decreased = factory
            .decrease_team_prototype_priority_for_failure("PriorityTeam")
            .expect("prototype should exist");
        assert_eq!(decreased, 11);
    }

    #[test]
    fn created_team_inherits_prototype_recruitable_flag() {
        let mut factory = TeamFactory::new();

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "DefaultRecruitableTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let _ = factory
            .init_team(
                AsciiString::from("DefaultRecruitableTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        let default_team = factory
            .create_inactive_team("DefaultRecruitableTeam")
            .expect("default team should be created");
        assert!(!default_team
            .read()
            .expect("team read lock")
            .is_recruitable());

        let mut recruitable_dict = Dict::new();
        recruitable_dict.set_ascii_string(key_team_name(), "RecruitableTeamTrue");
        recruitable_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        recruitable_dict.set_bool(key_team_is_ai_recruitable(), true);
        let _ = factory
            .init_team(
                AsciiString::from("RecruitableTeamTrue"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&recruitable_dict),
            )
            .expect("recruitable prototype should be created");
        let recruitable_team = factory
            .create_inactive_team("RecruitableTeamTrue")
            .expect("recruitable team should be created");
        assert!(recruitable_team
            .read()
            .expect("team read lock")
            .is_recruitable());
    }

    #[test]
    fn init_team_parses_extended_template_behavior_and_scripts() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ExtendedTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_bool(key_team_is_perimeter_defense(), true);
        dict.set_bool(key_team_auto_reinforce(), true);
        dict.set_int(key_team_aggressiveness(), 2);
        dict.set_bool(key_team_transports_return(), true);
        dict.set_bool(key_team_avoid_threats(), true);
        dict.set_bool(key_team_attack_common_target(), true);
        dict.set_ascii_string(key_team_on_create_script(), "TeamCreateHook");
        dict.set_ascii_string(key_team_on_idle_script(), "TeamIdleHook");
        dict.set_int(key_team_initial_idle_frames(), 45);
        dict.set_ascii_string(key_team_enemy_sighted_script(), "TeamEnemySightedHook");
        dict.set_ascii_string(key_team_all_clear_script(), "TeamAllClearHook");
        dict.set_ascii_string(key_team_on_destroyed_script(), "TeamDestroyedHook");
        dict.set_real(key_team_destroyed_threshold(), 0.5);
        dict.set_ascii_string(key_team_on_unit_destroyed_script(), "TeamUnitDestroyedHook");

        let prototype = factory
            .init_team(
                AsciiString::from("ExtendedTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(prototype.is_perimeter_defense());
        assert!(prototype.automatically_reinforce());
        assert_eq!(
            prototype.get_initial_team_attitude(),
            AttitudeType::Aggressive
        );
        assert!(prototype.transports_return());
        assert!(prototype.avoid_threats());
        assert!(prototype.attack_common_target());
        assert_eq!(prototype.get_script_on_create().as_str(), "TeamCreateHook");
        assert_eq!(prototype.get_script_on_idle().as_str(), "TeamIdleHook");
        assert_eq!(prototype.get_initial_idle_frames(), 45);
        assert_eq!(
            prototype.get_script_on_enemy_sighted().as_str(),
            "TeamEnemySightedHook"
        );
        assert_eq!(
            prototype.get_script_on_all_clear().as_str(),
            "TeamAllClearHook"
        );
        assert_eq!(
            prototype.get_script_on_destroyed().as_str(),
            "TeamDestroyedHook"
        );
        assert!((prototype.get_destroyed_threshold() - 0.5).abs() < f32::EPSILON);
        assert_eq!(
            prototype.get_script_on_unit_destroyed().as_str(),
            "TeamUnitDestroyedHook"
        );
    }

    #[test]
    fn attitude_from_ini_matches_cpp_values() {
        assert_eq!(AttitudeType::from_ini(-2), AttitudeType::Sleep);
        assert_eq!(AttitudeType::from_ini(-1), AttitudeType::Passive);
        assert_eq!(AttitudeType::from_ini(0), AttitudeType::Normal);
        assert_eq!(AttitudeType::from_ini(1), AttitudeType::Alert);
        assert_eq!(AttitudeType::from_ini(2), AttitudeType::Aggressive);
        assert_eq!(AttitudeType::from_ini(3), AttitudeType::Invalid);
        assert_eq!(AttitudeType::from_ini(99), AttitudeType::Normal);
    }

    #[test]
    fn team_state_and_death_queue_team_script_events() {
        let _ = drain_pending_team_script_events();

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ScriptHookTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_create_script(), "OnCreateTeamScript");
        dict.set_ascii_string(
            key_team_on_unit_destroyed_script(),
            "OnUnitDestroyedTeamScript",
        );

        let _ = factory
            .init_team(
                AsciiString::from("ScriptHookTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let team = factory
            .create_team("ScriptHookTeam")
            .expect("team should be created");

        {
            let mut guard = team.write().expect("team write lock");
            guard.update_state();
            guard.notify_team_of_object_death();
        }

        let queued = drain_pending_team_script_events();
        assert_eq!(queued.len(), 2);
        assert_eq!(queued[0].team_name, "ScriptHookTeam");
        assert_eq!(queued[0].script_name, "OnCreateTeamScript");
        assert_eq!(queued[1].team_name, "ScriptHookTeam");
        assert_eq!(queued[1].script_name, "OnUnitDestroyedTeamScript");
    }

    #[test]
    fn update_state_queues_on_create_when_dual_world_registry_is_empty() {
        let _ = drain_pending_team_script_events();
        assert!(
            crate::object::registry::OBJECT_REGISTRY.is_empty(),
            "this test names the empty-registry production path"
        );

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "EmptyRegistryTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_create_script(), "OnCreateEmptyRegistry");
        dict.set_ascii_string(key_team_on_idle_script(), "OnIdleEmptyRegistry");

        let _ = factory
            .init_team(
                AsciiString::from("EmptyRegistryTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let team = factory
            .create_team("EmptyRegistryTeam")
            .expect("team should be created");

        {
            let mut guard = team.write().expect("team write lock");
            guard.update_state();
        }

        let queued = drain_pending_team_script_events();
        assert!(
            queued
                .iter()
                .any(|event| event.script_name == "OnCreateEmptyRegistry"),
            "empty dual-world registry must not skip Team::updateState onCreate"
        );
    }

    fn host_hook_object(
        id: u32,
        team: u32,
        alive: bool,
        idle: bool,
        x: f32,
        z: f32,
        vision: f32,
    ) -> crate::scripting::HostScriptQueryObject {
        crate::scripting::HostScriptQueryObject {
            id,
            team,
            x,
            z,
            alive,
            effectively_dead: !alive,
            idle,
            vision_range: vision,
            ..Default::default()
        }
    }

    #[test]
    fn host_census_update_state_matches_cpp_team_hooks() {
        let _ = drain_pending_team_script_events();
        crate::scripting::clear_host_script_query_snapshot();
        assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "HostCensusTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_create_script(), "OnCreateHostCensus");
        dict.set_ascii_string(key_team_on_destroyed_script(), "OnDestroyedHostCensus");
        dict.set_real(key_team_destroyed_threshold(), 1.0);
        dict.set_ascii_string(key_team_on_idle_script(), "OnIdleHostCensus");
        dict.set_ascii_string(key_team_enemy_sighted_script(), "OnEnemySightedHostCensus");

        let _ = factory
            .init_team(
                AsciiString::from("HostCensusTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype");
        let team = factory
            .create_team("HostCensusTeam")
            .expect("team");

        crate::scripting::set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![
                    host_hook_object(11, 1, true, true, 0.0, 0.0, 50.0),
                    host_hook_object(12, 1, true, true, 1.0, 0.0, 50.0),
                    host_hook_object(99, 0, true, false, 5.0, 0.0, 50.0),
                ],
                team_instance_ids: [("HostCensusTeam".into(), vec![11, 12])]
                    .into_iter()
                    .collect(),
                team_ids: [
                    (1u32, vec![11, 12]),
                    (0u32, vec![99]),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            },
        );

        {
            let mut guard = team.write().expect("write");
            guard.update_state();
        }
        let first = drain_pending_team_script_events();
        assert_eq!(
            first
                .iter()
                .filter(|e| e.script_name == "OnCreateHostCensus")
                .count(),
            1
        );
        assert!(
            !first
                .iter()
                .any(|e| e.script_name == "OnDestroyedHostCensus"),
            "OnDestroyed must not fire at activation while host members are alive"
        );
        assert!(
            first
                .iter()
                .any(|e| e.script_name == "OnEnemySightedHostCensus"),
            "OnEnemySighted must fire from host snapshot vision"
        );
        assert!(
            !first
                .iter()
                .any(|e| e.script_name == "OnIdleHostCensus"),
            "OnIdle needs two consecutive idle frames"
        );

        {
            let mut guard = team.write().expect("write");
            guard.update_state();
        }
        let second = drain_pending_team_script_events();
        assert!(
            !second
                .iter()
                .any(|e| e.script_name == "OnCreateHostCensus"),
            "OnCreate must run only once"
        );
        assert!(
            second
                .iter()
                .any(|e| e.script_name == "OnIdleHostCensus"),
            "OnIdle fires on the second consecutive idle frame"
        );

        crate::scripting::set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![
                    host_hook_object(11, 1, false, true, 0.0, 0.0, 50.0),
                    host_hook_object(12, 1, false, true, 1.0, 0.0, 50.0),
                    host_hook_object(99, 0, true, false, 5.0, 0.0, 50.0),
                ],
                team_instance_ids: [("HostCensusTeam".into(), vec![11, 12])]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        );
        {
            let mut guard = team.write().expect("write");
            guard.update_state();
        }
        let third = drain_pending_team_script_events();
        assert!(
            third
                .iter()
                .any(|e| e.script_name == "OnDestroyedHostCensus"),
            "OnDestroyed fires when the last live host member dies"
        );

        crate::scripting::clear_host_script_query_snapshot();
    }

    #[test]
    fn empty_registry_without_host_snapshot_does_not_fire_on_destroyed() {
        let _ = drain_pending_team_script_events();
        crate::scripting::clear_host_script_query_snapshot();
        assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "NoSnapDestroyed");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_destroyed_script(), "OnDestroyedNoSnap");

        let _ = factory
            .init_team(
                AsciiString::from("NoSnapDestroyed"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype");
        let team = factory
            .create_team("NoSnapDestroyed")
            .expect("team");
        {
            let mut guard = team.write().expect("write");
            guard.add_member(7);
            guard.add_member(8);
            guard.update_state();
        }
        let queued = drain_pending_team_script_events();
        assert!(
            !queued
                .iter()
                .any(|e| e.script_name == "OnDestroyedNoSnap"),
            "empty registry without host snapshot must not treat members as dead"
        );
    }


    #[test]
    fn update_removes_empty_active_non_singleton_teams() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "TempTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("TempTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let _team = factory
            .create_team("TempTeam")
            .expect("team should be created");
        assert_eq!(factory.get_all_teams().len(), 1);

        factory.update();
        assert!(factory.get_all_teams().is_empty());
    }

    #[test]
    fn init_team_parses_generic_script_hook_slots() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "GenericHookTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let base =
            NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap_or_default();
        let hook0 = NameKeyGenerator::name_to_key(&format!("{}0", base));
        let hook3 = NameKeyGenerator::name_to_key(&format!("{}3", base));
        dict.set_ascii_string(hook0, "GenericHookScript0");
        dict.set_ascii_string(hook3, "GenericHookScript3");

        let prototype = factory
            .init_team(
                AsciiString::from("GenericHookTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert_eq!(
            prototype
                .get_generic_script(0)
                .map(|s| s.as_str())
                .unwrap_or_default(),
            "GenericHookScript0"
        );
        assert_eq!(
            prototype
                .get_generic_script(3)
                .map(|s| s.as_str())
                .unwrap_or_default(),
            "GenericHookScript3"
        );
        assert!(prototype
            .get_generic_script(1)
            .map(|s| s.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn generic_script_eval_disables_slot_when_script_missing() {
        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("GenericEvalTeam"),
            1,
        )));
        let mut prototype = TeamPrototype::new(AsciiString::from("GenericEvalTeam"));
        prototype.set_generic_script(0, AsciiString::from("DefinitelyMissingScript"));
        let prototype = Arc::new(prototype);

        assert!(team
            .read()
            .expect("team read lock")
            .should_attempt_generic_script(0));

        execute_pending_team_generic_script_evals(vec![PendingTeamGenericScriptEval {
            team: team.clone(),
            prototype,
            team_name: "GenericEvalTeam".to_string(),
            script_name: "DefinitelyMissingScript".to_string(),
            script_index: 0,
            current_player_name: None,
        }]);

        assert!(!team
            .read()
            .expect("team read lock")
            .should_attempt_generic_script(0));
    }

    #[test]
    fn set_team_target_object_requires_ai_controller() {
        let mut team = Team::new(AsciiString::from("TargetTeam"), 1);
        team.set_team_target_object(42);
        assert_eq!(team.get_team_target_object(), INVALID_ID);
    }

    #[test]
    fn get_team_target_object_rejects_missing_object() {
        let mut team = Team::new(AsciiString::from("TargetTeam"), 1);
        team.common_attack_target.store(999_999, Ordering::Relaxed);
        assert_eq!(team.get_team_target_object(), INVALID_ID);
        assert_eq!(
            team.common_attack_target.load(Ordering::Relaxed),
            INVALID_ID
        );
    }

    #[test]
    fn get_targetable_count_ignores_missing_member_entries() {
        let mut team = Team::new(AsciiString::from("TargetableTeam"), 1);
        team.add_member(111_111);
        team.add_member(222_222);
        assert_eq!(team.get_targetable_count(), 0);
    }

    #[test]
    fn set_override_team_relationship_ignores_invalid_id() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_team_relationship(TEAM_ID_INVALID, Relationship::Enemies);
        assert!(team.team_relations.is_none());
    }

    #[test]
    fn remove_override_team_relationship_invalid_id_clears_overrides() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_team_relationship(2, Relationship::Enemies);
        team.set_override_team_relationship(3, Relationship::Allies);
        assert!(team.remove_override_team_relationship(TEAM_ID_INVALID));
        assert!(team
            .team_relations
            .as_ref()
            .map(|m| m.map.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn remove_override_player_relationship_invalid_id_clears_overrides() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_player_relationship(0, Relationship::Enemies);
        team.set_override_player_relationship(1, Relationship::Allies);
        assert!(team.remove_override_player_relationship(crate::player::PLAYER_INDEX_INVALID));
        assert!(team
            .player_relations
            .as_ref()
            .map(|m| m.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn delete_team_does_not_deactivate_team() {
        let mut team = Team::new(AsciiString::from("DeleteTeam"), 1);
        team.set_active();
        assert!(team.is_active());
        team.delete_team(false);
        assert!(team.is_active());
    }

    #[test]
    fn team_area_events_remain_visible_on_next_script_frame() {
        // C++ Object::didEnterOrExit (Object.cpp:2467-2479) is true on now and now-1.
        use crate::common::ICoord3D;
        use crate::object::Object;
        use crate::polygon_trigger::PolygonTrigger;
        use crate::system::game_logic::get_game_logic;
        use crate::terrain::get_terrain_logic;

        let _lock = crate::test_sync::lock();
        let area_name = "TeamAreaPreviousFrameParity";
        let object_id = 0x00FE_DCBA;
        let trigger = PolygonTrigger::new(
            91,
            AsciiString::from(area_name),
            vec![
                ICoord3D::new(0, 0, 0),
                ICoord3D::new(10, 0, 0),
                ICoord3D::new(10, 10, 0),
                ICoord3D::new(0, 10, 0),
            ],
        );
        get_terrain_logic()
            .write()
            .expect("terrain")
            .add_trigger_area(trigger.clone());

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(41);

        let obj = Object::new_test(object_id, 100.0);
        let object_arc = Arc::new(RwLock::new(obj));
        OBJECT_REGISTRY.register_object(object_id, &object_arc);
        {
            let mut guard = object_arc.write().expect("object write");
            guard
                .set_position(&Coord3D::new(5.0, 5.0, 0.0))
                .expect("enter area");
        }

        assert!(Team::object_did_enter(object_id, &trigger));

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(42);
        assert!(Team::object_did_enter(object_id, &trigger));

        {
            let mut guard = object_arc.write().expect("object write");
            guard
                .set_position(&Coord3D::new(20.0, 20.0, 0.0))
                .expect("leave area");
        }

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(43);
        {
            let mut guard = object_arc.write().expect("object write");
            guard
                .set_position(&Coord3D::new(21.0, 21.0, 0.0))
                .expect("confirm exit");
        }
        assert!(Team::object_did_exit(object_id, &trigger));

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(44);
        assert!(Team::object_did_exit(object_id, &trigger));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn init_team_resolves_team_home_waypoint() {
        // C++ Team.cpp:669-679 last name match wins. load_map_data prepends, so
        // the first vec entry is the tail of getFirstWaypoint / getNext.
        let mut map_data = crate::system::map_loader::MapData::new();
        map_data.width = 2;
        map_data.height = 2;
        map_data.heightmap = vec![0; 4];
        map_data.waypoints = vec![
            crate::system::map_loader::MapWaypoint {
                id: 7701,
                name: "TeamHomeOriginA".to_string(),
                location: crate::system::map_loader::Coord3D::new(111.0, 222.0, 9.0),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            },
            crate::system::map_loader::MapWaypoint {
                id: 7702,
                name: "TeamHomeOriginA".to_string(),
                location: crate::system::map_loader::Coord3D::new(333.0, 444.0, 9.0),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            },
        ];
        crate::terrain::get_terrain_logic()
            .write()
            .expect("terrain write")
            .load_map_data(map_data);

        let expected = crate::terrain::get_terrain_logic()
            .read()
            .expect("terrain read")
            .get_first_waypoint()
            .and_then(|way| way.get_next())
            .map(|way| *way.get_location())
            .expect("duplicate teamHome tail waypoint");

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_home(), "TeamHomeOriginA");
        let prototype = factory
            .init_team(
                AsciiString::from("HomeTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");
        assert!(prototype.has_home_location());
        assert_eq!(prototype.home_location(), expected);

        let mut missing = Dict::new();
        missing.set_ascii_string(key_team_home(), "NoSuchTeamHome");
        let missing_proto = factory
            .init_team(
                AsciiString::from("NoHomeTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&missing),
            )
            .expect("missing-home prototype should be created");
        assert!(!missing_proto.has_home_location());

        crate::terrain::get_terrain_logic()
            .write()
            .expect("terrain reset")
            .reset();
    }

    #[test]
    fn init_team_links_prototype_onto_owning_player_list() {
        let owner = "HqFslr2Owner";
        let key = NameKeyGenerator::name_to_key(owner);
        let player_arc = {
            let mut player = crate::player::Player::new(91);
            player.set_player_name_key(key);
            Arc::new(RwLock::new(player))
        };
        {
            let Ok(mut list) = player_list().write() else {
                return;
            };
            list.add_player(Arc::clone(&player_arc));
        }

        let mut factory = TeamFactory::new();
        let proto = factory
            .init_team(
                AsciiString::from("HqFslr2AttackTeam"),
                AsciiString::from(owner),
                false,
                None,
            )
            .expect("prototype");
        {
            let player = player_arc.read().expect("player");
            assert!(
                player
                    .get_player_team_prototypes()
                    .iter()
                    .any(|existing| Arc::ptr_eq(existing, &proto)),
                "init_team must add the prototype to the owning player list"
            );
        }
        factory.reset();
        let player = player_arc.read().expect("player");
        assert!(
            !player
                .get_player_team_prototypes()
                .iter()
                .any(|existing| Arc::ptr_eq(existing, &proto)),
            "factory reset must unlink the prototype from the owning player"
        );
    }

    #[test]
    fn restore_save_script_state_keeps_team_state_string() {
        let mut team = Team::new(AsciiString::from("StateTeam"), 1);
        team.set_state(AsciiString::from("Attacking"));
        team.restore_save_script_state(
            false,
            true,
            false,
            false,
            false,
            0,
            0,
            None,
            &[true; MAX_GENERIC_SCRIPTS],
            false,
            false,
            "Retreating",
        );
        assert_eq!(team.get_state().as_str(), "Retreating");
        assert!(team.is_active());
        assert!(!team.is_created());
    }

    #[test]
    fn team_relation_override_pairs_round_trip_team_and_player_maps() {
        let mut team = Team::new(AsciiString::from("RelTeam"), 1);
        team.set_override_team_relationship(8, Relationship::Neutral);
        team.set_override_player_relationship(2, Relationship::Allies);
        assert_eq!(
            team.team_relation_override_pairs(),
            vec![(8, Relationship::Neutral)]
        );
        assert_eq!(
            team.player_relation_override_pairs(),
            vec![(2, Relationship::Allies)]
        );
    }

}
