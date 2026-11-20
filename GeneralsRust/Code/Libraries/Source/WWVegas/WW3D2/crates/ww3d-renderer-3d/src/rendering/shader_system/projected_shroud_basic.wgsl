// C++ W3DShroudMaterialPass for rigid meshes.
//
// The pass is deliberately separate from authored material shaders.  C++
// projects the shroud from camera-space/world coordinates and draws it after
// the object's regular material work with depth Equal, no depth writes and
// Zero/SrcColor destination multiplication.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    eye_position: vec4<f32>,
};

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    texture_stage_mask: vec4<u32>,
    texture_stage_uv_map: vec4<u32>,
    // For this pass material_diffuse stores
    // [uv_scale.x, uv_scale.y, uv_offset.x, uv_offset.y].
    material_diffuse: vec4<f32>,
    material_specular: vec4<f32>,
    material_emissive: vec4<f32>,
    // For this pass material_overrides stores [shroud_rgb, 1].
    material_overrides: vec4<f32>,
    visibility_alpha: f32,
    visibility_falloff: f32,
    is_explored: f32,
    visibility_pad: f32,
};

struct UVTransformUniform {
    mapper_meta: vec4<u32>,
    mapper_args: vec4<i32>,
    mapper_float_args: vec4<f32>,
    animation: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

// The compatibility pipeline keeps the normal basic mesh group-2 layout.
// Authored UVs are intentionally not used by this shader.
@group(2) @binding(0)
var<uniform> uv_transform: UVTransformUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) _tex_coords0: vec2<f32>,
    @location(3) _tex_coords1: vec2<f32>,
    @location(4) _tex_coords2: vec2<f32>,
    @location(5) _tex_coords3: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = model.model * vec4<f32>(vertex.position, 1.0);
    out.clip_position = camera.view_proj * world;
    out.world_position = world.xyz;
    return out;
}
