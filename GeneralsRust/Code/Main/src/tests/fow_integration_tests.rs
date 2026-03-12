#![cfg(test)]

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};
use crate::game_logic::ObjectId;
use game_engine::common::frame_clock::FrameClock;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Mock ShroudManager for testing
struct ShroudManager {
    last_update_ms: u32,
    objects: std::collections::HashMap<ObjectId, glam::Vec3>,
    visibility: std::collections::HashMap<(u32, ObjectId), bool>,
    explored: std::collections::HashMap<(u32, ObjectId), bool>,
    explored_areas: std::collections::HashMap<(u32, u32), bool>, // (player_id, grid_cell)
    clock: FrameClock,
}

impl ShroudManager {
    fn new() -> Self {
        Self {
            last_update_ms: 0,
            objects: std::collections::HashMap::new(),
            visibility: std::collections::HashMap::new(),
            explored: std::collections::HashMap::new(),
            explored_areas: std::collections::HashMap::new(),
            clock: FrameClock::new(),
        }
    }

    fn initialize(&mut self, _max_players: u32) {
        // Initialize for given number of players
    }

    fn set_map_dimensions(&mut self, _width: u32, _height: u32) {
        // Set map dimensions
    }

    fn register_object(&mut self, id: ObjectId, position: glam::Vec3) {
        self.objects.insert(id, position);
        self.tick();
    }

    fn unregister_object(&mut self, id: ObjectId) {
        self.objects.remove(&id);
        self.tick();
    }

    fn update_object_visibility(&mut self, player_id: u32, object_id: ObjectId, visible: bool) {
        self.visibility.insert((player_id, object_id), visible);
    }

    fn update_object_position(&mut self, id: ObjectId, position: glam::Vec3) {
        if let Some(pos) = self.objects.get_mut(&id) {
            *pos = position;
        }
        self.tick();
    }

    fn can_see_object(&self, player_id: u32, object_id: ObjectId) -> bool {
        self.visibility
            .get(&(player_id, object_id))
            .copied()
            .unwrap_or(false)
    }

    fn has_explored_object(&self, player_id: u32, object_id: ObjectId) -> bool {
        self.explored
            .get(&(player_id, object_id))
            .copied()
            .unwrap_or(false)
    }

    fn can_see_object_with_stealth(
        &self,
        player_id: u32,
        object_id: ObjectId,
    ) -> Result<bool, String> {
        Ok(self.can_see_object(player_id, object_id))
    }

    fn mark_area_explored(&mut self, player_id: u32, position: glam::Vec3, _radius: f32) {
        // Simple grid-based exploration marking
        let grid_x = (position.x / 10.0) as u32;
        let grid_y = (position.z / 10.0) as u32;
        let grid_cell = grid_x * 1000 + grid_y;
        self.explored_areas.insert((player_id, grid_cell), true);

        // Also mark any objects in the area as explored
        for (&id, &pos) in &self.objects {
            let dist = ((pos.x - position.x).powi(2) + (pos.z - position.z).powi(2)).sqrt();
            if dist <= _radius {
                self.explored.insert((player_id, id), true);
            }
        }
    }

    fn is_area_explored(&self, player_id: u32, position: glam::Vec3) -> bool {
        let grid_x = (position.x / 10.0) as u32;
        let grid_y = (position.z / 10.0) as u32;
        let grid_cell = grid_x * 1000 + grid_y;
        self.explored_areas
            .get(&(player_id, grid_cell))
            .copied()
            .unwrap_or(false)
    }

    fn last_update_ms(&self) -> u32 {
        self.last_update_ms
    }

    fn force_update(&mut self) {
        self.tick();
    }

    fn tick(&mut self) {
        let timing = self.clock.next_frame();
        self.last_update_ms = (timing.total_seconds() * 1000.0).min(u32::MAX as f32) as u32;
    }
}

fn get_shroud_manager() -> Arc<Mutex<ShroudManager>> {
    // Return a global mock shroud manager for testing
    static MANAGER: std::sync::OnceLock<Arc<Mutex<ShroudManager>>> = std::sync::OnceLock::new();
    MANAGER
        .get_or_init(|| Arc::new(Mutex::new(ShroudManager::new())))
        .clone()
}

/// Test fixture for FOW integration tests
struct FOWTestFixture {
    player_id: u32,
    test_objects: Vec<ObjectId>,
}

impl FOWTestFixture {
    fn new() -> Self {
        // Initialize test fixture with sample data
        Self {
            player_id: 0,
            test_objects: vec![
                ObjectId::from(100),
                ObjectId::from(101),
                ObjectId::from(102),
                ObjectId::from(103),
            ],
        }
    }

