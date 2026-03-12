//! Integration tests for N-Patch tessellation with curved surfaces

use ww3d_scene::{
    NPatchPipeline, NPatchShaderIntegration, NPatchTessellator, NPatchVertex, QualityLevel,
    TessellationLevel,
};

use glam::{Vec2, Vec3};

/// Create a curved cylinder cap mesh for testing
/// This simulates a typical game object that benefits from N-Patch tessellation
fn create_curved_cylinder_cap() -> Vec<(NPatchVertex, NPatchVertex, NPatchVertex)> {
    let segments = 8;
    let mut triangles = Vec::new();

    let center = NPatchVertex::new(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::new(0.5, 0.5));

    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let x1 = angle1.cos();
        let z1 = angle1.sin();
        let x2 = angle2.cos();
        let z2 = angle2.sin();

        // Create vertices with normals pointing outward and upward for curvature
        let v1 = NPatchVertex::new(
            Vec3::new(x1, 0.0, z1),
            Vec3::new(x1, 0.5, z1).normalize(),
            Vec2::new((angle1 / std::f32::consts::TAU), 0.0),
        );

        let v2 = NPatchVertex::new(
            Vec3::new(x2, 0.0, z2),
            Vec3::new(x2, 0.5, z2).normalize(),
            Vec2::new((angle2 / std::f32::consts::TAU), 0.0),
        );

        triangles.push((center, v1, v2));
    }

    triangles
}

/// Create a simple curved dome mesh
fn create_dome() -> Vec<(NPatchVertex, NPatchVertex, NPatchVertex)> {
    let mut triangles = Vec::new();

    // Create a simple 4-sided pyramid with curved normals
    let center = NPatchVertex::new(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::new(0.5, 0.5));

    let v0 = NPatchVertex::new(
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0).normalize(),
        Vec2::new(0.0, 0.0),
    );
    let v1 = NPatchVertex::new(
        Vec3::new(1.0, 0.0, -1.0),
        Vec3::new(1.0, 1.0, -1.0).normalize(),
        Vec2::new(1.0, 0.0),
    );
    let v2 = NPatchVertex::new(
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0).normalize(),
        Vec2::new(1.0, 1.0),
    );
    let v3 = NPatchVertex::new(
        Vec3::new(-1.0, 0.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0).normalize(),
        Vec2::new(0.0, 1.0),
    );

    // Create triangular faces
    triangles.push((center, v0, v1));
    triangles.push((center, v1, v2));
    triangles.push((center, v2, v3));
    triangles.push((center, v3, v0));

    triangles
}

#[test]
fn test_curved_cylinder_cap_tessellation() {
    let mesh = create_curved_cylinder_cap();
    let tessellator = NPatchTessellator::new(TessellationLevel::MEDIUM);

    let subdivided = tessellator.subdivide_mesh(&mesh);

    // Original mesh has 8 triangles
    // With MEDIUM (level 3), each becomes 9 triangles
    assert_eq!(subdivided.triangle_count(), 8 * 9);

    // Check that normals are normalized
    for vertex in &subdivided.vertices {
        let length = vertex.normal.length();
        assert!(
            (length - 1.0).abs() < 0.001,
            "Normal length {} should be 1.0",
            length
        );
    }

    // Check that UVs are in valid range
    for vertex in &subdivided.vertices {
        assert!(
            vertex.uv.x >= 0.0 && vertex.uv.x <= 1.0,
            "UV X {} out of range",
            vertex.uv.x
        );
        assert!(
            vertex.uv.y >= 0.0 && vertex.uv.y <= 1.0,
            "UV Y {} out of range",
            vertex.uv.y
        );
    }
}

#[test]
fn test_dome_tessellation_creates_smoothness() {
    let mesh = create_dome();
    let tessellator = NPatchTessellator::new(TessellationLevel::HIGH);

    let subdivided = tessellator.subdivide_mesh(&mesh);

    // Original mesh has 4 triangles
    // With HIGH (level 4), each becomes 16 triangles
    assert_eq!(subdivided.triangle_count(), 4 * 16);

    // The subdivided mesh should have vertices displaced from the original plane
    // This tests that actual curvature is happening
    let mut has_displacement = false;

    for vertex in &subdivided.vertices {
        // Check if any vertex is higher than the base vertices
        if vertex.position.y > 0.1 && vertex.position.y < 0.9 {
            has_displacement = true;
            break;
        }
    }

    assert!(
        has_displacement,
        "Tessellated dome should have displaced vertices creating curvature"
    );
}

