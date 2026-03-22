//! GAME START SEQUENCE
//!
//! Complete game startup and initialization based on:
//! - /GeneralsMD/Code/GameEngine/Source/GameLogic/GameLogic.cpp (startNewGame)
//! - /GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp
//! - /GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine.cpp
//!
//! This module orchestrates the game startup sequence:
//! - Running startup scripts
//! - Positioning camera
//! - Initializing fog of war
//! - Generating minimap
//! - Starting AI players

use super::map_loader::{Coord3D, MapLoader};
use super::player_init::{PlayerIndex, PlayerList};
use crate::scripting::engine::get_script_engine;
use std::collections::HashMap;

/// Camera position and orientation
#[derive(Debug, Clone, Copy)]
pub struct CameraPosition {
    pub position: Coord3D,
    pub pitch: f32,
    pub yaw: f32,
    pub zoom: f32,
}

impl CameraPosition {
    pub fn new(position: Coord3D) -> Self {
        Self {
            position,
            pitch: 45.0, // Default pitch (degrees)
            yaw: 0.0,    // Default yaw (degrees)
            zoom: 1.0,   // Default zoom level
        }
    }

    pub fn with_angles(position: Coord3D, pitch: f32, yaw: f32) -> Self {
        Self {
            position,
            pitch,
            yaw,
            zoom: 1.0,
        }
    }
}

impl Default for CameraPosition {
    fn default() -> Self {
        Self::new(Coord3D::origin())
    }
}

/// Fog of war system
/// Matches C++ fog of war from GameClient
#[derive(Debug)]
pub struct FogOfWar {
    width: usize,
    height: usize,
    /// Visibility grid (per player)
    visibility: HashMap<PlayerIndex, Vec<u8>>,
    /// Exploration grid (per player) - once explored, stays visible but not updated
    explored: HashMap<PlayerIndex, Vec<bool>>,
    enabled: bool,
}

impl FogOfWar {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            visibility: HashMap::new(),
            explored: HashMap::new(),
            enabled: true,
        }
    }

    /// Initialize fog of war for a player
    pub fn init_player(&mut self, player_index: PlayerIndex) {
        let grid_size = self.width * self.height;

        // All cells start unexplored and invisible
        self.visibility.insert(player_index, vec![0; grid_size]);
        self.explored.insert(player_index, vec![false; grid_size]);
    }

    /// Check if a cell is visible to a player
    pub fn is_visible(&self, player_index: PlayerIndex, x: usize, y: usize) -> bool {
        if !self.enabled {
            return true;
        }

        if x >= self.width || y >= self.height {
            return false;
        }

        let index = y * self.width + x;
        self.visibility
            .get(&player_index)
            .map(|grid| grid.get(index).map_or(false, |&v| v > 0))
            .unwrap_or(false)
    }

    /// Check if a cell has been explored by a player
    pub fn is_explored(&self, player_index: PlayerIndex, x: usize, y: usize) -> bool {
        if !self.enabled {
            return true;
        }

        if x >= self.width || y >= self.height {
            return false;
        }

        let index = y * self.width + x;
        self.explored
            .get(&player_index)
            .and_then(|grid| grid.get(index).copied())
            .unwrap_or(false)
    }

    /// Set visibility at a position
    pub fn set_visibility(&mut self, player_index: PlayerIndex, x: usize, y: usize, visible: bool) {
        if x >= self.width || y >= self.height {
            return;
        }

        let index = y * self.width + x;

        if let Some(grid) = self.visibility.get_mut(&player_index) {
            if let Some(cell) = grid.get_mut(index) {
                if visible {
                    *cell = (*cell).saturating_add(1);

                    // Mark as explored
                    if let Some(explored_grid) = self.explored.get_mut(&player_index) {
                        if let Some(explored_cell) = explored_grid.get_mut(index) {
                            *explored_cell = true;
                        }
                    }
                } else {
                    *cell = (*cell).saturating_sub(1);
                }
            }
        }
    }

    /// Reveal area around a point
    pub fn reveal_area(
        &mut self,
        player_index: PlayerIndex,
        center_x: usize,
        center_y: usize,
        radius: usize,
    ) {
        let radius_squared = (radius * radius) as i32;

        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                let dist_squared = dx * dx + dy * dy;
                if dist_squared <= radius_squared {
                    let x = (center_x as i32 + dx) as usize;
                    let y = (center_y as i32 + dy) as usize;

                    if x < self.width && y < self.height {
                        self.set_visibility(player_index, x, y, true);
                    }
                }
            }
        }
    }

    /// Enable or disable fog of war
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if fog of war is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Clear fog of war
    pub fn clear(&mut self) {
        self.visibility.clear();
        self.explored.clear();
    }
}

