// Lintel's scene shader: a sun with a shadow map, a sky, and every surface material drawn
// procedurally from its pattern id, its colour and pattern coordinates in metres.

struct Globals {
    view_proj: mat4x4f,
    light_vp: mat4x4f,
    // xyz eye position; w = 1 for an orthographic view.
    eye: vec4f,
    // xyz unit vector toward the sun; w = shadow map texel size in metres.
    sun: vec4f,
    // Cutting plane; all zero when off.
    cut: vec4f,
    // Camera axes scaled so `fwd + right * ndc.x + up * ndc.y` is the view ray (perspective).
    right: vec4f,
    up: vec4f,
    fwd: vec4f,
    // x selected pick id, y hovered pick id (0 none).
    sel: vec4f,
    // x sun strength 0..1, y sky strength 0..1, z number of lamps.
    light: vec4f,
    lamps: array<Lamp, 64>,
}

struct Lamp {
    // xyz position, w spread: 0 shines all round, 1 only the way it faces.
    pos: vec4f,
    axis: vec4f,
    // Colour times lumens, scaled so 150 lux draws as fully lit.
    color: vec4f,
    // The room it is in: nothing outside this box is lit.
    lo: vec4f,
    hi: vec4f,
}

// Light on a surface at `world` facing `n` from every lamp that is on.
fn lamplight(world: vec3f, n: vec3f) -> vec3f {
    var sum = vec3f(0.0);
    let count = i32(g.light.z);
    for (var k = 0; k < count; k++) {
        let l = g.lamps[k];
        if any(world < l.lo.xyz) || any(world > l.hi.xyz) {
            continue;
        }
        let to = l.pos.xyz - world;
        let d2 = max(dot(to, to), 0.02);
        let dir = to / sqrt(d2);
        let ndl = max(dot(n, dir), 0.0);
        let facing = max(dot(-dir, l.axis.xyz), 0.0);
        let candela = (1.0 - l.pos.w) / 12.566 + l.pos.w * facing / 3.1416;
        sum += l.color.rgb * (candela * ndl / d2);
    }
    return sum;
}

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;

struct VIn {
    @location(0) pos: vec3f,
    @location(1) pattern: f32,
    @location(2) normal: vec3f,
    @location(3) scale: f32,
    @location(4) color: vec4f,
    @location(5) uv: vec2f,
    @location(6) roughness: f32,
    @location(7) pick: f32,
}

struct VOut {
    @builtin(position) clip: vec4f,
    @location(0) world: vec3f,
    @location(1) normal: vec3f,
    @location(2) color: vec4f,
    @location(3) uv: vec2f,
    @location(4) @interpolate(flat) pattern: f32,
    @location(5) @interpolate(flat) scale: f32,
    @location(6) @interpolate(flat) roughness: f32,
    @location(7) @interpolate(flat) pick: f32,
}

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4f(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.color = v.color;
    o.uv = v.uv;
    o.pattern = v.pattern;
    o.scale = v.scale;
    o.roughness = v.roughness;
    o.pick = v.pick;
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4f {
    return g.light_vp * vec4f(v.pos, 1.0);
}

// ---------------------------------------------------------------------------------------------
// Sky

fn sky(dir: vec3f) -> vec3f {
    let zenith = vec3f(0.24, 0.47, 0.86);
    let horizon = vec3f(0.74, 0.83, 0.93);
    let ground = vec3f(0.60, 0.63, 0.66);
    let up = clamp(dir.z, -1.0, 1.0);
    var c = mix(horizon, zenith, pow(max(up, 0.0), 0.55));
    c = mix(c, ground, smoothstep(0.0, -0.25, up));
    // A soft glow around the sun.
    let s = max(dot(dir, g.sun.xyz), 0.0);
    c += vec3f(1.0, 0.9, 0.7) * (pow(s, 400.0) * 3.0 + pow(s, 12.0) * 0.12) * g.light.x;
    // Night: the sky goes to a deep blue.
    return mix(vec3f(0.012, 0.016, 0.03), c, g.light.y);
}

struct SkyOut {
    @builtin(position) clip: vec4f,
    @location(0) ndc: vec2f,
}

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4f(p, 1.0, 1.0);
    o.ndc = p;
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> @location(0) vec4f {
    var c: vec3f;
    if g.eye.w > 0.5 {
        // An elevation has no horizon to look at: a quiet backdrop, lighter toward the top.
        c = mix(vec3f(0.80, 0.86, 0.92), vec3f(0.55, 0.70, 0.90), i.ndc.y * 0.5 + 0.5);
    } else {
        c = sky(normalize(g.fwd.xyz + g.right.xyz * i.ndc.x + g.up.xyz * i.ndc.y));
    }
    return vec4f(tonemap(c), 1.0);
}

// ---------------------------------------------------------------------------------------------
// Noise

fn hash1(p: vec2f) -> f32 {
    var q = fract(vec3f(p.xyx) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash2(p: vec2f) -> vec2f {
    var q = fract(vec3f(p.xyx) * vec3f(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.xx + q.yz) * q.zy);
}

fn noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash1(i), hash1(i + vec2f(1.0, 0.0)), w.x), mix(hash1(i + vec2f(0.0, 1.0)), hash1(i + vec2f(1.0, 1.0)), w.x), w.y);
}

