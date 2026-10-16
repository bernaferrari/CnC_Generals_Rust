//! AIPlayer.cpp:2516-2614: inactive creation precedes the live recruit scan.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;

impl GameLogic {
    pub(crate) fn recruit_owned_specific_ai_team_with_ui(
        &mut self,
        player: u32,
        name: &str,
        radius: f32,
        interpreter: Option<&ScriptEngine>,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> bool {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return false;
        };
        let factory = ai.team_factory.clone();
        let admitted = {
            let definitions = factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(prototype) = definitions.find_team_prototype(name) else {
                return false;
            };
            if prototype.is_singleton()
                && definitions
                    .find_team_instances(name)
                    .first()
                    .is_some_and(|team| {
                        team.read()
                            .unwrap_or_else(|e| e.into_inner())
                            .has_any_objects()
                    })
            {
                return false;
            }
            definitions.find_team_prototype(name).is_some_and(|p| {
                AIManager::resolve_player_id(self, p.get_owner_name().as_str()) == Some(player)
            })
        };
        if !admitted {
            return false;
        }
        // Original creates even with no matching units. No AI/factory borrow
        // survives its null-team action, which may modify either owner.
        let Some((created, effects)) = factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .create_inactive_team_with_owned_effects(name)
        else {
            return false;
        };
        let identity = created.read().unwrap_or_else(|e| e.into_inner()).get_id();
        for effect in effects {
            if let Some(engine) = interpreter {
                self.execute_owned_ai_condition_with_ui(engine, &effect, None, ui);
            } else {
                self.with_owned_script_engine(|engine, world| {
                    world.execute_owned_ai_condition_with_ui(engine, &effect, None, ui)
                });
            }
        }
        let prototype = {
            let definitions = factory.lock().unwrap_or_else(|e| e.into_inner());
            if !definitions
                .find_team_by_id(identity)
                .is_some_and(|current| std::sync::Arc::ptr_eq(&current, &created))
            {
                return false;
            }
            let Some(prototype) = definitions.find_team_prototype(name) else {
                return false;
            };
            if AIManager::resolve_player_id(self, prototype.get_owner_name().as_str())
                != Some(player)
            {
                return false;
            }
            prototype
        };
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return false;
        };
        if !ai
            .team_factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .find_team_by_id(identity)
            .is_some_and(|current| std::sync::Arc::ptr_eq(&current, &created))
        {
            return false;
        }
        // CPP warns for an unauthored home but still uses the raw prototype
        // location; it never substitutes this AI's base center here.
        let location = prototype.home_location();
        let home = Vec3::new(location.x, location.z, location.y);
        let radius = if radius < 1. { 99_999. } else { radius };
        let mut recruited = 0u32;
        for unit in prototype.units_info() {
            if unit.unit_thing_name.is_empty() || !self.templates.contains_key(unit.unit_thing_name)
            {
                continue;
            }
            for _ in 0..unit.max_units.max(0) {
                let selected = self.ai_manager.ai_players.get(&player).and_then(|ai| {
                    let data = AiDataView::from_world(self);
                    ai.try_to_recruit(self, name, &unit.unit_thing_name, home, Some(radius), &data)
                });
                let Some(id) = selected else {
                    break;
                };
                let previous = self
                    .host_object(id)
                    .and_then(|object| object.team_instance_id);
                if let Some(previous) = previous {
                    if let Some(source) = factory
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .find_team_by_id(previous)
                    {
                        source
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove_member(id.0);
                    }
                }
                let Some(object) = self.host_object_mut(id) else {
                    continue;
                };
                object.team_instance_name = name.to_owned();
                object.apply_named_team_ai_profile(&factory, true);
                object.team_instance_id = Some(identity);
                created
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .add_member(id.0);
                recruited += 1;
                let _ = self.unit_command_ai_move_to(id, home);
            }
        }
        if recruited == 0 {
            if !prototype.is_singleton() {
                if let Some(engine) = interpreter {
                    self.delete_canonical_ai_team_with_interpreter(identity, engine);
                } else {
                    self.delete_canonical_ai_team(identity);
                }
            }
            return false;
        }
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return false;
        };
        // C++ prepends a ready node with NULL workOrders and the current frame.
        // Membership is the actual recruited roster, not another observed list.
        let mut ready = AITeamQueue::new(name.to_owned(), Vec::new(), false, self.frame);
        ready.team_id = Some(identity);
        ready.visit_token = ai.next_queue_visit_token;
        ai.next_queue_visit_token = ai
            .next_queue_visit_token
            .checked_add(1)
            .expect("AI queue node token exhaustion");
        ai.team_ready_queue.push_front(ready);
        ai.activity_count = ai.activity_count.saturating_add(1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    #[derive(Default)]
    struct Ui(Vec<bool>);
    impl crate::game_logic::ScriptUiEffects for Ui {
        fn weather_visible(&mut self, _visible: bool) {}
        fn letterbox(
            &mut self,
            _: &mut crate::presentation_frame::camera::SessionCamera,
            enabled: bool,
            _: bool,
        ) {
            self.0.push(enabled);
        }
    }
    fn action(kind: ScriptActionType, params: Vec<Parameter>) -> Box<ScriptAction> {
        let mut action = ScriptAction::new(kind);
        for p in params {
            action.add_parameter(p).unwrap();
        }
        Box::new(action)
    }
    fn recruit(name: &str) -> Box<ScriptAction> {
        action(
            ScriptActionType::RecruitTeam,
            vec![
                Parameter::with_string(ParameterType::Team, name.into()),
                Parameter::with_real(ParameterType::Real, 500.),
            ],
        )
    }
    fn prototype(world: &mut GameLogic, name: &str, condition: &str) {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(name.into(), "RecruitOwner".into(), false, None, Some(1))
            .unwrap();
        let mut proto = (*factory.find_team_prototype(name).unwrap()).clone();
        // Recruitment fixtures author the receiving definition directly.
        // Common name-key dictionary admission is a separate tested boundary.
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 2,
                max_units: 2,
                unit_thing_name: "RecruitInfantry",
            },
        );
        proto.set_production_priority(1000);
        proto.set_production_condition(condition.into());
        proto.set_execute_actions_on_create(!condition.is_empty());
        factory.replace_team_prototype(proto);
        let admitted = factory.find_team_prototype(name).unwrap();
        assert_eq!(admitted.units_info().len(), 1);
        assert_eq!(admitted.units_info()[0].unit_thing_name, "RecruitInfantry");
        assert_eq!(admitted.units_info()[0].min_units, 2);
        assert_eq!(admitted.units_info()[0].max_units, 2);
        assert_eq!(admitted.get_owner_name().as_str(), "RecruitOwner");
        assert_eq!(admitted.get_production_priority(), 1000);
    }
    fn fixture() -> (GameLogic, ObjectId) {
        fixture_with_ai(true)
    }
    fn fixture_with_ai(has_ai: bool) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "RecruitOwner", false));
        world
            .ai_manager
            .add_ai_player(1, Team::USA, AIDifficulty::Medium);
        let mut template = ThingTemplate::new("RecruitInfantry");
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.);
        template.set_authored_ai_update_interface(Some(has_ai));
        world.templates.insert(template.name.clone(), template);
        let object = world
            .create_object_for_player("RecruitInfantry", 1, Vec3::ZERO)
            .unwrap();
        prototype(&mut world, "MainRecruit", "MainCallback");
        prototype(&mut world, "NestedRecruit", "NestedCallback");
        (world, object)
    }
    fn engine() -> ScriptEngine {
        let mut list = ScriptList::new();
        for (name, first) in [
            ("Recruit", recruit("MainRecruit")),
            ("MainCallback", {
                let mut a = recruit("NestedRecruit");
                a.next_action = Some(action(ScriptActionType::CameraLetterboxBegin, vec![]));
                a
            }),
            (
                "NestedCallback",
                action(ScriptActionType::CameraLetterboxEnd, vec![]),
            ),
        ] {
            let mut script = Script::new();
            script.script_name = name.into();
            script.is_subroutine = true;
            script.is_one_shot = false;
            let mut condition = OrCondition::new();
            condition.set_first_and_condition(Some(Box::new(Condition::new(
                ConditionType::ConditionTrue,
            ))));
            script.condition = Some(Box::new(condition));
            script.action = Some(first);
            list.append_script(Box::new(script));
        }
        let engine = ScriptEngine::new().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine
    }
    #[test]
    fn zero_candidates_still_run_nested_creation_with_borrowed_interpreter_and_ui() {
        let (mut world, object) = fixture();
        world.host_object_mut(object).unwrap().is_recruitable = false;
        world.install_owned_script_engine(engine());
        let mut ui = Ui::default();
        world
            .with_owned_script_engine(|engine, world| {
                assert!(world.owned_script_engine.is_none());
                world.run_object_script_with_ui(engine, "Recruit", object, &mut Some(&mut ui));
                assert!(world.ai_manager.ai_players.contains_key(&1));
            })
            .unwrap();
        assert_eq!(
            ui.0,
            [false, true],
            "nested recruit runs while its AI and the parent interpreter remain available"
        );
        assert!(world.owned_script_engine.is_some());
        assert!(world.ai_manager.ai_players[&1].team_ready_queue.is_empty());
        let factory = world.team_factory.lock().unwrap();
        assert!(factory.find_team_instances("MainRecruit").is_empty());
        assert!(factory.find_team_instances("NestedRecruit").is_empty());
    }
    #[test]
    fn receiving_recruit_moves_actual_roster_and_prepends_current_frame_node() {
        let (mut world, object) = fixture();
        let (peer, peer_object) = fixture();
        assert_eq!(object, peer_object);
        // Disable the nested recruit so this test isolates actual roster transfer.
        prototype(&mut world, "MainRecruit", "");
        {
            let mut factory = world.team_factory.lock().unwrap();
            let mut proto = (*factory.find_team_prototype("MainRecruit").unwrap()).clone();
            proto.set_home_location(gamelogic::common::Coord3D {
                x: 1.,
                y: 2.,
                z: 0.,
            });
            factory.replace_team_prototype(proto);
        }
        world.frame = 91;
        assert!(world.host_object(object).unwrap().is_alive());
        assert!(world.host_object(object).unwrap().is_recruitable);
        world.host_object_mut(object).unwrap().last_command_source =
            crate::game_logic::HUNT_CMD_FROM_SCRIPT;
        let old = world.host_object(object).unwrap().team_instance_id;
        assert!(world.recruit_owned_specific_ai_team_with_ui(
            1,
            "MainRecruit",
            500.,
            None,
            &mut None
        ));
        let ai = &world.ai_manager.ai_players[&1];
        let ready = ai.team_ready_queue.front().unwrap();
        assert_eq!(ready.frame_started, 91);
        assert!(ready.work_orders.is_empty());
        assert_eq!(
            world.host_object(object).unwrap().team_instance_id,
            ready.team_id
        );
        assert_eq!(
            world.host_object(object).unwrap().team_instance_name,
            "MainRecruit"
        );
        assert_ne!(ready.team_id, old);
        assert_eq!(
            world.host_object(object).unwrap().last_command_source,
            crate::game_logic::HUNT_CMD_FROM_AI
        );
        let home = Vec3::new(1., 0., 2.);
        let unit = world.host_object(object).unwrap();
        assert_eq!(
            unit.requested_destination.or(unit.movement.target_position),
            Some(home),
            "AI goal remains raw; player clipping is not admitted"
        );
        let members = ai.leftover_instance_member_ids(ready.team_id);
        assert_eq!(members, [object.0]);
        assert_eq!(peer.host_object(peer_object).unwrap().team_instance_id, old);
        assert!(peer.ai_manager.ai_players[&1].team_ready_queue.is_empty());
    }
    #[test]
    fn recruit_without_ai_transfers_roster_without_issuing_move_or_stamping_source() {
        let (mut world, object) = fixture_with_ai(false);
        prototype(&mut world, "MainRecruit", "");
        world.host_object_mut(object).unwrap().last_command_source =
            crate::game_logic::HUNT_CMD_FROM_SCRIPT;
        let before = world.host_object(object).unwrap().ai_state.clone();
        assert!(!world.host_object(object).unwrap().has_ai_update_interface());
        assert!(world.recruit_owned_specific_ai_team_with_ui(
            1,
            "MainRecruit",
            500.,
            None,
            &mut None
        ));
        let unit = world.host_object(object).unwrap();
        assert_eq!(unit.team_instance_name, "MainRecruit");
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::HUNT_CMD_FROM_SCRIPT
        );
        assert_eq!(unit.ai_state, before);
        assert!(unit.requested_destination.is_none());
        assert!(unit.movement.target_position.is_none());
    }
}
