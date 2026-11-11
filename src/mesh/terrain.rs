//! The ground: the site's height grid, dug out under the building, and the patches draped on it.
//!
//! The terrain is resampled to cells of half a metre or less (coarser on a very large site).
//! A cell whose centre is under a room, or just outside one, is dug: drawn flat at the
//! underside of that room's slab, with earth walls up to its undug neighbours. So the ground
//! never comes up through a floor, and stays against the walls to within about a cell.

use super::Ctx;
use super::geom::{UP, V3, near, planar, smooth_tri, triangulate};
use crate::model::{Finish, P2, Pick, Project};
use crate::scene::{Vertex, normalize};

#[derive(Clone, Debug)]
pub struct Ground {
    origin: P2,
    step: P2,
    cols: usize,
    rows: usize,
    /// Undisturbed height at each grid post.
    natural: Vec<f32>,
    /// Per cell: the level it's dug to, if it is.
    dug: Vec<Option<f32>>,
}

/// Most grid cells the ground is resampled to (two triangles each).
const MAX_CELLS: f32 = 30_000.0;

impl Ground {
    pub fn new(project: &Project) -> Ground {
        let t = &project.site.terrain;
        let size = t.size();
        let usable = t.cols >= 2 && t.rows >= 2 && t.heights.len() >= t.cols * t.rows && t.spacing > 1e-4 && size[0] > 1e-3 && size[1] > 1e-3;
        if !usable || t.heights.iter().any(|h| !h.is_finite()) {
            return Ground { origin: t.origin, step: [1.0, 1.0], cols: 0, rows: 0, natural: vec![], dug: vec![] };
        }
        let mut s = t.spacing.min(0.5);
        if size[0] * size[1] / (s * s) > MAX_CELLS {
            s = (size[0] * size[1] / MAX_CELLS).sqrt();
        }
        let cols = (size[0] / s).ceil().max(1.0) as usize + 1;
        let rows = (size[1] / s).ceil().max(1.0) as usize + 1;
        let step = [size[0] / (cols - 1) as f32, size[1] / (rows - 1) as f32];
        let mut natural = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                natural.push(t.height([t.origin[0] + c as f32 * step[0], t.origin[1] + r as f32 * step[1]]));
            }
        }
        // Every room digs a pit: its outline, its box, and the underside of its slab. A cell is
        // in it when its centre is within `margin` of the outline, which keeps the pit's edge
        // outside the room whatever way the cells fall.
        let margin = (step[0].max(step[1]) * 0.5 + 0.05).max(0.3);
        let pits: Vec<(&[P2], [f32; 4], f32)> = project
            .levels
            .iter()
            .flat_map(|level| level.rooms.iter().map(move |room| (level, room)))
            .filter(|(_, room)| room.outline.len() >= 3)
            .map(|(level, room)| {
                let mut bb = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for p in &room.outline {
                    bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
                }
                (room.outline.as_slice(), bb, level.elevation + room.floor_offset - level.slab.max(0.0) - 0.02)
            })
            .collect();
        let mut dug = Vec::with_capacity((cols - 1) * (rows - 1));
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let p = [t.origin[0] + (c as f32 + 0.5) * step[0], t.origin[1] + (r as f32 + 0.5) * step[1]];
                let post = |c: usize, r: usize| natural[r * cols + c];
                let high = post(c, r).max(post(c + 1, r)).max(post(c, r + 1)).max(post(c + 1, r + 1));
                let mut depth: Option<f32> = None;
                for (outline, bb, z) in &pits {
                    let lower = *z < depth.unwrap_or(high);
                    if lower && p[0] >= bb[0] - margin && p[1] >= bb[1] - margin && p[0] <= bb[2] + margin && p[1] <= bb[3] + margin && near(outline, p, margin) {
                        depth = Some(*z);
                    }
                }
                dug.push(depth);
            }
        }
        Ground { origin: t.origin, step, cols, rows, natural, dug }
    }

    pub fn is_empty(&self) -> bool {
        self.cols < 2 || self.rows < 2
    }

    fn xy(&self, c: usize, r: usize) -> P2 {
        [self.origin[0] + c as f32 * self.step[0], self.origin[1] + r as f32 * self.step[1]]
    }

    fn post(&self, c: usize, r: usize) -> f32 {
        self.natural[r * self.cols + c]
    }

    fn cell(&self, c: usize, r: usize) -> Option<f32> {
        self.dug[r * (self.cols - 1) + c]
    }

    /// Height of a cell's corner (`dc`, `dr` are 0 or 1): the pit's floor if the cell is dug.
    fn corner(&self, c: usize, r: usize, dc: usize, dr: usize) -> V3 {
        let p = self.xy(c + dc, r + dr);
        [p[0], p[1], self.cell(c, r).unwrap_or_else(|| self.post(c + dc, r + dr))]
    }

    /// Ground height at a plan point, clamped at the site's edges.
    pub fn height(&self, p: P2) -> f32 {
        if self.is_empty() {
            return 0.0;
        }
        let fx = ((p[0] - self.origin[0]) / self.step[0]).clamp(0.0, (self.cols - 1) as f32);
        let fy = ((p[1] - self.origin[1]) / self.step[1]).clamp(0.0, (self.rows - 1) as f32);
        let (x0, y0) = ((fx as usize).min(self.cols - 2), (fy as usize).min(self.rows - 2));
        if let Some(z) = self.cell(x0, y0) {
            return z;
        }
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let a = self.post(x0, y0) + (self.post(x0 + 1, y0) - self.post(x0, y0)) * tx;
        let b = self.post(x0, y0 + 1) + (self.post(x0 + 1, y0 + 1) - self.post(x0, y0 + 1)) * tx;
        a + (b - a) * ty
    }

    /// The slope of the undisturbed ground around a post, as a normal.
    fn post_normal(&self, c: usize, r: usize) -> V3 {
        let (c0, c1) = (c.saturating_sub(1), (c + 1).min(self.cols - 1));
        let (r0, r1) = (r.saturating_sub(1), (r + 1).min(self.rows - 1));
        let dx = (self.post(c1, r) - self.post(c0, r)) / ((c1 - c0) as f32 * self.step[0]);
        let dy = (self.post(c, r1) - self.post(c, r0)) / ((r1 - r0) as f32 * self.step[1]);
        normalize([-dx, -dy, 1.0])
    }

    pub fn normal(&self, p: P2) -> V3 {
        if self.is_empty() {
            return UP;
        }
        let c = (((p[0] - self.origin[0]) / self.step[0]).round().max(0.0) as usize).min(self.cols - 1);
        let r = (((p[1] - self.origin[1]) / self.step[1]).round().max(0.0) as usize).min(self.rows - 1);
        self.post_normal(c, r)
    }
}

