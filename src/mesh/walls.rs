//! Walls: a box along the centre line with the openings cut out of its faces.

use super::geom::{DOWN, Frame3, UP, V3, planar, segment_distance};
use super::{Ctx, openings};
use crate::materials::Surface;
use crate::model::{Finish, Level, Opening, OpeningKind, Pick, Wall, WallSide};
use crate::scene::mul;

/// An opening's rectangle on the wall, clamped to it: along the wall and above the level's floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    pub u0: f32,
    pub u1: f32,
    pub z0: f32,
    pub z1: f32,
}

pub fn sign(side: WallSide) -> f32 {
    match side {
        WallSide::Left => 1.0,
        WallSide::Right => -1.0,
    }
}

/// The wall's vertical extent above the level's floor.
pub fn extent(level: &Level, wall: &Wall) -> (f32, f32) {
    (wall.base, wall.height.unwrap_or(level.height))
}

/// The openings that fit on the wall, in order along it, each with its clamped rectangle.
/// One that overlaps the opening before it is left out.
pub fn holes<'a>(level: &Level, wall: &'a Wall) -> Vec<(Hole, &'a Opening)> {
    let l = wall.length();
    let (zb, zt) = extent(level, wall);
    let mut sorted: Vec<&Opening> = wall.openings.iter().collect();
    sorted.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut out: Vec<(Hole, &Opening)> = Vec::new();
    for op in sorted {
        if ![op.at, op.width, op.height, op.sill].iter().all(|v| v.is_finite()) {
            continue;
        }
        let hole = Hole {
            u0: (op.at - op.width * 0.5).max(0.0),
            u1: (op.at + op.width * 0.5).min(l),
            z0: op.sill.max(zb),
            z1: (op.sill + op.height).min(zt),
        };
        if !(hole.u1 - hole.u0 >= 0.01) || !(hole.z1 - hole.z0 >= 0.01) {
            continue;
        }
        if out.last().is_some_and(|(prev, _)| hole.u0 < prev.u1 - 1e-4) {
            continue;
        }
        out.push((hole, op));
    }
    out
}

/// How far an end at `p` runs on to close a corner: half the thickness of a wall it meets, 0
/// against a wall it continues in line, `None` for a free end.
fn extension(level: &Level, wall: &Wall, p: [f32; 2]) -> Option<f32> {
    let (d, _) = wall.frame();
    let mut best: Option<f32> = None;
    for other in &level.walls {
        if other.id == wall.id || other.length() < 1e-4 {
            continue;
        }
        if segment_distance(p, other.a, other.b) > other.thickness * 0.5 + 0.02 {
            continue;
        }
        let (od, _) = other.frame();
        let parallel = (d[0] * od[1] - d[1] * od[0]).abs() < 0.2;
        let e = if parallel { 0.0 } else { other.thickness * 0.5 };
        best = Some(best.map_or(e, |b| b.max(e)));
    }
    best
}

/// One rectangle of a wall face at `y`, facing `sgn` (left +1, right -1), with the pattern
/// continuing along the wall.
fn face_rect(cx: &mut Ctx, wf: &Frame3, y: f32, sgn: f32, r: Hole, s: &Surface, pick: f32) {
    if !(r.u1 - r.u0 > 1e-5) || !(r.z1 - r.z0 > 1e-5) {
        return;
    }
    let p = |u: f32, z: f32| wf.at([u, y, z]);
    if sgn > 0.0 {
        cx.scene.quad([p(r.u1, r.z0), p(r.u0, r.z0), p(r.u0, r.z1), p(r.u1, r.z1)], [-r.u1, r.z0], s, pick);
    } else {
        cx.scene.quad([p(r.u0, r.z0), p(r.u1, r.z0), p(r.u1, r.z1), p(r.u0, r.z1)], [r.u0, r.z0], s, pick);
    }
}

/// A face rectangle with holes cut out: piers between them, a spandrel under each, a head over.
fn face(cx: &mut Ctx, wf: &Frame3, y: f32, sgn: f32, whole: Hole, holes: &[Hole], s: &Surface, pick: f32) {
    let mut cursor = whole.u0;
    for h in holes {
        face_rect(cx, wf, y, sgn, Hole { u0: cursor, u1: h.u0, ..whole }, s, pick);
        face_rect(cx, wf, y, sgn, Hole { z0: whole.z0, z1: h.z0, ..*h }, s, pick);
        face_rect(cx, wf, y, sgn, Hole { z0: h.z1, z1: whole.z1, ..*h }, s, pick);
        cursor = h.u1;
    }
    face_rect(cx, wf, y, sgn, Hole { u0: cursor, ..whole }, s, pick);
}

