use std::collections::VecDeque;

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

/// Pending UI notifications owned by one match. Producers and the UI drain
/// borrow that match mutably; presentation only reads a completed snapshot.
pub struct RadarNotifications {
    queue: VecDeque<RadarEntry>,
}

impl Default for RadarNotifications {
    fn default() -> Self {
        Self::new()
    }
}

impl RadarNotifications {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, entry: RadarEntry) {
        self.queue.push_back(entry);
    }

    pub fn drain(&mut self) -> Vec<RadarEntry> {
        self.queue.drain(..).collect()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Non-destructive copy for presentation freeze (UI drain remains authoritative).
    pub fn snapshot(&self) -> Vec<RadarEntry> {
        self.queue.iter().cloned().collect()
    }
}