/// Minimap generator
/// Matches C++ minimap generation from GameClient
#[derive(Debug)]
pub struct MinimapGenerator {
    width: usize,
    height: usize,
    /// RGBA pixel data
    pixel_data: Vec<u8>,
}

impl MinimapGenerator {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixel_data: vec![0; width * height * 4], // RGBA
        }
    }

    /// Generate minimap terrain layer from heightmap data.
    ///
    /// This matches the C++ startup intent (terrain-derived minimap during game init) while
    /// staying independent from later dynamic overlays (units, roads, reveal state).
    pub fn generate_from_heightmap(
        &mut self,
        heightmap_data: &[u8],
        source_width: usize,
        source_height: usize,
    ) {
        if source_width == 0 || source_height == 0 {
            self.clear();
            return;
        }
        if heightmap_data.len() < source_width.saturating_mul(source_height) {
            self.clear();
            return;
        }

        // Compute normalization range once to keep terrain contrast stable per map.
        let mut min_h = u8::MAX;
        let mut max_h = u8::MIN;
        for &h in heightmap_data
            .iter()
            .take(source_width.saturating_mul(source_height))
        {
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
        let range_h = (max_h as i16 - min_h as i16).max(1) as f32;

        let src_index =
            |x: usize, y: usize| -> usize { y.saturating_mul(source_width).saturating_add(x) };
        let sample = |x: usize, y: usize| -> f32 {
            let clamped_x = x.min(source_width.saturating_sub(1));
            let clamped_y = y.min(source_height.saturating_sub(1));
            heightmap_data[src_index(clamped_x, clamped_y)] as f32
        };

        // Fixed light direction for readable terrain embossing.
        let light_dir = nalgebra::Vector3::new(0.45_f32, 0.55_f32, 0.70_f32).normalize();

        for y in 0..self.height {
            for x in 0..self.width {
                let sx = x.saturating_mul(source_width.saturating_sub(1)) / self.width.max(1);
                let sy = y.saturating_mul(source_height.saturating_sub(1)) / self.height.max(1);

                let h = sample(sx, sy);
                let left = sample(sx.saturating_sub(1), sy);
                let right = sample(sx.saturating_add(1), sy);
                let up = sample(sx, sy.saturating_sub(1));
                let down = sample(sx, sy.saturating_add(1));

                let dx = (right - left) / 255.0;
                let dy = (down - up) / 255.0;
                let normal = nalgebra::Vector3::new(-dx, -dy, 1.0).normalize();
                let shade = normal.dot(&light_dir).clamp(0.2, 1.0);

                let elevation = ((h - min_h as f32) / range_h).clamp(0.0, 1.0);

                // Terrain gradient close to classic Generals minimap palette.
                let low = [48.0, 62.0, 44.0];
                let high = [201.0, 177.0, 128.0];
                let r = (low[0] + (high[0] - low[0]) * elevation) * shade;
                let g = (low[1] + (high[1] - low[1]) * elevation) * shade;
                let b = (low[2] + (high[2] - low[2]) * elevation) * shade;

                let pixel_idx = (y * self.width + x) * 4;
                self.pixel_data[pixel_idx] = r.clamp(0.0, 255.0) as u8;
                self.pixel_data[pixel_idx + 1] = g.clamp(0.0, 255.0) as u8;
                self.pixel_data[pixel_idx + 2] = b.clamp(0.0, 255.0) as u8;
                self.pixel_data[pixel_idx + 3] = 255;
            }
        }
    }

    /// Get pixel data
    pub fn get_pixel_data(&self) -> &[u8] {
        &self.pixel_data
    }

    /// Get dimensions
    pub fn get_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Clear minimap
    pub fn clear(&mut self) {
        self.pixel_data.fill(0);
    }
}

/// AI player controller state
#[derive(Debug)]
pub struct AIPlayerState {
    pub player_index: PlayerIndex,
    pub is_active: bool,
    pub script_name: String,
}

impl AIPlayerState {
    pub fn new(player_index: PlayerIndex) -> Self {
        Self {
            player_index,
            is_active: false,
            script_name: String::from("DefaultAI"),
        }
    }

    pub fn activate(&mut self, script_name: String) {
        self.is_active = true;
        self.script_name = script_name;
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}

/// Script execution result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptResult {
    Success,
    Failed(String),
}

