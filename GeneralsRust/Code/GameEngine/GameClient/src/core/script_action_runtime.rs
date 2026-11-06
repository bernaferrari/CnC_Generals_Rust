//! Per-client script action handoff state (was process TLS/atomics).
use gamelogic::common::types::ObjectID;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ScriptUiState {
    pub named_timers: HashMap<String, (String, bool)>,
    pub named_timer_display_shown: bool,
    pub superweapon_display_enabled: bool,
    pub superweapon_hidden_objects: HashSet<ObjectID>,
    pub cameo_flash_count: HashMap<String, i32>,
}

/// C++ `ScriptActions` members + LookAtTranslator reset latch + visual timing.
pub struct ScriptActionRuntime {
    pub audio_waits: HashMap<String, Vec<u32>>,
    pub speech_waits: HashMap<String, Vec<u32>>,
    pub music_waits: HashMap<String, Vec<u32>>,
    pub ui: ScriptUiState,
    pub look_at_reset: bool,
    pub visual_speed_multiplier: i32,
    pub fps_limit: i32,
}

impl Default for ScriptActionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptActionRuntime {
    pub fn new() -> Self {
        Self {
            audio_waits: Default::default(),
            speech_waits: Default::default(),
            music_waits: Default::default(),
            ui: Default::default(),
            look_at_reset: false,
            visual_speed_multiplier: 1,
            fps_limit: 0,
        }
    }

    pub fn take_look_at_reset_modes(&mut self) -> bool {
        std::mem::take(&mut self.look_at_reset)
    }

    pub fn request_look_at_reset_modes(&mut self) {
        self.look_at_reset = true;
    }
}