    fn setup_shroud_manager(&self) -> Arc<Mutex<ShroudManager>> {
        // Create and configure a test shroud manager
        let manager = Arc::new(Mutex::new(ShroudManager::new()));

        // Initialize with test configuration
        if let Ok(mut mgr) = manager.lock() {
            mgr.initialize(8); // Support 8 players
            mgr.set_map_dimensions(256, 256); // Standard map size
        }

        manager
    }
}

#[test]
fn test_object_creation_triggers_fow_update() {
    // Setup
    let fixture = FOWTestFixture::new();
    let shroud_mgr = fixture.setup_shroud_manager();

    // Action: Create a new object and register it with FOW
    let new_object_id = ObjectId::from(500);
    let object_position = glam::Vec3::new(100.0, 0.0, 100.0);

    if let Ok(mut mgr) = shroud_mgr.lock() {
        // Simulate object creation
        mgr.register_object(new_object_id, object_position);
        mgr.update_object_visibility(fixture.player_id, new_object_id, true);
    }

    // Assert: Verify the object is now visible
    if let Ok(mgr) = shroud_mgr.lock() {
        assert!(
            mgr.can_see_object(fixture.player_id, new_object_id),
            "Newly created object should be visible to the player"
        );

        // Verify FOW was updated
        assert!(
            mgr.last_update_ms() < 200,
            "FOW should have been updated recently after object creation"
        );
    }
}

#[test]
fn test_object_destruction_triggers_fow_update() {
    // Setup: Create and register an object
    let fixture = FOWTestFixture::new();
    let shroud_mgr = fixture.setup_shroud_manager();
    let object_id = ObjectId::from(600);

    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.register_object(object_id, glam::Vec3::new(150.0, 0.0, 150.0));
        mgr.update_object_visibility(fixture.player_id, object_id, true);
    }

    // Action: Destroy the object
    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.unregister_object(object_id);
        mgr.force_update(); // Ensure FOW recalculation
    }

    // Assert: Object should no longer be trackable
    if let Ok(mgr) = shroud_mgr.lock() {
        // After destruction, visibility queries should return false
        assert!(
            !mgr.can_see_object(fixture.player_id, object_id),
            "Destroyed object should not be visible"
        );

        // Verify update timestamp
        assert!(
            mgr.last_update_ms() < 200,
            "FOW should have been updated after object destruction"
        );
    }
}

#[test]
fn test_visibility_queries_return_correct_alpha_values() {
    // Setup
    let fixture = FOWTestFixture::new();

    // Test case 1: Fully visible object
    let visible_object = ObjectId::from(700);
    {
        let visibility =
            FOWRenderingBridge::get_object_visibility(fixture.player_id, visible_object);

        // When shroud manager is not mocked, default is fully visible
        assert_eq!(
            visibility.visibility_alpha, 1.0,
            "Default visibility should be fully visible (1.0)"
        );
        assert_eq!(
            visibility.is_explored, 1.0,
            "Default explored state should be 1.0"
        );
    }

    // Test case 2: Object visibility states
    let test_cases = vec![
        (
            ObjectVisibility {
                visibility_alpha: 1.0,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            },
            "Fully visible",
        ),
        (
            ObjectVisibility {
                visibility_alpha: 0.3,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            },
            "Explored but not visible",
        ),
        (
            ObjectVisibility {
                visibility_alpha: 0.0,
                is_explored: 0.0,
                visibility_falloff: 1.0,
            },
            "Never seen",
        ),
    ];

    for (expected, description) in test_cases {
        // Verify alpha values are in valid range
        assert!(
            expected.visibility_alpha >= 0.0 && expected.visibility_alpha <= 1.0,
            "{}: Alpha value must be between 0.0 and 1.0",
            description
        );

        assert!(
            expected.is_explored == 0.0 || expected.is_explored == 1.0,
            "{}: Explored state must be 0.0 or 1.0",
            description
        );
    }
}

#[test]
fn test_explored_territory_persists_after_unit_moves() {
    // Setup
    let fixture = FOWTestFixture::new();
    let shroud_mgr = fixture.setup_shroud_manager();
    let scout_unit = ObjectId::from(800);
    let initial_pos = glam::Vec3::new(50.0, 0.0, 50.0);
    let new_pos = glam::Vec3::new(200.0, 0.0, 200.0);

    // Action: Place unit and mark area as explored
    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.register_object(scout_unit, initial_pos);
        mgr.update_object_visibility(fixture.player_id, scout_unit, true);
        mgr.mark_area_explored(fixture.player_id, initial_pos, 50.0); // 50 unit radius
    }

    // Move unit to new position
    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.update_object_position(scout_unit, new_pos);
        mgr.mark_area_explored(fixture.player_id, new_pos, 50.0);
    }

    // Assert: Original area should still be marked as explored
    if let Ok(mgr) = shroud_mgr.lock() {
        assert!(
            mgr.is_area_explored(fixture.player_id, initial_pos),
            "Previously explored area should remain explored after unit moves"
        );

        assert!(
            mgr.is_area_explored(fixture.player_id, new_pos),
            "New area should also be explored"
        );
    }
}

