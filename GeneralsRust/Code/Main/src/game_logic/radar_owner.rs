//! One canonical match radar, accessed through ordinary owner borrows.
use crate::game_logic::host_radar::*;
use game_engine::common::system::radar::{RadarEventType, RadarSystem};

impl crate::game_logic::GameLogic {
    pub fn radar(&self) -> &RadarSystem {
        &self.radar
    }
    pub fn radar_mut(&mut self) -> &mut RadarSystem {
        self.radar.observe_frame(self.frame);
        &mut self.radar
    }

    /// C++ getLastEventLoc: beacon pulses do not replace this match's last event.
    pub fn last_the_radar_event_host_position(&self) -> Option<glam::Vec3> {
        self.radar
            .get_last_event_loc()
            .map(radar_coord_to_host_world)
    }
}

impl crate::game_logic::GameLogic {
    pub fn host_create_radar_event(&mut self, pos: glam::Vec3, event_type: RadarEventType) {
        self.host_create_radar_event_for(pos, event_type, RADAR_EVENT_SECONDS_TO_LIVE);
    }

    pub fn host_create_radar_event_for(
        &mut self,
        pos: glam::Vec3,
        event_type: RadarEventType,
        seconds: f32,
    ) {
        self.radar_mut()
            .create_event(&host_world_to_radar_coord(pos), event_type, seconds);
    }

    pub fn host_create_player_radar_event(
        &mut self,
        player_color: u32,
        pos: glam::Vec3,
        event_type: RadarEventType,
    ) {
        self.radar_mut().create_player_event(
            player_color,
            &host_world_to_radar_coord(pos),
            event_type,
            RADAR_EVENT_SECONDS_TO_LIVE,
        );
    }

    pub fn host_radar_queue_terrain_refresh(&mut self) {
        self.radar_mut().queue_terrain_refresh();
    }

    pub fn host_radar_refresh_terrain(&mut self) {
        self.host_radar_rescan_terrain();
        self.radar.resample_terrain_with_source(&self.radar_map);
        self.radar
            .refresh_owned_terrain_with_source(&self.radar_map, self.radar_map.water_color);
    }
}

#[cfg(test)]
#[path = "radar_owner_tests.rs"]
mod owner_tests;
