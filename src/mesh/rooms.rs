//! Floors, slabs, ceilings and stairs.

use super::Ctx;
use super::geom::{DOWN, Frame3, UP, ccw, cuboid, inside, triangulate};
use crate::model::{Finish, Level, P2, Pick, Railing, Room, Stair};

/// The room on a level that a plan point is in.
pub fn room_at(level: &Level, p: P2) -> Option<&Room> {
    level.rooms.iter().find(|r| r.outline.len() >= 3 && inside(&r.outline, p))
}

/// Floor height (above the datum) at a plan point: the level's, or the room's there.
pub fn floor_at(level: &Level, p: P2) -> f32 {
    level.elevation + room_at(level, p).map_or(0.0, |r| r.floor_offset)
}

/// Ceiling height (above the datum) at a plan point.
pub fn ceiling_at(level: &Level, p: P2) -> f32 {
    match room_at(level, p) {
        Some(r) => level.elevation + r.floor_offset + r.ceiling_height.unwrap_or(level.height) + r.ceiling_rise(p),
        None => level.elevation + level.height,
    }
}

pub fn build(cx: &mut Ctx, level: &Level, ceilings: bool) {
    for room in &level.rooms {
        room_mesh(cx, level, room, ceilings);
        trims(cx, level, room, ceilings);
    }
    for stair in &level.stairs {
        stair_mesh(cx, level, stair);
    }
}

fn room_mesh(cx: &mut Ctx, level: &Level, room: &Room, ceilings: bool) {
    let outline = ccw(&room.outline);
    let tris = triangulate(&outline);
    if tris.is_empty() {
        return;
    }
    let pick = cx.scene.pick_id(Some(Pick::Room(room.id)));
    let zf = level.elevation + room.floor_offset;
    let floor = cx.surf(&room.floor);
    let at = |p: P2, z: f32| [p[0], p[1], z];
    for t in &tris {
        cx.scene.polygon(&t.map(|p| at(p, zf)), t, UP, &floor, pick);
    }
    // The structure under the floor.
    let slab = level.slab.max(0.0);
    if slab > 1e-4 {
        let concrete = cx.surf(&Finish::of("concrete"));
        let zs = zf - slab;
        for t in &tris {
            cx.scene.polygon(&[at(t[0], zs), at(t[2], zs), at(t[1], zs)], &[t[0], t[2], t[1]], DOWN, &concrete, 0.0);
        }
        let mut along = 0.0;
        for i in 0..outline.len() {
            let (p, q) = (outline[i], outline[(i + 1) % outline.len()]);
            cx.scene.quad([at(p, zs), at(q, zs), at(q, zf), at(p, zf)], [along, zs], &concrete, 0.0);
            along += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        }
    }
    if ceilings && room.has_ceiling {
        // A hair under the wall tops, so the two don't fight when seen from above.
        let zc = zf + room.ceiling_height.unwrap_or(level.height) - 0.005;
        let ceiling = cx.surf(&room.ceiling);
        if room.ceiling_shape == crate::model::CeilingShape::Flat {
            for t in &tris {
                cx.scene.polygon(&[at(t[0], zc), at(t[2], zc), at(t[1], zc)], &[t[0], t[2], t[1]], DOWN, &ceiling, pick);
            }
        } else {
            // A vaulted ceiling folds along its ridge, so cut the outline there first.
            let halves: Vec<Vec<P2>> = match room.ridge() {
                Some((on, dir)) => [1.0f32, -1.0].iter().map(|side| clip(&outline, on, [-dir[1] * side, dir[0] * side])).filter(|h| h.len() >= 3).collect(),
                None => vec![outline.clone()],
            };
            for half in &halves {
                for t in triangulate(half) {
                    let p: [[f32; 3]; 3] = [t[0], t[2], t[1]].map(|q| at(q, zc + room.ceiling_rise(q)));
                    let n = crate::scene::normalize(crate::scene::cross(crate::scene::sub(p[1], p[0]), crate::scene::sub(p[2], p[0])));
                    cx.scene.polygon(&p, &[t[0], t[2], t[1]], n, &ceiling, pick);
                }
            }
        }
    }
}

/// The part of a polygon on the side of the line through `on` that `normal` points to.
fn clip(outline: &[P2], on: P2, normal: P2) -> Vec<P2> {
    let side = |p: P2| (p[0] - on[0]) * normal[0] + (p[1] - on[1]) * normal[1];
    let mut out = Vec::with_capacity(outline.len() + 2);
    for i in 0..outline.len() {
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            let k = sa / (sa - sb);
            out.push([a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]);
        }
    }
    out
}

