use std::collections::{HashMap, HashSet};

use crate::W3DDevice::GameClient::Drawable::Draw::wthree_d_model_draw::W3DModelDraw;

pub struct ModelDrawBridge {
    draws: HashMap<u32, W3DModelDraw>,
}

impl ModelDrawBridge {
    pub fn new() -> Self {
        Self {
            draws: HashMap::new(),
        }
    }

    pub fn flush(&mut self) {
        let submissions = game_client_rust::render_bridge::get_render_bridge()
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().map(|bridge| bridge.drain_draw_submissions()));

        let Some(submissions) = submissions else {
            return;
        };

        let active_ids: HashSet<u32> = submissions.iter().map(|s| s.drawable_id.0).collect();

        for submission in submissions {
            let drawable_id = submission.drawable_id.0;
            self.draws
                .entry(drawable_id)
                .or_insert_with(W3DModelDraw::new)
                .sync_from_bridge_submission(drawable_id, &submission);
        }

        self.draws
            .retain(|drawable_id, _| active_ids.contains(drawable_id));
    }
}

impl Default for ModelDrawBridge {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    pub static ref MODEL_DRAW_BRIDGE: std::sync::Mutex<ModelDrawBridge> =
        std::sync::Mutex::new(ModelDrawBridge::new());
}

pub fn flush_model_draws() {
    if let Ok(mut bridge) = MODEL_DRAW_BRIDGE.lock() {
        bridge.flush();
    }
}
