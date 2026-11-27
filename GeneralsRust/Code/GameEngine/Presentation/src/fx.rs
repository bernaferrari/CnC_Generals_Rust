use glam::{Mat4, Vec3};

/// The observed object state for an FX dispatch or an attached particle update.
/// Position and transform use the Z-up coordinates consumed by the client.
#[derive(Clone, Copy, Debug)]
pub struct HostFxObjectPose {
    pub id: u32,
    pub position: Vec3,
    pub transform: Mat4,
    pub player_index: i32,
    /// Source geometry radius used by object-form light pulses.
    pub bounding_circle_radius: f32,
    /// The receiving player's object shroud is fogged or denser.
    pub is_shrouded: bool,
}
