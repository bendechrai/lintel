//! The triangle soup `mesh.rs` builds from a `Project` and `gfx.rs` draws.

use bytemuck::{Pod, Zeroable};

use crate::materials::Surface;
use crate::model::Pick;

/// One vertex. 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct Vertex {
    /// World position: X east, Y north, Z up, metres.
    pub pos: [f32; 3],
    /// `materials::Pattern` as f32.
    pub pattern: f32,
    pub normal: [f32; 3],
    /// Pattern size in metres.
    pub scale: f32,
    /// Linear RGB and opacity.
    pub color: [f32; 4],
    /// Pattern coordinates in metres: along the surface's own axes, so boards run along a
    /// floor and bricks course along a wall whatever way it faces.
    pub uv: [f32; 2],
    pub roughness: f32,
    /// Index into `Scene::picks` plus one; 0 for nothing (terrain, slabs).
    pub pick: f32,
}

/// A whole project as triangles: opaque ones first, translucent ones (glass) in their own list
/// so the renderer can draw them after, without writing depth.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub vertices: Vec<Vertex>,
    pub opaque: Vec<u32>,
    pub translucent: Vec<u32>,
    /// What each `Vertex::pick` names.
    pub picks: Vec<Pick>,
    /// Bounding box of the building (levels and roofs, not terrain): min, max.
    pub bounds: ([f32; 3], [f32; 3]),
    /// The lamps that are on, as point sources.
    pub lamps: Vec<Lamp>,
}

/// A point of light. A strip or tube is several along its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub pos: [f32; 3],
    /// The way it faces: down from a ceiling, out from a wall.
    pub axis: [f32; 3],
    /// 0 shines equally all round; 1 only into the half-space it faces, strongest along `axis`.
    pub spread: f32,
    /// Linear RGB of its colour temperature, brightest channel 1.
    pub color: [f32; 3],
    pub lumens: f32,
    /// The room it is in, as a box: the live view lights nothing outside it, standing in
    /// for the walls' shadows.
    pub room: ([f32; 3], [f32; 3]),
}

impl Lamp {
    /// Luminous intensity in candela toward a unit direction `to` (from the lamp).
    pub fn candela(&self, to: [f32; 3]) -> f32 {
        let all_round = self.lumens / (4.0 * std::f32::consts::PI);
        let facing = self.lumens / std::f32::consts::PI * dot(to, self.axis).max(0.0);
        all_round * (1.0 - self.spread) + facing * self.spread
    }
}

impl Scene {
    /// The `Vertex::pick` value for a pick, registering it on first use.
    pub fn pick_id(&mut self, pick: Option<Pick>) -> f32 {
        let Some(pick) = pick else { return 0.0 };
        let at = self.picks.iter().position(|p| *p == pick).unwrap_or_else(|| {
            self.picks.push(pick);
            self.picks.len() - 1
        });
        (at + 1) as f32
    }

    /// Adds a convex planar polygon (counter-clockwise seen from the side `normal` points to),
    /// fanned from its first corner. `uv` gives each corner's pattern coordinates.
    pub fn polygon(&mut self, corners: &[[f32; 3]], uvs: &[[f32; 2]], normal: [f32; 3], s: &Surface, pick: f32) {
        if corners.len() < 3 {
            return;
        }
        let base = self.vertices.len() as u32;
        for (p, uv) in corners.iter().zip(uvs) {
            self.vertices.push(Vertex {
                pos: *p,
                pattern: s.pattern as u32 as f32,
                normal,
                scale: s.scale,
                color: [s.color[0], s.color[1], s.color[2], s.opacity],
                uv: *uv,
                roughness: s.roughness,
                pick,
            });
        }
        let list = if s.opacity < 1.0 { &mut self.translucent } else { &mut self.opaque };
        for i in 1..corners.len() as u32 - 1 {
            list.extend([base, base + i, base + i + 1]);
        }
    }

    /// Adds a quad `a b c d` (counter-clockwise seen from outside). The normal comes from the
    /// corners; pattern coordinates run along `a → b` (u) and `a → d` (v), in metres, offset by
    /// `uv0` so neighbouring quads on one surface continue each other's pattern.
    pub fn quad(&mut self, q: [[f32; 3]; 4], uv0: [f32; 2], s: &Surface, pick: f32) {
        let e1 = sub(q[1], q[0]);
        let e2 = sub(q[3], q[0]);
        let n = normalize(cross(e1, e2));
        let (lu, lv) = (len(e1), len(e2));
        let uvs = [uv0, [uv0[0] + lu, uv0[1]], [uv0[0] + lu, uv0[1] + lv], [uv0[0], uv0[1] + lv]];
        self.polygon(&q, &uvs, n, s, pick);
    }
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn mul(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

pub fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = len(a);
    if l > 1e-9 { mul(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}
