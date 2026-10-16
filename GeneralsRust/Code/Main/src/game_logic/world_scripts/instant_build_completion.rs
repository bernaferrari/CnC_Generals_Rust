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
            .or_else(|| {
                let factory = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
                let template = factory
                    .as_ref()?
                    .find_template(&object.template_name, false)?;
                let name = template.get_command_set_string().as_str().to_string();
                (!name.is_empty()).then_some(name)
            });
        let Some(set_name) = set_name else {
            return false;
        };
        // Release each catalog guard before acquiring another or executing game
        // behavior. These are existing content catalogs, never an active world.
        let button_names = {
            let Some(sets) = game_engine::common::ini::ini_command_set::get_command_set_manager()
            else {
                return false;
            };
            let Some(set) = sets.find_command_set_resolved(&set_name) else {
                return false;
            };
            (0..gamelogic::command_button::MAX_COMMANDS_PER_SET)
                .filter_map(|index| set.get_button_at_position(index).cloned())
                .collect::<Vec<_>>()
        };
        let buildable = {
            let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() else {
                return false;
            };
            button_names
                .iter()
                .filter_map(|name| bar.find_command_button_resolved(name))
                .filter(|button| {
                    matches!(button.command.as_str(), "UNIT_BUILD" | "DOZER_CONSTRUCT")
                })
                .map(|button| button.object.clone())
                .collect::<Vec<_>>()
        };
        // C++ retains the LAST equivalent build button, then checks that
        // button's template prerequisites. Cash is not part of this query.
        buildable
            .iter()
            .rev()
            .find(|name| {
                self.templates
                    .get(*name)
                    .is_some_and(|template| template.is_equivalent_to(wanted))
            })
            .is_some_and(|name| self.script_player_can_build_template(player, name))
    }

    /// Player::onStructureConstructionComplete precedes CreateModules and is
    /// distinct from DozerAIUpdate's completion voice/radar/recharge handling.
    pub(crate) fn complete_instant_line_build_player(
        &mut self,
        _producer: ObjectId,
        structure: ObjectId,
        owner: u32,
    ) {
        self.complete_instant_line_build_player_with_effects(
            _producer,
            structure,
            owner,
            &mut Default::default(),
        );
    }

    pub(crate) fn complete_instant_line_build_player_with_effects(
        &mut self,
        _producer: ObjectId,
        structure: ObjectId,
        owner: u32,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.complete_owned_structure_player_with_effects(structure, owner, false, None, ui);
    }

    pub(crate) fn complete_owned_structure_player(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
    ) {
        self.complete_owned_structure_player_with_engine(structure, owner, rebuilding, None);
    }

    pub(crate) fn complete_owned_structure_player_with_engine(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.complete_owned_structure_player_with_effects(
            structure,
            owner,
            rebuilding,
            interpreter,
            &mut Default::default(),
        );
    }

    pub(crate) fn complete_owned_structure_player_with_effects(
        &mut self,
        structure: ObjectId,
        owner: u32,
        rebuilding: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.host_object(structure) else {
            return;
        };
        if object.owner_player_id != Some(owner) || object.construction_power_admitted {
            return;
        }
        let counts_as_building = Self::live_score_counts_as_building_create(object);
        let template_name = object.template_name.clone();
        let base_cost = object.thing().template.build_cost.supplies;
        let cost = self.modified_build_cost_supplies(owner, &template_name, base_cost);
        // Player.cpp1639: invalidate this session's script census before
        // footprint, scoring, power, AI and CreateModule observations.
        self.script_object_count_changed_frame = self.frame;
        if let Some(object) = self.objects.get(&structure) {
            self.pathfinding_system
                .refresh_completed_structure_footprint(object);
        }
        // No withdrawal here; both ordinary and instant completion record
        // spending. Rebuilding skips ScoreKeeper, as Player.cpp1650 specifies.
        let scoring_enabled = self.is_scoring_enabled();
        if let Some(player) = self.players.get_mut(&owner) {
            if !rebuilding && scoring_enabled && counts_as_building {
                player.record_structure_built();
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
    }
}