#[test]
fn test_pipeline_with_curved_mesh() {
    let mesh = create_curved_cylinder_cap();
    let pipeline = NPatchPipeline::new(TessellationLevel::MEDIUM);

    let mesh_id = 999;
    let subdivided = pipeline.process_mesh(mesh_id, &mesh);

    // Verify subdivision
    assert_eq!(subdivided.triangle_count(), 8 * 9);

    // Second call should hit cache
    let subdivided2 = pipeline.process_mesh(mesh_id, &mesh);
    assert_eq!(subdivided2.triangle_count(), subdivided.triangle_count());

    // Check cache stats
    let stats = pipeline.get_stats();
    assert_eq!(stats.cache_hits, 1);
    assert_eq!(stats.cache_misses, 1);
    assert!((stats.hit_rate() - 0.5).abs() < 0.01);
}

#[test]
fn test_automatic_level_recommendation() {
    // Small detailed object - should use high tessellation
    let small_mesh_level = NPatchShaderIntegration::recommend_level(25, 5.0, QualityLevel::High);
    assert_eq!(small_mesh_level.as_raw(), TessellationLevel::HIGH.as_raw());

    // Large background object - should use low or no tessellation
    let large_mesh_level = NPatchShaderIntegration::recommend_level(500, 100.0, QualityLevel::High);
    assert_eq!(large_mesh_level.as_raw(), TessellationLevel::NONE.as_raw());

    // Medium object - should use low tessellation (100 tris is in the LOW range for High quality)
    let medium_mesh_level = NPatchShaderIntegration::recommend_level(100, 20.0, QualityLevel::High);
    assert_eq!(medium_mesh_level.as_raw(), TessellationLevel::LOW.as_raw());
}

#[test]
fn test_quality_levels_affect_recommendations() {
    let triangle_count = 80;

    let low = NPatchShaderIntegration::recommend_level(triangle_count, 10.0, QualityLevel::Low);
    let medium =
        NPatchShaderIntegration::recommend_level(triangle_count, 10.0, QualityLevel::Medium);
    let high = NPatchShaderIntegration::recommend_level(triangle_count, 10.0, QualityLevel::High);

    // Higher quality should give higher tessellation for same mesh
    assert!(high.as_raw() >= medium.as_raw());
    assert!(medium.as_raw() >= low.as_raw());
}

#[test]
fn test_memory_efficiency() {
    let mesh = create_dome();
    let pipeline = NPatchPipeline::new(TessellationLevel::LOW);

    let mesh_id = 1000;
    let _subdivided = pipeline.process_mesh(mesh_id, &mesh);

    // Check that memory usage is being tracked
    let memory = pipeline.cache_memory_usage();
    assert!(memory > 0, "Should track memory usage");

    // Verify it's reasonable (should be less than 100KB for this small test mesh)
    assert!(memory < 100_000, "Memory usage should be reasonable");
}

#[test]
fn test_cache_invalidation_on_level_change() {
    let mesh = create_curved_cylinder_cap();
    let mut pipeline = NPatchPipeline::new(TessellationLevel::LOW);

    // Process at LOW level
    let mesh_id = 2000;
    let result_low = pipeline.process_mesh(mesh_id, &mesh);
    assert_eq!(result_low.triangle_count(), 8 * 4); // LOW = 4 triangles per original

    // Change level
    pipeline.set_level(TessellationLevel::MEDIUM);

    // Process again - should recompute due to cache invalidation
    let result_medium = pipeline.process_mesh(mesh_id, &mesh);
    assert_eq!(result_medium.triangle_count(), 8 * 9); // MEDIUM = 9 triangles per original

    // Verify cache was cleared
    assert_eq!(pipeline.cache_size(), 1); // Only the new level is cached
}

