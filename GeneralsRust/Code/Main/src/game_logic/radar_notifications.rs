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