fn fbm(p: vec2f) -> f32 {
    var a = 0.5;
    var s = 0.0;
    var q = p;
    for (var i = 0; i < 4; i++) {
        s += a * noise(q);
        q = q * 2.03 + vec2f(17.3, 9.1);
        a *= 0.5;
    }
    return s;
}

// How much of a detail `size` metres across survives at a pixel footprint of `px` metres.
fn keep(px: f32, size: f32) -> f32 {
    return 1.0 - smoothstep(0.35 * size, 1.2 * size, px);
}

// Coverage of a line `width` wide centred `d` metres away, filtered over the pixel footprint.
fn line(d: f32, width: f32, px: f32) -> f32 {
    let w = max(px, 1e-5);
    return clamp((width * 0.5 - d) / w + 0.5, 0.0, 1.0) * min(1.0, width / w + 0.35);
}

struct Mat {
    albedo: vec3f,
    roughness: f32,
    // Extra mirror-like reflection of the sky, 0..1.
    reflect: f32,
    alpha: f32,
    emissive: f32,
}

fn lum(c: vec3f) -> f32 {
    return dot(c, vec3f(0.2126, 0.7152, 0.0722));
}

fn material(pattern: i32, base: vec4f, uv: vec2f, scale_in: f32, rough: f32, px: f32) -> Mat {
    var m: Mat;
    m.albedo = base.rgb;
    m.roughness = rough;
    m.reflect = 0.0;
    m.alpha = base.a;
    m.emissive = 0.0;
    let scale = max(scale_in, 0.001);
    switch pattern {
        case 1 { // Planks
            let row = floor(uv.y / scale);
            let length = 1.6;
            let along = uv.x + hash1(vec2f(row, 3.7)) * 11.0;
            let board = floor(along / length);
            let id = vec2f(board, row);
            let tone = hash1(id);
            var c = base.rgb * (0.84 + 0.3 * tone);
            c = mix(c, c * vec3f(1.06, 0.97, 0.9), hash1(id + 5.0) * 0.5);
            let gp = vec2f(uv.x * 1.3 + tone * 40.0, uv.y / scale * 9.0 + tone * 17.0);
            let streak = noise(vec2f(gp.x, gp.y * 3.0 + noise(gp * 0.7) * 3.0));
            c *= 1.0 + (streak - 0.5) * 0.22 * keep(px, scale * 0.06);
            let dv = min(fract(uv.y / scale), 1.0 - fract(uv.y / scale)) * scale;
            let du = min(fract(along / length), 1.0 - fract(along / length)) * length;
            let gap = max(line(dv, 0.0022, px), line(du, 0.0022, px)) * keep(px, scale * 0.6);
            m.albedo = mix(mix(base.rgb, c, keep(px, scale * 1.5)), c * 0.35, gap);
        }
        case 2 { // Grain
            let p = vec2f(uv.x * 0.9, uv.y / scale);
            let wave = fbm(p * vec2f(0.8, 1.6)) * 5.0;
            let rings = 0.5 + 0.5 * sin((p.y * 5.0 + wave) * 6.2832);
            let fine = noise(vec2f(uv.x * 6.0, uv.y / scale * 70.0 + wave));
            var c = base.rgb * (0.9 + 0.16 * rings * keep(px, scale * 0.12));
            c *= 1.0 + (fine - 0.5) * 0.14 * keep(px, scale * 0.02);
            m.albedo = c;
        }
        case 3 { // Tile
            let cell = floor(uv / scale);
            let f = fract(uv / scale);
            let d = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)) * scale;
            let k = keep(px, scale * 0.6);
            var c = base.rgb * (1.0 + (hash1(cell) - 0.5) * 0.07 * k);
            c *= 1.0 + (fbm(uv * 6.0 + cell * 3.1) - 0.5) * 0.05 * keep(px, 0.05);
            let grout = mix(vec3f(0.55, 0.54, 0.52), base.rgb, 0.35) * 0.8;
            m.albedo = mix(c, grout, line(d, 0.004, px) * k);
        }
        case 4 { // Carpet
            let n = noise(uv * 260.0) * 0.6 + noise(uv * 90.0) * 0.4;
            let slow = fbm(uv * 1.7);
            m.albedo = base.rgb * (1.0 + (n - 0.5) * 0.3 * keep(px, 0.006) + (slow - 0.5) * 0.08);
            m.roughness = 1.0;
        }
        case 5 { // Concrete
            let slow = fbm(uv * 0.9);
            let mid = fbm(uv * 7.0);
            let speck = noise(uv * 190.0);
            m.albedo = base.rgb * (1.0 + (slow - 0.5) * 0.22 + (mid - 0.5) * 0.1 * keep(px, 0.08) + (speck - 0.5) * 0.12 * keep(px, 0.008));
        }
        case 6 { // Brick
            let h = scale / 3.0;
            let row = floor(uv.y / h);
            let along = uv.x + 0.5 * scale * (row - 2.0 * floor(row * 0.5));
            let id = vec2f(floor(along / scale), row);
            let fx = fract(along / scale);
            let fy = fract(uv.y / h);
            let d = min(min(fx, 1.0 - fx) * scale, min(fy, 1.0 - fy) * h);
            let k = keep(px, h * 0.7);
            let tone = hash1(id);
            var c = base.rgb * (0.78 + 0.4 * tone);
            c = mix(c, c * vec3f(0.85, 0.9, 1.05), hash1(id + 9.0) * 0.4);
            c *= 1.0 + (noise(uv * 110.0) - 0.5) * 0.2 * keep(px, 0.012);
            let mortar = mix(vec3f(0.60, 0.58, 0.54), base.rgb, 0.25);
            m.albedo = mix(mix(base.rgb * 0.97, c, k), mortar, line(d, 0.011, px) * k);
        }
        case 7 { // Marble
            let p = uv * 1.4 / scale;
            let warp = vec2f(fbm(p * 1.3 + 3.1), fbm(p * 1.3 + vec2f(7.7, 1.9)));
            let q = p + warp * 1.6;
            let v1 = pow(1.0 - abs(sin((q.x * 1.7 + q.y * 1.1) * 3.1416)), 9.0);
            let v2 = pow(1.0 - abs(sin((q.x * -2.3 + q.y * 3.4 + fbm(q * 3.0) * 2.0) * 3.1416)), 14.0) * 0.6;
            let cloud = fbm(q * 2.2);
            let dark = lum(base.rgb) < 0.25;
            let vein = select(vec3f(0.42, 0.43, 0.46), vec3f(0.85, 0.82, 0.76), dark);
            var c = base.rgb * (1.0 + (cloud - 0.5) * 0.14);
            c = mix(c, vein, clamp(v1 * 0.75 + v2, 0.0, 1.0) * (0.35 + 0.65 * cloud) * keep(px, 0.02));
            m.albedo = c;
            m.reflect = 0.06;
        }
        case 8 { // Grass
            let slow = fbm(uv * 0.35);
            let mid = fbm(uv * 3.0);
            let blade = noise(vec2f(uv.x * 140.0, uv.y * 60.0)) * 0.6 + noise(uv * 330.0) * 0.4;
            var c = base.rgb * (0.62 + 0.34 * slow);
            c = mix(c, c * vec3f(1.25, 1.12, 0.6), (mid - 0.4) * 0.5);
            c *= 1.0 + (blade - 0.5) * 0.5 * keep(px, 0.01);
            m.albedo = max(c, vec3f(0.0));
            m.roughness = 1.0;
        }
        case 9 { // Glass
            m.reflect = 0.08;
        }
        case 10 { // Metal
            let streak = noise(vec2f(uv.x * 4.0, uv.y * 520.0)) * 0.6 + noise(vec2f(uv.x * 11.0, uv.y * 1400.0)) * 0.4;
            m.albedo = base.rgb * (1.0 + (streak - 0.5) * 0.12 * keep(px, 0.003));
            m.reflect = 0.3 * (1.0 - rough);
        }
        case 11 { // Asphalt
            let n = noise(uv * 150.0) * 0.55 + noise(uv * 420.0) * 0.45;
            let chip = smoothstep(0.72, 0.9, noise(uv * 95.0 + 31.0));
            let slow = fbm(uv * 0.6);
            var c = base.rgb * (1.0 + (n - 0.5) * 0.55 * keep(px, 0.008) + (slow - 0.5) * 0.15);
            c = mix(c, c * 1.7 + 0.03, chip * 0.5 * keep(px, 0.012));
            m.albedo = c;
            m.roughness = 1.0;
        }
        case 12 { // Shingle
            let h = scale * 0.45;
            let row = floor(uv.y / h);
            let along = uv.x + hash1(vec2f(row, 1.3)) * scale * 7.0;
            let id = vec2f(floor(along / scale), row);
            let fy = fract(uv.y / h);
            let fx = fract(along / scale);
            let k = keep(px, h * 0.7);
            var c = base.rgb * (0.8 + 0.4 * hash1(id));
            c *= 1.0 + (noise(uv * 240.0) - 0.5) * 0.3 * keep(px, 0.006);
            // The butt edge of each course casts a line onto the one below.
            c *= mix(1.0, 0.55 + 0.45 * smoothstep(0.0, 0.3, fy), k);
            c = mix(c, c * 0.5, line(min(fx, 1.0 - fx) * scale, 0.006, px) * k);
            m.albedo = mix(base.rgb * 0.9, c, k * 0.999 + 0.001);
        }
        case 13 { // Siding
            let fy = fract(uv.y / scale);
            let row = floor(uv.y / scale);
            let k = keep(px, scale * 0.7);
            var c = base.rgb * (1.0 + (hash1(vec2f(row, 2.0)) - 0.5) * 0.05);
            c *= 1.0 + (noise(vec2f(uv.x * 3.0, uv.y * 160.0)) - 0.5) * 0.06 * keep(px, 0.006);
            // Each board laps the one below: a shadow under its lower edge, a lighter face above.
            let shade = 0.5 + 0.5 * smoothstep(0.0, 0.16, fy);
            m.albedo = c * mix(1.0, shade * (1.0 + 0.04 * fy), k);
        }
        case 14 { // Fabric
            let t = 0.0035;
            let wx = sin(uv.x / t * 3.1416);
            let wy = sin(uv.y / t * 3.1416);
            let weave = wx * wy;
            let slub = noise(vec2f(uv.x * 30.0, uv.y * 400.0)) + noise(vec2f(uv.x * 400.0, uv.y * 30.0));
            m.albedo = base.rgb * (1.0 + weave * 0.16 * keep(px, t) + (slub - 1.0) * 0.1 * keep(px, 0.01) + (fbm(uv * 2.0) - 0.5) * 0.06);
            m.roughness = 1.0;
        }
        case 15 { // Emissive
            m.emissive = 1.0;
        }
        case 16 { // Speckle
            let a = noise(uv * 420.0);
            let b = noise(uv * 160.0 + 13.0);
            let c0 = noise(uv * 60.0 + 41.0);
            var c = base.rgb * (1.0 + (fbm(uv * 3.0) - 0.5) * 0.08);
            let k = keep(px, 0.006);
            c = mix(c, c * 0.45, smoothstep(0.62, 0.8, a) * 0.8 * k);
            c = mix(c, mix(c, vec3f(0.95), 0.6), smoothstep(0.7, 0.85, b) * 0.7 * k);
            c = mix(c, c * 0.7, smoothstep(0.66, 0.8, c0) * 0.5 * keep(px, 0.02));
            m.albedo = c;
            m.reflect = 0.05;
        }
        case 17 { // Soil
            let slow = fbm(uv * 1.2);
            let clod = noise(uv * 38.0) * 0.5 + noise(uv * 120.0) * 0.5;
            var c = base.rgb * (0.75 + 0.5 * slow);
            c *= 1.0 + (clod - 0.5) * 0.7 * keep(px, 0.02);
            m.albedo = c;
            m.roughness = 1.0;
        }
        case 18 { // Ribbed
            let f = fract(uv.y / scale);
            let k = keep(px, scale * 0.6);
            // Each rib is lit like a half cylinder: bright on its upper flank, dark in the valley.
            let rib = sin(f * 3.1416);
            m.albedo = base.rgb * mix(1.0, 0.62 + 0.42 * rib + 0.08 * (f - 0.5), k);
        }
        default { // Plain
            m.albedo = base.rgb * (1.0 + (noise(uv * 350.0) - 0.5) * 0.03 * keep(px, 0.004));
        }
    }
    return m;
}

