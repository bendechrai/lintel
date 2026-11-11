//! Light levels: illuminance in lux on a horizontal plane in every room, from the sun, the sky
//! and the lamps that are on, with shadows and light bounced off the room's surfaces.
//!
//! The method is a small path tracer over the same triangles the 3D view draws:
//!
//! - **Lamps** are point sources with a luminous intensity in candela from their lumens
//!   (`scene::Lamp::candela`); a surface gets `candela · cos θ / d²` lux when it can see one.
//! - **The sun** gives `133 800 · exp(-0.2 / sin altitude)` lux on a surface facing it under a
//!   clear sky, and the **sky** a uniform luminance that adds `800 + 15 500 · √(sin altitude)`
//!   lux to open level ground. Both are zero with the sun below the horizon.
//! - **Glass** passes 80 % a pane. Every other surface reflects diffusely, by the brightness
//!   of its colour.
//! - Light that arrives by **bounces** is gathered with cosine-weighted rays from each point.
//!
//! It is an estimate for planning, not a photometric report: fittings are ideal points with a
//! simple beam shape rather than measured (IES) distributions, the sky is uniform and clear,
//! and surfaces have no gloss.

use crate::geom;
use crate::mesh::{self, BuildOptions};
use crate::model::{Id, P2, Project};
use crate::scene::{Lamp, Scene, add, cross, dot, len, mul, normalize, sub};

const GLASS: f32 = 0.8;
const GROUND_REFLECTANCE: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Height of the measuring plane above each room's floor. 0.8 m is a work surface.
    pub height: f32,
    /// Distance between measuring points.
    pub spacing: f32,
    /// Rays gathered at each point for sky and bounced light.
    pub samples: u32,
    /// How many times light may bounce on its way to a point.
    pub bounces: u32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { height: 0.8, spacing: 0.3, samples: 128, bounces: 3 }
    }
}

/// The light levels in one room, on a grid.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomLux {
    pub room: Id,
    pub level: Id,
    pub name: String,
    /// Height of the measuring plane above the site datum.
    pub z: f32,
    /// Centre of the grid's first cell.
    pub origin: P2,
    pub spacing: f32,
    pub cols: usize,
    pub rows: usize,
    /// Row by row from `origin`; NaN where the cell is outside the room or inside furniture.
    pub lux: Vec<f32>,
    pub mean: f32,
    pub min: f32,
    pub max: f32,
}

