/*
**	Command & Conquer Generals Zero Hour(tm) Rust Port
**	Copyright 2025
**
**	Rendering Integration for WW3D Effects
**	Provides helpers for integrating decals and shatter fragments with the mesh renderer
*/

use super::decal_system::{DecalMesh, RigidDecalMesh};
use super::shatter_system_csg::MeshFragment;
use glam::{Mat4, Vec2, Vec3};

/// Helper to convert a RigidDecalMesh into renderable geometry
///
/// This function extracts the clipped decal geometry that was generated
/// by the Sutherland-Hodgeman algorithm and prepares it for rendering.
///
/// # Integration with MeshClass
///
/// To integrate with ww3d-renderer-3d's MeshClass:
///
/// 1. Create a MeshModelClass with the geometry from get_decal_geometry()
/// 2. Set the material from the decal's texture and shader names
/// 3. Apply the appropriate blend mode (usually AlphaBlend)
/// 4. Queue the mesh to the renderer
///
/// # Example
///
/// ```ignore
/// let (verts, normals, uvs, indices) = get_decal_geometry(&decal_mesh);
///
/// // Convert to MeshModelClass
/// let mut model = MeshModelClass::new("decal");
/// model.vertices = verts.iter().map(|v| W3dVectorStruct { x: v.x, y: v.y, z: v.z }).collect();
/// model.normals = normals.iter().map(|n| W3dVectorStruct { x: n.x, y: n.y, z: n.z }).collect();
/// // ... set triangles from indices
///
/// // Create MeshClass and queue to renderer
/// let mut mesh = MeshClass::new();
/// mesh.model = Some(Arc::new(model));
/// mesh.transform = decal_transform;
/// // ... add to render queue
/// ```
pub fn get_decal_geometry(
    decal_mesh: &RigidDecalMesh,
) -> (Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u32>) {
    let (verts, normals, uvs, indices) = decal_mesh.get_geometry();
    (
        verts.to_vec(),
        normals.to_vec(),
        uvs.to_vec(),
        indices.to_vec(),
    )
}

/// Helper to get the number of triangles in a decal mesh
pub fn get_decal_triangle_count(decal_mesh: &RigidDecalMesh) -> usize {
    let (_, _, _, indices) = decal_mesh.get_geometry();
    indices.len() / 3
}

/// Helper to convert a MeshFragment into renderable geometry
///
/// This function extracts the fragment geometry that was generated
/// by the CSG splitting algorithm and prepares it for rendering.
///
/// # Integration with MeshClass
///
/// To integrate with ww3d-renderer-3d's MeshClass:
///
/// 1. Create a MeshModelClass with the geometry from get_fragment_geometry()
/// 2. Copy the material from the original mesh that was shattered
/// 3. Apply the fragment's transform (position and rotation)
/// 4. Update physics each frame using fragment.update()
/// 5. Queue the mesh to the renderer
///
/// # Example
///
/// ```ignore
/// for fragment in shatter_system.get_fragments() {
///     let (verts, normals, uvs, indices, transform) = get_fragment_geometry(fragment);
///
///     // Convert to MeshModelClass
///     let mut model = MeshModelClass::new("fragment");
///     model.vertices = verts.iter().map(|v| W3dVectorStruct { x: v.x, y: v.y, z: v.z }).collect();
///     model.normals = normals.iter().map(|n| W3dVectorStruct { x: n.x, y: n.y, z: n.z }).collect();
///     // ... set triangles from indices
///
///     // Create MeshClass and queue to renderer
///     let mut mesh = MeshClass::new();
///     mesh.model = Some(Arc::new(model));
///     mesh.transform = transform;
///     // ... add to render queue
/// }
/// ```
pub fn get_fragment_geometry(
    fragment: &MeshFragment,
) -> (Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u32>, Mat4) {
    (
        fragment.vertices.clone(),
        fragment.normals.clone(),
        fragment.tex_coords.clone(),
        fragment.indices.clone(),
        fragment.transform,
    )
}

/// Helper to get the number of triangles in a fragment
pub fn get_fragment_triangle_count(fragment: &MeshFragment) -> usize {
    fragment.triangle_count()
}

/// Update all fragments physics for one timestep
///
/// # Arguments
///
/// * `fragments` - Mutable slice of mesh fragments to update
/// * `delta_time` - Time step in seconds
/// * `gravity` - Gravity vector (usually Vec3::new(0.0, 0.0, -9.8))
pub fn update_fragments(fragments: &mut [MeshFragment], delta_time: f32, gravity: Vec3) {
    for fragment in fragments {
        fragment.update(delta_time, gravity);
    }
}

/// Check if a fragment should be culled (out of bounds or too small)
///
/// # Arguments
///
/// * `fragment` - The fragment to check
/// * `camera_pos` - Camera position for distance culling
/// * `max_distance` - Maximum distance before culling
///
/// # Returns
///
/// true if the fragment should be removed from rendering
pub fn should_cull_fragment(fragment: &MeshFragment, camera_pos: Vec3, max_distance: f32) -> bool {
    // Check if fragment has valid geometry
    if !fragment.is_valid() {
        return true;
    }

    // Distance culling
    let fragment_pos = fragment.transform.w_axis.truncate();
    let distance = (fragment_pos - camera_pos).length();
    if distance > max_distance {
        return true;
    }

    // Check if fragment is too small (very low velocity)
    if fragment.velocity.length_squared() < 0.01 {
        // Fragment has settled, could be culled after a timeout
        return false;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decal_geometry_extraction() {
        let decal_mesh = RigidDecalMesh::new("test".to_string());
        let (verts, normals, uvs, indices) = get_decal_geometry(&decal_mesh);

        assert_eq!(verts.len(), 0); // Empty mesh
        assert_eq!(normals.len(), 0);
        assert_eq!(uvs.len(), 0);
        assert_eq!(indices.len(), 0);
    }

    #[test]
    fn test_fragment_geometry_extraction() {
        let fragment = MeshFragment::new();
        let (verts, normals, uvs, indices, transform) = get_fragment_geometry(&fragment);

        assert_eq!(verts.len(), 0); // Empty fragment
        assert_eq!(normals.len(), 0);
        assert_eq!(uvs.len(), 0);
        assert_eq!(indices.len(), 0);
        assert_eq!(transform, Mat4::IDENTITY);
    }

    #[test]
    fn test_fragment_physics_update() {
        let mut fragment = MeshFragment::new();
        fragment.vertices.push(Vec3::ZERO);
        fragment.indices.push(0);

        let gravity = Vec3::new(0.0, 0.0, -9.8);
        fragment.update(0.1, gravity);

        // Should have gained some velocity from gravity
        assert!(fragment.velocity.z < 0.0);
    }

    #[test]
    fn test_fragment_culling() {
        let mut fragment = MeshFragment::new();
        fragment.vertices.extend([Vec3::ZERO, Vec3::X, Vec3::Y]);
        fragment.indices.extend([0, 1, 2]);
        fragment.transform = Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0));

        let camera_pos = Vec3::ZERO;

        // Should be culled at short distance
        assert!(should_cull_fragment(&fragment, camera_pos, 50.0));

        // Should not be culled at long distance
        assert!(!should_cull_fragment(&fragment, camera_pos, 200.0));
    }
}
