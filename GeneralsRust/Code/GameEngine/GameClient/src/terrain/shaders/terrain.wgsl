// Terrain Shader for Command & Conquer Generals Zero Hour
// Diffuse-layer terrain blending matching the live Rust terrain binding model.

const MAX_BLEND_WEIGHTS : u32 = 4u;
const MAX_TEXTURES_PER_CHUNK : u32 = 4u;

struct VertexInput {
    @location(0) position : vec3<f32>;
    @location(1) normal : vec3<f32>;
    @location(2) tex_coords : vec2<f32>;
    @location(3) blend_indices : vec4<u32>;
    @location(4) blend_weights : vec4<f32>;
    @location(5) color : vec4<f32>;
}

struct VertexOutput {
    @builtin(position) clip_position : vec4<f32>;
    @location(0) world_position : vec3<f32>;
    @location(1) normal : vec3<f32>;
    @location(2) tex_coords : vec2<f32>;
    @location(3) blend_indices : vec4<u32>;
    @location(4) blend_weights : vec4<f32>;
    @location(5) color : vec4<f32>;
    @location(6) view_direction : vec3<f32>;
}

struct TerrainUniforms {
    view_proj : mat4x4<f32>;
    view_matrix : mat4x4<f32>;
    projection_matrix : mat4x4<f32>;
    camera_position : vec4<f32>;
    time : f32;
    sun_direction : vec3<f32>;
    _padding0 : f32;
    sun_color : vec3<f32>;
    _padding1 : f32;
    ambient_color : vec3<f32>;
    _padding2 : f32;
    fog_color : vec3<f32>;
    fog_start : f32;
    fog_end : f32;
    _padding3 : f32;
    _padding4 : f32;
}

@group(0) @binding(0)
var<uniform> uniforms : TerrainUniforms;

// Diffuse textures (bindings 0-3)
@group(1) @binding(0) var terrain_diffuse_0 : texture_2d<f32>;
@group(1) @binding(1) var terrain_diffuse_1 : texture_2d<f32>;
@group(1) @binding(2) var terrain_diffuse_2 : texture_2d<f32>;
@group(1) @binding(3) var terrain_diffuse_3 : texture_2d<f32>;
@group(1) @binding(4) var terrain_sampler : sampler;

@vertex
fn vs_main(input : VertexInput) -> VertexOutput {
    var out : VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    out.world_position = input.position;
    out.normal = normalize(input.normal);
    out.tex_coords = input.tex_coords;
    out.blend_indices = input.blend_indices;
    out.blend_weights = input.blend_weights;
    out.color = input.color;
    out.view_direction = normalize(uniforms.camera_position.xyz - input.position);
    return out;
}

// Helper function to sample diffuse texture by index
fn sample_diffuse(index: u32, coords: vec2<f32>) -> vec4<f32> {
    switch index {
        case 0u: { return textureSample(terrain_diffuse_0, terrain_sampler, coords); }
        case 1u: { return textureSample(terrain_diffuse_1, terrain_sampler, coords); }
        case 2u: { return textureSample(terrain_diffuse_2, terrain_sampler, coords); }
        case 3u: { return textureSample(terrain_diffuse_3, terrain_sampler, coords); }
        default: { return vec4<f32>(0.5, 0.5, 0.5, 1.0); }
    }
}

@fragment
fn fs_main(input : VertexOutput) -> @location(0) vec4<f32> {
    var blended_color : vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    for (var i : u32 = 0u; i < MAX_BLEND_WEIGHTS; i = i + 1u) {
        let weight = input.blend_weights[i];
        if (weight <= 0.0) {
            continue;
        }

        let texture_index = input.blend_indices[i];
        if (texture_index >= MAX_TEXTURES_PER_CHUNK) {
            continue;
        }

        let sample_color = sample_diffuse(texture_index, input.tex_coords);
        blended_color = blended_color + sample_color * weight;
    }

    blended_color = blended_color * input.color;

    let final_normal = normalize(input.normal);
    let sun_dir = normalize(-uniforms.sun_direction);
    let diffuse = max(dot(final_normal, sun_dir), 0.0);
    let reflect_dir = reflect(-sun_dir, final_normal);
    let specular = pow(max(dot(input.view_direction, reflect_dir), 0.0), 32.0);
    let lighting = uniforms.ambient_color + uniforms.sun_color * diffuse + uniforms.sun_color * specular * 0.2;

    var final_color = blended_color.rgb * lighting;
    let distance_to_camera = length(uniforms.camera_position - input.world_position);
    let fog_factor = clamp((distance_to_camera - uniforms.fog_start) / (uniforms.fog_end - uniforms.fog_start), 0.0, 1.0);
    final_color = mix(final_color, uniforms.fog_color, fog_factor);

    return vec4<f32>(final_color, blended_color.a);
}
