use std::collections::HashMap;

use cgmath::Point3;

use crate::W3DDevice::GameClient::Drawable::Draw::wthree_d_projectile_stream_draw::W3DProjectileStreamDraw;
use crate::W3DDevice::GameClient::Module::wthree_d_projectile_stream_draw::W3DProjectileStreamDrawModuleData;

pub struct ProjectileStreamBridge {
    streams: HashMap<u32, W3DProjectileStreamDraw>,
}

impl ProjectileStreamBridge {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn flush(&mut self) {
        let submissions = game_client_rust::render_bridge::get_render_bridge()
            .lock()
            .ok()
            .and_then(|mut guard| {
                guard
                    .as_mut()
                    .map(|b| b.drain_projectile_stream_submissions())
            });

        let Some(submissions) = submissions else {
            return;
        };

        let active_ids: std::collections::HashSet<u32> =
            submissions.iter().map(|s| s.drawable_id).collect();

        for submission in submissions {
            let flat =
                game_client_rust::render_bridge::projectile_stream_to_flat_points(&submission);
            let points: Vec<Point3<f32>> =
                flat.iter().map(|v| Point3::new(v.x, v.y, v.z)).collect();

            let entry = self
                .streams
                .entry(submission.drawable_id)
                .or_insert_with(|| {
                    let data = W3DProjectileStreamDrawModuleData {
                        texture_name: submission.texture_name.clone(),
                        width: submission.width,
                        tile_factor: submission.tile_factor,
                        scroll_rate: submission.scroll_rate,
                        max_segments: 0,
                    };
                    W3DProjectileStreamDraw::new(data)
                });

            entry.update_points(&points);
        }

        let stale_ids: Vec<u32> = self
            .streams
            .keys()
            .filter(|id| !active_ids.contains(id))
            .copied()
            .collect();

        for id in stale_ids {
            self.streams.remove(&id);
        }
    }
}

impl Default for ProjectileStreamBridge {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    pub static ref PROJECTILE_STREAM_BRIDGE: std::sync::Mutex<ProjectileStreamBridge> =
        std::sync::Mutex::new(ProjectileStreamBridge::new());
}

pub fn flush_projectile_streams() {
    if let Ok(mut bridge) = PROJECTILE_STREAM_BRIDGE.lock() {
        bridge.flush();
    }
}
