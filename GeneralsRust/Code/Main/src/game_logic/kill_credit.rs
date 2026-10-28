//! C++ Object::scoreTheKill completes before onDie, independently of removal.
use super::{GameLogic, ObjectId};

/// Once-only admission for a complete source-aware score phase. An in-flight
/// phase is not serializable: callbacks must finish before a save is admitted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum KillScorePhase {
    #[default]
    Ready,
    Running,
    Complete,
}

/// Ordered Rust mid-phase save continuation, not an original wire record.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PendingObjectRemoval {
    pub(crate) id: ObjectId,
    pub(crate) killer: Option<super::Team>,
    pub(crate) after_death: bool,
}

impl GameLogic {
    /// Keep both source and victim installed. Capture only the immutable facts
    /// required to release their borrows before Player and native callbacks.
    pub(in crate::game_logic) fn record_owned_score_the_kill(
        &mut self,
        killer_id: ObjectId,
        victim_id: ObjectId,
    ) {
        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        let Some(killer) = self.objects.get(&killer_id) else {
            return;
        };
        if !self.score_the_kill_victim_counts(victim) {
            return;
        }
        let victim_owner = self.player_owner_for_host_object(victim);
        let killer_owner = self.player_owner_for_host_object(killer);
        let under_construction = victim.status.under_construction;
        let is_building = Self::live_score_counts_as_building_destroy(victim);
        let is_unit = Self::live_score_counts_as_unit_destroy(victim);
        let template = victim.template_name.clone();
        let score_kind = super::score_kind_of(victim.get_template());
        // Object::scoreTheKill first asks the killer's live Object for its
        // relationship to the victim. Team-instance overrides and undetected
        // defection can differ from the two Players' default relationship.
        let enemy = self.object_relationship(killer, victim)
            == gamelogic::common::Relationship::Enemies
            && !(killer_owner.is_some() && killer_owner == victim_owner);
        let cost = victim_owner
            .map(|owner| {
                self.modified_build_cost_supplies(
                    owner,
                    &template,
                    victim.thing().template.build_cost.supplies,
                )
            })
            .unwrap_or(0);
        let skill = victim.kill_skill_point_value();
        let killer_position = killer.get_position();
        let scoring = self.is_scoring_enabled();
        // ScoreKeeper handles under-construction and scoring-disabled policy;
        // victim loss precedes relationship/self-kill filtering in Object.cpp.
        if !under_construction && (is_building || is_unit) && scoring {
            if let Some(owner) = victim_owner {
                if let Some(player) = self.players.get_mut(&owner) {
                    player.statistics.record_lost(&template, &score_kind, false);
                }
                gamelogic::player::notify_live_object_lost(owner, &template, false);
            }
        }
        if !enemy {
            return;
        }
        let Some(owner) = killer_owner else {
            return;
        };
        // Preserve scoreTheKill order: destroyed, skill, bounty, then caller XP.
        if !under_construction && (is_building || is_unit) && scoring {
            if let Some(player) = self.players.get_mut(&owner) {
                if let Some(victim_owner) = victim_owner {
                    player
                        .statistics
                        .record_destroyed(&template, &score_kind, victim_owner, false);
                }
            }
            if let Some(victim_owner) = victim_owner {
                gamelogic::player::notify_live_object_destroyed(
                    owner,
                    victim_owner,
                    &template,
                    false,
                );
            }
        }
        if under_construction {
            return;
        }
        if skill != 0 {
            if let Some(player) = self.players.get_mut(&owner) {
                player.add_skill_points_limited(skill, self.rank_level_limit);
            }
        }
        let bounty = self
            .players
            .get_mut(&owner)
            .map(|player| player.do_bounty_for_kill(cost, &mut self.economy_events))
            .unwrap_or(0);
        if bounty > 0 {
            self.cash_bounty.record_last_damage_source_kill();
            self.cash_bounty.record_floating_text(
                super::host_cash_bounty::HostCashBountyFloatingText::new(
                    killer_id,
                    victim_id,
                    killer_position,
                    bounty,
                    self.frame,
                ),
            );
            self.cash_bounty.record_bounty_award(bounty);
        }
    }
}
