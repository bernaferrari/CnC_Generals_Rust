//! Read-only inputs for host strategic AI decisions.
//!
//! `AiWorldView` borrows live player, object, template and partition state so decisions see
//! synchronous command effects in the same phase. `AiDataView` shares immutable
//! definition values captured once before an AI manager pass; its store guards do not cross
//! player callbacks. Mutable command execution remains in the host world adapters. Per-unit
//! AI executes on Main objects; this module only provides strategic read inputs.

use crate::game_logic::{
    GameLogic, HostObjectRead, Object, ObjectId, Player, PlayerTemplateIdentity, Team,
    ThingTemplate,
};
use glam::Vec3;
use std::collections::HashMap;

/// Immutable definitions captured once at the start of a synchronous AI manager
/// update. This shares an immutable snapshot while callbacks mutate the
/// driving GameLogic.
pub(super) struct AiDataView {
    definitions: std::sync::Arc<game_engine::common::ini::AIData>,
    sciences: Option<std::sync::Arc<game_engine::common::rts::science::ScienceNameFacts>>,
}

impl AiDataView {
    pub(super) fn from_world(world: &GameLogic) -> Self {
        Self {
            definitions: world.ai_definitions.snapshot(),
            sciences: world
                .receiving_module_content
                .as_ref()
                .map(|content| content.science_facts()),
        }
    }
    pub(super) fn from_source(source: &(impl AiReadSource + ?Sized)) -> Self {
        Self::from_world(source.ai_view().world)
    }
    pub(super) fn catalog(&self) -> Option<&game_engine::common::ini::AIData> {
        Some(&self.definitions)
    }

    pub(super) fn science_name(&self, id: i32) -> Option<&str> {
        self.sciences.as_ref()?.name(id)
    }
}

#[derive(Clone, Copy)]
pub(super) struct AiWorldView<'a> {
    world: &'a GameLogic,
}

/// Allows recursive decisions to reuse a view and execution adapters to borrow a world.
/// There is no operation that recovers a mutable world from this interface.
pub(super) trait AiReadSource {
    fn ai_view(&self) -> AiWorldView<'_>;

    /// Logic frame observed from this exact simulation owner. Condition
    /// evaluation must receive it as data rather than resolving TheGameLogic.
    fn logic_frame(&self) -> u32 {
        self.ai_view().world.frame
    }

    fn local_player_id(&self) -> Option<u32> {
        self.ai_view().world.local_player_id()
    }
}

impl AiReadSource for GameLogic {
    fn ai_view(&self) -> AiWorldView<'_> {
        AiWorldView { world: self }
    }
}
impl AiReadSource for AiWorldView<'_> {
    fn ai_view(&self) -> AiWorldView<'_> {
        *self
    }
}

