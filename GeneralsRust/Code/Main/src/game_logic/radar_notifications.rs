use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::OnceLock;

use glam::Vec3;

#[derive(Debug, Clone)]
pub struct RadarEntry {
    pub text: String,
    pub position: Vec3,
    pub timestamp: f32,
    /// Optional tag for audio throttling (e.g., Attack/Ally/Generic)
    pub kind: RadarKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarKind {
    Generic,
    Attack,
    Ally,
}

pub struct RadarNotifications {
    queue: Mutex<VecDeque<RadarEntry>>,
}

impl RadarNotifications {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, entry: RadarEntry) {
        self.queue.lock().push_back(entry);
    }

    pub fn drain(&self) -> Vec<RadarEntry> {
        let mut guard = self.queue.lock();
        if guard.is_empty() {
            Vec::new()
        } else {
            guard.drain(..).collect()
        }
    }
}

static GLOBAL_RADAR_NOTIFICATIONS: OnceLock<RadarNotifications> = OnceLock::new();

pub fn global_radar_notifications() -> &'static RadarNotifications {
    GLOBAL_RADAR_NOTIFICATIONS.get_or_init(RadarNotifications::new)
}