// ---------------------------------------------------------------------------------------------
// Light

fn shadow(world: vec3f, n: vec3f) -> f32 {
    let texel = g.sun.w;
    let ndl = clamp(dot(n, g.sun.xyz), 0.0, 1.0);
    // Push the lookup off the surface along its normal, further the more the surface grazes
    // the light, so walls don't shadow themselves.
    let p = world + n * texel * (1.2 + 2.5 * (1.0 - ndl));
    let c = g.light_vp * vec4f(p, 1.0);
    let uv = vec2f(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || c.z >= 1.0 || c.z <= 0.0 {
        return 1.0;
    }
    let dims = vec2f(textureDimensions(shadow_map));
    let step = 1.0 / dims;
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + vec2f(f32(x), f32(y)) * step * 1.25, c.z - 0.0006);
        }
    }
    return s / 9.0;
}

fn tonemap(c: vec3f) -> vec3f {
    // ACES fit (Narkowicz), at an exposure that keeps a sunlit white wall just under white.
    let x = c * 0.62;
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

@fragment
fn fs(i: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    // Pixel footprint on the surface, in metres. Taken before any branch.
    let fw = fwidth(i.uv);
    let px = max(max(fw.x, fw.y), 1e-5);

    let cutting = dot(g.cut, g.cut) > 0.0;
    if cutting && dot(g.cut.xyz, i.world) + g.cut.w < 0.0 {
        discard;
    }

    var view: vec3f;
    if g.eye.w > 0.5 {
        view = -normalize(g.fwd.xyz);
    } else {
        view = normalize(g.eye.xyz - i.world);
    }
    var n = normalize(i.normal);
    if dot(n, view) < 0.0 {
        n = -n;
    }

    let pattern = i32(round(i.pattern));
    var m = material(pattern, i.color, i.uv, i.scale, i.roughness, px);

    var out: vec3f;
    var alpha = m.alpha;
    if m.emissive > 0.5 {
        out = m.albedo * 1.6 + vec3f(0.25);
        out = tonemap(out * 1.7);
    } else if cutting && !front {
        // The inside of a solid the section plane sliced open: poché.
        out = vec3f(0.07, 0.075, 0.085);
        alpha = 1.0;
    } else {
        let l = g.sun.xyz;
        let ndl = max(dot(n, l), 0.0);
        let open = shadow(i.world, n);
        let lit = ndl * open;
        // A low sun is weaker and warmer.
        let sun_col = mix(vec3f(1.0, 0.6, 0.32), vec3f(1.0, 0.95, 0.86), smoothstep(0.0, 0.45, l.z)) * 2.35 * g.light.x;
        let lamps = lamplight(i.world, n);
        // Sky from above, warm bounce from the ground below, and a fill from the eye so a
        // room under a ceiling still reads.
        let day = max(g.light.y, 0.035);
        let hemi = mix(vec3f(0.36, 0.33, 0.30), vec3f(0.50, 0.60, 0.74), n.z * 0.5 + 0.5) * day;
        let fill = vec3f(0.22) * max(dot(n, view), 0.0) * max(day, 0.12);
        var c = m.albedo * (sun_col * lit + hemi * 0.9 + fill + lamps);

        let h = normalize(l + view);
        let r = clamp(m.roughness, 0.03, 1.0);
        let shin = 2.0 / (r * r * r * r + 1e-4);
        let spec_power = min(shin, 4000.0);
        let fres = pow(1.0 - max(dot(n, view), 0.0), 5.0);
        let f0 = select(0.04, 0.6, pattern == 10);
        let spec = pow(max(dot(n, h), 0.0), spec_power) * (spec_power + 8.0) / 25.0 * (f0 + (1.0 - f0) * fres) * (1.0 - r * 0.85);
        let spec_tint = select(vec3f(1.0), m.albedo * 1.2, pattern == 10);
        c += sun_col * spec_tint * min(spec, 6.0) * lit;

        let env = sky(reflect(-view, n));
        let gloss = (1.0 - r) * (1.0 - r);
        // Under a roof there is less sky to mirror.
        let refl = clamp(m.reflect + gloss * (0.02 + 0.45 * fres), 0.0, 1.0) * (0.3 + 0.7 * open);
        if pattern == 10 {
            c = mix(c, env * m.albedo * 1.2, refl);
        } else {
            c = mix(c, env, refl);
        }
        if pattern == 9 {
            let fg = 0.06 + 0.94 * fres;
            c = mix(m.albedo * (hemi * 0.9 + sun_col * lit * 0.4 + lamps * 0.5), env * 1.1, clamp(fg + 0.12, 0.0, 1.0));
            c += sun_col * min(spec, 8.0) * lit;
            alpha = clamp(mix(m.alpha, 0.92, fg) + min(spec, 1.0) * lit, 0.0, 1.0);
        }
        out = tonemap(c);
    }

    // Selection and hover.
    let accent = vec3f(0.10, 0.62, 1.0);
    if g.sel.x > 0.5 && abs(i.pick - g.sel.x) < 0.5 {
        out = mix(out, accent, 0.38) + accent * 0.08;
        alpha = max(alpha, 0.6);
    } else if g.sel.y > 0.5 && abs(i.pick - g.sel.y) < 0.5 {
        out = mix(out, accent, 0.16) + accent * 0.03;
    }
    return vec4f(out, alpha);
}
