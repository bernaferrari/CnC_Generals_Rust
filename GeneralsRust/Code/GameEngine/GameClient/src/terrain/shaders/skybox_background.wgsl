struct VsOut {
    @builtin(position) position : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@group(0) @binding(0) var sky_texture : texture_2d<f32>;
@group(0) @binding(1) var sky_sampler : sampler;

fn horizon_sky(uv : vec2<f32>) -> vec3<f32> {
    // uv.y: 0 = top of screen (zenith), 1 = bottom (haze / ground line).
    let t = clamp(uv.y, 0.0, 1.0);
    let zenith = vec3<f32>(0.18, 0.38, 0.78);
    let mid = vec3<f32>(0.45, 0.66, 0.92);
    let horizon = vec3<f32>(0.93, 0.84, 0.68);
    let haze = vec3<f32>(0.62, 0.68, 0.74);
    var color : vec3<f32>;
    if (t < 0.52) {
        color = mix(zenith, mid, t / 0.52);
    } else if (t < 0.78) {
        color = mix(mid, horizon, (t - 0.52) / 0.26);
    } else {
        color = mix(horizon, haze, (t - 0.78) / 0.22);
    }
    let sun = (uv.x - 0.5) * 0.08;
    color = vec3<f32>(
        clamp(color.r + sun * 0.85, 0.0, 1.0),
        clamp(color.g + sun * 0.35, 0.0, 1.0),
        clamp(color.b - sun * 0.12, 0.0, 1.0),
    );
    return color;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index : u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );

    let pos = positions[vertex_index];
    var out : VsOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = vec2<f32>(pos.x * 0.5 + 0.5, 1.0 - (pos.y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(input : VsOut) -> @location(0) vec4<f32> {
    let uv = clamp(input.uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let dims = textureDimensions(sky_texture);
    // Real Water.ini / map faces are large. Tiny textures are the old fog card
    // or an unused stub — never fill the world with a solid color.
    if (dims.x <= 4u && dims.y <= 4u) {
        return vec4<f32>(horizon_sky(uv), 1.0);
    }
    return textureSample(sky_texture, sky_sampler, uv);
}
