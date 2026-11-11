// Draws the resolved 3D image into the UI frame, at the viewport element.

struct Blit {
    // Slot rectangle in window NDC: x0, y0 (top-left), x1, y1.
    rect: vec4f,
    // x = 1 when the frame holds sRGB-encoded values.
    flags: vec4f,
}

@group(0) @binding(0) var<uniform> blit: Blit;
@group(0) @binding(1) var blit_tex: texture_2d<f32>;
@group(0) @binding(2) var blit_samp: sampler;

struct BlitOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BlitOut {
    let t = vec2f(f32(i & 1u), f32((i >> 1u) & 1u));
    var o: BlitOut;
    o.clip = vec4f(mix(blit.rect.x, blit.rect.z, t.x), mix(blit.rect.y, blit.rect.w, t.y), 0.0, 1.0);
    o.uv = t;
    return o;
}

fn encode(c: vec3f) -> vec3f {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3f(0.0)), vec3f(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3f(0.0031308));
}

@fragment
fn fs_blit(i: BlitOut) -> @location(0) vec4f {
    var c = textureSampleLevel(blit_tex, blit_samp, i.uv, 0.0).rgb;
    if blit.flags.x > 0.5 {
        c = encode(c);
    }
    return vec4f(c, 1.0);
}
