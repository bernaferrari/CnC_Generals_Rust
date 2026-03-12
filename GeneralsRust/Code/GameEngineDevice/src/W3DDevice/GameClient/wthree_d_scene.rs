//! W3D scene management (port of W3DScene).
//!
//! Provides a minimal but functional render-object registry used by draw modules.

use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub type RenderObjectId = u64;

#[derive(Default)]
pub struct W3DScene {
    next_id: RenderObjectId,
    segmented_lines: HashMap<RenderObjectId, Arc<RwLock<SegmentedLine>>>,
}

impl std::fmt::Debug for W3DScene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("W3DScene")
            .field("segmented_line_count", &self.segmented_lines.len())
            .finish()
    }
}

impl W3DScene {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            segmented_lines: HashMap::new(),
        }
    }

    pub fn add_segmented_line(&mut self, line: SegmentedLine) -> RenderObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.segmented_lines.insert(id, Arc::new(RwLock::new(line)));
        id
    }

    pub fn remove_render_object(&mut self, id: RenderObjectId) -> Option<Arc<RwLock<SegmentedLine>>> {
        self.segmented_lines.remove(&id)
    }

    pub fn get_segmented_line(&self, id: RenderObjectId) -> Option<Arc<RwLock<SegmentedLine>>> {
        self.segmented_lines.get(&id).cloned()
    }

    pub fn iter_segmented_lines(&self) -> impl Iterator<Item = Arc<RwLock<SegmentedLine>>> + '_ {
        self.segmented_lines.values().cloned()
    }

    pub fn update(&mut self, delta_time_seconds: f32) {
        for line in self.segmented_lines.values() {
            if let Some(mut guard) = line.try_write() {
                guard.advance_uv(delta_time_seconds);
            }
        }
    }
}
