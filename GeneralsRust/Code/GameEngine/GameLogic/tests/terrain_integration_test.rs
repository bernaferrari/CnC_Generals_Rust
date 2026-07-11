//! Terrain Integration Test
//! Verifies TerrainQuery implementation and PhysicsEngine integration

use gamelogic::common::Coord3D;
use gamelogic::physics::{get_physics_engine, PhysicsState, PhysicsType, TerrainQuery};
use gamelogic::terrain::{
    init_terrain_physics_integration, TerrainLogic, TerrainQueryWrapper, THE_TERRAIN_LOGIC,
};

#[test]
fn test_terrain_query_trait_implementation() {
    // Test that TerrainLogic implements TerrainQuery
    let terrain = TerrainLogic::new();
    let query: &dyn TerrainQuery = &terrain;

    // Should return 0.0 for no terrain loaded
    let height = query.get_ground_height(50.0, 50.0);
    assert_eq!(height, 0.0, "Empty terrain should return 0 height");

    let slope = query.get_terrain_slope(50.0, 50.0);
    assert_eq!(slope, 0.0, "Empty terrain should return 0 slope");

    let depth = query.get_water_depth(50.0, 50.0);
    assert_eq!(depth, 0.0, "Empty terrain should return 0 water depth");
}

#[test]
fn test_terrain_query_wrapper() {
    

    // Create wrapper for global terrain instance
    let wrapper = TerrainQueryWrapper::new(THE_TERRAIN_LOGIC.clone());

    // Test via trait interface
    let query: &dyn TerrainQuery = &wrapper;
    let height = query.get_ground_height(100.0, 100.0);
    assert!(height >= 0.0, "Height should be non-negative");
}

#[test]
fn test_physics_terrain_integration() {
    // Initialize terrain-physics integration
    init_terrain_physics_integration();

    // Verify physics engine has terrain query set
    let physics = get_physics_engine();
    let engine = physics.read().unwrap();

    // PhysicsEngine should have terrain_query set (we can't directly check private field,
    // but we can verify it doesn't panic when using terrain features)
    drop(engine);

    // Add a physics object and verify terrain integration
    let mut physics_mut = physics.write().unwrap();
    let mut state = PhysicsState::new();
    state.position = Coord3D::new(100.0, 100.0, 10.0);
    state.physics_type = PhysicsType::Normal;
    state.enabled = true;

    physics_mut.add_object(42, state);

    // Update physics - this will use terrain queries internally
    let result = physics_mut.update();
    assert!(
        result.is_ok(),
        "Physics update should succeed with terrain integration"
    );
}

#[test]
fn test_global_terrain_logic_access() {
    // Test global terrain logic accessor
    let terrain_arc = gamelogic::terrain::get_terrain_logic();

    let terrain = terrain_arc.read().unwrap();
    let height = terrain.get_ground_height(0.0, 0.0, None);
    assert_eq!(height, 0.0, "Empty terrain should return 0 height");
}

#[test]
fn test_terrain_slope_calculation() {
    use gamelogic::terrain::TerrainLogic;

    let terrain = TerrainLogic::new();
    let query: &dyn TerrainQuery = &terrain;

    // On flat terrain, slope should be 0
    let slope = query.get_terrain_slope(50.0, 50.0);
    assert_eq!(slope, 0.0, "Flat terrain should have 0 slope");
}

#[test]
fn test_terrain_bridge_detection() {
    use gamelogic::terrain::TerrainLogic;

    let terrain = TerrainLogic::new();
    let query: &dyn TerrainQuery = &terrain;

    // No bridge loaded, should return false
    let pos = Coord3D::new(100.0, 100.0, 5.0);
    let (on_bridge, height) = query.is_on_bridge(&pos);
    assert!(!on_bridge, "Empty terrain should have no bridges");
    assert_eq!(height, 0.0, "Bridge height should be 0 when no bridge");
}

#[test]
fn test_terrain_cliff_detection() {
    use gamelogic::terrain::TerrainLogic;

    let terrain = TerrainLogic::new();
    let query: &dyn TerrainQuery = &terrain;

    // Flat terrain should not be a cliff
    let pos = Coord3D::new(50.0, 50.0, 0.0);
    let is_cliff = query.is_cliff(&pos);
    assert!(!is_cliff, "Flat terrain should not be detected as cliff");
}

#[test]
fn test_terrain_surface_type() {
    use gamelogic::physics::SurfaceType;
    use gamelogic::terrain::TerrainLogic;

    let terrain = TerrainLogic::new();
    let query: &dyn TerrainQuery = &terrain;

    // Default surface should be Ground
    let surface = query.get_surface_type(50.0, 50.0);
    assert_eq!(
        surface,
        SurfaceType::Ground,
        "Default surface should be Ground"
    );
}

#[test]
fn test_physics_state_terrain_updates() {
    init_terrain_physics_integration();

    let physics = get_physics_engine();
    let mut engine = physics.write().unwrap();

    // Create a physics object
    let mut state = PhysicsState::new();
    state.position = Coord3D::new(100.0, 100.0, 50.0);
    state.physics_type = PhysicsType::Normal;
    state.enabled = true;
    state.affected_by_gravity = true;

    engine.add_object(123, state);

    // Update should query terrain for ground height
    let result = engine.update();
    assert!(
        result.is_ok(),
        "Physics update with terrain queries should succeed"
    );

    // Get the state back
    let state_arc = engine.get_physics_state(123).unwrap();
    let state = state_arc.read().unwrap();

    // Terrain data should be updated (even if empty terrain, values should be set)
    assert!(
        state.ground_height >= 0.0,
        "Ground height should be updated"
    );
    assert!(
        state.terrain_slope >= 0.0,
        "Terrain slope should be updated"
    );
}
