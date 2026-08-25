//! Host vs GameWorld shadow probe types.

use super::*;
use std::collections::HashMap;

/// Host player facts for C++ HordeUpdate AlliesOnly (`getRelationship == ALLIES`).
#[derive(Debug, Clone)]
pub(super) struct HordePlayerRel {
    pub alliance_team: i32,
    pub is_alive: bool,
    pub map_relations: HashMap<u32, gamelogic::common::Relationship>,
}

/// Compact probe comparing host authority vs GameWorld shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWorldShadowProbe {
    pub host_frame: u32,
    pub shadow_frame: u64,
    pub host_objects: usize,
    pub shadow_entities: usize,
    pub host_players: usize,
    pub shadow_players: usize,
    pub host_supplies_sum: u64,
    pub shadow_supplies_sum: u64,
    /// Mapped host objects present in the ID table.
    pub mapped_objects: usize,
    pub counts_match: bool,
    pub economy_match: bool,
    /// Health samples agree for all mapped live objects (within 0.01).
    pub health_match: bool,
    /// Mapped pose (position + orientation) within epsilon.
    pub pose_match: bool,
    /// Mapped attack-target ids agree.
    pub attack_target_match: bool,
    /// Mapped move-target positions agree within epsilon.
    pub move_target_match: bool,
    /// Primary/secondary/tertiary/mine-clear weapon ammo + clip agree.
    pub weapon_match: bool,
    /// contained_by + occupant list agree.
    pub contain_match: bool,
    /// Marked-but-not-removed destroy visibility agrees.
    pub destroy_visibility_match: bool,
    /// Queue-head construction_frames + door phase agree (warning-only).
    pub production_match: bool,
    /// Host match-over residual (evaluate_victory_condition).
    pub host_match_over: bool,
    pub victory_label: Option<String>,
    pub detail: String,
}

impl GameWorldShadowProbe {
    pub fn format_report(&self) -> String {
        format!(
            "gameworld_shadow host_f={} shadow_f={} objs={}/{} players={}/{} supplies={}/{} mapped={} match={} econ={} health={} pose={} atk={} move={} weap={} contain={} dvis={} prod={} victory_over={} label={:?} {}",
            self.host_frame,
            self.shadow_frame,
            self.host_objects,
            self.shadow_entities,
            self.host_players,
            self.shadow_players,
            self.host_supplies_sum,
            self.shadow_supplies_sum,
            self.mapped_objects,
            self.counts_match,
            self.economy_match,
            self.health_match,
            self.pose_match,
            self.attack_target_match,
            self.move_target_match,
            self.weapon_match,
            self.contain_match,
            self.destroy_visibility_match,
            self.production_match,
            self.host_match_over,
            self.victory_label,
            self.detail
        )
    }

    #[inline]
    pub fn full_match(&self) -> bool {
        self.counts_match && self.economy_match && self.health_match
    }
}
