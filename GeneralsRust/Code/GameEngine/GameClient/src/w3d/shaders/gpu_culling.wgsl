// W3D GPU Culling Compute Shader
// Frustum and occlusion culling for GPU-driven rendering

struct CameraData {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    view_projection_matrix: mat4x4<f32>,
    prev_view_projection_matrix: mat4x4<f32>,
    inverse_view_matrix: mat4x4<f32>,
    inverse_projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    camera_direction: vec3<f32>,
    near_plane: f32,
    far_plane: f32,
    fov: f32,
    aspect_ratio: f32,
};

// Frustum planes extracted from view-projection matrix
struct FrustumPlanes {
    planes: array<vec4<f32>, 6>, // Left, Right, Bottom, Top, Near, Far
};

// Object instance data for culling
struct ObjectInstance {
    model_matrix: mat4x4<f32>,
    bounding_sphere: vec4<f32>, // xyz = center, w = radius
    lod_distances: vec4<f32>,    // Distance thresholds for LOD levels
    flags: u32,                  // Visibility flags, enabled/disabled
    mesh_index: u32,             // Index into mesh buffer
    material_index: u32,         // Index into material buffer
    padding: u32,
};

// Draw command for indirect rendering
struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

// Culling statistics
struct CullingStats {
    total_objects: atomic<u32>,
    visible_objects: atomic<u32>,
    frustum_culled: atomic<u32>,
    occlusion_culled: atomic<u32>,
    distance_culled: atomic<u32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(0) @binding(1)
var<storage, read> input_instances: array<ObjectInstance>;

@group(0) @binding(2)
var<storage, read_write> output_instances: array<ObjectInstance>;

@group(0) @binding(3)
var<storage, read_write> draw_commands: array<DrawIndexedIndirect>;

@group(0) @binding(4)
var<storage, read_write> visible_count: atomic<u32>;

@group(0) @binding(5)
var<storage, read_write> stats: CullingStats;

@group(1) @binding(0)
var t_hierarchical_z: texture_2d<f32>; // Hi-Z buffer for occlusion culling

@group(1) @binding(1)
var s_point: sampler;

// Extract frustum planes from view-projection matrix
fn extract_frustum_planes(vp: mat4x4<f32>) -> FrustumPlanes {
    var frustum: FrustumPlanes;

    // Left plane
    frustum.planes[0] = vec4<f32>(
        vp[0][3] + vp[0][0],
        vp[1][3] + vp[1][0],
        vp[2][3] + vp[2][0],
        vp[3][3] + vp[3][0]
    );

    // Right plane
    frustum.planes[1] = vec4<f32>(
        vp[0][3] - vp[0][0],
        vp[1][3] - vp[1][0],
        vp[2][3] - vp[2][0],
        vp[3][3] - vp[3][0]
    );

    // Bottom plane
    frustum.planes[2] = vec4<f32>(
        vp[0][3] + vp[0][1],
        vp[1][3] + vp[1][1],
        vp[2][3] + vp[2][1],
        vp[3][3] + vp[3][1]
    );

    // Top plane
    frustum.planes[3] = vec4<f32>(
        vp[0][3] - vp[0][1],
        vp[1][3] - vp[1][1],
        vp[2][3] - vp[2][1],
        vp[3][3] - vp[3][1]
    );

    // Near plane
    frustum.planes[4] = vec4<f32>(
        vp[0][3] + vp[0][2],
        vp[1][3] + vp[1][2],
        vp[2][3] + vp[2][2],
        vp[3][3] + vp[3][2]
    );

    // Far plane
    frustum.planes[5] = vec4<f32>(
        vp[0][3] - vp[0][2],
        vp[1][3] - vp[1][2],
        vp[2][3] - vp[2][2],
        vp[3][3] - vp[3][2]
    );

    // Normalize planes
    for (var i = 0u; i < 6u; i++) {
        let length = length(frustum.planes[i].xyz);
        frustum.planes[i] /= length;
    }

    return frustum;
}

// Sphere-plane distance test
fn sphere_plane_distance(sphere_center: vec3<f32>, sphere_radius: f32, plane: vec4<f32>) -> f32 {
    return dot(plane.xyz, sphere_center) + plane.w;
}

// Frustum culling test
fn is_sphere_in_frustum(sphere_center: vec3<f32>, sphere_radius: f32, frustum: FrustumPlanes) -> bool {
    for (var i = 0u; i < 6u; i++) {
        let distance = sphere_plane_distance(sphere_center, sphere_radius, frustum.planes[i]);
        if (distance < -sphere_radius) {
            return false; // Outside frustum
        }
    }
    return true; // Inside or intersecting frustum
}

// Project sphere to screen space AABB
fn project_sphere_to_screen(sphere_center: vec3<f32>, sphere_radius: f32, vp: mat4x4<f32>) -> vec4<f32> {
    let center_clip = vp * vec4<f32>(sphere_center, 1.0);
    let center_ndc = center_clip.xyz / center_clip.w;

    // Approximate screen-space radius
    let right = normalize(vec3<f32>(vp[0][0], vp[1][0], vp[2][0]));
    let edge = sphere_center + right * sphere_radius;
    let edge_clip = vp * vec4<f32>(edge, 1.0);
    let edge_ndc = edge_clip.xyz / edge_clip.w;

    let screen_radius = length(edge_ndc.xy - center_ndc.xy);

    // Convert NDC to [0,1] screen space
    let screen_center = center_ndc.xy * 0.5 + 0.5;

    return vec4<f32>(
        screen_center.x - screen_radius,
        screen_center.y - screen_radius,
        screen_center.x + screen_radius,
        screen_center.y + screen_radius
    );
}

// Hierarchical Z-buffer occlusion test
fn is_occluded(screen_aabb: vec4<f32>, depth: f32) -> bool {
    let tex_size = vec2<f32>(textureDimensions(t_hierarchical_z, 0));

    // Clamp AABB to screen bounds
    let aabb = vec4<f32>(
        clamp(screen_aabb.x, 0.0, 1.0),
        clamp(screen_aabb.y, 0.0, 1.0),
        clamp(screen_aabb.z, 0.0, 1.0),
        clamp(screen_aabb.w, 0.0, 1.0)
    );

    // Calculate appropriate mip level based on screen size
    let width = (aabb.z - aabb.x) * tex_size.x;
    let height = (aabb.w - aabb.y) * tex_size.y;
    let max_dim = max(width, height);
    let mip_level = i32(ceil(log2(max_dim)));

    // Sample hierarchical Z at appropriate level
    let uv = (aabb.xy + aabb.zw) * 0.5; // Center of AABB
    let sample_depth = textureSampleLevel(t_hierarchical_z, s_point, uv, f32(mip_level)).r;

    // Object is occluded if it's behind the Z-buffer
    return depth > sample_depth;
}

// Calculate LOD level based on distance
fn calculate_lod(distance: f32, lod_distances: vec4<f32>) -> u32 {
    if (distance < lod_distances.x) {
        return 0u; // LOD 0 (highest detail)
    } else if (distance < lod_distances.y) {
        return 1u; // LOD 1
    } else if (distance < lod_distances.z) {
        return 2u; // LOD 2
    } else if (distance < lod_distances.w) {
        return 3u; // LOD 3 (lowest detail)
    } else {
        return 4u; // Beyond max distance, don't render
    }
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;

    // Bounds check
    if (instance_index >= arrayLength(&input_instances)) {
        return;
    }

    let instance = input_instances[instance_index];

    // Increment total objects counter
    atomicAdd(&stats.total_objects, 1u);

    // Check if instance is enabled
    if ((instance.flags & 1u) == 0u) {
        return; // Disabled, skip
    }

    // Extract world-space bounding sphere
    let world_center = (instance.model_matrix * vec4<f32>(instance.bounding_sphere.xyz, 1.0)).xyz;
    let world_radius = instance.bounding_sphere.w * length(instance.model_matrix[0].xyz); // Uniform scale approximation

    // Distance culling
    let distance_to_camera = length(world_center - camera.camera_position);
    let lod_level = calculate_lod(distance_to_camera, instance.lod_distances);

    if (lod_level >= 4u) {
        atomicAdd(&stats.distance_culled, 1u);
        return; // Too far, don't render
    }

    // Extract frustum planes
    let frustum = extract_frustum_planes(camera.view_projection_matrix);

    // Frustum culling
    if (!is_sphere_in_frustum(world_center, world_radius, frustum)) {
        atomicAdd(&stats.frustum_culled, 1u);
        return; // Outside frustum
    }

    // Occlusion culling (optional, requires Hi-Z buffer)
    let screen_aabb = project_sphere_to_screen(world_center, world_radius, camera.view_projection_matrix);
    let view_space_pos = camera.view_matrix * vec4<f32>(world_center, 1.0);
    let depth = view_space_pos.z / camera.far_plane; // Normalized depth

    if (is_occluded(screen_aabb, depth)) {
        atomicAdd(&stats.occlusion_culled, 1u);
        return; // Occluded by Hi-Z
    }

    // Object is visible, add to output
    let output_index = atomicAdd(&visible_count, 1u);
    output_instances[output_index] = instance;

    // Update draw command (assumes one draw per instance)
    // In a real implementation, this would batch by mesh/material
    draw_commands[output_index].instance_count = 1u;
    draw_commands[output_index].first_instance = output_index;

    atomicAdd(&stats.visible_objects, 1u);
}
