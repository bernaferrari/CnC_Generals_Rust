// Advanced Particle Shader with GPU Instancing
// Supports all features from the original WW3D particle system

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
}

struct InstanceInput {
    @location(2) instance_transform: vec4<f32>,  // xyz = position, w = size
    @location(3) instance_color: vec4<f32>,      // rgba
    @location(4) instance_rotation_frame: vec4<f32>, // x = rotation, y = frame, z = blur_time, w = unused
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) frame_info: vec2<f32>,  // frame index, blur_time
}

struct Uniforms {
    view_projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    screen_size: vec2<f32>,
    depth_fade_params: vec2<f32>,  // x = fade_distance, y = enabled
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let rotation = instance.instance_rotation_frame.x;
    let size = instance.instance_transform.w;
    let position = instance.instance_transform.xyz;

    // Billboard rotation to face camera
    let to_camera = normalize(uniforms.camera_position - position);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, to_camera));
    let billboard_up = cross(to_camera, right);

    // Apply particle rotation
    let cos_rot = cos(rotation);
    let sin_rot = sin(rotation);

    let rotated_pos = vec2<f32>(
        vertex.position.x * cos_rot - vertex.position.y * sin_rot,
        vertex.position.x * sin_rot + vertex.position.y * cos_rot,
    );

    // Scale and position the particle
    let scaled_pos = rotated_pos * size;
    let world_offset = right * scaled_pos.x + billboard_up * scaled_pos.y;
    let world_pos = vec4<f32>(position + world_offset, 1.0);

    var out: VertexOutput;
    out.clip_position = uniforms.view_projection * world_pos;
    out.tex_coord = vertex.tex_coord;
    out.color = instance.instance_color;
    out.world_position = world_pos.xyz;
    out.frame_info = vec2<f32>(
        instance.instance_rotation_frame.y,  // frame
        instance.instance_rotation_frame.z   // blur_time
    );
    return out;
}

@group(1) @binding(0)
var texture_sampler: sampler;
@group(1) @binding(1)
var texture_view: texture_2d<f32>;
@group(1) @binding(2)
var depth_texture: texture_depth_2d;

// Frame animation support for texture atlases
fn get_frame_tex_coord(base_coord: vec2<f32>, frame: f32, frames_per_row: f32) -> vec2<f32> {
    let frame_index = floor(frame);
    let frame_row = floor(frame_index / frames_per_row);
    let frame_col = frame_index - frame_row * frames_per_row;

    let frame_size = 1.0 / frames_per_row;
    let frame_offset = vec2<f32>(frame_col * frame_size, frame_row * frame_size);

    return frame_offset + base_coord * frame_size;
}

// Soft particles implementation
fn calculate_soft_fade(world_pos: vec3<f32>, screen_pos: vec4<f32>) -> f32 {
    if (uniforms.depth_fade_params.y < 0.5) {
        return 1.0; // Soft particles disabled
    }

    let screen_uv = screen_pos.xy / screen_pos.w * 0.5 + 0.5;
    let scene_depth = textureSample(depth_texture, texture_sampler, screen_uv);
    let particle_depth = screen_pos.z / screen_pos.w;

    let depth_diff = scene_depth - particle_depth;
    let fade_factor = saturate(depth_diff / uniforms.depth_fade_params.x);

    return fade_factor;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture with frame animation support
    let frames_per_row = 4.0; // Could be a uniform
    let tex_coord = get_frame_tex_coord(in.tex_coord, in.frame_info.x, frames_per_row);
    let tex_color = textureSample(texture_view, texture_sampler, tex_coord);

    // Base color
    var final_color = tex_color * in.color;

    // Soft particle fade
    let soft_fade = calculate_soft_fade(in.world_position, in.clip_position);
    final_color.a *= soft_fade;

    // Motion blur effect (simplified)
    if (in.frame_info.y > 0.0) {
        // In a full implementation, this would sample multiple texture offsets
        // For now, just reduce alpha slightly for blur effect
        final_color.a *= 0.8;
    }

    // Distance fade for LOD
    let distance_to_camera = length(uniforms.camera_position - in.world_position);
    let distance_fade = saturate(1.0 - distance_to_camera / 1000.0);
    final_color.a *= distance_fade;

    return final_color;
}