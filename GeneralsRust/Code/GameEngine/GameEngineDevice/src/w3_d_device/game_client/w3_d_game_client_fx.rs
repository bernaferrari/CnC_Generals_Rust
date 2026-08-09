//! W3DGameClient FX shims → GameClient scorch + ray-effect registries.

pub use game_client::effects::ray_effect_system::{
    create_ray_effect_by_template, live_ray_effects, reset_ray_effects,
};
pub use game_client::terrain::scorch_mesh::{
    add_terrain_scorch, clear_terrain_scorches, terrain_scorch_count,
};

/// C++ `W3DGameClient::addScorch`.
pub fn add_scorch(pos: [f32; 3], radius: f32, scorch_type: i32) -> bool {
    add_terrain_scorch(pos, radius, scorch_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_scorch_and_ray_effect_hit_gameclient_registries() {
        clear_terrain_scorches();
        assert!(add_scorch([40.0, 50.0, 0.0], 20.0, 1));
        assert_eq!(terrain_scorch_count(), 1);
        clear_terrain_scorches();

        reset_ray_effects();
        let ray = create_ray_effect_by_template([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], "GenericLaser")
            .expect("template present");
        assert_eq!(ray.midpoint, [5.0, 0.0, 0.0]);
        assert_eq!(live_ray_effects().len(), 1);
        reset_ray_effects();
    }
}
