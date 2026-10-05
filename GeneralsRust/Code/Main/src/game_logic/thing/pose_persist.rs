//! Current raw Thing serde boundary. Caches are derived from the saved matrix.
//! The current seven-field wire order stays unchanged. No notification runs.
use super::*;

#[derive(Deserialize)]
struct ThingPersist {
    template: ThingTemplate,
    geometry: GeometryInfo,
    transform: Mat4,
    cached_position: Vec3,
    cached_angle: f32,
    cached_dir_vector: Vec3,
    cache_valid: bool,
}

impl<'de> Deserialize<'de> for Thing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let persisted = ThingPersist::deserialize(deserializer)?;
        let mut thing = Self {
            template: persisted.template,
            geometry: persisted.geometry,
            transform: persisted.transform,
            cached_position: persisted.cached_position,
            cached_angle: persisted.cached_angle,
            cached_dir_vector: persisted.cached_dir_vector,
            cache_valid: persisted.cache_valid,
        };
        thing.update_cache();
        Ok(thing)
    }
}
