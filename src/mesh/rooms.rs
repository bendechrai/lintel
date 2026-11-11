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
        Some(r) => level.elevation + r.floor_offset + r.ceiling_height.unwrap_or(level.height),
        None => level.elevation + level.height,
    }
}

pub fn build(cx: &mut Ctx, level: &Level, ceilings: bool) {
    for room in &level.rooms {
        room_mesh(cx, level, room, ceilings);
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
        for t in &tris {
            cx.scene.polygon(&[at(t[0], zc), at(t[2], zc), at(t[1], zc)], &[t[0], t[2], t[1]], DOWN, &ceiling, pick);
        }
    }
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
