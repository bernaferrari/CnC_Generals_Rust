// Terrain Rendering Shader for C&C Generals Zero Hour
//
// Corresponds to C++ shader setup in:
// - GameEngineDevice/Source/W3DDevice/GameClient/HeightMap.cpp
// - GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp
//
// Implements multi-layer texture blending, lighting, and fog for terrain rendering

struct TerrainUniforms {
    view_proj: mat4x4<f32>,
    ambient_light: vec3<f32>,
    light_direction: vec3<f32>,
    light_color: vec3<f32>,
    fog_params: vec4<f32>, // start, end, density, unused
    time: f32,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) diffuse: u32,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) diffuse: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: TerrainUniforms;

@group(0) @binding(1)
var base_texture: texture_2d<f32>;

@group(0) @binding(2)
var base_sampler: sampler;

@group(0) @binding(3)
var detail_texture: texture_2d<f32>;

@group(0) @binding(4)
var detail_sampler: sampler;

@group(0) @binding(5)
var blend_texture: texture_2d<f32>;

@group(0) @binding(6)
var blend_sampler: sampler;

// Unpack RGBA8 color from u32 (matches C++ diffuse color format)
fn unpack_color(packed: u32) -> vec4<f32> {
    let r = f32((packed >> 16u) & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32(packed & 0xFFu) / 255.0;
    let a = f32((packed >> 24u) & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform position to clip space
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_position = in.position;

    // Unpack vertex color (contains pre-computed lighting)
    out.diffuse = unpack_color(in.diffuse);

    // Pass through UVs
    out.uv0 = in.uv0;
    out.uv1 = in.uv1;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base texture (terrain tiles)
    var base_color = textureSample(base_texture, base_sampler, in.uv0);

    // Sample detail texture (fine details, cliff textures)
    var detail_color = textureSample(detail_texture, detail_sampler, in.uv1);

    // Sample blend map (controls texture mixing)
    var blend_factor = textureSample(blend_texture, blend_sampler, in.uv0).r;

    // Blend base and detail textures
    // C++ uses alpha blending for texture layers (BaseHeightMap.cpp)
    var terrain_color = mix(base_color, detail_color, blend_factor);

    // Apply vertex lighting (pre-computed static + dynamic)
    // C++ computes this in updateVB and updateVBForLight
    var lit_color = terrain_color * in.diffuse;

    // Add ambient light
    lit_color.r = lit_color.r + uniforms.ambient_light.r * 0.1;
    lit_color.g = lit_color.g + uniforms.ambient_light.g * 0.1;
    lit_color.b = lit_color.b + uniforms.ambient_light.b * 0.1;

    // Apply directional light (sun)
    // Note: Normals would be passed from vertex shader for proper lighting
    // For now, using vertex color which contains pre-lit values

    // Apply distance fog
    let fog_start = uniforms.fog_params.x;
    let fog_end = uniforms.fog_params.y;
    let fog_density = uniforms.fog_params.z;

    let distance = length(in.world_position);
    let fog_factor = clamp((fog_end - distance) / (fog_end - fog_start), 0.0, 1.0);

    // Fog color (usually sky color)
    let fog_color = vec3<f32>(0.7, 0.8, 0.9);

    // Mix terrain color with fog
    var final_color = mix(vec3<f32>(fog_color), lit_color.rgb, fog_factor);

    return vec4<f32>(final_color, 1.0);
}

// Fragment shader variant for cliff rendering
// Uses different texture coordinates and blending for steep slopes
@fragment
fn fs_cliff(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate slope based on world position derivative (approximation)
    // C++ calculates this in isCliffCell and uses cliff textures

    // Sample cliff texture with different UV scaling
    let cliff_uv = in.uv0 * 2.0; // Tighter tiling for cliffs
    var cliff_color = textureSample(detail_texture, detail_sampler, cliff_uv);

    // Blend with base terrain
    let base_color = textureSample(base_texture, base_sampler, in.uv0);
    let cliff_factor = 0.7; // Would be computed from slope in real implementation

    var terrain_color = mix(base_color, cliff_color, cliff_factor);

    // Apply lighting
    var lit_color = terrain_color * in.diffuse;

    return vec4<f32>(lit_color.rgb, 1.0);
}

// Fragment shader variant for shoreline blending
// Special handling for terrain/water transitions
@fragment
fn fs_shoreline(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample terrain
    var terrain_color = textureSample(base_texture, base_sampler, in.uv0);

    // Calculate water edge alpha (from C++ renderShoreLines)
    // Uses depth-based alpha for smooth transitions
    let water_depth = in.world_position.z; // Simplified
    let edge_alpha = clamp(water_depth / 10.0, 0.0, 1.0);

    // Apply lighting
    var lit_color = terrain_color * in.diffuse;

    return vec4<f32>(lit_color.rgb, edge_alpha);
}
