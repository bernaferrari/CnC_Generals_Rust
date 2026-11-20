struct ShroudParams {
    origin: vec2<f32>,
    cell_size: f32,
    enabled: f32,
    tex_size: vec2<f32>,
    _pad: vec2<f32>,
    tint: vec4<f32>,
}

fn tinted_shroud_texel(
    mask: texture_2d<f32>, tint: vec3<f32>, position: vec2<i32>, last: vec2<i32>,
) -> vec3<f32> {
    let level = textureLoad(mask, clamp(position, vec2<i32>(0), last), 0).r;
    // W3DShroud makes fully clear texels white and tints the others before filtering.
    return select(level * tint, vec3<f32>(1.0), level == 1.0);
}

fn sample_projected_shroud(
    mask: texture_2d<f32>, mask_sampler: sampler, shroud: ShroudParams, world_xz: vec2<f32>,
) -> vec3<f32> {
    if (shroud.enabled < 0.5 || shroud.cell_size <= 0.0001 || shroud.tex_size.x < 1.0) {
        return vec3<f32>(1.0);
    }
    let uv = (world_xz - shroud.origin) / (shroud.cell_size * shroud.tex_size);
    let tint = shroud.tint.rgb;
    if all(tint == vec3<f32>(1.0)) {
        return vec3<f32>(saturate(textureSample(mask, mask_sampler, uv).r));
    }

    let texel = uv * shroud.tex_size - vec2<f32>(0.5);
    let base = vec2<i32>(floor(texel));
    let weight = fract(texel);
    let last = vec2<i32>(shroud.tex_size) - vec2<i32>(1);
    let top = mix(tinted_shroud_texel(mask, tint, base, last), tinted_shroud_texel(mask, tint, base + vec2<i32>(1, 0), last), weight.x);
    let bottom = mix(tinted_shroud_texel(mask, tint, base + vec2<i32>(0, 1), last), tinted_shroud_texel(mask, tint, base + vec2<i32>(1, 1), last), weight.x);
    return mix(top, bottom, weight.y);
}
