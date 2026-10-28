//! Score advice frozen from the receiving match before its players are reset.
use super::GameLogic;
use game_engine::common::rts::academy_stats::{AcademyAdviceContext, AcademyAdviceInfo};

impl GameLogic {
    pub(crate) fn score_academy_context(&self) -> AcademyAdviceContext {
        AcademyAdviceContext {
            frame: self.frame,
            // Input preference is app-owned; capture it once with match observations.
            use_alternate_mouse: game_engine::common::global_data::read_safe()
                .map(|data| data.use_alternate_mouse)
                .unwrap_or(false),
            local_player_mines: self
                .local_player_id()
                .and_then(|id| self.players.get(&id))
                .map(|p| p.academy_stats.get_mines())
                .unwrap_or(0),
            neutral_vehicles_sniped: self
                .players
                .values()
                .filter(|p| p.is_reserved_neutral())
                .min_by_key(|p| p.id)
                .map(|p| p.academy_stats.get_vehicles_sniped())
                .unwrap_or(0),
        }
    }

    pub(crate) fn score_academy_advice(&self) -> AcademyAdviceInfo {
        let mut advice = AcademyAdviceInfo::default();
        if let Some(player) = self.local_player_id().and_then(|id| self.players.get(&id)) {
            player
                .academy_stats
                .calculate_academy_advice_with_context(&mut advice, self.score_academy_context());
        }
        advice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::game_logic::PlayerSideRole;
    use crate::game_logic::{Player, Team};

    fn world(frame: u32) -> GameLogic {
        let mut world = GameLogic::new();
        world.frame = frame;
        let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
        neutral.map_side.role = PlayerSideRole::Neutral;
        neutral.map_side.map_player_name.clear();
        world.add_player(neutral);
        let mut observer = Player::new(9, Team::Neutral, "ReplayObserver", true);
        observer.map_side.role = PlayerSideRole::ReplayObserver;
        observer.map_side.map_player_name = "ReplayObserver".into();
        observer.is_observer = true;
        observer.academy_stats.record_vehicle_sniped();
        world.add_player(observer);
        let mut local = Player::new(1, Team::USA, "Local", true);
        local.academy_stats.record_mine();
        world.add_player(local);
        world
    }

    #[test]
    fn score_context_uses_reserved_neutral_and_exact_receiving_frame() {
        let mut actual = world(100);
        let mut peer = world(900);
        actual
            .players
            .get_mut(&0)
            .unwrap()
            .academy_stats
            .record_vehicle_sniped();
        let frozen = actual.score_academy_context();
        for _ in 0..4 {
            peer.players
                .get_mut(&0)
                .unwrap()
                .academy_stats
                .record_vehicle_sniped();
        }
        assert_eq!(frozen.frame, 100);
        assert_eq!(frozen.local_player_mines, 1);
        assert_eq!(frozen.neutral_vehicles_sniped, 1);
        assert_eq!(peer.score_academy_context().neutral_vehicles_sniped, 4);
        assert_eq!(actual.score_academy_context(), frozen);
        peer.reset();
        drop(peer);
        assert_eq!(actual.score_academy_context(), frozen);
    }

    #[test]
    fn encoded_save_retains_the_receiving_score_counters() {
        let mut actual = world(100);
        actual
            .players
            .get_mut(&0)
            .unwrap()
            .academy_stats
            .record_vehicle_sniped();
        let expected = actual.score_academy_context();
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&actual).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut loaded = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut loaded)
            .unwrap();
        assert_eq!(loaded.score_academy_context(), expected);
    }
}
