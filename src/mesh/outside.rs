//! What stands outside: fences, gates, retaining walls and balustrades along their lines,
//! trees and shrubs, and the posts and balustrades of raised decks.

use super::Ctx;
use super::geom::{Frame3, cuboid, prism};
use super::terrain::Ground;
use crate::materials::{Pattern, Surface, linear};
use crate::model::{Barrier, BarrierKind, Finish, Level, P2, Pick, Plant, PlantKind, Project, Room, rgb};

/// A wall-like slab over the plan segment `a → b`: `t` thick, with its foot and its top
/// each at their own height at either end, so it can follow falling ground.
fn slab(cx: &mut Ctx, a: P2, b: P2, t: f32, foot: [f32; 2], top: [f32; 2], s: &Surface, pick: f32) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if len < 1e-3 || t < 1e-4 {
        return;
    }
    let n = [-d[1] / len * t / 2.0, d[0] / len * t / 2.0];
    let p = |q: P2, side: f32, z: f32| [q[0] + n[0] * side, q[1] + n[1] * side, z];
    // Left face, right face, top, the two ends.
    cx.scene.quad([p(b, 1.0, foot[1]), p(a, 1.0, foot[0]), p(a, 1.0, top[0]), p(b, 1.0, top[1])], [0.0, foot[0]], s, pick);
    cx.scene.quad([p(a, -1.0, foot[0]), p(b, -1.0, foot[1]), p(b, -1.0, top[1]), p(a, -1.0, top[0])], [0.0, foot[0]], s, pick);
    cx.scene.quad([p(a, -1.0, top[0]), p(b, -1.0, top[1]), p(b, 1.0, top[1]), p(a, 1.0, top[0])], [0.0, 0.0], s, pick);
    cx.scene.quad([p(a, 1.0, foot[0]), p(a, -1.0, foot[0]), p(a, -1.0, top[0]), p(a, 1.0, top[0])], [0.0, foot[0]], s, pick);
    cx.scene.quad([p(b, -1.0, foot[1]), p(b, 1.0, foot[1]), p(b, 1.0, top[1]), p(b, -1.0, top[1])], [0.0, foot[1]], s, pick);
}

/// A square post standing at a plan point.
fn post(cx: &mut Ctx, at: P2, dir: P2, size: f32, z0: f32, z1: f32, s: &Surface, pick: f32) {
    if z1 - z0 < 1e-3 {
        return;
    }
    let f = Frame3::upright([at[0], at[1], z0], dir);
    cuboid(cx.scene, &f, [-size / 2.0, -size / 2.0, 0.0], [size / 2.0, size / 2.0, z1 - z0], s, pick);
}

/// The points along a line where posts go: its ends and corners, and between them no further
/// apart than `bay`.
fn stations(path: &[P2], bay: f32) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    for w in path.windows(2) {
        let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if len < 1e-3 {
            continue;
        }
        let dir = [d[0] / len, d[1] / len];
        let n = (len / bay).ceil().max(1.0) as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            out.push(([w[0][0] + d[0] * t, w[0][1] + d[1] * t], dir));
        }
        out.push((w[1], dir));
    }
    out
}

fn barrier(cx: &mut Ctx, project: &Project, ground: &Ground, b: &Barrier) {
    let pick = cx.scene.pick_id(Some(Pick::Barrier(b.id)));
    let s = cx.surf(&b.finish);
    let foot = |p: P2| b.base.unwrap_or_else(|| ground.natural_height(p));
    let h = b.height.max(0.05);
    let t = b.thickness.max(0.005);
    match b.kind {
        BarrierKind::Retaining => {
            // A level top, set by the highest ground it meets as that ground was before any
            // filling behind it; the foot goes below the ground in front.
            let terrain = &project.site.terrain;
            let top = b.level_top(terrain);
            let foot = |p: P2| b.base.unwrap_or_else(|| terrain.height(p));
            let pts = stations(&b.path, 1.0);
            for w in pts.windows(2) {
                slab(cx, w[0].0, w[1].0, t, [foot(w[0].0) - 0.3, foot(w[1].0) - 0.3], [top, top], &s, pick);
            }
        }
        BarrierKind::Fence | BarrierKind::Gate => {
            let gate = b.kind == BarrierKind::Gate;
            let pts = stations(&b.path, if gate { 1.2 } else { 2.4 });
            let post_finish = if gate { cx.surf(&Finish::colored("aluminium", rgb(0x2f3134))) } else { s };
            for (p, dir) in &pts {
                post(cx, *p, *dir, if gate { 0.06 } else { 0.09 }, foot(*p) - 0.05, foot(*p) + h + 0.06, &post_finish, pick);
            }
            // A gate hangs clear of the ground; a fence's boards come nearly down to it.
            let clear = if gate { 0.08 } else { 0.03 };
            for w in pts.windows(2) {
                let (a, c) = (w[0].0, w[1].0);
                slab(cx, a, c, t, [foot(a) + clear, foot(c) + clear], [foot(a) + h, foot(c) + h], &s, pick);
            }
        }
        BarrierKind::Balustrade => {
            let glass = cx.surf(&Finish::of("glass"));
            let pts = stations(&b.path, 1.2);
            for (p, dir) in &pts {
                post(cx, *p, *dir, 0.04, foot(*p), foot(*p) + h, &s, pick);
            }
            for w in pts.windows(2) {
                let (a, c) = (w[0].0, w[1].0);
                slab(cx, a, c, 0.012, [foot(a) + 0.08, foot(c) + 0.08], [foot(a) + h - 0.06, foot(c) + h - 0.06], &glass, pick);
                slab(cx, a, c, 0.05, [foot(a) + h - 0.04, foot(c) + h - 0.04], [foot(a) + h, foot(c) + h], &s, pick);
            }
        }
    }
}

