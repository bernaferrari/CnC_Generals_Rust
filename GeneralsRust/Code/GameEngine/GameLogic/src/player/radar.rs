//! Player.cpp:3117–3202: edge audio is admitted by the driving session.
use super::{Player, PlayerIndex};

/// Synchronous facts and audio delivery for a single radar operation. The
/// implementation belongs to the driving match; Player never selects a world.
pub trait RadarEdgeServices {
    fn has_single_player_been_defeated(&self, player: &Player) -> bool;
    fn client_quiet(&self) -> bool;
    fn in_logic_update(&self) -> bool;
    fn frame(&self) -> u32;
    fn play_radar_edge(&mut self, player_index: PlayerIndex, online: bool);
}

impl Player {
    fn finish_radar_edge(&self, had_radar: bool, services: &mut dyn RadarEdgeServices) {
        let has_radar = self.has_radar();
        if had_radar != has_radar
            && !services.has_single_player_been_defeated(self)
            && !self.is_player_dead()
            && !services.client_quiet()
            && services.in_logic_update()
            && services.frame() > 0
        {
            services.play_radar_edge(self.get_player_index(), has_radar);
        }
    }

    pub fn add_radar_owned(&mut self, disable_proof: bool, services: &mut dyn RadarEdgeServices) {
        let had_radar = self.has_radar();
        self.add_radar(disable_proof);
        self.finish_radar_edge(had_radar, services);
    }

    pub fn remove_radar_owned(
        &mut self,
        disable_proof: bool,
        services: &mut dyn RadarEdgeServices,
    ) {
        let had_radar = self.has_radar();
        self.remove_radar(disable_proof);
        self.finish_radar_edge(had_radar, services);
    }

    pub fn disable_radar_owned(&mut self, services: &mut dyn RadarEdgeServices) {
        let had_radar = self.has_radar();
        self.disable_radar();
        self.finish_radar_edge(had_radar, services);
    }

    pub fn enable_radar_owned(&mut self, services: &mut dyn RadarEdgeServices) {
        let had_radar = self.has_radar();
        self.enable_radar();
        self.finish_radar_edge(had_radar, services);
    }
}