/// Game startup coordinator
/// Matches C++ GameLogic::startNewGame flow
pub struct GameStartSequence {
    camera_position: CameraPosition,
    fog_of_war: FogOfWar,
    minimap: MinimapGenerator,
    ai_players: Vec<AIPlayerState>,
    startup_scripts_run: bool,
}

impl GameStartSequence {
    pub fn new(map_width: usize, map_height: usize) -> Self {
        Self {
            camera_position: CameraPosition::default(),
            fog_of_war: FogOfWar::new(map_width, map_height),
            minimap: MinimapGenerator::new(256, 256), // Standard minimap size
            ai_players: Vec::new(),
            startup_scripts_run: false,
        }
    }

    /// Step 1: Run startup scripts
    /// Matches C++ ScriptEngine::runStartupScripts()
    pub fn run_startup_scripts(&mut self, script_names: &[String]) -> ScriptResult {
        let startup_names: Vec<&str> = script_names
            .iter()
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .collect();

        if !startup_names.is_empty() {
            let engine_lock = get_script_engine();
            let mut guard = match engine_lock.write() {
                Ok(guard) => guard,
                Err(_) => {
                    self.startup_scripts_run = true;
                    return ScriptResult::Success;
                }
            };

            if let Some(engine) = guard.as_mut() {
                for script_name in startup_names {
                    let _ = engine.set_script_active_by_name(script_name, true);
                }
            }
        }

        self.startup_scripts_run = true;
        ScriptResult::Success
    }

    /// Step 2: Position camera from map defaults
    /// Matches C++ camera positioning from InitialCameraPosition waypoint
    pub fn position_camera_from_map(&mut self, map_loader: &MapLoader) {
        if let Some(camera_pos) = map_loader.get_initial_camera_position() {
            self.camera_position = CameraPosition::new(camera_pos);
        } else {
            // Default to map center
            let extent = map_loader.get_heightmap().get_extent();
            let center = Coord3D::new(
                (extent.lo.x + extent.hi.x) / 2.0,
                (extent.lo.y + extent.hi.y) / 2.0,
                0.0,
            );
            self.camera_position = CameraPosition::new(center);
        }
    }

    /// Step 3: Initialize fog of war for all players
    /// Matches C++ fog of war initialization
    pub fn init_fog_of_war(&mut self, player_list: &PlayerList, start_radius: usize) {
        // Initialize for each player
        for player in player_list.get_all_players() {
            self.fog_of_war.init_player(player.index);

            // Reveal area around player start position
            if let Some((x, y, _z)) = player.start_position {
                let grid_x = (x / super::map_loader::MAP_XY_FACTOR) as usize;
                let grid_y = (y / super::map_loader::MAP_XY_FACTOR) as usize;

                self.fog_of_war
                    .reveal_area(player.index, grid_x, grid_y, start_radius);
            }
        }
    }

    /// Step 4: Generate minimap
    /// Matches C++ minimap generation
    pub fn generate_minimap(&mut self, map_loader: &MapLoader) {
        let heightmap = map_loader.get_heightmap();
        self.minimap.generate_from_heightmap(
            &heightmap.data,
            heightmap.width.max(0) as usize,
            heightmap.height.max(0) as usize,
        );
    }

    /// Step 5: Start AI players
    /// Matches C++ AI initialization
    pub fn start_ai_players(&mut self, player_list: &PlayerList, ai_script_name: &str) {
        self.ai_players.clear();

        for player in player_list.get_all_players() {
            if player.is_ai {
                let mut ai_state = AIPlayerState::new(player.index);
                ai_state.activate(ai_script_name.to_string());
                self.ai_players.push(ai_state);
            }
        }
    }

    /// Complete startup sequence
    /// Orchestrates all startup steps in correct order
    pub fn execute_full_sequence(
        &mut self,
        map_loader: &MapLoader,
        player_list: &PlayerList,
        startup_scripts: &[String],
        ai_script: &str,
    ) -> ScriptResult {
        // Step 1: Run startup scripts
        let script_result = self.run_startup_scripts(startup_scripts);
        if script_result != ScriptResult::Success {
            return script_result;
        }

        // Step 2: Position camera
        self.position_camera_from_map(map_loader);

        // Step 3: Initialize fog of war (10 grid cells radius initially visible)
        self.init_fog_of_war(player_list, 10);

        // Step 4: Generate minimap
        self.generate_minimap(map_loader);

        // Step 5: Start AI players
        self.start_ai_players(player_list, ai_script);

        ScriptResult::Success
    }