fn stair_mesh(cx: &mut Ctx, level: &Level, stair: &Stair) {
    let n = stair.steps.clamp(1, 200);
    if !(stair.run > 1e-3) || !(stair.width > 1e-3) || !stair.rise.is_finite() || stair.rise.abs() < 1e-3 {
        return;
    }
    let pick = cx.scene.pick_id(Some(Pick::Stair(stair.id)));
    let s = cx.surf(&stair.tread);
    let (sin, cos) = stair.direction.sin_cos();
    let f = Frame3::upright([stair.at[0], stair.at[1], floor_at(level, stair.at)], [cos, sin]);
    let (going, riser) = (stair.run / n as f32, stair.rise / n as f32);
    let hw = stair.width * 0.5;
    for i in 0..n {
        let (x0, top) = (i as f32 * going, (i + 1) as f32 * riser);
        if stair.closed {
            // Each step sits on the one before, with a waist under the flight.
            let under = (top - riser - 0.18).max(0.0).min(top - 0.02);
            cuboid(cx.scene, &f, [x0, -hw, under], [x0 + going, hw, top], &s, pick);
        } else {
            cuboid(cx.scene, &f, [x0, -hw, top - 0.04], [x0 + going + 0.02, hw, top], &s, pick);
        }
    }
    // The pitch line through the nosings, for stringers and handrails.
    let slope = Frame3::new(f.o, f.dir([stair.run, 0.0, stair.rise]), f.y);
    let length = (stair.run * stair.run + stair.rise * stair.rise).sqrt();
    let cos_pitch = stair.run / length;
    if !stair.closed {
        let dark = cx.surf(&Finish::colored("aluminium", [0.18, 0.18, 0.19]));
        for y in [-hw, hw - 0.04] {
            cuboid(cx.scene, &slope, [0.0, y, -0.22], [length, y + 0.04, riser * cos_pitch - 0.045], &dark, pick);
        }
    }
    let sides: &[f32] = match stair.railing {
        Railing::None => &[],
        Railing::Left => &[1.0],
        Railing::Right => &[-1.0],
        Railing::Both => &[1.0, -1.0],
    };
    if !sides.is_empty() {
        let post = cx.surf(&Finish::colored("aluminium", [0.18, 0.18, 0.19]));
        let rail_height = 0.9;
        for side in sides {
            let y = side * (hw - 0.04);
            let every = (n / 6).max(1);
            for i in (0..n).step_by(every as usize).chain([n - 1]) {
                let (x, top) = ((i as f32 + 0.5) * going, (i + 1) as f32 * riser);
                cuboid(cx.scene, &f, [x - 0.015, y - 0.015, top], [x + 0.015, y + 0.015, top + rail_height + (x - i as f32 * going) * stair.rise / stair.run], &post, pick);
            }
            let z = (riser + rail_height) * cos_pitch;
            cuboid(cx.scene, &slope, [0.0, y - 0.025, z], [length, y + 0.025, z + 0.04], &s, pick);
        }
    }
}