impl RoomLux {
    /// The light level at a plan point, if it is on this room's grid.
    pub fn at(&self, p: P2) -> Option<f32> {
        let c = ((p[0] - self.origin[0]) / self.spacing + 0.5).floor();
        let r = ((p[1] - self.origin[1]) / self.spacing + 0.5).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        Some(self.lux[r as usize * self.cols + c as usize]).filter(|v| v.is_finite())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Levels {
    pub rooms: Vec<RoomLux>,
    /// Sunlight on a surface facing the sun, and skylight on open level ground, in lux.
    pub sun: f32,
    pub sky: f32,
    pub lamps: usize,
    pub seconds: f32,
}

// ---------------------------------------------------------------------------------------------
// Sun and sky

/// Lux on a surface facing the sun, for a sun `altitude` radians up under a clear sky.
pub fn sun_lux(altitude: f32) -> f32 {
    if altitude <= 0.005 { 0.0 } else { 133_800.0 * (-0.2 / altitude.sin()).exp() }
}

/// Lux the sky alone puts on open level ground.
pub fn sky_lux(altitude: f32) -> f32 {
    if altitude <= 0.0 { 0.0 } else { 800.0 + 15_500.0 * altitude.sin().sqrt() }
}

// ---------------------------------------------------------------------------------------------
// Ray tracing

struct Tri {
    a: [f32; 3],
    e1: [f32; 3],
    e2: [f32; 3],
    n: [f32; 3],
    reflectance: f32,
    glass: bool,
}

struct Node {
    lo: [f32; 3],
    hi: [f32; 3],
    /// A leaf holds `count` triangles from `first`; an inner node's children are `first` and
    /// `first + 1`.
    first: u32,
    count: u32,
}

/// The scene's triangles in a bounding volume hierarchy.
pub struct Tracer {
    tris: Vec<Tri>,
    nodes: Vec<Node>,
}

struct Hit {
    t: f32,
    tri: usize,
}

impl Tracer {
    pub fn new(scene: &Scene) -> Tracer {
        let mut tris = Vec::with_capacity((scene.opaque.len() + scene.translucent.len()) / 3);
        for (list, glass) in [(&scene.opaque, false), (&scene.translucent, true)] {
            for t in list.chunks_exact(3) {
                let v = [scene.vertices[t[0] as usize], scene.vertices[t[1] as usize], scene.vertices[t[2] as usize]];
                let (e1, e2) = (sub(v[1].pos, v[0].pos), sub(v[2].pos, v[0].pos));
                let n = cross(e1, e2);
                if len(n) < 1e-10 {
                    continue;
                }
                let c = v[0].color;
                // Luminance of the surface colour is how much light it sends back.
                let reflectance = (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).clamp(0.02, 0.9);
                tris.push(Tri { a: v[0].pos, e1, e2, n: normalize(n), reflectance, glass });
            }
        }
        let mut tracer = Tracer { tris, nodes: Vec::new() };
        let n = tracer.tris.len();
        if n > 0 {
            let centres: Vec<[f32; 3]> = tracer.tris.iter().map(|t| add(t.a, mul(add(t.e1, t.e2), 1.0 / 3.0))).collect();
            let mut order: Vec<u32> = (0..n as u32).collect();
            tracer.nodes.push(Node { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
            tracer.split(0, &mut order, 0, n, &centres);
            // Put the triangles in leaf order.
            let mut slots: Vec<Option<Tri>> = std::mem::take(&mut tracer.tris).into_iter().map(Some).collect();
            tracer.tris = order.iter().filter_map(|i| slots[*i as usize].take()).collect();
        }
        tracer
    }

    fn split(&mut self, node: usize, order: &mut [u32], start: usize, end: usize, centres: &[[f32; 3]]) {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let (mut clo, mut chi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for i in &order[start..end] {
            let t = &self.tris[*i as usize];
            for p in [t.a, add(t.a, t.e1), add(t.a, t.e2)] {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let c = centres[*i as usize];
            for k in 0..3 {
                clo[k] = clo[k].min(c[k]);
                chi[k] = chi[k].max(c[k]);
            }
        }
        self.nodes[node].lo = lo;
        self.nodes[node].hi = hi;
        let axis = (0..3).max_by(|a, b| (chi[*a] - clo[*a]).partial_cmp(&(chi[*b] - clo[*b])).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
        if end - start <= 4 || chi[axis] - clo[axis] < 1e-6 {
            self.nodes[node].first = start as u32;
            self.nodes[node].count = (end - start) as u32;
            return;
        }
        let mid = (start + end) / 2;
        order[start..end].select_nth_unstable_by(mid - start, |a, b| centres[*a as usize][axis].partial_cmp(&centres[*b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal));
        let left = self.nodes.len();
        self.nodes.push(Node { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
        self.nodes.push(Node { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
        self.nodes[node].first = left as u32;
        self.nodes[node].count = 0;
        self.split(left, order, start, mid, centres);
        self.split(left + 1, order, mid, end, centres);
    }

    /// The nearest triangle a ray meets between `tmin` and `tmax`.
    fn nearest(&self, o: [f32; 3], d: [f32; 3], tmin: f32, tmax: f32) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = d.map(|v| if v.abs() > 1e-12 { 1.0 / v } else { f32::MAX.copysign(v) });
        let mut best: Option<Hit> = None;
        let mut far = tmax;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top] as usize];
            // Slab test.
            let (mut t0, mut t1) = (tmin, far);
            let mut miss = false;
            for k in 0..3 {
                let (a, b) = ((node.lo[k] - o[k]) * inv[k], (node.hi[k] - o[k]) * inv[k]);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
                if t0 > t1 {
                    miss = true;
                    break;
                }
            }
            if miss {
                continue;
            }
            if node.count == 0 {
                if top + 2 <= stack.len() {
                    stack[top] = node.first;
                    stack[top + 1] = node.first + 1;
                    top += 2;
                }
                continue;
            }
            for i in node.first as usize..(node.first + node.count) as usize {
                let t = &self.tris[i];
                // Möller–Trumbore.
                let p = cross(d, t.e2);
                let det = dot(t.e1, p);
                if det.abs() < 1e-10 {
                    continue;
                }
                let s = sub(o, t.a);
                let u = dot(s, p) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = cross(s, t.e1);
                let v = dot(d, q) / det;
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let at = dot(t.e2, q) / det;
                if at > tmin && at < far {
                    far = at;
                    best = Some(Hit { t: at, tri: i });
                }
            }
        }
        best
    }

    /// The first opaque surface along a ray, and how much light gets that far through glass.
    fn surface(&self, o: [f32; 3], d: [f32; 3], tmax: f32) -> (Option<Hit>, f32) {
        let mut through = 1.0;
        let mut tmin = 1e-4;
        for _ in 0..8 {
            match self.nearest(o, d, tmin, tmax) {
                Some(hit) if self.tris[hit.tri].glass => {
                    through *= GLASS;
                    tmin = hit.t + 1e-4;
                }
                other => return (other, through),
            }
        }
        (None, 0.0)
    }

    /// How much light travels the open path from `o` along `d` for `distance`: 0 when
    /// something solid is in the way, 0.8 for each pane of glass.
    pub fn open(&self, o: [f32; 3], d: [f32; 3], distance: f32) -> f32 {
        match self.surface(o, d, distance) {
            (Some(_), _) => 0.0,
            (None, through) => through,
        }
    }
}

/// Everything that sheds light on the scene.
pub struct Sources<'a> {
    pub lamps: &'a [Lamp],
    /// Unit vector toward the sun, and its height in radians.
    pub sun: [f32; 3],
    pub altitude: f32,
}

impl Sources<'_> {
    fn sun_lux(&self) -> f32 {
        sun_lux(self.altitude)
    }

    /// Luminance of the sky or the ground seen along a direction that leaves the scene.
    fn backdrop(&self, d: [f32; 3]) -> f32 {
        let sky = sky_lux(self.altitude);
        if d[2] >= 0.0 {
            sky / std::f32::consts::PI
        } else {
            (sky + self.sun_lux() * self.altitude.sin().max(0.0)) * GROUND_REFLECTANCE / std::f32::consts::PI
        }
    }
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift32.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    /// A direction in the hemisphere about `n`, more often near it (cosine-weighted).
    fn hemisphere(&mut self, n: [f32; 3]) -> [f32; 3] {
        let (u, v) = (self.next(), self.next());
        let (r, phi) = (u.sqrt(), std::f32::consts::TAU * v);
        let t = normalize(cross(if n[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }, n));
        let b = cross(n, t);
        normalize(add(add(mul(t, r * phi.cos()), mul(b, r * phi.sin())), mul(n, (1.0 - u).max(0.0).sqrt())))
    }
}

/// Lux arriving straight from the sun and from the lamps on a surface at `p` facing `n`.
/// `one_lamp` samples a single lamp (weighted by output) instead of adding them all.
fn direct(tracer: &Tracer, src: &Sources, p: [f32; 3], n: [f32; 3], one_lamp: Option<&mut Rng>) -> f32 {
    let mut e = 0.0;
    let facing = dot(n, src.sun);
    if facing > 0.0 && src.altitude > 0.005 {
        e += src.sun_lux() * facing * tracer.open(p, src.sun, 1e4);
    }
    let from = |lamp: &Lamp| {
        let to = sub(lamp.pos, p);
        let d = len(to);
        if d < 1e-3 {
            return 0.0;
        }
        let dir = mul(to, 1.0 / d);
        let cos = dot(n, dir);
        if cos <= 0.0 {
            return 0.0;
        }
        let candela = lamp.candela(mul(dir, -1.0));
        if candela <= 0.0 {
            return 0.0;
        }
        // Clamp the distance so a point almost touching a lamp doesn't blow up.
        candela * cos / (d * d).max(0.01) * tracer.open(p, dir, d - 2e-3)
    };
    match one_lamp {
        None => e += src.lamps.iter().map(from).sum::<f32>(),
        Some(rng) => {
            let total: f32 = src.lamps.iter().map(|l| l.lumens).sum();
            if total > 0.0 {
                let mut pick = rng.next() * total;
                for lamp in src.lamps {
                    pick -= lamp.lumens;
                    if pick <= 0.0 {
                        e += from(lamp) * total / lamp.lumens;
                        break;
                    }
                }
            }
        }
    }
    e
}

/// Luminance arriving at `o` from direction `d`: the sky, or a surface lit directly and by
/// further bounces.
fn incoming(tracer: &Tracer, src: &Sources, o: [f32; 3], d: [f32; 3], bounces: u32, rng: &mut Rng) -> f32 {
    let (hit, through) = tracer.surface(o, d, 1e4);
    let Some(hit) = hit else { return src.backdrop(d) * through };
    let tri = &tracer.tris[hit.tri];
    let n = if dot(tri.n, d) > 0.0 { mul(tri.n, -1.0) } else { tri.n };
    let p = add(add(o, mul(d, hit.t)), mul(n, 2e-3));
    let mut e = direct(tracer, src, p, n, Some(rng));
    if bounces > 1 {
        let next = rng.hemisphere(n);
        e += std::f32::consts::PI * incoming(tracer, src, p, next, bounces - 1, rng);
    }
    through * tri.reflectance / std::f32::consts::PI * e
}

/// Lux on a surface at `p` facing `n`: what arrives straight from the sun and the lamps, and
/// what arrives from the sky and by bounces. The first is exact; the second is an average
/// over `settings.samples` random rays, so it carries some noise.
pub fn parts(tracer: &Tracer, src: &Sources, p: [f32; 3], n: [f32; 3], settings: &Settings, seed: u32) -> (f32, f32) {
    let mut rng = Rng(seed.wrapping_mul(2_654_435_761).max(1));
    let straight = direct(tracer, src, p, n, None);
    let mut gathered = 0.0;
    if settings.bounces > 0 && settings.samples > 0 {
        let mut sum = 0.0;
        for _ in 0..settings.samples {
            let d = rng.hemisphere(n);
            sum += incoming(tracer, src, p, d, settings.bounces, &mut rng);
        }
        gathered = std::f32::consts::PI * sum / settings.samples as f32;
    }
    (straight, gathered)
}

/// Lux on a surface at `p` facing `n`.
pub fn illuminance(tracer: &Tracer, src: &Sources, p: [f32; 3], n: [f32; 3], settings: &Settings, seed: u32) -> f32 {
    let (straight, gathered) = parts(tracer, src, p, n, settings, seed);
    straight + gathered
}

// ---------------------------------------------------------------------------------------------
// A whole project

/// Light levels in every room of a project, as it stands (sun setting, lamps on or off).
pub fn compute(project: &Project, settings: &Settings) -> Levels {
    let started = crate::time::now();
    // Lamps come from the fittings; the fittings themselves stay out of the rays' way.
    let lamps = mesh::build(project, &BuildOptions { terrain: false, roofs: false, furniture: false, ..Default::default() }).lamps;
    let scene = mesh::build(project, &BuildOptions { fixtures: false, ..Default::default() });
    let tracer = Tracer::new(&scene);
    let (sun, altitude) = project.site.sun_direction();
    let src = Sources { lamps: &lamps, sun, altitude };

    // Lay the grids out, then trace every point.
    let mut rooms = Vec::new();
    let mut points: Vec<(usize, usize, [f32; 3])> = Vec::new();
    for level in project.levels.iter().filter(|l| l.visible) {
        for room in &level.rooms {
            if room.outline.len() < 3 {
                continue;
            }
            let (lo, hi) = room.outline.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]));
            let s = settings.spacing.max(0.05);
            let cols = (((hi[0] - lo[0]) / s).floor() as usize).max(1);
            let rows = (((hi[1] - lo[1]) / s).floor() as usize).max(1);
            // Centre the cells in the room's extent.
            let origin = [lo[0] + (hi[0] - lo[0] - (cols - 1) as f32 * s) / 2.0, lo[1] + (hi[1] - lo[1] - (rows - 1) as f32 * s) / 2.0];
            let floor = level.elevation + room.floor_offset;
            let z = floor + settings.height.max(0.01);
            let index = rooms.len();
            for r in 0..rows {
                for c in 0..cols {
                    let p = [origin[0] + c as f32 * s, origin[1] + r as f32 * s];
                    // Inside the room, clear of its walls, and not inside a piece of furniture.
                    let inside = geom::inside(p, &room.outline) && (0..room.outline.len()).all(|i| geom::segment_distance(p, room.outline[i], room.outline[(i + 1) % room.outline.len()]) > 0.12);
                    if inside && tracer.nearest([p[0], p[1], floor + 0.02], [0.0, 0.0, 1.0], 0.0, z - floor - 0.02).is_none() {
                        points.push((index, r * cols + c, [p[0], p[1], z]));
                    }
                }
            }
            rooms.push(RoomLux { room: room.id, level: level.id, name: room.name.clone(), z, origin, spacing: s, cols, rows, lux: vec![f32::NAN; cols * rows], mean: 0.0, min: 0.0, max: 0.0 });
        }
    }

    let values = trace_points(&tracer, &src, &points, settings);
    // Sky and bounced light vary slowly across a room, so average each cell's with its
    // neighbours' to take the sampling noise down; direct light keeps its sharp edges.
    let mut gathered: Vec<Vec<f32>> = rooms.iter().map(|r| vec![f32::NAN; r.lux.len()]).collect();
    for ((room, cell, _), (_, g)) in points.iter().zip(&values) {
        gathered[*room][*cell] = *g;
    }
    for ((room, cell, _), (straight, _)) in points.iter().zip(&values) {
        let (cols, rows) = (rooms[*room].cols as i32, rooms[*room].rows as i32);
        let (c, r) = ((*cell as i32) % cols, (*cell as i32) / cols);
        let (mut sum, mut weight) = (0.0, 0.0);
        for dr in -2..=2 {
            for dc in -2..=2 {
                let (cc, rr) = (c + dc, r + dr);
                if cc < 0 || rr < 0 || cc >= cols || rr >= rows {
                    continue;
                }
                let g = gathered[*room][(rr * cols + cc) as usize];
                if g.is_finite() {
                    let w = 1.0 / (1.0 + (dc * dc + dr * dr) as f32);
                    sum += g * w;
                    weight += w;
                }
            }
        }
        rooms[*room].lux[*cell] = straight + if weight > 0.0 { sum / weight } else { 0.0 };
    }
    for room in &mut rooms {
        let seen: Vec<f32> = room.lux.iter().copied().filter(|v| v.is_finite()).collect();
        if !seen.is_empty() {
            room.mean = seen.iter().sum::<f32>() / seen.len() as f32;
            room.min = seen.iter().copied().fold(f32::MAX, f32::min);
            room.max = seen.iter().copied().fold(f32::MIN, f32::max);
        }
    }
    Levels { rooms, sun: sun_lux(altitude), sky: sky_lux(altitude), lamps: lamps.len(), seconds: crate::time::since(started) }
}

fn trace_point(tracer: &Tracer, src: &Sources, settings: &Settings, i: usize, p: [f32; 3]) -> (f32, f32) {
    parts(tracer, src, p, [0.0, 0.0, 1.0], settings, i as u32 + 1)
}

#[cfg(not(target_arch = "wasm32"))]
fn trace_points(tracer: &Tracer, src: &Sources, points: &[(usize, usize, [f32; 3])], settings: &Settings) -> Vec<(f32, f32)> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 16);
    let chunk = points.len().div_ceil(threads).max(1);
    let mut out = vec![(0.0, 0.0); points.len()];
    std::thread::scope(|scope| {
        for (k, (slice, points)) in out.chunks_mut(chunk).zip(points.chunks(chunk)).enumerate() {
            scope.spawn(move || {
                for (j, (v, (_, _, p))) in slice.iter_mut().zip(points).enumerate() {
                    *v = trace_point(tracer, src, settings, k * chunk + j, *p);
                }
            });
        }
    });
    out
}

#[cfg(target_arch = "wasm32")]
fn trace_points(tracer: &Tracer, src: &Sources, points: &[(usize, usize, [f32; 3])], settings: &Settings) -> Vec<(f32, f32)> {
    points.iter().enumerate().map(|(i, (_, _, p))| trace_point(tracer, src, settings, i, *p)).collect()
}

/// A colour for a light level, on a logarithmic scale from 1 lux (dark blue) through 100
/// (green) to 10 000 and over (white-hot), in sRGB.
pub fn color(lux: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 6] = [[0.07, 0.06, 0.25], [0.16, 0.30, 0.74], [0.10, 0.62, 0.55], [0.55, 0.80, 0.25], [0.98, 0.78, 0.18], [1.0, 0.97, 0.88]];
    let t = (lux.max(1.0).log10() / 4.0).clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (t as usize).min(STOPS.len() - 2);
    let f = t - i as f32;
    std::array::from_fn(|k| STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * f)
}