#[test]
fn test_disabled_pipeline_passthrough() {
    let mesh = create_dome();
    let pipeline = NPatchPipeline::disabled();

    assert!(!pipeline.is_enabled());

    let result = pipeline.process_mesh(1, &mesh);

    // Should return original mesh without subdivision
    assert_eq!(result.triangle_count(), mesh.len());
}

#[test]
fn test_curved_normal_interpolation() {
    // Create a triangle with different normals at each vertex to test interpolation
    let v0 = NPatchVertex::new(
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 1.0).normalize(),
        Vec2::ZERO,
    );
    let v1 = NPatchVertex::new(
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0).normalize(),
        Vec2::X,
    );
    let v2 = NPatchVertex::new(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 1.0).normalize(),
        Vec2::Y,
    );

    let tessellator = NPatchTessellator::new(TessellationLevel::HIGH);
    let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

    // Check that normals vary smoothly across the surface
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for vertex in &result.vertices {
        min_y = min_y.min(vertex.normal.y);
        max_y = max_y.max(vertex.normal.y);
    }

    // Normals should vary due to interpolation
    assert!(
        max_y - min_y > 0.1,
        "Normals should vary across the surface"
    );
}

#[test]
fn test_large_mesh_performance() {
    // Create a larger mesh to test performance
    let mut large_mesh = Vec::new();

    for i in 0..100 {
        let angle = (i as f32 / 100.0) * std::f32::consts::TAU;
        let x = angle.cos();
        let z = angle.sin();

        let v0 = NPatchVertex::new(Vec3::ZERO, Vec3::Y, Vec2::ZERO);
        let v1 = NPatchVertex::new(
            Vec3::new(x, 0.0, z),
            Vec3::new(x, 1.0, z).normalize(),
            Vec2::X,
        );
        let v2 = NPatchVertex::new(
            Vec3::new(x * 0.5, 0.5, z * 0.5),
            Vec3::new(x, 1.0, z).normalize(),
            Vec2::Y,
        );

        large_mesh.push((v0, v1, v2));
    }

    let pipeline = NPatchPipeline::new(TessellationLevel::LOW);

    // This should complete reasonably fast even for 100 triangles
    let result = pipeline.process_mesh(3000, &large_mesh);

    assert_eq!(result.triangle_count(), 100 * 4);
    assert!(result.vertices.len() > 300);
}

#[test]
fn test_vertex_position_continuity() {
    // Test that subdivision maintains C0 continuity (corner vertices exist in result)
    let v0 = NPatchVertex::new(Vec3::ZERO, Vec3::Z, Vec2::ZERO);
    let v1 = NPatchVertex::new(Vec3::X, Vec3::Z, Vec2::X);
    let v2 = NPatchVertex::new(Vec3::Y, Vec3::Z, Vec2::Y);

    let tessellator = NPatchTessellator::new(TessellationLevel::MEDIUM);
    let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

    // Find vertices that match the original corners
    // The N-Patch algorithm evaluates the Bezier surface at barycentric coordinates
    // (1,0,0), (0,1,0), and (0,0,1) which should give the original vertices
    let mut found_v0 = false;
    let mut found_v1 = false;
    let mut found_v2 = false;

    for vertex in &result.vertices {
        if (vertex.position - v0.position).length() < 0.001 {
            found_v0 = true;
        }
        if (vertex.position - v1.position).length() < 0.001 {
            found_v1 = true;
        }
        if (vertex.position - v2.position).length() < 0.001 {
            found_v2 = true;
        }
    }

    assert!(found_v0, "Should find vertex matching v0");
    assert!(found_v1, "Should find vertex matching v1");
    assert!(found_v2, "Should find vertex matching v2");

    // Check that the subdivided mesh has reasonable bounds
    for vertex in &result.vertices {
        // All positions should be within or near the original triangle
        assert!(
            vertex.position.x >= -0.1 && vertex.position.x <= 1.1,
            "X position out of bounds: {}",
            vertex.position.x
        );
        assert!(
            vertex.position.y >= -0.1 && vertex.position.y <= 1.1,
            "Y position out of bounds: {}",
            vertex.position.y
        );
    }
}
