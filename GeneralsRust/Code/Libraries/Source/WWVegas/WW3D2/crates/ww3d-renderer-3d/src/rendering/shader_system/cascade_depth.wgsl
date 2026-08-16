// Live first-light cascade depth. C++ W3DDisplay.cpp:1840
// TheW3DProjectedShadowManager->updateRenderTargetTextures() writes
// occluder depth before the scene; this is the wgpu analog.

struct LightViewProj {
    view_proj: mat4x4<f32>,
};

struct ModelUniform {
    model: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> light: LightViewProj;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return light.view_proj * model.model * vec4<f32>(position, 1.0);
}
