// Water Shader for Command & Conquer Generals
// Handles water rendering with reflection and transparency

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct WaterVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(vertex: WaterVertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = vertex.position;
    out.world_position = world_position;
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    out.normal = vertex.normal;
    out.tex_coords = vertex.tex_coords;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let wave = sin(in.tex_coords.x * 12.0) * cos(in.tex_coords.y * 9.0);
    let perturbed_normal = normalize(in.normal + vec3<f32>(wave * 0.08, 0.12, wave * 0.05));
    let view_dir = normalize(camera.position - in.world_position);
    let fresnel = pow(1.0 - max(dot(view_dir, perturbed_normal), 0.0), 3.0);
    let shallow = vec3<f32>(0.05, 0.16, 0.22);
    let deep = vec3<f32>(0.14, 0.34, 0.56);
    let final_color = mix(shallow, deep, 0.45 + fresnel * 0.35);
    let alpha = 0.45 + fresnel * 0.25;
    return vec4<f32>(final_color, alpha);
}
