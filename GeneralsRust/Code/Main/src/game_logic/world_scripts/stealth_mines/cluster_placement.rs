//! C++ ClusterMinesBomb field placement at the original accepted drop.
use super::*;

impl GameLogic {
    /// Cluster Mines special-power residual: SmartBorder field around the drop.
    /// C++ ClusterMinesBomb GenerateMinefieldBehavior SmartBorder + DistanceAroundObject 80.
    pub fn place_cluster_mines(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        self.place_cluster_mines_with_effects(team, center, producer, &mut Default::default())
    }

    pub(crate) fn place_cluster_mines_with_effects(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            apply_cluster_mines_drop_variance, cluster_mines_drop_unit_samples,
        };
        let seed = producer.map(|p| p.0).unwrap_or(0).wrapping_add(self.frame);
        let (ux, uy) = cluster_mines_drop_unit_samples(seed);
        let center = apply_cluster_mines_drop_variance(center, ux, uy);
        self.place_cluster_mines_unvaried_with_effects(team, center, producer, effects)
    }

    /// SmartBorder around an already-offset drop (cargo-bomb impact).
    pub fn place_cluster_mines_unvaried(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        self.place_cluster_mines_unvaried_with_effects(
            team,
            center,
            producer,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_cluster_mines_unvaried_with_effects(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            CLUSTER_MINES_MINE_TEMPLATE, cluster_smart_border_positions,
        };
        // Leftover GenerateMinefieldBehavior::place_mines play_fx GenerationFX
        // at the bomb/object center (`TheFXList::do_fx_at_position`).
        let _ = self.dispatch_fx_list_at_host_position_with_effects(
            &crate::game_logic::host_cluster_mines_flight::cluster_mines_generation_fx_name(),
            center,
            effects,
        );

        let positions = cluster_smart_border_positions(center);
        let mut ids = Vec::with_capacity(positions.len());
        for pos in positions {
            if self.mine_spot_blocked(pos) {
                continue;
            }
            if let Some(id) = self.place_land_mine_named_with_effects(
                CLUSTER_MINES_MINE_TEMPLATE,
                team,
                pos,
                producer,
                effects,
            ) {
                // C++ ClusterMinesBomb is the producer; scoot from impact, not the plane.
                self.apply_land_mine_scoot_with_effects(id, center, pos, effects);
                ids.push(id);
            }
        }
        if !ids.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new("MineFieldPlaced")
                    .with_position(center)
                    .with_priority(160),
            );
        }
        ids
    }
}