/// The four sides of a hole between `ya` and `yb` across the wall. A door-height hole gets a
/// threshold just proud of the floor.
fn reveals(cx: &mut Ctx, wf: &Frame3, h: Hole, ya: f32, yb: f32, floor: f32, s: &Surface, pick: f32) {
    let p = |u: f32, y: f32, z: f32| wf.at([u, y, z]);
    planar(cx.scene, &[p(h.u0, ya, h.z0), p(h.u0, yb, h.z0), p(h.u0, yb, h.z1), p(h.u0, ya, h.z1)], wf.x, [ya, h.z0], s, pick);
    planar(cx.scene, &[p(h.u1, ya, h.z0), p(h.u1, yb, h.z0), p(h.u1, yb, h.z1), p(h.u1, ya, h.z1)], mul(wf.x, -1.0), [ya, h.z0], s, pick);
    planar(cx.scene, &[p(h.u0, ya, h.z1), p(h.u1, ya, h.z1), p(h.u1, yb, h.z1), p(h.u0, yb, h.z1)], DOWN, [h.u0, ya], s, pick);
    let z = if h.z0 <= floor + 1e-3 { h.z0 + 0.005 } else { h.z0 };
    planar(cx.scene, &[p(h.u0, ya, z), p(h.u1, ya, z), p(h.u1, yb, z), p(h.u0, yb, z)], UP, [h.u0, ya], s, pick);
}

pub fn build(cx: &mut Ctx, level: &Level) {
    for wall in &level.walls {
        wall_mesh(cx, level, wall);
    }
}

fn wall_mesh(cx: &mut Ctx, level: &Level, wall: &Wall) {
    let l = wall.length();
    let (zb, zt) = extent(level, wall);
    if !(l >= 1e-4) || !(zt - zb >= 1e-4) || !wall.thickness.is_finite() {
        return;
    }
    let t = wall.thickness.max(0.01);
    let h = t * 0.5;
    let (d, _) = wall.frame();
    let wf = Frame3::upright([wall.a[0], wall.a[1], level.elevation], d);
    let (ea, eb) = (extension(level, wall, wall.a), extension(level, wall, wall.b));
    let (ua, ub) = (-ea.unwrap_or(0.0), l + eb.unwrap_or(0.0));
    let pick = cx.scene.pick_id(Some(Pick::Wall(wall.id)));
    let (left, right) = (cx.surf(&wall.left), cx.surf(&wall.right));
    let cap = cx.surf(&Finish::paint(0xe8e4dc));

    let holes = holes(level, wall);
    let cut = |skip: WallSide| -> Vec<Hole> {
        holes.iter().filter(|(_, op)| !matches!(&op.kind, OpeningKind::Recess { side, .. } if *side == skip)).map(|(h, _)| *h).collect()
    };
    let whole = Hole { u0: ua, u1: ub, z0: zb, z1: zt };
    face(cx, &wf, h, 1.0, whole, &cut(WallSide::Right), &left, pick);
    face(cx, &wf, -h, -1.0, whole, &cut(WallSide::Left), &right, pick);

    let p = |u: f32, y: f32, z: f32| -> V3 { wf.at([u, y, z]) };
    planar(cx.scene, &[p(ua, -h, zt), p(ub, -h, zt), p(ub, h, zt), p(ua, h, zt)], UP, [ua, -h], &cap, pick);
    if zb > 1e-3 {
        planar(cx.scene, &[p(ua, -h, zb), p(ub, -h, zb), p(ub, h, zb), p(ua, h, zb)], DOWN, [ua, -h], &cap, pick);
    }
    if ea.is_none() {
        planar(cx.scene, &[p(ua, -h, zb), p(ua, h, zb), p(ua, h, zt), p(ua, -h, zt)], mul(wf.x, -1.0), [-h, zb], &left, pick);
    }
    if eb.is_none() {
        planar(cx.scene, &[p(ub, -h, zb), p(ub, h, zb), p(ub, h, zt), p(ub, -h, zt)], wf.x, [-h, zb], &left, pick);
    }

    for (hole, op) in &holes {
        let op_pick = cx.scene.pick_id(Some(Pick::Opening(wall.id, op.id)));
        match &op.kind {
            OpeningKind::Recess { side, depth, finish } => {
                let sgn = sign(*side);
                let back = sgn * (h - depth.clamp(0.005, (t - 0.01).max(0.005)));
                let s = cx.surf(finish);
                face_rect(cx, &wf, back, sgn, *hole, &s, op_pick);
                reveals(cx, &wf, *hole, back, sgn * h, f32::NEG_INFINITY, &s, op_pick);
            }
            kind => {
                // Each face's finish turns the corner and meets the other mid-wall.
                reveals(cx, &wf, *hole, 0.0, h, zb, &left, pick);
                reveals(cx, &wf, *hole, -h, 0.0, zb, &right, pick);
                match kind {
                    OpeningKind::Door(door) => openings::door(cx, &wf, h, *hole, door, op_pick),
                    OpeningKind::Garage(door) => openings::garage(cx, &wf, h, *hole, door, op_pick),
                    OpeningKind::Window(window) => openings::window(cx, &wf, h, *hole, window, op_pick),
                    OpeningKind::Passage { frame } => {
                        if let Some(frame) = frame {
                            openings::casing(cx, &wf, h, *hole, frame, false, op_pick);
                        }
                    }
                    OpeningKind::Recess { .. } => {}
                }
            }
        }
    }
}