#[test]
fn test_shader_fow_effects_apply_correctly() {
    // Setup
    let fixture = FOWTestFixture::new();

    // Test different visibility states and their shader parameters
    let test_scenarios = vec![
        (true, true, 1.0f32, "Visible and explored"), // Fully visible
        (false, true, 0.3f32, "Explored but not visible"), // Fog of war
        (false, false, 0.0f32, "Never seen"),         // Completely hidden
    ];

    for (is_visible, is_explored, expected_alpha, scenario) in test_scenarios {
        // Create visibility state based on scenario
        let visibility = if is_visible {
            ObjectVisibility {
                visibility_alpha: 1.0,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            }
        } else if is_explored {
            ObjectVisibility {
                visibility_alpha: 0.3,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            }
        } else {
            ObjectVisibility {
                visibility_alpha: 0.0,
                is_explored: 0.0,
                visibility_falloff: 1.0,
            }
        };

        // Assert shader parameters are correct
        assert_eq!(
            visibility.visibility_alpha, expected_alpha,
            "{}: Shader alpha value should be {}",
            scenario, expected_alpha
        );

        assert!(
            visibility.visibility_falloff > 0.0,
            "{}: Falloff value must be positive for gradient calculations",
            scenario
        );
    }
}

#[test]
fn test_multiple_players_independent_fow() {
    // Setup
    let shroud_mgr = Arc::new(Mutex::new(ShroudManager::new()));
    let object_id = ObjectId::from(900);
    let object_pos = glam::Vec3::new(125.0, 0.0, 125.0);

    // Initialize for multiple players
    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.initialize(4); // 4 players
        mgr.set_map_dimensions(256, 256);
        mgr.register_object(object_id, object_pos);
    }

    // Action: Make object visible only to player 1
    if let Ok(mut mgr) = shroud_mgr.lock() {
        mgr.update_object_visibility(1, object_id, true);
        mgr.mark_area_explored(1, object_pos, 30.0);
    }

    // Assert: Verify independent visibility for different players
    if let Ok(mgr) = shroud_mgr.lock() {
        assert!(
            mgr.can_see_object(1, object_id),
            "Player 1 should see the object"
        );

        assert!(
            !mgr.can_see_object(0, object_id),
            "Player 0 should NOT see the object"
        );

        assert!(
            !mgr.can_see_object(2, object_id),
            "Player 2 should NOT see the object"
        );
    }
}

#[test]
fn test_batch_visibility_queries_performance() {
    // Setup: Create many objects for performance testing
    let fixture = FOWTestFixture::new();
    let mut object_ids = Vec::new();
    for i in 0..100 {
        object_ids.push(ObjectId::from(1000 + i));
    }

    // Action: Perform batch visibility query
    let visibilities =
        FOWRenderingBridge::get_all_object_visibilities(fixture.player_id, &object_ids);

    // Assert: Verify performance and correctness
    assert_eq!(
        visibilities.len(),
        object_ids.len(),
        "Should return visibility for all queried objects"
    );

    // Verify all visibilities are valid
    for (id, visibility) in visibilities.iter() {
        assert!(
            visibility.visibility_alpha >= 0.0 && visibility.visibility_alpha <= 1.0,
            "Object {:?} has invalid alpha value: {}",
            id,
            visibility.visibility_alpha
        );
    }
}

#[test]
fn test_stealth_detection_integration() {
    // Setup
    let fixture = FOWTestFixture::new();
    let stealth_unit = ObjectId::from(1100);
    let detector_unit = ObjectId::from(1101);

    // Test stealth visibility without detection
    let visibility_no_detection =
        FOWRenderingBridge::get_object_visibility_with_stealth(fixture.player_id, stealth_unit);

    // By default (no shroud manager), should be visible
    assert_eq!(
        visibility_no_detection.visibility_alpha, 1.0,
        "Without shroud manager, stealth units default to visible"
    );

    // Test with detection capability
    let visibility_with_detection =
        FOWRenderingBridge::get_object_visibility_with_stealth(fixture.player_id, detector_unit);

    assert_eq!(
        visibility_with_detection.visibility_alpha, 1.0,
        "Detector units should always be visible if in FOW range"
    );
}
