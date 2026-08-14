// C++ W3DShroudMaterialPass for skinned meshes.
// See projected_shroud_basic.wgsl for pass ordering, blending and texture
// semantics.  Skinning remains in the vertex stage before world X/Z
// projection, matching the normal skinned material path.

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
    material_diffuse: vec4<f32>,
    material_specular: vec4<f32>,
    material_emissive: vec4<f32>,
    material_overrides: vec4<f32>,
    visibility_alpha: f32,
    visibility_falloff: f32,
    is_explored: f32,
    visibility_pad: f32,
};

struct BoneUniform {
    bones: array<mat4x4<f32>, 64>,
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
@group(2) @binding(0)
var<uniform> bones: BoneUniform;
@group(2) @binding(1)
var<uniform> uv_transform: UVTransformUniform;
@group(3) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(3) @binding(2)
var shroud_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) _tex_coords0: vec2<f32>,
    @location(3) _tex_coords1: vec2<f32>,
    @location(4) _tex_coords2: vec2<f32>,
    @location(5) _tex_coords3: vec2<f32>,
    @location(6) bone_indices: vec4<u32>,
    @location(7) bone_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var skinned = vec4<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        if vertex.bone_weights[i] > 0.0 {
            skinned += bones.bones[vertex.bone_indices[i]]
                * vec4<f32>(vertex.position, 1.0)
                * vertex.bone_weights[i];
        }
    }
    let world = model.model * skinned;
    out.clip_position = camera.view_proj * world;
    out.world_position = world.xyz;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let projection = model.material_diffuse;
    let uv = input.world_position.xz * projection.xy + projection.zw;
    let level = textureSample(shroud_texture, shroud_sampler, uv).r;
    let tint = model.material_overrides.rgb;
    let factor = select(level * tint, vec3<f32>(1.0), level >= (254.5 / 255.0));
    return vec4<f32>(factor, 1.0);
}
