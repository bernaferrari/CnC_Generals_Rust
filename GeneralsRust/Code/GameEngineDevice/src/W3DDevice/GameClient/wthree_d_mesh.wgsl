// Mesh Rendering Shader with Texture Sampling and Directional Lighting
//
// Corresponds to C++ shader setup in:
// - W3DDevice/GameClient/W3DMesh.cpp
// - W3DDevice/GameClient/W3DMeshMatInfoClass.cpp (material properties)
//
// Supports diffuse texture sampling, basic directional lighting (N dot L),
// material ambient/diffuse/emissive properties, and vertex-color fallback
// when no texture is bound.

struct MeshUniforms {
    view_proj: mat4x4<f32>,
    world: mat4x4<f32>,
};

struct SceneLighting {
    light_direction: vec4<f32>,  // xyz = normalized light direction (toward light), w = unused
    light_color: vec4<f32>,      // xyz = directional light color, w = unused
    ambient_color: vec4<f32>,    // xyz = ambient light color, w = unused
};

struct Material {
    ambient_color: vec4<f32>,    // xyz = material ambient reflectance, w = unused
    diffuse_color: vec4<f32>,    // xyz = material diffuse reflectance, w = unused
    emissive_color: vec4<f32>,   // xyz = self-illumination, w = unused
    shininess: f32,              // specular exponent (reserved for future use)
    opacity: f32,                // per-material opacity override
    has_texture: f32,            // 1.0 if diffuse_map bound, 0.0 for vertex-color fallback
    pad: f32,
};

// Group 0: per-frame + per-object uniforms
@group(0) @binding(0)
var<uniform> uniforms: MeshUniforms;

@group(0) @binding(1)
var<uniform> lighting: SceneLighting;

// Group 1: per-material / per-texture
@group(1) @binding(0)
var diffuse_map: texture_2d<f32>;

@group(1) @binding(1)
var diffuse_sampler: sampler;

@group(1) @binding(2)
var<uniform> material: Material;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.world * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_pos;
    output.world_position = world_pos.xyz;
    // Transform normal by upper-left 3x3 of world matrix.
    // Works correctly for uniform scale; for non-uniform scale the inverse
    // transpose would be needed (C++ W3D uses the same simplified path).
    output.world_normal = normalize((uniforms.world * vec4<f32>(input.normal, 0.0)).xyz);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Base color: texture sample or vertex-color fallback
    var base_color: vec4<f32>;
    if (material.has_texture > 0.5) {
        base_color = textureSample(diffuse_map, diffuse_sampler, input.uv);
    } else {
        base_color = input.color;
    }

    // Directional lighting: classic Lambertian N dot L
    let N = normalize(input.world_normal);
    let L = normalize(lighting.light_direction.xyz);
    let NdotL = max(dot(N, L), 0.0);

    // Combine ambient + diffuse + emissive
    // Matches C++ MeshMatInfoClass lighting pipeline
    let ambient = lighting.ambient_color.xyz * material.ambient_color.xyz;
    let diffuse = lighting.light_color.xyz * NdotL * material.diffuse_color.xyz * base_color.xyz;
    let emissive = material.emissive_color.xyz;

    let final_color = ambient + diffuse + emissive;

    return vec4<f32>(final_color, base_color.a * material.opacity);
}
