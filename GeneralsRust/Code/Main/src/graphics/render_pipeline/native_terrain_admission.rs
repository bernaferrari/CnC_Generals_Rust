//! Detached receiving-map preparation, followed by same-owner GPU hydration.
use super::*;
impl RenderPipeline {
    /// Apply C++ `setTerrainTracksDetail` to this receiving pipeline's visual.
    /// The call is intentionally unconditional: Custom LOD is reapplied on
    /// every accept and clears live tread geometry even when values repeat.
    pub(crate) fn set_native_terrain_track_detail(
        &mut self,
        rules: &game_engine::common::ini::ini_game_data::TerrainTrackRules,
    ) {
        if let Some(terrain) = self.native_terrain.as_mut() {
            let current = terrain.terrain_tracks().config();
            let mut config = game_client::terrain::TerrainTracksConfig::from_rules(rules);
            // MaxTerrainTracks and MakeTrackMarks come from the receiving
            // map's GameData, not from StaticGameLOD's three edge controls.
            config.max_terrain_tracks = current.max_terrain_tracks;
            config.make_track_marks = current.make_track_marks;
            terrain.set_terrain_tracks_detail_with_config(config);
        }
    }

    /// Convert the receiving presentation's C++ terrain-light records to the
    /// Y-up values used by terrain geometry, retaining the original light order.
    pub(crate) fn static_terrain_lighting(
        env: &crate::presentation_frame::PresentationWorldEnv,
    ) -> Option<game_client::terrain::chunk::TerrainStaticLighting> {
        let ambient = env.terrain_global_lights[0]?.ambient;
        let lights = env
            .terrain_global_lights
            .iter()
            .flatten()
            .filter(|light| light.object_light_active)
            .map(|light| (Vec3::from_array(light.render_light_pos()), light.diffuse))
            .collect();
        Some(game_client::terrain::chunk::TerrainStaticLighting { lights, ambient })
    }

    pub(crate) fn prepare_native_terrain_cpu(
        env: &crate::presentation_frame::PresentationWorldEnv,
    ) -> Result<Option<game_client::terrain::terrain_visual::TerrainVisualImpl>> {
        let Some(frozen) = env.runtime_heightmap.as_ref().filter(|map| map.is_usable()) else {
            return Ok(None);
        };
        let bounds = env.world_bounds_vec3();
        let world_size = (
            (bounds.1.x - bounds.0.x).abs().max(1.0),
            (bounds.1.z - bounds.0.z).abs().max(1.0),
        );
        let hint = env.heightmap_hint.as_deref().map(std::path::Path::new);
        let mut visual =
            game_client::terrain::terrain_visual::TerrainVisualImpl::new_with_track_rules(
                &env.terrain_track_rules,
            );
        visual.admit_water_rules(env.terrain_water_rules);
        visual.admit_road_presentation(env.terrain_roads.clone());
        visual
            .admit_water_presentation(&env.terrain_water)
            .map_err(|error| anyhow::anyhow!("Terrain water admission failed: {error}"))?;
        visual.set_skybox_enabled(env.skybox_enabled);
        if let Some(lighting) = Self::static_terrain_lighting(env) {
            visual.admit_static_lighting(lighting);
        }
        visual.admit_terrain_definitions(
            env.terrain_definitions
                .as_ref()
                .map(Arc::clone)
                .unwrap_or_else(|| {
                    Arc::new(game_engine::common::ini::ini_terrain::TerrainTypes::new())
                }),
        );
        visual
            .load_heightmap_from_data(frozen.to_height_map(), hint, Some(world_size))
            .map_err(|error| anyhow::anyhow!("Terrain CPU map admission failed: {error}"))?;
        for mark in &env.authored_scorches {
            visual.add_scorch(mark.location, mark.radius, mark.kind);
        }
        Ok(Some(visual))
    }

    /// Install only at an accepted world boundary. Later GPU preparation keeps
    /// this same CPU map, including already-admitted water and saved scorches.
    pub(crate) fn install_native_terrain_cpu(
        &mut self,
        visual: Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) {
        self.native_terrain = visual;
        self.radar_terrain_colors = None;
        self.heightmap_world_size = self
            .native_terrain
            .as_ref()
            .map(|visual| visual.world_size());
    }

    pub(crate) fn hydrate_native_terrain_gpu(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<bool> {
        // Read immutable presentation inputs before taking the disjoint owner loan.
        let source_tile_classes: Vec<game_client::terrain::terrain_visual::TerrainSourceTileClass> =
            self.presentation_frame
                .as_ref()
                .into_iter()
                .flat_map(|frame| &frame.world_env.terrain_texture_classes)
                .map(
                    |class| game_client::terrain::terrain_visual::TerrainSourceTileClass {
                        first_tile: class.first_tile,
                        num_tiles: class.num_tiles,
                        width: class.width,
                        name: class.name.clone(),
                    },
                )
                .collect();
        let skybox = (self.skybox_enabled && self.has_explicit_skybox_hint())
            .then(|| self.resolved_skybox_hint());
        let explicit_skybox = self.has_explicit_skybox_hint();
        let lighting = self
            .cached_terrain_lighting
            .as_ref()
            .or(self.cached_lighting.as_ref())
            .cloned();
        let Some(visual) = self.native_terrain.as_mut() else {
            return Ok(false);
        };
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .map_err(|error| anyhow::anyhow!("Terrain GPU hydration failed: {error}"))?;
        if !source_tile_classes.is_empty() {
            match visual.load_source_tiles_from_texture_classes(&source_tile_classes) {
                Ok(loaded) => debug!("Loaded {loaded} source tiles for receiving terrain"),
                Err(error) => warn!("Terrain source tile load failed: {error}"),
            }
        }
        if let Some(textures) = skybox {
            let borrowed = [
                textures[0].as_str(),
                textures[1].as_str(),
                textures[2].as_str(),
                textures[3].as_str(),
                textures[4].as_str(),
            ];
            if let Err(error) = visual.replace_skybox_textures(&[""; 5], &borrowed) {
                if explicit_skybox {
                    warn!("Failed to apply authored skybox textures: {error}");
                } else {
                    debug!("Mounted assets omit fallback skybox: {error}");
                }
            }
        }
        if let Some(lighting) = lighting {
            visual.set_lighting(
                lighting.sun_direction,
                lighting.sun_color,
                lighting.ambient_color,
                lighting.fog_color,
                lighting.fog_range,
            );
        }
        self.radar_terrain_colors = visual.freeze_radar_terrain_colors();
        self.heightmap_world_size = Some(visual.world_size());
        self.pending_heightmap_hint_load = false;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "native_terrain_cpu_tests.rs"]
mod tests;