impl<'a> AiWorldView<'a> {
    pub(super) fn new(source: &'a (impl AiReadSource + ?Sized)) -> Self {
        source.ai_view()
    }
    pub(super) fn script_engine(&self) -> Option<&'a gamelogic::scripting::engine::ScriptEngine> {
        self.world.owned_script_engine.as_ref()
    }
    pub(super) fn player_is_skirmish_ai(&self, id: u32) -> bool {
        self.world
            .get_player(id)
            .is_some_and(|player| player.map_side.is_skirmish_ai)
    }
    pub(super) fn upgrade_action_definitions(
        &self,
    ) -> Option<&'a game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions>
    {
        self.world
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
    }
    pub(super) fn control_bar_overrides(
        &self,
    ) -> &crate::game_logic::game_logic::command_overrides::ControlBarOverrides {
        self.world.control_bar_overrides()
    }
    pub(super) fn host_objects(&self) -> HostObjectRead<'a> {
        self.world.host_objects()
    }
    pub(super) fn host_object(&self, id: ObjectId) -> Option<&'a Object> {
        self.world.host_object(id)
    }
    pub(super) fn get_players(&self) -> &'a HashMap<u32, Player> {
        self.world.get_players()
    }
    pub(super) fn get_player(&self, id: u32) -> Option<&'a Player> {
        self.world.get_player(id)
    }
    pub(super) fn object_owned_by_player(&self, object: &Object, player_id: u32) -> bool {
        self.world.object_owned_by_player(object, player_id)
    }
    pub(super) fn build_facility_template_names(&self) -> std::collections::HashSet<String> {
        self.world.build_facility_template_names()
    }
    pub(super) fn template(&self, name: &str) -> Option<&'a ThingTemplate> {
        self.world.templates.get(name)
    }
    pub(super) fn contains_template(&self, name: &str) -> bool {
        self.world.templates.contains_key(name)
    }
    pub(super) fn get_frame(&self) -> u32 {
        self.world.get_frame()
    }
    pub(super) fn world_bounds(&self) -> (Vec3, Vec3) {
        self.world.world_bounds()
    }
    pub(super) fn player_template_identity(&self, id: u32) -> Option<&'a PlayerTemplateIdentity> {
        self.world.player_template_identity(id)
    }
    pub(super) fn resolved_player_template(
        &self,
        id: u32,
    ) -> Option<game_engine::common::rts::player_template::PlayerTemplate> {
        self.world.resolved_player_template(id)
    }
    pub(super) fn owned_waypoint_hops(&self, start: u32) -> Vec<gamelogic::common::Coord3D> {
        let terrain = self
            .world
            .world_services
            .terrain()
            .read()
            .expect("owned AI waypoint chain");
        let mut result = Vec::new();
        let mut current = terrain.get_waypoint_by_id(start);
        while let Some(way) = current {
            result.push(*way.get_location());
            current = way.get_next();
        }
        result
    }
    pub(super) fn default_host_team_instance_name(&self, owner: Option<u32>, team: Team) -> String {
        self.world.default_host_team_instance_name(owner, team)
    }
    pub(super) fn is_location_legal_to_build(
        &self,
        player_id: u32,
        position: Vec3,
        template: &str,
    ) -> bool {
        self.world
            .is_location_legal_to_build_for_ai(player_id, position, template, None)
    }
    pub(super) fn modified_build_cost_supplies(
        &self,
        player: u32,
        template: &str,
        base: u32,
    ) -> u32 {
        self.world
            .modified_build_cost_supplies(player, template, base)
    }
    /// C++ `Player::getRelationship(otherPlayer->getDefaultTeam())`: a team
    /// override for the target's default team wins, then the player-index
    /// relation, else NEUTRAL (Player.cpp:542-572).
    pub(super) fn player_relationship_to_default_team(
        &self,
        source: u32,
        target: u32,
    ) -> gamelogic::common::Relationship {
        let Some(target_player) = self.get_player(target) else {
            return gamelogic::common::Relationship::Neutral;
        };
        let Some(target_team_id) = self.world.host_default_team_id(target) else {
            // Player::getRelationship(nullptr) returns NEUTRAL; selecting a
            // same-named sibling would observe a different Team's override.
            return gamelogic::common::Relationship::Neutral;
        };
        let target_team = self.default_host_team_instance_name(Some(target), target_player.team);
        GameLogic::object_relationship_from_owner_to_team_identity(
            &self.world.team_factory,
            self.get_players(),
            Some(source),
            Some(target),
            Some(target_team_id),
            &target_team,
        )
    }
    pub(super) fn player_relationship(
        &self,
        source: u32,
        target: u32,
    ) -> gamelogic::common::Relationship {
        self.world.player_relationship(source, target)
    }
    pub(super) fn can_make_unit(&self, producer: ObjectId, template: &str) -> u32 {
        self.world.can_make_unit(producer, template)
    }
    pub(super) fn quick_path_exists(
        &self,
        from: Vec3,
        to: Vec3,
        surfaces: u32,
        crusher: bool,
    ) -> bool {
        self.world
            .pathfinding_system
            .client_safe_quick_does_path_exist_for_crusher(from, to, surfaces, crusher)
    }
    pub(super) fn find_broken_bridge(&self, from: Vec3, to: Vec3) -> Option<ObjectId> {
        self.world.pathfinding_system.find_broken_bridge(from, to)
    }
}
