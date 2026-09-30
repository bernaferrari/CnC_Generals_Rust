//! Read-only inputs for host strategic AI decisions.
//!
//! C++ AIPlayer queries its player, ThingFactory, object list and PartitionManager before
//! issuing synchronous commands. Keep those reads on the driving world without exposing
//! mutable simulation storage. This view borrows live values; it neither snapshots nor
//! caches them, so decisions after a command see the command's effects in the same phase.
//! Mutable command execution remains in the host world adapters. Native per-unit AI stays
//! in GameLogic; this module is the strategic AI query seam, not a second unit simulator.

use crate::game_logic::{
    GameLogic, Object, ObjectId, Player, PlayerTemplateIdentity, Team, ThingTemplate,
};
use glam::Vec3;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct AiWorldView<'a> {
    world: &'a GameLogic,
}

/// Allows recursive decisions to reuse a view and execution adapters to borrow a world.
/// There is no operation that recovers a mutable world from this interface.
pub(super) trait AiReadSource {
    fn ai_view(&self) -> AiWorldView<'_>;
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
    pub(super) fn host_objects(&self) -> &'a HashMap<ObjectId, Object> {
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
    pub(super) fn default_host_team_instance_name(&self, owner: Option<u32>, team: Team) -> String {
        self.world.default_host_team_instance_name(owner, team)
    }
    pub(super) fn is_location_legal_to_build(
        &self,
        team: Team,
        position: Vec3,
        template: &str,
    ) -> bool {
        self.world
            .is_location_legal_to_build(team, position, template)
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
