// W3D Terrain Rendering Shader
// Advanced terrain rendering with height maps, multi-texturing, and roads

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

struct TerrainData {
    world_size: vec2<f32>,
    height_scale: f32,
    lod_distances: vec4<f32>,
    texture_scales: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(1) @binding(0)
var<uniform> terrain: TerrainData;

@group(2) @binding(0)
var t_heightmap: texture_2d<f32>;
@group(2) @binding(1)
var t_texture0: texture_2d<f32>; // Grass
@group(2) @binding(2)
var t_texture1: texture_2d<f32>; // Rock
@group(2) @binding(3)
var t_texture2: texture_2d<f32>; // Sand
@group(2) @binding(4)
var t_texture3: texture_2d<f32>; // Snow
@group(2) @binding(5)
var t_splat_map: texture_2d<f32>; // RGBA weights for 4 textures
@group(2) @binding(6)
var t_normal_map: texture_2d<f32>;
@group(2) @binding(7)
var s_terrain: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>, // XZ coordinates
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) texture_coords: vec4<f32>, // UV for 4 different texture scales
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Sample heightmap for Y coordinate
    let height = textureSampleLevel(t_heightmap, s_terrain, vertex.uv, 0.0).r * terrain.height_scale;
    let world_pos = vec3<f32>(vertex.position.x, height, vertex.position.y);
    
    // Calculate normal from heightmap
    let texel_size = 1.0 / textureDimensions(t_heightmap);
    let h_left = textureSampleLevel(t_heightmap, s_terrain, vertex.uv - vec2<f32>(texel_size.x, 0.0), 0.0).r;
    let h_right = textureSampleLevel(t_heightmap, s_terrain, vertex.uv + vec2<f32>(texel_size.x, 0.0), 0.0).r;
    let h_down = textureSampleLevel(t_heightmap, s_terrain, vertex.uv - vec2<f32>(0.0, texel_size.y), 0.0).r;
    let h_up = textureSampleLevel(t_heightmap, s_terrain, vertex.uv + vec2<f32>(0.0, texel_size.y), 0.0).r;
    
    let normal = normalize(vec3<f32>(
        (h_left - h_right) * terrain.height_scale,
        2.0,
        (h_down - h_up) * terrain.height_scale
    ));
    
    out.world_position = world_pos;
    out.world_normal = normal;
    out.uv = vertex.uv;
    out.clip_position = camera.view_projection_matrix * vec4<f32>(world_pos, 1.0);
    
    // Calculate texture coordinates for different scales
    out.texture_coords = vec4<f32>(
        vertex.uv * terrain.texture_scales.x,  // Detail scale
        vertex.uv * terrain.texture_scales.y   // Mid scale
    );
    
    return out;
}

fn sample_terrain_texture(uv: vec2<f32>, weights: vec4<f32>) -> vec3<f32> {
    let tex0 = textureSample(t_texture0, s_terrain, uv).rgb * weights.x;
    let tex1 = textureSample(t_texture1, s_terrain, uv).rgb * weights.y;
    let tex2 = textureSample(t_texture2, s_terrain, uv).rgb * weights.z;
    let tex3 = textureSample(t_texture3, s_terrain, uv).rgb * weights.w;
    
    return tex0 + tex1 + tex2 + tex3;
}

fn compute_tbn(world_pos: vec3<f32>, uv: vec2<f32>, normal: vec3<f32>) -> mat3x3<f32> {
    let dp1 = dpdx(world_pos);
    let dp2 = dpdy(world_pos);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);

    let n = normalize(normal);
    let t = normalize(dp1 * duv2.y - dp2 * duv1.y);
    let tangent = normalize(t - n * dot(n, t));
    let bitangent = normalize(cross(n, tangent));

    return mat3x3<f32>(tangent, bitangent, n);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample splat map for texture weights
    let splat_weights = textureSample(t_splat_map, s_terrain, in.uv);
    
    // Normalize weights
    let total_weight = splat_weights.x + splat_weights.y + splat_weights.z + splat_weights.w;
    let weights = select(splat_weights / total_weight, vec4<f32>(1.0, 0.0, 0.0, 0.0), total_weight < 0.001);
    
    // Sample terrain textures at different scales and blend
    let detail_color = sample_terrain_texture(in.texture_coords.xy, weights);
    let mid_color = sample_terrain_texture(in.texture_coords.zw, weights);
    
    // Blend detail and mid textures based on distance
    let distance_to_camera = length(camera.camera_position - in.world_position);
    let blend_factor = smoothstep(50.0, 200.0, distance_to_camera);
    let final_color = mix(detail_color, mid_color, blend_factor);
    
    // Sample normal map and apply
    let normal_sample = textureSample(t_normal_map, s_terrain, in.texture_coords.xy);
    let normal_map = normalize(normal_sample.rgb * 2.0 - 1.0);
    let tbn = compute_tbn(in.world_position, in.texture_coords.xy, in.world_normal);
    let final_normal = normalize(tbn * normal_map);
    
    // Basic lighting
    let sun_direction = normalize(vec3<f32>(-0.5, 0.8, -0.3));
    let ndotl = max(dot(final_normal, sun_direction), 0.0);
    let ambient = 0.2;
    let lighting = ambient + ndotl * 0.8;
    
    return vec4<f32>(final_color * lighting, 1.0);
}
