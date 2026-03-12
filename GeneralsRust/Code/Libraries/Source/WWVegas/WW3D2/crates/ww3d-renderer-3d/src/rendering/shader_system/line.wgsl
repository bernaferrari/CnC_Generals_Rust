// Line rendering shader for debug wireframes and UI elements
// Supports both 2D and 3D line rendering

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    eye_position: vec4<f32>,
};

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct PackedLight {
    direction: vec4<f32>,
    color: vec4<f32>,
    position_range: vec4<f32>,
    spot_params: vec4<f32>,
};

struct LightingUniform {
    ambient_color: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    light_meta: vec4<f32>,
    lights: array<PackedLight, 8>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

@group(1) @binding(1)
var<uniform> lighting: LightingUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) fog_factor: f32,
};

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Transform position to world space
    let world_pos = model.model * vec4<f32>(vertex.position, 1.0);

    // Transform to clip space
    out.clip_position = camera.view_proj * world_pos;

    // Pass through vertex color
    out.color = vertex.color;

    // Calculate fog factor if fog is enabled
    if lighting.fog_params.z > 0.0 {
        let fog_start = lighting.fog_params.x;
        let fog_end = lighting.fog_params.y;
        let view_distance = length(camera.eye_position.xyz - world_pos.xyz);
        out.fog_factor = clamp((fog_end - view_distance) / (fog_end - fog_start), 0.0, 1.0);
    } else {
        out.fog_factor = 1.0;
    }

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = in.color;

    // Apply fog if enabled (blend with fog color)
    if lighting.fog_params.z > 0.0 {
        color = vec4<f32>(
            mix(lighting.fog_color.xyz, color.rgb, in.fog_factor),
            color.a
        );
    }

    return color;
}
