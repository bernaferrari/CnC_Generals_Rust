// Terrain Shader for Command & Conquer Generals
// Handles terrain rendering with multi-texturing support

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct TerrainVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) blend_indices: vec4<u32>,
    @location(4) blend_weights: vec4<f32>,
    @location(5) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) blend_weights: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var terrain_texture_0: texture_2d<f32>;
@group(1) @binding(1)
var terrain_texture_1: texture_2d<f32>;
@group(1) @binding(2)
var terrain_texture_2: texture_2d<f32>;
@group(1) @binding(3)
var terrain_texture_3: texture_2d<f32>;
@group(1) @binding(4)
var terrain_sampler: sampler;

@vertex
fn vs_main(vertex: TerrainVertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = vertex.position;
    out.world_position = world_position;
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    out.normal = vertex.normal;
    out.tex_coords = vertex.tex_coords;
    out.blend_weights = vertex.blend_weights;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample all terrain textures
    let color_0 = textureSample(terrain_texture_0, terrain_sampler, in.tex_coords);
    let color_1 = textureSample(terrain_texture_1, terrain_sampler, in.tex_coords);
    let color_2 = textureSample(terrain_texture_2, terrain_sampler, in.tex_coords);
    let color_3 = textureSample(terrain_texture_3, terrain_sampler, in.tex_coords);
    
    // Blend based on vertex blend weights
    let final_color = color_0 * in.blend_weights.x +
                     color_1 * in.blend_weights.y +
                     color_2 * in.blend_weights.z +
                     color_3 * in.blend_weights.w;
    
    // Simple lighting calculation
    let light_dir = normalize(vec3<f32>(0.3, 0.7, 0.2));
    let light_factor = max(dot(normalize(in.normal), light_dir), 0.2);
    
    return vec4<f32>(final_color.rgb * light_factor, final_color.a);
}