pub fn build(cx: &mut Ctx, project: &Project, ground: &Ground) {
    if !ground.is_empty() {
        surface(cx, project, ground);
        skirt(cx, ground);
    }
    for patch in &project.site.patches {
        let pick = cx.scene.pick_id(Some(Pick::Patch(patch.id)));
        let s = cx.surf(&patch.finish);
        let mut stack = triangulate(&patch.outline);
        let mut budget = 40_000;
        while let Some(t) = stack.pop() {
            let d = |a: P2, b: P2| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
            let edges = [d(t[0], t[1]), d(t[1], t[2]), d(t[2], t[0])];
            let longest = (0..3).max_by(|a, b| edges[*a].total_cmp(&edges[*b])).unwrap_or(0);
            if edges[longest] > 0.75 * 0.75 && budget > 0 {
                budget -= 1;
                let (a, b, c) = (t[longest], t[(longest + 1) % 3], t[(longest + 2) % 3]);
                let m = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
                stack.push([a, m, c]);
                stack.push([m, b, c]);
            } else {
                smooth_tri(cx.scene, t.map(|p| [p[0], p[1], ground.height(p) + 0.03]), t.map(|p| ground.normal(p)), t, &s, pick);
            }
        }
    }
}

fn surface(cx: &mut Ctx, project: &Project, g: &Ground) {
    let s = cx.surf(&project.site.terrain.finish);
    let soil = cx.surf(&Finish::of("soil"));
    // Undug cells share the grid's posts, so the ground shades smoothly.
    let base = cx.scene.vertices.len() as u32;
    for r in 0..g.rows {
        for c in 0..g.cols {
            let p = g.xy(c, r);
            cx.scene.vertices.push(Vertex {
                pos: [p[0], p[1], g.post(c, r)],
                pattern: s.pattern as u32 as f32,
                normal: g.post_normal(c, r),
                scale: s.scale,
                color: [s.color[0], s.color[1], s.color[2], s.opacity],
                uv: p,
                roughness: s.roughness,
                pick: 0.0,
            });
        }
    }
    let mut indices = Vec::new();
    for r in 0..g.rows - 1 {
        for c in 0..g.cols - 1 {
            let Some(z) = g.cell(c, r) else {
                let i = base + (r * g.cols + c) as u32;
                let (j, k, l) = (i + 1, i + g.cols as u32, i + g.cols as u32 + 1);
                indices.extend([i, j, l, i, l, k]);
                continue;
            };
            let q = [g.corner(c, r, 0, 0), g.corner(c, r, 1, 0), g.corner(c, r, 1, 1), g.corner(c, r, 0, 1)];
            cx.scene.quad(q, [q[0][0], q[0][1]], &soil, 0.0);
            // Earth walls where a neighbour stands at another height: an undug cell, or a
            // shallower pit (the deeper one draws the wall between two pits).
            let sides: [(isize, isize, [usize; 2], [usize; 2]); 4] = [(0, -1, [0, 0], [1, 0]), (1, 0, [1, 0], [1, 1]), (0, 1, [1, 1], [0, 1]), (-1, 0, [0, 1], [0, 0])];
            for (dc, dr, a, b) in sides {
                let (nc, nr) = (c as isize + dc, r as isize + dr);
                if nc < 0 || nr < 0 || nc >= g.cols as isize - 1 || nr >= g.rows as isize - 1 {
                    continue;
                }
                let (nc, nr) = (nc as usize, nr as usize);
                if g.cell(nc, nr).is_some_and(|nz| nz <= z) {
                    continue;
                }
                // The shared edge, as this cell and as its neighbour see it.
                let (a0, b0) = (g.corner(c, r, a[0], a[1]), g.corner(c, r, b[0], b[1]));
                let other = |d: [usize; 2]| g.corner(nc, nr, (c + d[0]) - nc, (r + d[1]) - nr);
                let (a1, b1) = (other(a), other(b));
                // The wall faces whichever side is lower. Where the neighbour's ground crosses
                // the pit's floor along the edge, it's two triangles facing opposite ways.
                let into = [-(dc as f32), -(dr as f32), 0.0];
                let facing = |rise: f32| if rise > 0.0 { into } else { [-into[0], -into[1], 0.0] };
                let (ra, rb) = (a1[2] - z, b1[2] - z);
                if ra * rb < 0.0 {
                    let t = ra / (ra - rb);
                    let x = [a0[0] + (b0[0] - a0[0]) * t, a0[1] + (b0[1] - a0[1]) * t, z];
                    planar(cx.scene, &[a0, x, a1], facing(ra), [0.0, z], &soil, 0.0);
                    planar(cx.scene, &[x, b0, b1], facing(rb), [0.0, z], &soil, 0.0);
                } else {
                    planar(cx.scene, &[a0, b0, b1, a1], facing(ra + rb), [0.0, z], &soil, 0.0);
                }
            }
        }
    }
    let list = if s.opacity < 1.0 { &mut cx.scene.translucent } else { &mut cx.scene.opaque };
    list.extend(indices);
}

