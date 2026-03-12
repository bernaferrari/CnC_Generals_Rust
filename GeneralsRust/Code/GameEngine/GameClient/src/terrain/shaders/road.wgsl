// Road Shader for Command & Conquer Generals Zero Hour
// Road and path rendering with blending and wear effects

struct RoadVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) road_width: f32,
}

struct RoadVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) road_width: f32,
    @location(4) distance_from_center: f32,
    @location(5) view_direction: vec3<f32>,
}

struct RoadUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    road_color: vec4<f32>,
    wear_intensity: f32,
    edge_fade: f32,
    specular_strength: f32,
    ambient_color: vec3<f32>,
    sun_direction: vec3<f32>,
    sun_color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: RoadUniforms;

@group(0) @binding(1)
var road_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var road_sampler: sampler;

@group(0) @binding(3)
var road_normal: texture_2d<f32>;

@group(0) @binding(4)
var wear_mask: texture_2d<f32>;

@group(0) @binding(5)
var edge_blend: texture_2d<f32>;

@group(0) @binding(6)
var terrain_texture: texture_2d<f32>;

@vertex
fn vs_main(input: RoadVertexInput) -> RoadVertexOutput {
    var out: RoadVertexOutput;
    
    out.world_position = input.position;
    
    // Transform to clip space
    let view_pos = uniforms.view_matrix * vec4<f32>(input.position, 1.0);
    out.clip_position = uniforms.projection_matrix * view_pos;
    
    out.normal = normalize(input.normal);
    out.tex_coords = input.tex_coords;
    out.road_width = input.road_width;
    
    // Calculate distance from road center (for edge blending)
    // This is a simplified calculation - in reality, road geometry would provide this
    out.distance_from_center = abs(input.tex_coords.x - 0.5) * 2.0;
    
    // Calculate view direction
    out.view_direction = normalize(uniforms.camera_position - input.position);
    
    return out;
}

@fragment
fn fs_main(input: RoadVertexOutput) -> @location(0) vec4<f32> {
    // Sample road diffuse texture
    let road_color = textureSample(road_diffuse, road_sampler, input.tex_coords);
    
    // Sample road normal map
    let normal_sample = textureSample(road_normal, road_sampler, input.tex_coords);
    let detail_normal = normalize(normal_sample.xyz * 2.0 - 1.0);
    
    // Combine with vertex normal
    let final_normal = normalize(mix(input.normal, detail_normal, 0.5));
    
    // Sample wear mask for road aging effects
    let wear = textureSample(wear_mask, road_sampler, input.tex_coords * 2.0);
    let wear_factor = wear.r * uniforms.wear_intensity;
    
    // Apply wear to road color
    var final_road_color = road_color.rgb * uniforms.road_color.rgb;
    final_road_color = mix(final_road_color, final_road_color * 0.7, wear_factor);
    
    // Sample terrain texture for blending at edges
    let terrain_color = textureSample(terrain_texture, road_sampler, input.world_position.xz * 0.1);
    
    // Calculate edge blend factor
    let edge_factor = smoothstep(0.7, 1.0, input.distance_from_center);
    let edge_blend_sample = textureSample(edge_blend, road_sampler, input.tex_coords);
    let final_edge_factor = edge_factor * edge_blend_sample.r * uniforms.edge_fade;
    
    // Blend road with terrain at edges
    final_road_color = mix(final_road_color, terrain_color.rgb, final_edge_factor);
    
    // Calculate lighting
    let sun_dir = normalize(-uniforms.sun_direction);
    let diffuse = max(dot(final_normal, sun_dir), 0.0);
    
    // Specular reflection (roads are somewhat reflective when wet)
    let reflect_dir = reflect(-sun_dir, final_normal);
    let specular = pow(max(dot(input.view_direction, reflect_dir), 0.0), 64.0) * uniforms.specular_strength;
    
    // Combine lighting
    let lighting = uniforms.ambient_color + 
                  uniforms.sun_color * diffuse + 
                  uniforms.sun_color * specular;
    
    // Apply lighting
    final_road_color *= lighting;
    
    // Calculate alpha based on distance from center (for soft edges)
    let alpha = 1.0 - smoothstep(0.8, 1.0, input.distance_from_center);
    alpha = max(alpha, 1.0 - final_edge_factor);
    
    return vec4<f32>(final_road_color, alpha);
}

// Utility functions for road effects

fn calculate_road_fade(distance_from_center: f32, road_width: f32, fade_width: f32) -> f32 {
    let edge_start = (road_width - fade_width) * 0.5;
    let edge_end = road_width * 0.5;
    
    if distance_from_center < edge_start {
        return 1.0;
    } else if distance_from_center > edge_end {
        return 0.0;
    } else {
        return 1.0 - smoothstep(edge_start, edge_end, distance_from_center);
    }
}

fn apply_road_wear(base_color: vec3<f32>, wear_factor: f32, dirt_color: vec3<f32>) -> vec3<f32> {
    // Apply wear effects - roads get darker and more brown over time
    let worn_color = mix(base_color, dirt_color, wear_factor);
    return mix(base_color, worn_color, wear_factor);
}

fn calculate_wet_road_effect(base_color: vec3<f32>, normal: vec3<f32>, view_dir: vec3<f32>, wetness: f32) -> vec3<f32> {
    // Wet roads are darker and more reflective
    let darkened = base_color * (1.0 - wetness * 0.3);
    
    // Add specular highlight for wet effect
    let reflect_factor = pow(max(dot(reflect(-view_dir, normal), normalize(vec3<f32>(0.0, 1.0, 0.0))), 0.0), 32.0);
    let wet_highlight = reflect_factor * wetness;
    
    return darkened + wet_highlight;
}

fn calculate_bridge_blend(road_color: vec3<f32>, bridge_color: vec3<f32>, blend_factor: f32) -> vec3<f32> {
    // Special blending for bridge surfaces
    return mix(road_color, bridge_color, blend_factor);
}
