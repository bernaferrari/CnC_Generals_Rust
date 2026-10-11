//! Immutable observation of one session's canonical player radar state.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostRadarEvent {
    pub player_id: u32,
    pub radar_count: i32,
    pub radar_disabled: bool,
}

impl crate::game_logic::GameLogic {
    pub(crate) fn host_radar_snapshot(&self) -> Vec<HostRadarEvent> {
        self.get_players()
            .values()
            .map(|player| HostRadarEvent {
                player_id: player.id,
                radar_count: player.radar_count,
                radar_disabled: player.radar_disabled,
            })
            .collect()
    }
}
