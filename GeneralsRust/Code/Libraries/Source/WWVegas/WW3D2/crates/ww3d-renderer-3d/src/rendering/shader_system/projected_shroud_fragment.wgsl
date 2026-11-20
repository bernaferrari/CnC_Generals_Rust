// Shared by the rigid and skinned projected material pass. Texture stage 0
// carries the frozen scalar shroud on the same device as receiving terrain.
@group(3) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(3) @binding(2)
var shroud_sampler: sampler;

fn tinted_texel(position: vec2<i32>, last: vec2<i32>, tint: vec3<f32>) -> vec3<f32> {
    let level = textureLoad(shroud_texture, clamp(position, vec2<i32>(0), last), 0).r;
    // W3DShroud::setShroudLevel applies the authored tint to each texel and
    // forces a texel of value 255 to white before the texture is filtered.
    return select(level * tint, vec3<f32>(1.0), level == 1.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let projection = model.material_diffuse;
    let uv = input.world_position.xz * projection.xy + projection.zw;
    let tint = model.material_overrides.rgb;
    if all(tint == vec3<f32>(1.0)) {
        let level = textureSample(shroud_texture, shroud_sampler, uv).r;
        return vec4<f32>(select(level, 1.0, level >= (254.5 / 255.0)) * tint, 1.0);
    }

    let size = textureDimensions(shroud_texture, 0);
    let texel = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(size) - 0.5;
    let base = vec2<i32>(floor(texel));
    let weight = fract(texel);
    let last = vec2<i32>(size) - 1;
    let top = mix(tinted_texel(base, last, tint), tinted_texel(base + vec2<i32>(1, 0), last, tint), weight.x);
    let bottom = mix(tinted_texel(base + vec2<i32>(0, 1), last, tint), tinted_texel(base + vec2<i32>(1, 1), last, tint), weight.x);
    return vec4<f32>(mix(top, bottom, weight.y), 1.0);
}