/// What a light level is good for, in a few words.
pub fn describe(lux: f32) -> &'static str {
    match lux {
        l if l < 5.0 => "dark",
        l if l < 50.0 => "dim: finding your way",
        l if l < 150.0 => "low: hallways, relaxing",
        l if l < 300.0 => "moderate: living areas",
        l if l < 500.0 => "good: kitchens, reading",
        l if l < 1000.0 => "bright: desks, detailed work",
        l if l < 10_000.0 => "very bright: beside a window in daylight",
        _ => "direct sunlight",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::{Pattern, Surface};

    fn surface(reflectance: f32) -> Surface {
        Surface { pattern: Pattern::Plain, color: [reflectance; 3], scale: 0.0, roughness: 1.0, opacity: 1.0 }
    }

    fn lamp(pos: [f32; 3], spread: f32, lumens: f32) -> Lamp {
        Lamp { pos, axis: [0.0, 0.0, -1.0], spread, color: [1.0; 3], lumens, room: ([-99.0; 3], [99.0; 3]) }
    }

    /// A closed box room, `w × d × h`, floor at z = 0, every face inward.
    fn room(scene: &mut Scene, w: f32, d: f32, h: f32, s: &Surface) {
        let p = |x: f32, y: f32, z: f32| [x * w, y * d, z * h];
        scene.quad([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)], [0.0; 2], s, 0.0);
        scene.quad([p(0., 0., 1.), p(0., 1., 1.), p(1., 1., 1.), p(1., 0., 1.)], [0.0; 2], s, 0.0);
        scene.quad([p(0., 0., 0.), p(0., 0., 1.), p(1., 0., 1.), p(1., 0., 0.)], [0.0; 2], s, 0.0);
        scene.quad([p(0., 1., 0.), p(1., 1., 0.), p(1., 1., 1.), p(0., 1., 1.)], [0.0; 2], s, 0.0);
        scene.quad([p(0., 0., 0.), p(0., 1., 0.), p(0., 1., 1.), p(0., 0., 1.)], [0.0; 2], s, 0.0);
        scene.quad([p(1., 0., 0.), p(1., 0., 1.), p(1., 1., 1.), p(1., 1., 0.)], [0.0; 2], s, 0.0);
    }

    const UP: [f32; 3] = [0.0, 0.0, 1.0];

    #[test]
    fn a_bare_lamp_follows_the_inverse_square_law() {
        let tracer = Tracer::new(&Scene::default());
        // 1000 lm all round is 79.6 cd: 19.9 lux at 2 m, a quarter of that at 4 m.
        let lamps = [lamp([0.0, 0.0, 2.0], 0.0, 1000.0)];
        let src = Sources { lamps: &lamps, sun: UP, altitude: -1.0 };
        let s = Settings { samples: 0, ..Default::default() };
        let near = illuminance(&tracer, &src, [0.0; 3], UP, &s, 1);
        assert!((near - 19.9).abs() < 0.1, "{near}");
        let lamps = [lamp([0.0, 0.0, 4.0], 0.0, 1000.0)];
        let far = illuminance(&tracer, &Sources { lamps: &lamps, sun: UP, altitude: -1.0 }, [0.0; 3], UP, &s, 1);
        assert!((far - near / 4.0).abs() < 0.05, "{far}");
        // A downlight puts all of it below: 1000/π = 318 cd on its axis.
        let lamps = [lamp([0.0, 0.0, 2.0], 1.0, 1000.0)];
        let down = illuminance(&tracer, &Sources { lamps: &lamps, sun: UP, altitude: -1.0 }, [0.0; 3], UP, &s, 1);
        assert!((down - 79.6).abs() < 0.3, "{down}");
    }

    #[test]
    fn open_ground_gets_the_whole_sky_and_the_sun_by_its_height() {
        let tracer = Tracer::new(&Scene::default());
        let altitude = 50f32.to_radians();
        let sun = [0.0, altitude.cos(), altitude.sin()];
        let src = Sources { lamps: &[], sun, altitude };
        let s = Settings { samples: 64, ..Default::default() };
        let e = illuminance(&tracer, &src, [0.0; 3], UP, &s, 3);
        let expected = sun_lux(altitude) * altitude.sin() + sky_lux(altitude);
        assert!((e / expected - 1.0).abs() < 0.01, "{e} vs {expected}");
        assert!(expected > 80_000.0 && expected < 110_000.0, "a clear summer day is about 100 000 lux: {expected}");
        // At night there is none.
        assert_eq!(illuminance(&tracer, &Sources { lamps: &[], sun, altitude: -0.2 }, [0.0; 3], UP, &s, 3), 0.0);
    }

    #[test]
    fn walls_shadow_and_glass_passes_four_fifths() {
        let mut scene = Scene::default();
        // A sheet over the point: opaque, then glass.
        scene.quad([[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], [0.0; 2], &surface(0.5), 0.0);
        let lamps = [lamp([0.0, 0.0, 2.0], 0.0, 1000.0)];
        let src = Sources { lamps: &lamps, sun: UP, altitude: -1.0 };
        let s = Settings { samples: 0, ..Default::default() };
        assert_eq!(illuminance(&Tracer::new(&scene), &src, [0.0; 3], UP, &s, 1), 0.0);
        let mut glazed = Scene::default();
        glazed.quad([[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], [0.0; 2], &Surface { opacity: 0.25, ..surface(0.5) }, 0.0);
        let e = illuminance(&Tracer::new(&glazed), &src, [0.0; 3], UP, &s, 1);
        assert!((e - 19.9 * 0.8).abs() < 0.1, "{e}");
    }

    #[test]
    fn bounced_light_in_a_closed_room_balances_the_energy_put_in() {
        // A 4 × 4 × 2.5 m room with a 3000 lm lamp in the middle. Whatever the lamp emits is
        // absorbed by the surfaces, so averaged over them the light that arrives by bounces is
        // Φ·ρ / (A·(1 − ρ)) lux, less the tail the bounce limit cuts off.
        let (w, d, h) = (4.0f32, 4.0f32, 2.5f32);
        let mean = |reflectance: f32, bounces: u32| {
            let mut scene = Scene::default();
            room(&mut scene, w, d, h, &surface(reflectance));
            let lamps = [lamp([2.0, 2.0, 1.25], 0.0, 3000.0)];
            let src = Sources { lamps: &lamps, sun: UP, altitude: -1.0 };
            let tracer = Tracer::new(&scene);
            let s = Settings { samples: 200, bounces, ..Default::default() };
            // Points spread evenly over the six faces, weighted by the area each stands for.
            let mut total = 0.0;
            let mut area = 0.0;
            let mut seed = 0;
            let mut face = |origin: [f32; 3], u: [f32; 3], v: [f32; 3], n: [f32; 3]| {
                let k = 5;
                for i in 0..k {
                    for j in 0..k {
                        let (a, b) = ((i as f32 + 0.5) / k as f32, (j as f32 + 0.5) / k as f32);
                        let p = add(add(add(origin, mul(u, a)), mul(v, b)), mul(n, 1e-3));
                        seed += 1;
                        let cell = len(u) * len(v) / (k * k) as f32;
                        total += illuminance(&tracer, &src, p, n, &s, seed) * cell;
                        area += cell;
                    }
                }
            };
            face([0.0, 0.0, 0.0], [w, 0.0, 0.0], [0.0, d, 0.0], [0.0, 0.0, 1.0]);
            face([0.0, 0.0, h], [w, 0.0, 0.0], [0.0, d, 0.0], [0.0, 0.0, -1.0]);
            face([0.0, 0.0, 0.0], [w, 0.0, 0.0], [0.0, 0.0, h], [0.0, 1.0, 0.0]);
            face([0.0, d, 0.0], [w, 0.0, 0.0], [0.0, 0.0, h], [0.0, -1.0, 0.0]);
            face([0.0, 0.0, 0.0], [0.0, d, 0.0], [0.0, 0.0, h], [1.0, 0.0, 0.0]);
            face([w, 0.0, 0.0], [0.0, d, 0.0], [0.0, 0.0, h], [-1.0, 0.0, 0.0]);
            total / area
        };
        let area = 2.0 * (w * d + w * h + d * h);
        let direct = mean(0.7, 0);
        // All the lamp's light lands somewhere: Φ / A on average.
        assert!((direct / (3000.0 / area) - 1.0).abs() < 0.08, "direct {direct}, Φ/A is {}", 3000.0 / area);
        let pale = mean(0.7, 10);
        let dark = mean(0.1, 10);
        assert!(pale > dark && dark > direct, "{direct} {dark} {pale}");
        let predicted = 3000.0 * 0.7 / (area * 0.3) * (1.0 - 0.7f32.powi(10));
        assert!(((pale - direct) / predicted - 1.0).abs() < 0.1, "bounced {}, energy balance says {predicted}", pale - direct);
    }

    #[test]
    fn colours_run_dark_to_bright() {
        let lum = |c: [f32; 3]| c[0] + c[1] + c[2];
        assert!(lum(color(1.0)) < lum(color(100.0)) && lum(color(100.0)) < lum(color(10_000.0)));
        assert_eq!(color(1e9), color(10_000.0));
    }
}

#[cfg(test)]
mod house_tests {
    use super::*;
    use crate::model::SunSetting;

    #[test]
    fn the_sample_house_is_lit_by_day_and_by_its_lamps_at_night() {
        let mut project = crate::sample::house();
        let day = compute(&project, &Settings { samples: 48, ..Default::default() });
        let room = |l: &Levels, name: &str| l.rooms.iter().find(|r| r.name == name).unwrap().clone();
        // A deck in the open gets tens of thousands of lux; a windowed room hundreds to
        // thousands; a workshop with no window and no lamp next to nothing.
        assert!(room(&day, "Deck").mean > 20_000.0, "{}", room(&day, "Deck").mean);
        let living = room(&day, "Living room").mean;
        assert!(living > 200.0 && living < 20_000.0, "{living}");
        assert!(room(&day, "Workshop").mean < living / 20.0);

        project.site.sun = SunSetting { altitude: -10.0, ..project.site.sun };
        let night = compute(&project, &Settings { samples: 48, ..Default::default() });
        assert_eq!((night.sun, night.sky), (0.0, 0.0));
        // Six 700 lm downlights over 34 m²: about 120 lux before bounces.
        let living = room(&night, "Living room").mean;
        assert!(living > 100.0 && living < 260.0, "{living}");
        assert!(room(&night, "Workshop").mean < 1.0);
        // The grid answers for a point in the room, and not for one outside it.
        assert!(room(&night, "Living room").at([11.0, 2.5]).is_some());
        assert!(room(&night, "Living room").at([30.0, 2.5]).is_none());

        // Switching the living room lights off takes its level down to what leaks in.
        for f in project.levels.iter_mut().flat_map(|l| &mut l.fixtures) {
            if let Some(lamp) = f.lamp() {
                f.lamp = Some(crate::model::Lamp { on: false, ..lamp });
            }
        }
        let off = compute(&project, &Settings { samples: 16, ..Default::default() });
        assert_eq!(off.lamps, 0);
        assert!(room(&off, "Living room").mean < 0.01);
    }
}