fn plant(cx: &mut Ctx, ground: &Ground, p: &Plant) {
    let pick = cx.scene.pick_id(Some(Pick::Plant(p.id)));
    let z = ground.natural_height(p.at);
    let f = Frame3::upright([p.at[0], p.at[1], z], [1.0, 0.0]);
    let (h, r) = (p.height.max(0.2), p.spread.max(0.2) / 2.0);
    let bark = cx.surf(&Finish::colored("plastic", rgb(0x5b4636)));
    let leaf = |hex: u32| Surface { pattern: Pattern::Grass, color: linear(rgb(hex)), scale: 1.0, roughness: 1.0, opacity: 1.0 };
    match p.kind {
        PlantKind::Tree => {
            let trunk = h * 0.38;
            prism(cx.scene, &f, (r * 0.07).clamp(0.05, 0.35), 0.0, trunk + 0.2, 8, &bark, &bark, pick);
            // A rounded head as stacked rings: an ellipsoid sampled in six slices.
            let leaves = leaf(0x4f7d3a);
            let (zc, hz) = (trunk + (h - trunk) / 2.0, (h - trunk) / 2.0);
            let n = 6;
            for i in 0..n {
                let (a, c) = (-1.0 + 2.0 * i as f32 / n as f32, -1.0 + 2.0 * (i + 1) as f32 / n as f32);
                let mid = (a + c) / 2.0;
                prism(cx.scene, &f, r * (1.0 - mid * mid).max(0.0).sqrt(), zc + a * hz, zc + c * hz, 10, &leaves, &leaves, pick);
            }
        }
        PlantKind::Conifer => {
            let trunk = h * 0.12;
            prism(cx.scene, &f, (r * 0.08).clamp(0.04, 0.25), 0.0, trunk + 0.1, 8, &bark, &bark, pick);
            let leaves = leaf(0x35603a);
            let n = 7;
            for i in 0..n {
                let (a, c) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
                prism(cx.scene, &f, r * (1.0 - (a + c) / 2.0 * 0.96), trunk + a * (h - trunk), trunk + c * (h - trunk), 10, &leaves, &leaves, pick);
            }
        }
        PlantKind::Shrub => {
            let leaves = leaf(0x5a8a44);
            let n = 4;
            for i in 0..n {
                let (a, c) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
                let mid = (a + c) / 2.0;
                prism(cx.scene, &f, r * (1.0 - (mid * 1.6 - 0.6).powi(2) * 0.6).max(0.2), a * h, c * h, 10, &leaves, &leaves, pick);
            }
        }
    }
}

/// True when a room's edge `a → b` has a wall along it.
fn walled(level: &Level, a: P2, b: P2) -> bool {
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    level.walls.iter().any(|w| crate::geom::segment_distance(mid, w.a, w.b) < w.thickness / 2.0 + 0.05)
}

/// A floor with nothing over it, standing clear of the ground: posts under its edge down to
/// the ground, and a balustrade along the open edges if it asks for one.
fn deck(cx: &mut Ctx, ground: &Ground, level: &Level, room: &Room) {
    if room.has_ceiling || room.outline.len() < 3 {
        return;
    }
    let pick = cx.scene.pick_id(Some(Pick::Room(room.id)));
    let floor = level.elevation + room.floor_offset;
    let under = floor - level.slab.max(0.0);
    let timber = cx.surf(&room.floor);
    let n = room.outline.len();
    for i in 0..n {
        let (a, b) = (room.outline[i], room.outline[(i + 1) % n]);
        if walled(level, a, b) {
            continue;
        }
        // Posts sit just inside the edge.
        let c = crate::geom::centroid(&room.outline);
        let inset = |p: P2| {
            let d = [c[0] - p[0], c[1] - p[1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
            [p[0] + d[0] / l * 0.12, p[1] + d[1] / l * 0.12]
        };
        for (p, dir) in stations(&[a, b], 2.4) {
            let q = inset(p);
            let g = ground.natural_height(q);
            if under - g > 0.4 {
                post(cx, q, dir, 0.1, g - 0.05, under, &timber, pick);
            }
        }
        if let Some(h) = room.balustrade.filter(|h| *h > 0.05) {
            let rail = cx.surf(&Finish::colored("aluminium", rgb(0x2f3134)));
            let glass = cx.surf(&Finish::of("glass"));
            let pts = stations(&[a, b], 1.2);
            for (p, dir) in &pts {
                post(cx, inset(*p), *dir, 0.04, floor, floor + h, &rail, pick);
            }
            let (ia, ib) = (inset(a), inset(b));
            slab(cx, ia, ib, 0.012, [floor + 0.08; 2], [floor + h - 0.06; 2], &glass, pick);
            slab(cx, ia, ib, 0.05, [floor + h - 0.04; 2], [floor + h; 2], &rail, pick);
        }
    }
}

pub fn build(cx: &mut Ctx, project: &Project, ground: &Ground, shown: &dyn Fn(usize) -> bool) {
    for b in &project.site.barriers {
        barrier(cx, project, ground, b);
    }
    for p in &project.site.plants {
        plant(cx, ground, p);
    }
    for (i, level) in project.levels.iter().enumerate() {
        if level.visible && shown(i) {
            for room in &level.rooms {
                deck(cx, ground, level, room);
            }
        }
    }
}
