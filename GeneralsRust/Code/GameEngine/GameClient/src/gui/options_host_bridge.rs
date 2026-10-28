//! Camera preferences queued by the receiving client for its native host.
use std::collections::VecDeque;

/// A user preference selected through the retail OptionsMenu WND whose live
/// effect belongs to Main's input/camera authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOptionsRequest {
    /// Match C++ `GlobalData::m_useAlternateMouse`: Main owns physical world
    /// input in an AuthorityOnly match, while standalone GameClient retains
    /// its legacy translator/global-data route.
    AlternateMouse { enabled: bool },
    /// Match C++ `MoveRMBScrollAnchor`: during RMB drag scrolling, keep the
    /// camera anchor within half a display width/height of the cursor.
    MoveRmbScrollAnchor { enabled: bool },
    /// Match C++ `DrawRMBScrollAnchor`: render the active RMB drag anchor as
    /// the retail green/black cross in Main's presentation path.
    DrawRmbScrollAnchor { enabled: bool },
    /// Applied static LOD track controls for the receiving Main world. This
    /// is an effect of one accepted Options event, not ambient Common state.
    TerrainTrackRules {
        selected_level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel,
        rules: game_engine::common::ini::ini_game_data::TerrainTrackRules,
    },
    /// C++ disabled-static-LOD behavior: update only the session comparison
    /// marker and do not mutate track rules or reset its visual.
    StaticLodMarkerOnly {
        level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel,
    },
}

/// Plain per-client delivery state. Construction has no publication effects.
#[derive(Default)]
pub struct HostOptionsBridge {
    requests: VecDeque<HostOptionsRequest>,
}
impl HostOptionsBridge {
    pub fn publish(&mut self, request: HostOptionsRequest) {
        self.requests.push_back(request);
    }
    pub fn take_requests(&mut self) -> Vec<HostOptionsRequest> {
        self.requests.drain(..).collect()
    }
    pub fn clear_pending(&mut self) {
        self.requests.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn peers_cannot_drain_or_reset_each_others_preferences() {
        let mut a = HostOptionsBridge::default();
        let mut b = HostOptionsBridge::default();
        a.publish(HostOptionsRequest::AlternateMouse { enabled: true });
        b.publish(HostOptionsRequest::AlternateMouse { enabled: false });
        assert_eq!(
            b.take_requests(),
            vec![HostOptionsRequest::AlternateMouse { enabled: false }]
        );
        b.publish(HostOptionsRequest::DrawRmbScrollAnchor { enabled: false });
        b.clear_pending();
        drop(b);
        assert_eq!(
            a.take_requests(),
            vec![HostOptionsRequest::AlternateMouse { enabled: true }]
        );
        assert!(a.take_requests().is_empty());
    }

    #[test]
    fn session_track_rule_effects_stay_in_the_receiving_options_mailbox() {
        let mut a = HostOptionsBridge::default();
        let mut b = HostOptionsBridge::default();
        let mut first = game_engine::common::ini::ini_game_data::TerrainTrackRules::default();
        first.max_tank_track_edges = 19;
        let mut second = first;
        second.max_tank_track_edges = 7;
        a.publish(HostOptionsRequest::TerrainTrackRules {
            selected_level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel::High,
            rules: first,
        });
        b.publish(HostOptionsRequest::TerrainTrackRules {
            selected_level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel::Low,
            rules: second,
        });

        assert_eq!(
            a.take_requests(),
            vec![HostOptionsRequest::TerrainTrackRules {
                selected_level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel::High,
                rules: first
            }]
        );
        assert_eq!(
            b.take_requests(),
            vec![HostOptionsRequest::TerrainTrackRules {
                selected_level: game_engine::common::ini::ini_game_lod::StaticGameLODLevel::Low,
                rules: second
            }]
        );
    }
}