/// Skirting boards and cornices: along every stretch of wall that bounds the room, on the
/// room's side of it, with gaps in the skirting where a door or opening reaches the floor.
fn trims(cx: &mut Ctx, level: &Level, room: &Room, ceilings: bool) {
    let shaped = room.has_ceiling && room.ceiling_shape != crate::model::CeilingShape::Flat;
    if room.skirting.is_none() && room.cornice.is_none() && !shaped {
        return;
    }
    let outline = ccw(&room.outline);
    if outline.len() < 3 {
        return;
    }
    let pick = cx.scene.pick_id(Some(Pick::Room(room.id)));
    let floor = room.floor_offset;
    let top = floor + room.ceiling_height.unwrap_or(level.height);
    for wall in &level.walls {
        let len = wall.length();
        if len < 1e-3 {
            continue;
        }
        let (d, n) = wall.frame();
        let h = wall.thickness / 2.0;
        let wf = Frame3::upright([wall.a[0], wall.a[1], level.elevation], d);
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let e = [b[0] - a[0], b[1] - a[1]];
            let el = (e[0] * e[0] + e[1] * e[1]).sqrt();
            if el < 1e-3 {
                continue;
            }
            // The edge must lie along this wall: parallel to it and on its centre line.
            let across = |p: P2| (p[0] - wall.a[0]) * n[0] + (p[1] - wall.a[1]) * n[1];
            let along = |p: P2| (p[0] - wall.a[0]) * d[0] + (p[1] - wall.a[1]) * d[1];
            if (e[0] * n[0] + e[1] * n[1]).abs() / el > 0.02 || across(a).abs() > h + 0.03 || across(b).abs() > h + 0.03 {
                continue;
            }
            let (t0, t1) = (along(a).min(along(b)).max(0.0), along(a).max(along(b)).min(len));
            if t1 - t0 < 0.05 {
                continue;
            }
            // The room is to the left of a counter-clockwise edge; which face of the wall is that?
            let inward = [-e[1] / el, e[0] / el];
            let side = if inward[0] * n[0] + inward[1] * n[1] >= 0.0 { 1.0 } else { -1.0 };
            let strip = |cx: &mut Ctx, from: f32, to: f32, z0: f32, z1: f32, depth: f32, finish: &Finish| {
                if to - from < 0.01 {
                    return;
                }
                let s = cx.surf(finish);
                let (y0, y1) = if side > 0.0 { (h, h + depth) } else { (-h - depth, -h) };
                cuboid(cx.scene, &wf, [from, y0, z0], [to, y1, z1], &s, pick);
            };
            if shaped {
                infill(cx, level, room, wall, &wf, (t0, t1), side);
            }
            if let Some(trim) = &room.skirting {
                // Leave out wherever something opens down to the floor.
                let mut gaps: Vec<(f32, f32)> = wall.openings.iter().filter(|o| o.sill < trim.height && !matches!(o.kind, crate::model::OpeningKind::Recess { .. })).map(|o| (o.at - o.width / 2.0 - 0.07, o.at + o.width / 2.0 + 0.07)).collect();
                gaps.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut from = t0;
                for (g0, g1) in gaps {
                    if g1 <= from || g0 >= t1 {
                        continue;
                    }
                    strip(cx, from, g0.min(t1), floor, floor + trim.height, trim.depth, &trim.finish);
                    from = from.max(g1);
                }
                strip(cx, from, t1, floor, floor + trim.height, trim.depth, &trim.finish);
            }
            if let Some(trim) = room.cornice.as_ref().filter(|_| room.has_ceiling && ceilings && room.ceiling_shape == crate::model::CeilingShape::Flat) {
                strip(cx, t0, t1, top - trim.height - 0.005, top - 0.005, trim.depth, &trim.finish);
            }
        }
    }
}

/// The piece of wall between a wall's flat top and a sloped or vaulted ceiling over it: the
/// wall's own thickness, following the ceiling's line, in the wall's finishes.
fn infill(cx: &mut Ctx, level: &Level, room: &Room, wall: &crate::model::Wall, wf: &Frame3, (t0, t1): (f32, f32), side: f32) {
    let h = wall.thickness / 2.0;
    let top = wall.base + wall.height.unwrap_or(level.height);
    // The ceiling's height above the level's floor along the wall, just inside the room.
    let ceiling = |t: f32| {
        let p = wall.point(t, side * (h + 0.02));
        room.floor_offset + room.ceiling_height.unwrap_or(level.height) + room.ceiling_rise(p)
    };
    // Break the run where the ceiling folds, so each stretch has a straight top.
    let mut stops = vec![t0, t1];
    let n = 16;
    for i in 1..n {
        stops.push(t0 + (t1 - t0) * i as f32 / n as f32);
    }
    stops.sort_by(|a, b| a.total_cmp(b));
    let pick = cx.scene.pick_id(Some(Pick::Wall(wall.id)));
    let (left, right) = (cx.surf(&wall.left), cx.surf(&wall.right));
    let cap = cx.surf(&Finish::paint(0xe8e4dc));
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (za, zb) = (ceiling(a).max(top), ceiling(b).max(top));
        if b - a < 1e-4 || (za - top < 1e-3 && zb - top < 1e-3) {
            continue;
        }
        let p = |t: f32, y: f32, z: f32| wf.at([t, y, z]);
        let d = wf.x;
        let n3 = [-d[1], d[0], 0.0];
        cx.scene.polygon(&[p(b, h, top), p(a, h, top), p(a, h, za), p(b, h, zb)], &[[-b, top], [-a, top], [-a, za], [-b, zb]], n3, &left, pick);
        cx.scene.polygon(&[p(a, -h, top), p(b, -h, top), p(b, -h, zb), p(a, -h, za)], &[[a, top], [b, top], [b, zb], [a, za]], [d[1], -d[0], 0.0], &right, pick);
        cx.scene.quad([p(a, -h, za), p(b, -h, zb), p(b, h, zb), p(a, h, za)], [a, -h], &cap, pick);
    }
}
