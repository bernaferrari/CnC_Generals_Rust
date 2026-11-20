// C++ ST_SHROUD_TEXTURE / setShroudTex analog.
// Extra multiplicative pass: dest * projected dest-texture (world XZ).
// Water uses ZFUNC LESSEQUAL because the water pass does not write Z.

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct ShroudParams {
    origin: vec2<f32>,
    cell_size: f32,
    enabled: f32,
    tex_size: vec2<f32>,
    _pad: vec2<f32>,
    tint: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(1) @binding(1)
var shroud_sampler: sampler;
@group(1) @binding(2)
var<uniform> shroud: ShroudParams;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = position;
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    return out;
}

fn tinted_texel(position: vec2<i32>, last: vec2<i32>) -> vec3<f32> {
    let level = textureLoad(shroud_texture, clamp(position, vec2<i32>(0), last), 0).r;
    // W3DShroud makes fully clear texels white, and tints other texels before filtering.
    return select(level * shroud.tint.rgb, vec3<f32>(1.0), level == 1.0);
}

fn sample_tinted_shroud(uv: vec2<f32>) -> vec3<f32> {
    let texel = uv * shroud.tex_size - vec2<f32>(0.5);
    let base = vec2<i32>(floor(texel));
    let weight = fract(texel);
    let last = vec2<i32>(shroud.tex_size) - vec2<i32>(1);
    let top = mix(tinted_texel(base, last), tinted_texel(base + vec2<i32>(1, 0), last), weight.x);
    let bottom = mix(tinted_texel(base + vec2<i32>(0, 1), last), tinted_texel(base + vec2<i32>(1, 1), last), weight.x);
    return mix(top, bottom, weight.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (shroud.enabled < 0.5 || shroud.cell_size <= 0.0001 || shroud.tex_size.x < 1.0) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let uv = (in.world_position.xz - shroud.origin) / (shroud.cell_size * shroud.tex_size);
    if all(shroud.tint.rgb == vec3<f32>(1.0)) {
        let alpha = saturate(textureSample(shroud_texture, shroud_sampler, uv).r);
        return vec4<f32>(alpha, alpha, alpha, 1.0);
    }
    return vec4<f32>(sample_tinted_shroud(uv), 1.0);
}