    /// Get camera position
    pub fn get_camera(&self) -> &CameraPosition {
        &self.camera_position
    }

    /// Set camera position
    pub fn set_camera(&mut self, camera: CameraPosition) {
        self.camera_position = camera;
    }

    /// Get fog of war
    pub fn get_fog_of_war(&self) -> &FogOfWar {
        &self.fog_of_war
    }

    /// Get mutable fog of war
    pub fn get_fog_of_war_mut(&mut self) -> &mut FogOfWar {
        &mut self.fog_of_war
    }

    /// Get minimap
    pub fn get_minimap(&self) -> &MinimapGenerator {
        &self.minimap
    }

    /// Get AI players
    pub fn get_ai_players(&self) -> &[AIPlayerState] {
        &self.ai_players
    }

    /// Check if startup complete
    pub fn is_startup_complete(&self) -> bool {
        self.startup_scripts_run
    }
}

#[cfg(test)]
mod tests {
    use super::super::player_init::{make_player_template, Player};
    use super::*;
    use crate::scripting::core::{Condition, ConditionType, OrCondition, Script, ScriptList};
    use crate::scripting::engine::{get_script_engine, initialize_script_engine};

    #[test]
    fn test_camera_position() {
        let pos = Coord3D::new(100.0, 200.0, 0.0);
        let camera = CameraPosition::new(pos);

        assert_eq!(camera.position.x, 100.0);
        assert_eq!(camera.position.y, 200.0);
        assert_eq!(camera.pitch, 45.0);
        assert_eq!(camera.zoom, 1.0);
    }

    #[test]
    fn test_fog_of_war_visibility() {
        let mut fog = FogOfWar::new(10, 10);
        fog.init_player(0);

        // Initially not visible
        assert!(!fog.is_visible(0, 5, 5));
        assert!(!fog.is_explored(0, 5, 5));

        // Set visible
        fog.set_visibility(0, 5, 5, true);
        assert!(fog.is_visible(0, 5, 5));
        assert!(fog.is_explored(0, 5, 5));

        // Remove visibility (but stays explored)
        fog.set_visibility(0, 5, 5, false);
        assert!(!fog.is_visible(0, 5, 5));
        assert!(fog.is_explored(0, 5, 5));
    }

    #[test]
    fn test_fog_of_war_reveal_area() {
        let mut fog = FogOfWar::new(20, 20);
        fog.init_player(0);

        fog.reveal_area(0, 10, 10, 3);

        // Center should be visible
        assert!(fog.is_visible(0, 10, 10));

        // Points within radius should be visible
        assert!(fog.is_visible(0, 11, 10));
        assert!(fog.is_visible(0, 10, 11));

        // Points outside radius should not be visible
        assert!(!fog.is_visible(0, 14, 10));
        assert!(!fog.is_visible(0, 10, 14));
    }

    #[test]
    fn test_fog_of_war_disabled() {
        let mut fog = FogOfWar::new(10, 10);
        fog.init_player(0);

        fog.set_enabled(false);

        // Everything is visible when disabled
        assert!(fog.is_visible(0, 5, 5));
        assert!(fog.is_explored(0, 5, 5));
    }

    #[test]
    fn test_minimap_generator() {
        let minimap = MinimapGenerator::new(256, 256);

        let (width, height) = minimap.get_dimensions();
        assert_eq!(width, 256);
        assert_eq!(height, 256);

        let pixel_data = minimap.get_pixel_data();
        assert_eq!(pixel_data.len(), 256 * 256 * 4); // RGBA
    }

    #[test]
    fn test_minimap_generation_populates_pixels_from_heightmap() {
        let mut minimap = MinimapGenerator::new(8, 8);
        let mut heights = vec![0_u8; 16];
        for (i, h) in heights.iter_mut().enumerate() {
            *h = (i as u8).saturating_mul(16);
        }

        minimap.generate_from_heightmap(&heights, 4, 4);

        let pixels = minimap.get_pixel_data();
        assert!(pixels.iter().any(|&v| v != 0));
        for alpha in pixels.iter().skip(3).step_by(4) {
            assert_eq!(*alpha, 255);
        }
    }

    #[test]
    fn test_minimap_generation_invalid_input_clears_pixels() {
        let mut minimap = MinimapGenerator::new(4, 4);
        minimap.generate_from_heightmap(&[1, 2, 3], 4, 4); // data too short
        assert!(minimap.get_pixel_data().iter().all(|&v| v == 0));
    }

