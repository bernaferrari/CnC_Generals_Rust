// 3D Camera System for C&C Generals - W3D Model Rendering
// This shader supports proper 3D transformation pipeline: model space -> world space -> view space -> clip space

// Uniforms for camera transformation and lighting
struct Camera {
    view_proj: mat4x4<f32>,
}

struct Model {
    transform: mat4x4<f32>,  // Model transformation matrix (position, rotation, scale)
}

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> model: Model;

@group(2) @binding(0)
var<uniform> light: Light;

// Texture binding for W3D models
@group(3) @binding(0)
var model_texture: texture_2d<f32>;
@group(3) @binding(1)
var model_sampler: sampler;

// W3D Vertex format matching the Rust W3DVertex struct
struct VertexInput {
    @location(0) position: vec3<f32>,    // 3D position in model space
    @location(1) normal: vec3<f32>,      // Normal vector for lighting
    @location(2) uv: vec2<f32>,         // Texture coordinates
    @location(3) color: vec4<f32>,       // Vertex color (RGBA)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,  // Final position in clip space
    @location(0) world_position: vec3<f32>,       // Position in world space for lighting
    @location(1) world_normal: vec3<f32>,         // Transformed normal in world space
    @location(2) uv: vec2<f32>,                  // Pass-through texture coordinates
    @location(3) color: vec4<f32>,               // Pass-through vertex color
}

// Vertex shader - Complete 3D transformation pipeline
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Step 1: Transform from model space to world space using model matrix
    let world_position = model.transform * vec4<f32>(input.position, 1.0);
    out.world_position = world_position.xyz;
    
    // Step 2: Transform normal to world space (use upper-left 3x3 for normals)
    // Note: This assumes uniform scaling. For non-uniform scaling, use inverse transpose
    let normal_matrix = mat3x3<f32>(
        model.transform[0].xyz,
        model.transform[1].xyz,
        model.transform[2].xyz
    );
    out.world_normal = normalize(normal_matrix * input.normal);
    
    // Step 3: Transform from world space to clip space using camera view-projection matrix
    out.clip_position = camera.view_proj * world_position;
    
    // Step 4: Pass through other vertex attributes
    out.uv = input.uv;
    out.color = input.color;
    
    return out;
}

// Fragment shader with proper lighting and texture support
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture using UV coordinates
    let texture_color = textureSample(model_texture, model_sampler, in.uv);
    
    // Calculate lighting
    let light_direction = normalize(light.position - in.world_position);
    let light_factor = max(dot(in.world_normal, light_direction), 0.0);
    
    // Combine texture, vertex color, and lighting
    let base_color = texture_color.rgb * in.color.rgb;
    let lit_color = base_color * light.color * light.intensity * light_factor;
    let ambient = base_color * 0.3; // Ambient lighting component
    
    let final_color = lit_color + ambient;
    let final_alpha = texture_color.a * in.color.a;
    
    return vec4<f32>(final_color, final_alpha);
}

// Legacy vertex shader for simple colored models (backward compatibility)
struct LegacyVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct LegacyVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_legacy(input: LegacyVertexInput) -> LegacyVertexOutput {
    var out: LegacyVertexOutput;
    
    // Apply model transformation and camera projection
    let world_position = model.transform * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.view_proj * world_position;
    out.color = input.color;
    
    return out;
}

@fragment
fn fs_legacy(in: LegacyVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}