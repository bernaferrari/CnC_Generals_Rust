// W3DTreeBuffer wgpu draw: C++ Set_Texture(0, m_treeTexture) + detailAlphaShader.
// Group 0 = camera (shared). Group 1 = tree atlas only — not the road/terrain groups.
struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct TreeVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) packed_diffuse: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var tree_atlas: texture_2d<f32>;
@group(1) @binding(1)
var tree_sampler: sampler;

@vertex
fn vs_main(vertex: TreeVertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(vertex.position, 1.0);
    out.color = vertex.color;
    out.tex_coords = vertex.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(tree_atlas, tree_sampler, in.tex_coords);
    // C++ detailAlphaShader alpha-test leaves; keep a floor so unlit verts are visible.
    if (tex.a < 0.25) {
        discard;
    }
    let lit = max(in.color, vec3<f32>(0.28));
    return vec4<f32>(lit * tex.rgb, tex.a);
}