    #[test]
    fn test_ai_player_state() {
        let mut ai_state = AIPlayerState::new(1);

        assert!(!ai_state.is_active);

        ai_state.activate("AdvancedAI".to_string());
        assert!(ai_state.is_active);
        assert_eq!(ai_state.script_name, "AdvancedAI");

        ai_state.deactivate();
        assert!(!ai_state.is_active);
    }

    #[test]
    fn test_game_start_sequence() {
        let mut sequence = GameStartSequence::new(100, 100);

        // Run startup scripts
        let result = sequence.run_startup_scripts(&vec!["Init.lua".to_string()]);
        assert_eq!(result, ScriptResult::Success);
        assert!(sequence.is_startup_complete());

        // Create player list
        let mut player_list = PlayerList::new();
        let template = make_player_template("Player 1", "USA");
        let mut player = Player::new(0, template, true);
        player.start_position = Some((100.0, 100.0, 0.0));
        player_list.add_player(player);

        // Initialize fog of war
        sequence.init_fog_of_war(&player_list, 5);

        let fog = sequence.get_fog_of_war();
        assert!(fog.is_visible(0, 10, 10)); // Should be visible near start
    }

    #[test]
    fn test_ai_player_start() {
        let mut sequence = GameStartSequence::new(100, 100);
        let mut player_list = PlayerList::new();

        // Add AI player
        let template = make_player_template("AI Player", "China");
        let player = Player::new(0, template, false); // is_ai = true
        player_list.add_player(player);

        sequence.start_ai_players(&player_list, "DefaultAI");

        let ai_players = sequence.get_ai_players();
        assert_eq!(ai_players.len(), 1);
        assert_eq!(ai_players[0].player_index, 0);
        assert!(ai_players[0].is_active);
        assert_eq!(ai_players[0].script_name, "DefaultAI");
    }

    #[test]
    fn test_run_startup_scripts_reactivates_subroutine_by_name() {
        initialize_script_engine().unwrap();
        let mut sequence = GameStartSequence::new(64, 64);

        let mut condition = Condition::new(ConditionType::ConditionTrue);
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));

        let mut startup_subroutine = Script::new();
        startup_subroutine.set_name("StartupSubroutine".to_string());
        startup_subroutine.set_subroutine(true);
        startup_subroutine.set_active(false);
        startup_subroutine.set_one_shot(true);
        startup_subroutine.set_or_condition(Some(Box::new(or_condition)));

        let mut script_list = ScriptList::new();
        script_list.append_script(Box::new(startup_subroutine));

        let engine_lock = get_script_engine();
        {
            let mut engine_guard = engine_lock.write().unwrap();
            let engine = engine_guard.as_mut().expect("script engine should exist");
            engine.clear_script_lists();
            engine
                .set_script_list_for_player(0, Some(Box::new(script_list)))
                .unwrap();
        }

        let result = sequence.run_startup_scripts(&["StartupSubroutine".to_string()]);
        assert_eq!(result, ScriptResult::Success);

        let engine_guard = engine_lock.read().unwrap();
        let engine = engine_guard.as_ref().expect("script engine should exist");
        let stored_script = engine
            .find_script_clone_by_name("StartupSubroutine")
            .unwrap();
        assert!(stored_script.is_active);
    }

    #[test]
    fn test_run_startup_scripts_reactivates_named_script() {
        initialize_script_engine().unwrap();
        let mut sequence = GameStartSequence::new(64, 64);

        let mut condition = Condition::new(ConditionType::ConditionTrue);
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));

        let mut script = Script::new();
        script.set_name("MapStartRule".to_string());
        script.set_active(false);
        script.set_one_shot(false);
        script.set_or_condition(Some(Box::new(or_condition)));

        let mut script_list = ScriptList::new();
        script_list.append_script(Box::new(script));

        let engine_lock = get_script_engine();
        {
            let mut engine_guard = engine_lock.write().unwrap();
            let engine = engine_guard.as_mut().expect("script engine should exist");
            engine.clear_script_lists();
            engine
                .set_script_list_for_player(0, Some(Box::new(script_list)))
                .unwrap();
        }

        let result = sequence.run_startup_scripts(&["MapStartRule".to_string()]);
        assert_eq!(result, ScriptResult::Success);

        let engine_guard = engine_lock.read().unwrap();
        let engine = engine_guard.as_ref().expect("script engine should exist");
        let stored_script = engine.find_script_clone_by_name("MapStartRule").unwrap();
        assert!(stored_script.is_active);
    }
}
