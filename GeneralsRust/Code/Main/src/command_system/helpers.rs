use super::*;

pub(super) const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(250);

pub(super) fn screen_to_world(
    screen: Vec2,
    viewport_size: Vec2,
    world_min: Vec3,
    world_max: Vec3,
) -> Vec3 {
    let viewport_width = viewport_size.x.max(1.0);
    let viewport_height = viewport_size.y.max(1.0);
    let normalized_x = (screen.x / viewport_width).clamp(0.0, 1.0);
    let normalized_y = (screen.y / viewport_height).clamp(0.0, 1.0);
    let world_width = (world_max.x - world_min.x).max(1.0);
    let world_height = (world_max.z - world_min.z).max(1.0);

    Vec3::new(
        world_min.x + normalized_x * world_width,
        0.0,
        world_min.z + normalized_y * world_height,
    )
}

pub(super) fn default_max_shots_cmd() -> i32 {
    -1
}
