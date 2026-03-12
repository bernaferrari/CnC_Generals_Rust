// Skinned mesh shader with GPU skinning support
// Ported from WW3D's skinned mesh rendering

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
    // Fog-of-War visibility fields (Week 8 rendering integration)
    visibility_alpha: f32,      // 0.0 (hidden) to 1.0 (visible)
    visibility_falloff: f32,    // Gradient strength for smooth transitions
    is_explored: f32,           // 1.0 = explored territory, 0.0 = unexplored
    visibility_pad: f32,        // Padding for alignment
};

struct BoneUniform {
    bones: array<mat4x4<f32>, 64>,
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

/// UV Texture Transform Uniform - texture coordinate mapper parameters
struct UVTransformUniform {
    /// Mapper type ID (0=UV, 4=LinearOffset, 7=Grid, 8=Rotate, 9=SineLinearOffset, etc.)
    mapper_meta: vec4<u32>,
    /// Generic integer arguments for mapper-specific parameters
    mapper_args: vec4<i32>,
    /// Float arguments for advanced mapper control
    mapper_float_args: vec4<f32>,
    /// Current animation time in seconds
    animation: vec4<f32>,
    /// Padding for alignment
    
};

const PI: f32 = 3.14159265359;
const MAX_LIGHTS: u32 = 8u;
const LIGHT_TYPE_DIRECTIONAL: f32 = 0.0;
const LIGHT_TYPE_POINT: f32 = 1.0;
const LIGHT_TYPE_SPOT: f32 = 2.0;

struct MaterialLayers {
    diffuse: vec3<f32>,
    alpha: f32,
    emissive: vec3<f32>,
    environment: vec3<f32>,
    specular_weight: f32,
    env_stage_mask: u32,
    shiny_stage_mask: u32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords0: vec2<f32>,
    @location(3) tex_coords1: vec2<f32>,
    @location(4) tex_coords2: vec2<f32>,
    @location(5) tex_coords3: vec2<f32>,
    @location(6) bone_indices: vec4<u32>,
    @location(7) bone_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords0: vec2<f32>,
    @location(1) tex_coords1: vec2<f32>,
    @location(2) tex_coords2: vec2<f32>,
    @location(3) tex_coords3: vec2<f32>,
    @location(4) world_normal: vec3<f32>,
    @location(5) world_position: vec3<f32>,
    @location(6) fog_factor: f32,
    @location(7) vertex_diffuse: vec4<f32>,
    @location(8) vertex_illumination: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model: ModelUniform;
@group(1) @binding(1) var<uniform> lighting: LightingUniform;
@group(2) @binding(0) var<uniform> bones: BoneUniform;
@group(2) @binding(1) var<uniform> uv_transform: UVTransformUniform;
@group(3) @binding(0) var t_stage0_2d: texture_2d<f32>;
@group(3) @binding(1) var t_stage0_cube: texture_cube<f32>;
@group(3) @binding(2) var s_stage0: sampler;

@group(3) @binding(3) var t_stage1_2d: texture_2d<f32>;
@group(3) @binding(4) var t_stage1_cube: texture_cube<f32>;
@group(3) @binding(5) var s_stage1: sampler;

@group(4) @binding(0) var t_stage2_2d: texture_2d<f32>;
@group(4) @binding(1) var t_stage2_cube: texture_cube<f32>;
@group(4) @binding(2) var s_stage2: sampler;

@group(4) @binding(3) var t_stage3_2d: texture_2d<f32>;
@group(4) @binding(4) var t_stage3_cube: texture_cube<f32>;
@group(4) @binding(5) var s_stage3: sampler;

@group(5) @binding(0)
var t_stage4_2d: texture_2d<f32>;
@group(5) @binding(1)
var t_stage4_cube: texture_cube<f32>;
@group(5) @binding(2)
var s_stage4: sampler;
@group(5) @binding(3)
var t_stage5_2d: texture_2d<f32>;
@group(5) @binding(4)
var t_stage5_cube: texture_cube<f32>;
@group(5) @binding(5)
var s_stage5: sampler;
@group(6) @binding(0)
var t_stage6_2d: texture_2d<f32>;
@group(6) @binding(1)
var t_stage6_cube: texture_cube<f32>;
@group(6) @binding(2)
var s_stage6: sampler;
@group(6) @binding(3)
var t_stage7_2d: texture_2d<f32>;
@group(6) @binding(4)
var t_stage7_cube: texture_cube<f32>;
@group(6) @binding(5)
var s_stage7: sampler;

@group(7) @binding(0) var<storage, read> vertex_diffuse_colors: array<vec4<f32>>;
@group(7) @binding(1) var<storage, read> vertex_illumination_colors: array<vec4<f32>>;

fn read_diffuse_color(index: u32) -> vec4<f32> {
    let len = arrayLength(&vertex_diffuse_colors);
    if len == 0u {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    var clamped = index;
    if clamped >= len {
        clamped = len - 1u;
    }
    return vertex_diffuse_colors[clamped];
}

fn read_illumination_color(index: u32) -> vec4<f32> {
    let len = arrayLength(&vertex_illumination_colors);
    if len == 0u {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    var clamped = index;
    if clamped >= len {
        clamped = len - 1u;
    }
    return vertex_illumination_colors[clamped];
}

fn decode_hint(hints: u32, index: u32) -> u32 {
    return (hints >> (index * 4u)) & 0xFu;
}

fn stage_alpha_binary(alpha_bits: u32, index: u32) -> bool {
    return ((alpha_bits >> index) & 1u) != 0u;
}

fn stage_uv_channel(index: u32) -> u32 {
    let packed = model.texture_stage_uv_map.x;
    return (packed >> (index * 2u)) & 0x3u;
}

fn tex_coords_for_stage(input: VertexOutput, index: u32) -> vec2<f32> {
    let channel = stage_uv_channel(index);
    if channel == 0u {
        return input.tex_coords0;
    }
    if channel == 1u {
        return input.tex_coords1;
    }
    if channel == 2u {
        return input.tex_coords2;
    }
    return input.tex_coords3;
}


fn sample_stage_direct(index: u32, coords: vec2<f32>) -> vec4<f32> {
    if index == 0u {
        return textureSample(t_stage0_2d, s_stage0, coords);
    } else if index == 1u {
        return textureSample(t_stage1_2d, s_stage1, coords);
    } else if index == 2u {
        return textureSample(t_stage2_2d, s_stage2, coords);
    } else if index == 3u {
        return textureSample(t_stage3_2d, s_stage3, coords);
    } else if index == 4u {
        return textureSample(t_stage4_2d, s_stage4, coords);
    } else if index == 5u {
        return textureSample(t_stage5_2d, s_stage5, coords);
    } else if index == 6u {
        return textureSample(t_stage6_2d, s_stage6, coords);
    } else if index == 7u {
        return textureSample(t_stage7_2d, s_stage7, coords);
    } else {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
}

fn sample_stage_cube(index: u32, direction: vec3<f32>) -> vec4<f32> {
    let normalised = normalize(direction);
    if index == 0u {
        return textureSample(t_stage0_cube, s_stage0, normalised);
    } else if index == 1u {
        return textureSample(t_stage1_cube, s_stage1, normalised);
    } else if index == 2u {
        return textureSample(t_stage2_cube, s_stage2, normalised);
    } else if index == 3u {
        return textureSample(t_stage3_cube, s_stage3, normalised);
    } else if index == 4u {
        return textureSample(t_stage4_cube, s_stage4, normalised);
    } else if index == 5u {
        return textureSample(t_stage5_cube, s_stage5, normalised);
    } else if index == 6u {
        return textureSample(t_stage6_cube, s_stage6, normalised);
    } else if index == 7u {
        return textureSample(t_stage7_cube, s_stage7, normalised);
    } else {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
}

fn sample_stage_with_alpha(
    mask: u32,
    alpha_bits: u32,
    cube_mask: u32,
    index: u32,
    input: VertexOutput,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
) -> vec4<f32> {
    let bit = 1u << index;
    if (mask & bit) == 0u {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }

    var texel: vec4<f32>;
    if (cube_mask & bit) != 0u {
        let reflection_dir = reflect(-view_dir, normal);
        texel = sample_stage_cube(index, reflection_dir);
    } else {
        let coords = tex_coords_for_stage(input, index);
        texel = sample_stage_direct(index, coords);
    };

    if stage_alpha_binary(alpha_bits, index) {
        if texel.a > 0.5 {
            texel.a = 1.0;
        } else {
            texel.a = 0.0;
        }
    }

    return texel;
}

fn sample_environment_stage(
    index: u32,
    cube_mask: u32,
    reflection_dir: vec3<f32>,
    alpha_bits: u32,
) -> vec4<f32> {
    let bit = 1u << index;
    var texel: vec4<f32>;
    if (cube_mask & bit) != 0u {
        texel = sample_stage_cube(index, reflection_dir);
    } else {
        let uv = reflection_to_uv(reflection_dir);
        texel = sample_stage_direct(index, uv);
    };

    if stage_alpha_binary(alpha_bits, index) {
    if texel.a > 0.5 {
        texel.a = 1.0;
    } else {
        texel.a = 0.0;
    }
    }

    return texel;
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn safe_normalize(value: vec3<f32>) -> vec3<f32> {
    let len_sq = dot(value, value);
    if len_sq > 0.0 {
        return value * inverseSqrt(len_sq);
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}

fn compute_material_layers(
    input: VertexOutput,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
) -> MaterialLayers {
    let mask = model.texture_stage_mask.x;
    let hints = model.texture_stage_mask.y;
    let alpha_bits = model.texture_stage_mask.z;
    let cube_mask = model.texture_stage_mask.w;

    var layers = MaterialLayers(
        vec3<f32>(1.0, 1.0, 1.0),
        1.0,
        vec3<f32>(0.0, 0.0, 0.0),
        vec3<f32>(0.0, 0.0, 0.0),
        0.0,
        0u,
        0u,
    );
    var has_diffuse = false;
    let reflection_dir = reflect(-view_dir, normal);

    for (var stage: u32 = 0u; stage < 8u; stage = stage + 1u) {
        if (mask & (1u << stage)) == 0u {
            continue;
        }

        let hint = decode_hint(hints, stage);

        if hint == 2u {
            let texel = sample_environment_stage(stage, cube_mask, reflection_dir, alpha_bits);
            layers.environment = layers.environment + texel.rgb;
            layers.env_stage_mask = layers.env_stage_mask | (1u << stage);
            layers.specular_weight = max(layers.specular_weight, luminance(texel.rgb));
            continue;
        }

        var texel = sample_stage_with_alpha(mask, alpha_bits, cube_mask, stage, input, normal, view_dir);

        if hint == 1u {
            layers.emissive = layers.emissive + texel.rgb * texel.a;
            continue;
        }
        if hint == 3u {
            let mask_strength = max(max(texel.r, texel.g), texel.b) * texel.a;
            layers.specular_weight = clamp(layers.specular_weight + mask_strength, 0.0, 1.0);
            layers.shiny_stage_mask = layers.shiny_stage_mask | (1u << stage);
            continue;
        }

        if has_diffuse {
            layers.diffuse = layers.diffuse * texel.rgb;
            layers.alpha = layers.alpha * texel.a;
        } else {
            layers.diffuse = texel.rgb;
            layers.alpha = texel.a;
            has_diffuse = true;
        }
    }

    if !has_diffuse {
        layers.diffuse = vec3<f32>(1.0, 1.0, 1.0);
        layers.alpha = 1.0;
    }

    var vertex_diffuse = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if (cube_mask & (1u << 8u)) != 0u {
        vertex_diffuse = input.vertex_diffuse;
    }

    var vertex_illumination = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (cube_mask & (1u << 9u)) != 0u {
        vertex_illumination = input.vertex_illumination;
    }

    layers.diffuse = layers.diffuse * model.material_diffuse.xyz * vertex_diffuse.rgb;
    layers.alpha = clamp(layers.alpha * model.material_diffuse.w * vertex_diffuse.a, 0.0, 1.0);
    layers.emissive = layers.emissive + model.material_emissive.xyz + vertex_illumination.rgb;
    layers.specular_weight = clamp(layers.specular_weight, 0.0, 1.0);
    return layers;
}

/// Apply UV texture coordinate transformation based on mapper type
/// Supports LinearOffset (type 4), Grid (type 7), Rotate (type 8), SineLinearOffset (type 9)
fn apply_uv_mapper(uv: vec2<f32>, channel: u32) -> vec2<f32> {
    let mapper_type = uv_transform.mapper_meta.x;

    // Mapper type 0 (UV) = pass-through
    if mapper_type == 0u {
        return uv;
    }

    // Mapper type 4 = LinearOffset
    // args[0] = u_speed (units/sec * 1000)
    // args[1] = v_speed (units/sec * 1000)
    if mapper_type == 4u {
        let u_speed = f32(uv_transform.mapper_args[0]) / 1000.0;
        let v_speed = f32(uv_transform.mapper_args[1]) / 1000.0;

        return vec2<f32>(
            uv.x + u_speed * uv_transform.animation.x,
            uv.y + v_speed * uv_transform.animation.x
        );
    }

    // Mapper type 7 = Grid
    // args[0] = u_tiles
    // args[1] = v_tiles
    // args[2] = u_offset
    // args[3] = v_offset
    if mapper_type == 7u {
        let u_tiles = max(1.0, f32(uv_transform.mapper_args[0]));
        let v_tiles = max(1.0, f32(uv_transform.mapper_args[1]));
        let u_offset = f32(uv_transform.mapper_args[2]) / 1000.0;
        let v_offset = f32(uv_transform.mapper_args[3]) / 1000.0;

        return vec2<f32>(
            uv.x * u_tiles + u_offset,
            uv.y * v_tiles + v_offset
        );
    }

    // Mapper type 8 = Rotate
    // args[0] = rotation_speed (degrees/sec * 100)
    // args[1] = center_u (* 1000)
    // args[2] = center_v (* 1000)
    if mapper_type == 8u {
        let rotation_speed = (f32(uv_transform.mapper_args[0]) / 100.0) * PI / 180.0;
        let center_u = f32(uv_transform.mapper_args[1]) / 1000.0;
        let center_v = f32(uv_transform.mapper_args[2]) / 1000.0;

        let angle = rotation_speed * uv_transform.animation.x;
        let cos_angle = cos(angle);
        let sin_angle = sin(angle);

        // Translate to origin
        let u = uv.x - center_u;
        let v = uv.y - center_v;

        // Rotate
        let rotated_u = u * cos_angle - v * sin_angle;
        let rotated_v = u * sin_angle + v * cos_angle;

        // Translate back
        return vec2<f32>(rotated_u + center_u, rotated_v + center_v);
    }

    // Mapper type 9 = SineLinearOffset
    // args[0] = u_amplitude (* 1000)
    // args[1] = v_amplitude (* 1000)
    // args[2] = frequency (cycles/sec * 100)
    // args[3] = phase (* 100, in degrees)
    if mapper_type == 9u {
        let u_amp = f32(uv_transform.mapper_args[0]) / 1000.0;
        let v_amp = f32(uv_transform.mapper_args[1]) / 1000.0;
        let frequency = f32(uv_transform.mapper_args[2]) / 100.0;
        let phase = (f32(uv_transform.mapper_args[3]) / 100.0) * PI / 180.0;

        let angle = 2.0 * PI * frequency * uv_transform.animation.x + phase;
        let wave = sin(angle);

        return vec2<f32>(
            uv.x + u_amp * wave,
            uv.y + v_amp * wave
        );
    }

    if mapper_type == 10u {
        let u_step = f32(uv_transform.mapper_args[0]) / 1000.0;
        let v_step = f32(uv_transform.mapper_args[1]) / 1000.0;
        let steps_per_second = max(0.0, f32(uv_transform.mapper_args[2]) / 1000.0);

        if steps_per_second <= 0.0 {
            return uv;
        }

        let steps = floor(steps_per_second * uv_transform.animation.x);
        let offset_u = u_step * steps;
        let offset_v = v_step * steps;

        return vec2<f32>(
            fract(uv.x + offset_u),
            fract(uv.y + offset_v)
        );
    }

    if mapper_type == 11u {
        let u_speed = f32(uv_transform.mapper_args[0]) / 1000.0;
        let v_speed = f32(uv_transform.mapper_args[1]) / 1000.0;
        let period = max(0.0001, f32(uv_transform.mapper_args[2]) / 1000.0);
        let cycles = floor(uv_transform.animation.x / period);
        let remainder = uv_transform.animation.x - cycles * period;
        let half_period = 0.5 * period;
        let time = select(remainder, period - remainder, remainder > half_period);

        return vec2<f32>(
            uv.x + u_speed * time,
            uv.y + v_speed * time
        );
    }

    if mapper_type == 18u {
        let u_speed = f32(uv_transform.mapper_args[0]) / 1000.0;
        let v_speed = f32(uv_transform.mapper_args[1]) / 1000.0;
        let bump_scale = uv_transform.mapper_float_args[0];
        let rotation_rate = uv_transform.mapper_float_args[1];
        let angle = rotation_rate * uv_transform.animation.x;
        let c = bump_scale * cos(angle);
        let s = bump_scale * sin(angle);
        let base = vec2<f32>(
            uv.x + u_speed * uv_transform.animation.x,
            uv.y + v_speed * uv_transform.animation.x
        );
        let centered = vec2<f32>(uv.x - 0.5, uv.y - 0.5);
        let bump_offset = vec2<f32>(
            c * centered.y - s * centered.x,
            s * centered.y + c * centered.x
        );
        return base + bump_offset;
    }

    // Unknown mapper type, return UV unmodified
    return uv;
}

@vertex
fn vs_main(
    input: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var output: VertexOutput;

    // Apply skinning transformation
    var skinned_position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var skinned_normal = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Apply bone transformations with weights
    for (var i = 0u; i < 4u; i = i + 1u) {
        if input.bone_weights[i] > 0.0 {
            let bone_matrix = bones.bones[input.bone_indices[i]];
            skinned_position += bone_matrix * vec4<f32>(input.position, 1.0) * input.bone_weights[i];
            skinned_normal += bone_matrix * vec4<f32>(input.normal, 0.0) * input.bone_weights[i];
        }
    }

    // Apply model transformation
    let world_pos = model.model * skinned_position;
    output.clip_position = camera.view_proj * world_pos;
    output.tex_coords0 = apply_uv_mapper(input.tex_coords0, 0u);
    output.tex_coords1 = apply_uv_mapper(input.tex_coords1, 1u);
    output.tex_coords2 = apply_uv_mapper(input.tex_coords2, 2u);
    output.tex_coords3 = apply_uv_mapper(input.tex_coords3, 3u);
    output.world_normal = normalize((model.normal_matrix * skinned_normal).xyz);
    output.world_position = world_pos.xyz;

    // Calculate fog factor if fog is enabled
    if lighting.fog_params.z > 0.0 {
        let fog_start = lighting.fog_params.x;
        let fog_end = lighting.fog_params.y;
        let view_distance = length(camera.eye_position.xyz - world_pos.xyz);
        output.fog_factor = clamp((fog_end - view_distance) / (fog_end - fog_start), 0.0, 1.0);
    } else {
        output.fog_factor = 1.0;
    }

    let diffuse_color = read_diffuse_color(vertex_index);
    let illum_color = read_illumination_color(vertex_index);
    output.vertex_diffuse = diffuse_color;
    output.vertex_illumination = illum_color;

    return output;
}

/// Apply FOW visibility effects to an object color
fn apply_fow_effects(color: vec4<f32>, visibility_alpha: f32, is_explored: f32) -> vec4<f32> {
    // visibility_alpha: 0.0 = hidden, 1.0 = fully visible
    // is_explored: 1.0 = explored territory (seen before), 0.0 = unexplored
    var result = color;

    // If explored but not currently visible, apply darkening
    if is_explored > 0.5 && visibility_alpha < 0.5 {
        // Darken and desaturate explored territories
        let gray = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
        result = vec4<f32>(mix(vec3<f32>(gray, gray, gray) * 0.5, color.rgb, visibility_alpha), result.a);
    }

    // Apply alpha blending for visibility
    result.a = result.a * visibility_alpha;

    return result;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = safe_normalize(input.world_normal);
    let view_dir = safe_normalize(camera.eye_position.xyz - input.world_position);

    let layers = compute_material_layers(input, normal, view_dir);

    let overrides = model.material_overrides;
    let alpha_override = overrides.x;
    let pass_alpha_override = overrides.y;
    let emissive_scale = overrides.z;
    let blend_mode = overrides.w;

    let specular_tint = model.material_specular.xyz;
    let shininess = max(model.material_specular.w, 1.0);

    var shiny_factor = 1.0;
    if layers.shiny_stage_mask != 0u {
        var accum = 0.0;
        var count = 0.0;
        for (var stage: u32 = 0u; stage < 8u; stage = stage + 1u) {
            if (layers.shiny_stage_mask & (1u << stage)) != 0u {
                let texel = sample_stage_with_alpha(
                    model.texture_stage_mask.x,
                    model.texture_stage_mask.z,
                    model.texture_stage_mask.w,
                    stage,
                    input,
                    normal,
                    view_dir,
                );
                accum = accum + texel.r;
                count = count + 1.0;
            }
        }
        if count > 0.0 {
            shiny_factor = clamp(accum / count, 0.0, 1.0);
        }
    }

    let cos_theta = clamp(dot(normal, view_dir), 0.0, 1.0);
    let fresnel = pow(1.0 - cos_theta, 5.0);
    let fresnel_term = mix(0.08, 1.0, fresnel);
    let specular_strength = layers.specular_weight * shiny_factor;
    let specular_scale = specular_strength * fresnel_term;

    let light_count = u32(clamp(lighting.light_meta.x, 0.0, f32(MAX_LIGHTS)));
    var diffuse_sum = vec3<f32>(0.0, 0.0, 0.0);
    var specular_sum = vec3<f32>(0.0, 0.0, 0.0);

    for (var i: u32 = 0u; i < light_count; i = i + 1u) {
        let light = lighting.lights[i];
        var attenuation = 1.0;
        var light_dir = vec3<f32>(0.0, 0.0, 0.0);

        if light.direction.w == LIGHT_TYPE_DIRECTIONAL {
            light_dir = safe_normalize(-light.direction.xyz);
        } else {
            let to_light = light.position_range.xyz - input.world_position;
            let distance = length(to_light);
            if distance <= 0.0001 {
                continue;
            }
            if distance >= light.position_range.w {
                continue;
            }
            light_dir = to_light / distance;
            let denom = light.color.w
                + light.spot_params.z * distance
                + light.spot_params.w * distance * distance;
            if denom <= 0.0 {
                continue;
            }
            attenuation = clamp(1.0 / denom, 0.0, 1.0);

            if light.direction.w == LIGHT_TYPE_SPOT {
                let spot_cos = dot(light_dir, -light.direction.xyz);
                let inner = light.spot_params.x;
                let outer = light.spot_params.y;
                if spot_cos <= outer {
                    continue;
                }
                var spot = 0.0;
                if (spot_cos >= inner) {
                    spot = 1.0;
                } else {
                    spot = clamp((spot_cos - outer) / max(inner - outer, 0.001), 0.0, 1.0);
                }
                attenuation = attenuation * spot;
                if attenuation <= 0.0 {
                    continue;
                }
            }
        }

        let n_dot_l = max(dot(normal, light_dir), 0.0);
        if n_dot_l <= 0.0 {
            continue;
        }

        let light_color = light.color.xyz;
        diffuse_sum = diffuse_sum + light_color * n_dot_l * attenuation;

        let half_vec_input = light_dir + view_dir;
        let half_len_sq = dot(half_vec_input, half_vec_input);
        if half_len_sq > 0.0 {
            let half_vec = half_vec_input * inverseSqrt(half_len_sq);
            let spec_factor = pow(max(dot(normal, half_vec), 0.0), shininess);
            if spec_factor > 0.0 {
                specular_sum = specular_sum + light_color * spec_factor * attenuation;
            }
        }
    }

    let specular = specular_sum * specular_tint * specular_scale;

    let environment_rgb = layers.environment;
    let env_specular = specular_strength * fresnel_term;
    let env_contrib = environment_rgb * (specular_tint * env_specular);

    let alpha_scale = clamp(alpha_override * pass_alpha_override, 0.0, 1.0);
    let emissive = layers.emissive * emissive_scale;
    let ambient = lighting.ambient_color.xyz;

    var color_rgb = layers.diffuse * (ambient + diffuse_sum) + specular + env_contrib + emissive;
    if blend_mode == 2.0 {
        color_rgb = color_rgb * alpha_override;
    }
    color_rgb = clamp(color_rgb, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    let final_alpha = clamp(layers.alpha * alpha_scale, 0.0, 1.0);
    var object_color = vec4<f32>(color_rgb, final_alpha);

    if lighting.fog_params.z > 0.0 {
        object_color = vec4<f32>(
            mix(lighting.fog_color.xyz, object_color.rgb, input.fog_factor),
            object_color.a
        );
    }

    // Apply FOW visibility
    object_color = apply_fow_effects(object_color, model.visibility_alpha, model.is_explored);

    return object_color;
}

fn reflection_to_uv(direction: vec3<f32>) -> vec2<f32> {
    let dir = normalize(direction);
    let u = 0.5 + atan2(dir.z, dir.x) / (2.0 * PI);
    let v = 0.5 - asin(clamp(dir.y, -1.0, 1.0)) / PI;
    return vec2<f32>(fract(u), clamp(v, 0.0, 1.0));
}
