//! The Player side of BuildAssistant's instant structure creation path.
use super::super::*;

impl GameLogic {
    /// C++ BuildAssistant::isPossibleToMakeUnit. Content definitions are resolved
    /// before callbacks; every segment rereads the constructor's current set.
    pub(crate) fn constructor_can_build_template(
        &self,
        builder: ObjectId,
        requested: &str,
    ) -> bool {
        let Some(object) = self.host_object(builder) else {
            return false;
        };
        let Some(player) = object.owner_player_id.and_then(|id| self.get_player(id)) else {
            return false;
        };
        let Some(wanted) = self.templates.get(requested) else {
            return false;
        };
        let set_name = object
            .command_set_override
            .clone()
            .filter(|name| !name.is_empty())
            .or_else(|| self.command_set_for_template(&object.template_name));
        let Some(set_name) = set_name else {
            return false;
        };
        // Build eligibility is a receiving-world query. Generic standalone
        // helpers can consult process-wide registries, so this query requires
        // an action catalog owned by this GameLogic or its ReceivingModuleContent.
        let Some(definitions) = self.build_eligibility_action_definitions() else {
            return false;
        };
        let Some(command_set) = definitions.sets.find_command_set_resolved(&set_name) else {
            return false;
        };
        // BuildAssistant keeps the last equivalent button. Search backward
        // and borrow the winning template name from the receiving catalog.
        let has_overrides = !self.control_bar_overrides().is_empty();
        let buildable = (0..gamelogic::command_button::MAX_COMMANDS_PER_SET)
            .rev()
            .find_map(|slot| {
                let override_name = has_overrides
                    .then(|| self.find_control_bar_override(&set_name, slot as i32))
                    .flatten();
                let name = override_name
                    .unwrap_or_else(|| command_set.get_button_at_position(slot).map(String::as_str))?;
                let button = definitions.buttons.find_command_button_resolved(name)?;
                if !matches!(button.command.as_str(), "UNIT_BUILD" | "DOZER_CONSTRUCT") {
                    return None;
                }
                let template = self.templates.get(&button.object)?;
                template.is_equivalent_to(wanted).then_some(button.object.as_str())
            });
        // Prerequisites belong to the winning button. Cash is separate.
        buildable.is_some_and(|name| self.script_player_can_build_template(player, name))
    }

    /// Player::onStructureConstructionComplete precedes CreateModules and is
    /// distinct from DozerAIUpdate's completion voice/radar/recharge handling.
    pub(crate) fn complete_instant_line_build_player(
        &mut self,
        _producer: ObjectId,
        structure: ObjectId,
        owner: u32,
    ) -> gamelogic::GameLogicResult<()> {
        self.complete_instant_line_build_player_with_effects(
            _producer,
            structure,
            owner,
            &mut Default::default(),
        )
    }

    pub(crate) fn complete_instant_line_build_player_with_effects(
        &mut self,
        _producer: ObjectId,
        structure: ObjectId,
        owner: u32,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.complete_owned_structure_player_with_effects(structure, owner, false, None, ui)
    }

    pub(crate) fn complete_owned_structure_player(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
    ) -> gamelogic::GameLogicResult<()> {
        self.complete_owned_structure_player_with_engine(structure, owner, rebuilding, None)
    }

    pub(crate) fn complete_owned_structure_player_with_engine(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.complete_owned_structure_player_with_effects(
            structure,
            owner,
            rebuilding,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn complete_owned_structure_player_with_effects(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.host_object(structure) else {
            return Ok(());
        };
        if object.owner_player_id != Some(owner) || object.construction_power_admitted {
            return Ok(());
        }
        let counts_as_building = Self::live_score_counts_as_building_create(object);
        let template_name = object.template_name.clone();
        let score_kind = super::super::score_kind_of(object.get_template());
        let base_cost = object.thing().template.build_cost.supplies;
        let cost = self.modified_build_cost_supplies(owner, &template_name, base_cost);
        // Player.cpp1639: invalidate this session's script census before
        // footprint, scoring, power, AI and CreateModule observations.
        self.script_object_count_changed_frame = self.frame;
        self.release_mobile_path_footprint(structure);
        self.apply_live_wall_footprint_removal(structure, interpreter, ui)?;
        if let Some(object) = self.objects.get(&structure) {
            ({
                let ground_owner = self.world_services.terrain();
                let ground_loan = ground_owner
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                    &ground_loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                self.pathfinding_system
                    .refresh_completed_structure_footprint(object, &ground_source)
            });
        }
        // No withdrawal here; both ordinary and instant completion record
        // spending. Rebuilding skips ScoreKeeper, as Player.cpp1650 specifies.
        let scoring_enabled = self.is_scoring_enabled();
        if let Some(player) = self.players.get_mut(&owner) {
            if !rebuilding && scoring_enabled && counts_as_building {
                player.statistics.record_built(&template_name, &score_kind);
            }
            // CPP ScoreKeeper::addMoneySpent has no scoring-enabled gate.
            if !rebuilding {
                player.record_resources_spent(cost);
            }
        }
        if let Some(object) = self.objects.get_mut(&structure) {
            object.construction_power_admitted = true;
        }
        self.admit_completed_structure_power(owner);
        // The AI owner callback follows power admission; CreateModules are
        // invoked by BuildAssistant only after this entire Player callback.
        self.notify_owned_ai_structure_produced_with_effects(structure, interpreter, ui);
        self.try_eva_superweapon_detected_for_source(structure);
        Ok(())
    }

    pub(in crate::game_logic) fn apply_live_wall_footprint_removal(
        &mut self,
        structure: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::pathfinding::PathfindingSystem;
        use gamelogic::ai::pathfind_astar::PathfindLayerEnum;

        let Some(wall) = self.objects.lookup(&structure) else {
            return Ok(());
        };
        if !self.pathfinding_system.begin_live_wall_footprint_removal(wall) {
            return Ok(());
        }
        let wall_birth = std::sync::Arc::downgrade(&wall.visual_draw_identity_token);
        let epoch = self.visual_world_epoch;
        let mut effects = effects.reborrow();
        if let Some(interpreter) = interpreter {
            effects.script_events = Some(crate::game_logic::ScriptEventRecipient::new(interpreter));
        }
        // C++ visits the linked object list. New births prepend and damage
        // can change a later victim's position or layer before it is visited.
        let roster: Vec<_> = self.objects.registered_values()
            .map(|object| (object.id, object.visual_object_generation))
            .collect();
        for (id, generation) in roster {
            let Some(wall) = self.objects.lookup(&structure) else {
                break;
            };
            if !wall_birth.ptr_eq(&std::sync::Arc::downgrade(&wall.visual_draw_identity_token)) {
                break;
            }
            let splats = self.objects.get_registered(&id).is_some_and(|object| {
                object.visual_object_generation == generation
                    && object.pathfind_layer == PathfindLayerEnum::Wall as u8
                    && PathfindingSystem::live_wall_footprint_contains(wall, object.get_position())
            });
            if splats {
                self.apply_residual_damage_owned_with_effects(
                    id,
                    gamelogic::damage::HUGE_DAMAGE_AMOUNT,
                    Some(id), // AIPathfind.cpp:4146 uses the falling occupant's ID.
                    crate::game_logic::combat::DamageType::Falling,
                    crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
                    None,
                    &mut effects,
                )?;
                if self.visual_world_epoch != epoch {
                    return Err(gamelogic::GameLogicError::InvalidObject(structure.0));
                }
            }
        }
        self.pathfinding_system.reclassify_wall_after_footprint_removal();
        Ok(())
    }
}
