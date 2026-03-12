// Modern WGSL Cube Map Shader for WWShade
// Environment mapping with reflection

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) reflection_dir: vec3<f32>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view_position: vec3<f32>,
}

struct MaterialUniform {
    specular: vec3<f32>,
    reflection_strength: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> material: MaterialUniform;
@group(0) @binding(2) var diffuse_texture: texture_2d<f32>;
@group(0) @binding(3) var environment_map: texture_cube<f32>;
@group(0) @binding(4) var texture_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.world_position = input.position;
    out.world_normal = normalize(input.normal);
    out.uv = input.uv;
    
    // Calculate reflection direction for cubemap lookup
    let view_dir = normalize(input.position - camera.view_position);
    out.reflection_dir = reflect(view_dir, out.world_normal);
    
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base texture
    let base_color = textureSample(diffuse_texture, texture_sampler, input.uv);
    
    // Sample environment map using reflection direction
    let env_color = textureSample(environment_map, texture_sampler, input.reflection_dir);
    
    // Blend base color with environment reflection
    let final_color = mix(base_color.rgb, env_color.rgb * material.specular, material.reflection_strength);
    
    return vec4<f32>(final_color, base_color.a);
}