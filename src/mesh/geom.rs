//! Geometry helpers the builders share: oriented frames, boxes, prisms, planar polygons with
//! the winding fixed for them, and 2D polygon maths (triangulation, containment).

use crate::materials::Surface;
use crate::model::P2;
use crate::scene::{Scene, Vertex, add, cross, dot, mul, normalize, sub};

pub type V3 = [f32; 3];
pub const UP: V3 = [0.0, 0.0, 1.0];
pub const DOWN: V3 = [0.0, 0.0, -1.0];

/// A right-handed orthonormal frame: local `[x, y, z]` → world.
#[derive(Clone, Copy, Debug)]
pub struct Frame3 {
    pub o: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Frame3 {
    /// `x` as given (normalised), `y` as near the given one as stays square to it, `z = x × y`.
    pub fn new(o: V3, x: V3, y: V3) -> Frame3 {
        let x = normalize(x);
        let z = normalize(cross(x, y));
        let y = cross(z, x);
        Frame3 { o, x, y, z }
    }

    /// `x` along a plan direction, `z` up, `y` to its left.
    pub fn upright(o: V3, dir: P2) -> Frame3 {
        let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        let d = if l > 1e-9 { [dir[0] / l, dir[1] / l] } else { [1.0, 0.0] };
        Frame3 { o, x: [d[0], d[1], 0.0], y: [-d[1], d[0], 0.0], z: UP }
    }

    pub fn dir(&self, v: V3) -> V3 {
        add(add(mul(self.x, v[0]), mul(self.y, v[1])), mul(self.z, v[2]))
    }

    pub fn at(&self, p: V3) -> V3 {
        add(self.o, self.dir(p))
    }
}

/// A convex planar polygon, wound whichever way makes it face `toward`. Pattern coordinates
/// run along the first edge (u) and square to it in the plane (v, pointing the way the last
/// corner lies), from `uv0` at the first corner.
pub fn planar(scene: &mut Scene, pts: &[V3], toward: V3, uv0: [f32; 2], s: &Surface, pick: f32) {
    if pts.len() < 3 {
        return;
    }
    let p0 = pts[0];
    let mut n = [0.0f32; 3];
    for i in 1..pts.len() - 1 {
        n = add(n, cross(sub(pts[i], p0), sub(pts[i + 1], p0)));
    }
    if dot(n, n) < 1e-14 {
        return;
    }
    let mut n = normalize(n);
    let flip = dot(n, toward) < 0.0;
    if flip {
        n = mul(n, -1.0);
    }
    let u = normalize(sub(pts[1], p0));
    let mut v = cross(n, u);
    if dot(v, sub(pts[pts.len() - 1], p0)) < 0.0 {
        v = mul(v, -1.0);
    }
    let uv = |p: &V3| [uv0[0] + dot(sub(*p, p0), u), uv0[1] + dot(sub(*p, p0), v)];
    if flip {
        let rev: Vec<V3> = pts.iter().rev().copied().collect();
        let uvs: Vec<[f32; 2]> = rev.iter().map(uv).collect();
        scene.polygon(&rev, &uvs, n, s, pick);
    } else {
        let uvs: Vec<[f32; 2]> = pts.iter().map(uv).collect();
        scene.polygon(pts, &uvs, n, s, pick);
    }
}

/// A box in a frame with a surface per face, in the order -x, +x, -y, +y, -z, +z.
pub fn cuboid6(scene: &mut Scene, f: &Frame3, a: V3, b: V3, s: [&Surface; 6], pick: f32) {
    let lo: V3 = std::array::from_fn(|i| a[i].min(b[i]));
    let hi: V3 = std::array::from_fn(|i| a[i].max(b[i]));
    if (0..3).any(|i| !(hi[i] - lo[i] >= 1e-5)) {
        return;
    }
    let c = |i: usize, j: usize, k: usize| [if i == 0 { lo[0] } else { hi[0] }, if j == 0 { lo[1] } else { hi[1] }, if k == 0 { lo[2] } else { hi[2] }];
    // Corners counter-clockwise from outside, and the first corner's pattern coordinates.
    let faces: [([V3; 4], [f32; 2]); 6] = [
        ([c(0, 1, 0), c(0, 0, 0), c(0, 0, 1), c(0, 1, 1)], [-hi[1], lo[2]]),
        ([c(1, 0, 0), c(1, 1, 0), c(1, 1, 1), c(1, 0, 1)], [lo[1], lo[2]]),
        ([c(0, 0, 0), c(1, 0, 0), c(1, 0, 1), c(0, 0, 1)], [lo[0], lo[2]]),
        ([c(1, 1, 0), c(0, 1, 0), c(0, 1, 1), c(1, 1, 1)], [-hi[0], lo[2]]),
        ([c(0, 1, 0), c(1, 1, 0), c(1, 0, 0), c(0, 0, 0)], [lo[0], -hi[1]]),
        ([c(0, 0, 1), c(1, 0, 1), c(1, 1, 1), c(0, 1, 1)], [lo[0], lo[1]]),
    ];
    for (i, (q, uv0)) in faces.iter().enumerate() {
        scene.quad(q.map(|p| f.at(p)), *uv0, s[i], pick);
    }
}

pub fn cuboid(scene: &mut Scene, f: &Frame3, a: V3, b: V3, s: &Surface, pick: f32) {
    cuboid6(scene, f, a, b, [s; 6], pick);
}

/// A regular prism around the frame's z axis from `z0` to `z1`, capped at both ends.
pub fn prism(scene: &mut Scene, f: &Frame3, r: f32, z0: f32, z1: f32, sides: usize, s: &Surface, cap: &Surface, pick: f32) {
    if !(r >= 1e-5) || !((z1 - z0).abs() >= 1e-5) {
        return;
    }
    let (z0, z1) = (z0.min(z1), z0.max(z1));
    let ring = |z: f32| -> Vec<V3> {
        (0..sides)
            .map(|i| {
                let a = (i as f32 + 0.5) / sides as f32 * std::f32::consts::TAU;
                f.at([r * a.cos(), r * a.sin(), z])
            })
            .collect()
    };
    let (lo, hi) = (ring(z0), ring(z1));
    planar(scene, &hi, f.z, [-r, -r], cap, pick);
    planar(scene, &lo, mul(f.z, -1.0), [-r, -r], cap, pick);
    let axis = f.at([0.0, 0.0, z0]);
    for i in 0..sides {
        let j = (i + 1) % sides;
        let out = sub(mul(add(lo[i], lo[j]), 0.5), axis);
        planar(scene, &[lo[i], lo[j], hi[j], hi[i]], out, [i as f32 * r * 0.5, z0], s, pick);
    }
}

/// One triangle with its own normals (smooth ground).
pub fn smooth_tri(scene: &mut Scene, p: [V3; 3], n: [V3; 3], uv: [[f32; 2]; 3], s: &Surface, pick: f32) {
    let base = scene.vertices.len() as u32;
    for i in 0..3 {
        scene.vertices.push(Vertex {
            pos: p[i],
            pattern: s.pattern as u32 as f32,
            normal: n[i],
            scale: s.scale,
            color: [s.color[0], s.color[1], s.color[2], s.opacity],
            uv: uv[i],
            roughness: s.roughness,
            pick,
        });
    }
    let list = if s.opacity < 1.0 { &mut scene.translucent } else { &mut scene.opaque };
    list.extend([base, base + 1, base + 2]);
}

// ---------------------------------------------------------------------------------------------
// 2D

fn cross2(a: P2, b: P2) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

/// Signed area: positive for a counter-clockwise outline.
pub fn area(outline: &[P2]) -> f32 {
    let n = outline.len();
    (0..n).map(|i| cross2(outline[i], outline[(i + 1) % n])).sum::<f32>() * 0.5
}

/// The outline without repeated points, counter-clockwise.
pub fn ccw(outline: &[P2]) -> Vec<P2> {
    let same = |a: &P2, b: &P2| (a[0] - b[0]).abs() <= 1e-6 && (a[1] - b[1]).abs() <= 1e-6;
    let mut pts: Vec<P2> = Vec::with_capacity(outline.len());
    for p in outline {
        if p[0].is_finite() && p[1].is_finite() && pts.last().is_none_or(|q| !same(q, p)) {
            pts.push(*p);
        }
    }
    while pts.len() > 1 && same(&pts[0], &pts[pts.len() - 1]) {
        pts.pop();
    }
    if area(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

fn in_tri(p: P2, a: P2, b: P2, c: P2) -> bool {
    let e = 1e-7;
    cross2(sub2(b, a), sub2(p, a)) > e && cross2(sub2(c, b), sub2(p, b)) > e && cross2(sub2(a, c), sub2(p, c)) > e
}

/// Ear-clipping triangulation of a simple polygon (either winding, concave allowed).
/// Triangles come out counter-clockwise.
pub fn triangulate(outline: &[P2]) -> Vec<[P2; 3]> {
    let pts = ccw(outline);
    let mut out = Vec::new();
    if pts.len() < 3 {
        return out;
    }
    let turn = |a: P2, b: P2, c: P2| cross2(sub2(b, a), sub2(c, b));
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    while idx.len() > 3 {
        let n = idx.len();
        let corner = |i: usize| (pts[idx[(i + n - 1) % n]], pts[idx[i]], pts[idx[(i + 1) % n]]);
        let ear = (0..n).find(|&i| {
            let (a, b, c) = corner(i);
            turn(a, b, c) > 1e-9 && !idx.iter().any(|&j| in_tri(pts[j], a, b, c))
        });
        match ear {
            Some(i) => {
                let (a, b, c) = corner(i);
                out.push([a, b, c]);
                idx.remove(i);
            }
            None => {
                // No ear: a collinear run or a self-touching outline. Drop a flat corner if
                // there is one, else clip a convex one so the loop always ends.
                let flat = (0..n).find(|&i| {
                    let (a, b, c) = corner(i);
                    turn(a, b, c).abs() <= 1e-9
                });
                let i = flat.unwrap_or_else(|| {
                    (0..n)
                        .find(|&i| {
                            let (a, b, c) = corner(i);
                            turn(a, b, c) > 0.0
                        })
                        .unwrap_or(0)
                });
                let (a, b, c) = corner(i);
                if flat.is_none() && turn(a, b, c) > 0.0 {
                    out.push([a, b, c]);
                }
                idx.remove(i);
            }
        }
    }
    let (a, b, c) = (pts[idx[0]], pts[idx[1]], pts[idx[2]]);
    if turn(a, b, c) > 1e-9 {
        out.push([a, b, c]);
    }
    out
}

pub fn inside(outline: &[P2], p: P2) -> bool {
    let n = outline.len();
    let mut hit = false;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) {
            hit = !hit;
        }
    }
    hit
}

pub fn segment_distance(p: P2, a: P2, b: P2) -> f32 {
    let (ab, ap) = (sub2(b, a), sub2(p, a));
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 > 1e-12 { ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// Inside the outline, or within `margin` of its edge.
pub fn near(outline: &[P2], p: P2, margin: f32) -> bool {
    if outline.len() < 3 {
        return false;
    }
    inside(outline, p) || (0..outline.len()).any(|i| segment_distance(p, outline[i], outline[(i + 1) % outline.len()]) <= margin)
}
