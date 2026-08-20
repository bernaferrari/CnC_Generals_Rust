//! C++ `W3DRadar::drawEvents` (W3DRadar.cpp:546-576).

use super::{RadarEvent, RadarEventType, RadarSystem};
use crate::common::audio::audio_event_rts::AudioEventRts;
use crate::common::audio::game_audio::get_global_audio_manager;

impl RadarSystem {
    /// Active, drawable events — Fake is spacebar-only and never drawn.
    #[must_use]
    pub fn drawable_events(&self) -> Vec<&RadarEvent> {
        self.events
            .iter()
            .filter(|e| e.active && e.event_type != RadarEventType::Fake)
            .collect()
    }

    /// Play `RadarEvent` on the first visible frame of a non-beacon event.
    pub fn play_unplayed_event_sounds(&mut self) {
        for event in &mut self.events {
            if !event.active || event.event_type == RadarEventType::Fake {
                continue;
            }
            if event.sound_played {
                continue;
            }
            if event.event_type != RadarEventType::BeaconPulse {
                play_radar_event_chirp();
            }
            event.sound_played = true;
        }
    }

    /// C++ `W3DRadar::drawEvents` bookkeeping used by HUD / W3D adapter.
    pub fn draw_events(&mut self) -> Vec<RadarEvent> {
        self.play_unplayed_event_sounds();
        self.drawable_events().into_iter().cloned().collect()
    }
}

fn play_radar_event_chirp() {
    if let Some(fb) = super::radar_feedback() {
        fb.play_radar_audio("RadarEvent", -1);
        return;
    }
    if let Some(manager) = get_global_audio_manager() {
        if let Ok(mut guard) = manager.lock() {
            let event = AudioEventRts::with_event_name("RadarEvent");
            let _ = guard.add_audio_event(&event);
        }
    }
}
