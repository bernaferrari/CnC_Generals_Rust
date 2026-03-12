// GPU Culling Compute Shader for W3D Performance Optimization
// Performs frustum culling and occlusion culling on the GPU

struct CullData {
    bounds_min: vec3<f32>,
    _padding1: f32,
    bounds_max: vec3<f32>,
    _padding2: f32,
    transform: mat4x4<f32>,
}

struct FrustumData {
    planes: array<vec4<f32>, 6>, // Six frustum planes (normal.xyz, distance)
    camera_position: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(0) var<storage, read> objects: array<CullData>;
@group(0) @binding(1) var<storage, read_write> visibility: array<u32>;
@group(0) @binding(2) var<uniform> frustum: FrustumData;

// Test if axis-aligned bounding box is visible in frustum
fn is_aabb_visible(bounds_min: vec3<f32>, bounds_max: vec3<f32>) -> bool {
    for (var i = 0u; i < 6u; i++) {
        let plane = frustum.planes[i];
        let normal = plane.xyz;
        let distance = plane.w;
        
        // Get positive vertex (farthest point from plane)
        var positive_vertex = vec3<f32>(
            select(bounds_min.x, bounds_max.x, normal.x >= 0.0),
            select(bounds_min.y, bounds_max.y, normal.y >= 0.0),
            select(bounds_min.z, bounds_max.z, normal.z >= 0.0)
        );
        
        // If positive vertex is behind plane, AABB is outside frustum
        if (dot(normal, positive_vertex) + distance < 0.0) {
            return false;
        }
    }
    
    return true;
}

// Calculate screen-space size of bounding box for LOD
fn calculate_screen_size(bounds_min: vec3<f32>, bounds_max: vec3<f32>, camera_pos: vec3<f32>) -> f32 {
    let center = (bounds_min + bounds_max) * 0.5;
    let size = length(bounds_max - bounds_min);
    let distance = length(center - camera_pos);
    
    // Approximate screen size based on distance
    return size / max(distance, 1.0);
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    // Bounds check
    if (index >= arrayLength(&objects)) {
        return;
    }
    
    let object = objects[index];
    
    // Transform bounding box to world space
    let world_min = (object.transform * vec4<f32>(object.bounds_min, 1.0)).xyz;
    let world_max = (object.transform * vec4<f32>(object.bounds_max, 1.0)).xyz;
    
    // Ensure min/max are correct after transformation
    let actual_min = min(world_min, world_max);
    let actual_max = max(world_min, world_max);
    
    // Frustum culling
    var is_visible = is_aabb_visible(actual_min, actual_max);
    
    // Additional culling tests could go here:
    // - Occlusion culling
    // - Back-face culling for large objects
    // - Distance culling
    
    // Distance culling (objects beyond far plane)
    let center = (actual_min + actual_max) * 0.5;
    let distance_to_camera = length(center - frustum.camera_position);
    if (distance_to_camera > 2000.0) { // Far cull distance
        is_visible = false;
    }
    
    // Screen-space culling (objects too small to see)
    let screen_size = calculate_screen_size(actual_min, actual_max, frustum.camera_position);
    if (screen_size < 0.001) { // Minimum screen size threshold
        is_visible = false;
    }
    
    // Write visibility result
    visibility[index] = select(0u, 1u, is_visible);
}