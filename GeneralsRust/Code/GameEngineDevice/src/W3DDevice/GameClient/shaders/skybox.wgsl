// Skybox shader for Command & Conquer Generals Zero Hour
// Renders cubemap skybox with camera-centered positioning and time-of-day tinting

struct SkyboxUniforms {
    view_proj: mat4x4<f32>,  // Camera-centered view-projection matrix
    tint_color: vec4<f32>,    // Sky tint based on time of day
}

@group(0) @binding(0)
var<uniform> uniforms: SkyboxUniforms;

@group(0) @binding(1)
var skybox_texture: texture_cube<f32>;

@group(0) @binding(2)
var skybox_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) texture_coords: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Use vertex position directly as cubemap texture coordinates
    // Since skybox is centered at camera, local position = direction
    output.texture_coords = input.position;

    // Transform position to clip space
    // Note: Position is already camera-centered in the view matrix
    output.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);

    // Set depth to maximum (far plane) to ensure skybox renders behind everything
    // This ensures skybox appears at infinite distance
    output.clip_position.z = output.clip_position.w;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample cubemap using direction vector
    var sky_color = textureSample(skybox_texture, skybox_sampler, input.texture_coords);

    // Apply time-of-day tint
    // Tint color comes from terrain lighting ambient color
    sky_color = sky_color * uniforms.tint_color;

    return sky_color;
}
