// Modern WGSL Simple Texture Shader for WWShade
// Basic textured surface with ambient + diffuse lighting

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) vertex_color: vec4<f32>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view_position: vec3<f32>,
}

struct MaterialUniform {
    ambient: vec3<f32>,
    diffuse: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> material: MaterialUniform;
@group(0) @binding(2) var diffuse_texture: texture_2d<f32>;
@group(0) @binding(3) var texture_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.world_position = input.position;
    out.world_normal = normalize(input.normal);
    out.uv = input.uv;
    out.vertex_color = input.color;
    
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(diffuse_texture, texture_sampler, input.uv);
    
    // Simple ambient + diffuse lighting
    let final_color = (material.ambient + material.diffuse) * texture_color.rgb;
    
    return vec4<f32>(final_color * input.vertex_color.rgb, texture_color.a * input.vertex_color.a);
}