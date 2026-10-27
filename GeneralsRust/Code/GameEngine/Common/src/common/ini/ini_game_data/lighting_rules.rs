//! Receiving GameData lighting definitions, never a published GlobalData owner.
use super::*;
/// GameData inputs used to size and fade one world's terrain tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TerrainTrackRules {
    pub max_terrain_tracks: i32,
    pub max_tank_track_edges: i32,
    pub max_tank_track_opaque_edges: i32,
    pub max_tank_track_fade_delay: i32,
    pub make_track_marks: bool,
}
impl TerrainTrackRules {
    pub fn from_global_data(data: &GlobalData) -> Self {
        Self {
            max_terrain_tracks: data.max_terrain_tracks,
            max_tank_track_edges: data.max_tank_track_edges,
            max_tank_track_opaque_edges: data.max_tank_track_opaque_edges,
            max_tank_track_fade_delay: data.max_tank_track_fade_delay,
            make_track_marks: data.make_track_marks,
        }
    }
}
impl Default for TerrainTrackRules {
    fn default() -> Self {
        Self::from_global_data(&GlobalData::new())
    }
}

/// Per-world GameData observed by the terrain water presentation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerrainWaterRules {
    pub position: [f32; 3],
    pub extent: [f32; 2],
    pub water_type: i32,
    pub show_soft_water_edge: bool,
    pub feather_water: i32,
    pub time_of_day: u8,
}
impl Default for TerrainWaterRules {
    fn default() -> Self {
        LightingContentRules::from_global_data(&GlobalData::new()).terrain_water_rules()
    }
}
impl TerrainWaterRules {
    /// C++ draws the GameData sea plane only for water type 2 with positive extents.
    pub fn sea_plane_bounds(&self) -> Option<([f32; 2], [f32; 2], f32)> {
        if self.water_type != 2 || self.extent[0] <= 0.0 || self.extent[1] <= 0.0 {
            return None;
        }
        Some((
            [self.position[0], self.position[1]],
            [
                self.position[0] + self.extent[0],
                self.position[1] + self.extent[1],
            ],
            self.position[2],
        ))
    }
}
#[derive(Debug, Clone)]
pub struct LightingContentRules {
    pub time_of_day: TimeOfDay,
    pub terrain: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub objects: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub num_global_lights: i32,
    pub infantry_light_scale: [f32; TIME_OF_DAY_COUNT],
    pub infantry_override: f32,
    pub force_models_to_follow_time_of_day: bool,
    pub force_models_to_follow_weather: bool,
    pub use_draw_module_lod: bool,
    pub use_tree_sway: bool,
    /// Per-session presentation gate for physics drawable transforms.
    pub show_client_physics: bool,
    /// Shroud clear/fog alpha inputs used by the receiving presentation frame.
    pub clear_alpha: u8,
    pub fog_alpha: u8,
    /// W3D shroud tint and hidden-cell floor, frozen for this session.
    pub shroud_color: [f32; 3],
    pub shroud_alpha: u8,
    /// GameData water extent used by legacy version-1 PolygonTriggers.
    pub water_extent_x: f32,
    pub water_extent_y: f32,
    pub water_position: [f32; 3],
    pub water_type: i32,
    pub show_soft_water_edge: bool,
    pub feather_water: i32,
    /// Receiving GameData track definitions, including subsequent INI overlays.
    pub terrain_tracks: TerrainTrackRules,
    /// Application Water.ini and the receiving map's transparency overrides.
    pub water_definitions: crate::common::ini::ini_water::WaterDefinitions,
}
impl Default for LightingContentRules {
    fn default() -> Self {
        Self::from_global_data(&GlobalData::new())
    }
}
impl LightingContentRules {
    pub fn from_global_data(data: &GlobalData) -> Self {
        Self {
            time_of_day: data.time_of_day,
            terrain: data.terrain_lighting.clone(),
            objects: data.terrain_objects_lighting.clone(),
            num_global_lights: data.num_global_lights,
            infantry_light_scale: data.infantry_light_scale,
            infantry_override: data.script_override_infantry_light_scale,
            force_models_to_follow_time_of_day: data.force_models_to_follow_time_of_day,
            force_models_to_follow_weather: data.force_models_to_follow_weather,
            use_draw_module_lod: data.use_draw_module_lod,
            use_tree_sway: data.use_tree_sway,
            show_client_physics: data.show_client_physics,
            clear_alpha: data.clear_alpha,
            fog_alpha: data.fog_alpha,
            shroud_color: [
                data.shroud_color.r,
                data.shroud_color.g,
                data.shroud_color.b,
            ],
            shroud_alpha: data.shroud_alpha,
            water_extent_x: data.water_extent_x,
            water_extent_y: data.water_extent_y,
            water_position: [
                data.water_position_x,
                data.water_position_y,
                data.water_position_z,
            ],
            water_type: data.water_type,
            show_soft_water_edge: data.show_soft_water_edge,
            feather_water: data.feather_water,
            terrain_tracks: TerrainTrackRules::from_global_data(data),
            water_definitions: Default::default(),
        }
    }
    pub fn terrain_water_rules(&self) -> TerrainWaterRules {
        TerrainWaterRules {
            position: self.water_position,
            extent: [self.water_extent_x, self.water_extent_y],
            water_type: self.water_type,
            show_soft_water_edge: self.show_soft_water_edge,
            feather_water: self.feather_water,
            time_of_day: self.time_of_day as u8,
        }
    }
    /// Same GameData field table and original source order; no singleton writes.
    pub fn overlay_ini_text(&self, text: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.time_of_day = self.time_of_day;
        draft.terrain_lighting = self.terrain.clone();
        draft.terrain_objects_lighting = self.objects.clone();
        draft.num_global_lights = self.num_global_lights;
        draft.infantry_light_scale = self.infantry_light_scale;
        draft.script_override_infantry_light_scale = self.infantry_override;
        draft.force_models_to_follow_time_of_day = self.force_models_to_follow_time_of_day;
        draft.force_models_to_follow_weather = self.force_models_to_follow_weather;
        draft.use_draw_module_lod = self.use_draw_module_lod;
        draft.use_tree_sway = self.use_tree_sway;
        draft.show_client_physics = self.show_client_physics;
        draft.clear_alpha = self.clear_alpha;
        draft.fog_alpha = self.fog_alpha;
        draft.shroud_color = super::RGBColor::new(
            self.shroud_color[0],
            self.shroud_color[1],
            self.shroud_color[2],
        );
        draft.shroud_alpha = self.shroud_alpha;
        draft.water_extent_x = self.water_extent_x;
        draft.water_extent_y = self.water_extent_y;
        draft.water_position_x = self.water_position[0];
        draft.water_position_y = self.water_position[1];
        draft.water_position_z = self.water_position[2];
        draft.water_type = self.water_type;
        draft.show_soft_water_edge = self.show_soft_water_edge;
        draft.feather_water = self.feather_water;
        draft.max_terrain_tracks = self.terrain_tracks.max_terrain_tracks;
        draft.max_tank_track_edges = self.terrain_tracks.max_tank_track_edges;
        draft.max_tank_track_opaque_edges = self.terrain_tracks.max_tank_track_opaque_edges;
        draft.max_tank_track_fade_delay = self.terrain_tracks.max_tank_track_fade_delay;
        draft.make_track_marks = self.terrain_tracks.make_track_marks;
        let mut ini = INI::new();
        ini.with_inline_source(text, |ini| {
            loop {
                ini.read_line()?;
                if ini.is_eof() {
                    break;
                }
                if ini
                    .get_next_token_or_null()
                    .is_some_and(|key| key.eq_ignore_ascii_case("GameData"))
                {
                    parse_game_data_block(ini, &mut draft)?;
                }
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        let mut result = Self::from_global_data(&draft);
        result.water_definitions = self.water_definitions.clone();
        Ok(result)
    }
}

#[cfg(test)]
mod show_client_physics_tests {
    use super::LightingContentRules;

    #[test]
    fn show_client_physics_is_admitted_per_session_and_overlaid_in_order() {
        let enabled = LightingContentRules::default();
        let disabled = enabled
            .overlay_ini_text("GameData\n ShowClientPhysics = No\nEnd\n")
            .unwrap();
        let reenabled = disabled
            .overlay_ini_text("GameData\n ShowClientPhysics = Yes\nEnd\n")
            .unwrap();

        assert!(enabled.show_client_physics);
        assert!(!disabled.show_client_physics);
        assert!(reenabled.show_client_physics);
        assert!(
            enabled.show_client_physics,
            "another session's parse must not mutate this owner"
        );
    }

    #[test]
    fn force_time_follow_survives_later_ordered_game_data_overlays() {
        let base = LightingContentRules::default()
            .overlay_ini_text("GameData\n ForceModelsToFollowTimeOfDay = No\nEnd\n")
            .unwrap();
        let later = base
            .overlay_ini_text("GameData\n FogAlpha = 91\nEnd\n")
            .unwrap();

        assert!(!base.force_models_to_follow_time_of_day);
        assert!(!later.force_models_to_follow_time_of_day);
        assert_eq!(later.fog_alpha, 91);
    }

    #[test]
    fn shroud_alphas_follow_ordered_game_data_overlays() {
        let base = LightingContentRules::default()
            .overlay_ini_text("GameData\n ClearAlpha = 211\n FogAlpha = 63\nEnd\n")
            .unwrap();
        let overlay = base
            .overlay_ini_text("GameData\n FogAlpha = 92\nEnd\n")
            .unwrap();

        assert_eq!((base.clear_alpha, base.fog_alpha), (211, 63));
        assert_eq!((overlay.clear_alpha, overlay.fog_alpha), (211, 92));
        let defaults = LightingContentRules::default();
        assert_eq!((defaults.clear_alpha, defaults.fog_alpha), (255, 127));
    }

    #[test]
    fn projected_shroud_inputs_follow_ordered_game_data_overlays() {
        let base = LightingContentRules::default()
            .overlay_ini_text("GameData\n ShroudColor = R:64 G:128 B:192\n ShroudAlpha = 33\nEnd\n")
            .unwrap();
        let overlay = base
            .overlay_ini_text("GameData\n ShroudAlpha = 41\nEnd\n")
            .unwrap();

        assert_eq!(
            base.shroud_color,
            [64.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0]
        );
        assert_eq!(base.shroud_alpha, 33);
        assert_eq!(overlay.shroud_color, base.shroud_color);
        assert_eq!(overlay.shroud_alpha, 41);
        assert_eq!(LightingContentRules::default().shroud_color, [1.0; 3]);
        assert_eq!(LightingContentRules::default().shroud_alpha, 0);
    }
}
