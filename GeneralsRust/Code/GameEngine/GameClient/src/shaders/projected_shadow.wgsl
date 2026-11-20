struct Camera { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var shadow: texture_2d<f32>;
@group(1) @binding(1) var shadow_sampler: sampler;

struct ProjectedVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>) -> ProjectedVertex {
    return ProjectedVertex(camera.view_proj * vec4<f32>(position, 1.0), uv);
}

@fragment
fn fs_main(in: ProjectedVertex) -> @location(0) vec4<f32> {
    // W3D SC_MULTIPLICATIVE uses source RGB as the destination factor.
    return textureSample(shadow, shadow_sampler, in.uv);
}
