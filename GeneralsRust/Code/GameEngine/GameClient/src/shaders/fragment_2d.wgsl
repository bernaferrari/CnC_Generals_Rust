// 2D Fragment Shader for Command & Conquer Generals Zero Hour
// Modern WGSL shader for 2D rendering operations with multiple blend modes

struct FragmentInput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct RenderSettings {
    draw_mode: u32,    // 0: Solid, 1: Grayscale, 2: Alpha, 3: Additive
    gamma: f32,
    brightness: f32,
    contrast: f32,
}

@group(1) @binding(0)
var<uniform> settings: RenderSettings;

@group(1) @binding(1)
var texture_sampler: sampler;

@group(1) @binding(2)
var texture_2d: texture_2d<f32>;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    var base_color: vec4<f32>;
    
    // Sample texture if available
    if (textureNumLevels(texture_2d) > 0u) {
        base_color = textureSample(texture_2d, texture_sampler, in.tex_coords);
    } else {
        base_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    
    // Apply vertex color
    base_color = base_color * in.color;
    
    // Apply draw mode
    switch settings.draw_mode {
        case 0u: { // Solid
            // No blending, use color as-is
            base_color.a = 1.0;
        }
        case 1u: { // Grayscale  
            let gray = dot(base_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            base_color = vec4<f32>(gray, gray, gray, 1.0);
        }
        case 2u: { // Alpha
            // Use alpha channel for blending (handled by render pipeline)
        }
        case 3u: { // Additive
            // Additive blending (handled by render pipeline blend state)
        }
        default: {}
    }
    
    // Apply gamma, brightness, and contrast
    let gamma_color = pow(base_color.rgb, vec3<f32>(1.0 / settings.gamma));
    let brightness_color = gamma_color * settings.brightness;
    let contrast_color = (brightness_color - vec3<f32>(0.5)) * settings.contrast + vec3<f32>(0.5);
    base_color = vec4<f32>(contrast_color, base_color.a);
    
    // Clamp to valid range
    base_color = clamp(base_color, vec4<f32>(0.0), vec4<f32>(1.0));
    
    return base_color;
}