/// Earth down from the site's edge, so it reads as a block cut out of the ground.
fn skirt(cx: &mut Ctx, g: &Ground) {
    let soil = cx.surf(&Finish::of("soil"));
    let floor = g.natural.iter().copied().chain(g.dug.iter().flatten().copied()).fold(f32::MAX, f32::min) - 2.0;
    let side = |cx: &mut Ctx, a: V3, b: V3, out: V3, along: f32| {
        planar(cx.scene, &[[a[0], a[1], floor], [b[0], b[1], floor], b, a], out, [along, floor], &soil, 0.0);
    };
    let (lc, lr) = (g.cols - 2, g.rows - 2);
    for c in 0..g.cols - 1 {
        let along = c as f32 * g.step[0];
        side(cx, g.corner(c, 0, 0, 0), g.corner(c, 0, 1, 0), [0.0, -1.0, 0.0], along);
        side(cx, g.corner(c, lr, 0, 1), g.corner(c, lr, 1, 1), [0.0, 1.0, 0.0], along);
    }
    for r in 0..g.rows - 1 {
        let along = r as f32 * g.step[1];
        side(cx, g.corner(0, r, 0, 0), g.corner(0, r, 0, 1), [-1.0, 0.0, 0.0], along);
        side(cx, g.corner(lc, r, 1, 0), g.corner(lc, r, 1, 1), [1.0, 0.0, 0.0], along);
    }
}
