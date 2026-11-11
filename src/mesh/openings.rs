//! What fills a hole in a wall: door leaves and frames, garage doors, windows.
//!
//! Everything is built in the wall's frame: x along the wall from its `a` end, y across it
//! (positive toward the left face), z up from the level's floor.

use super::Ctx;
use super::geom::{Frame3, cuboid};
use super::walls::{Hole, sign};
use crate::model::{Door, DoorStyle, Finish, Frame, GarageDoor, GarageStyle, Hinge, Window, WindowStyle};
use crate::scene::{add, mul};

const LEAF: f32 = 0.04;

/// A jamb lining through the wall and an architrave on both faces. Returns the clear hole
/// inside the lining.
pub fn casing(cx: &mut Ctx, wf: &Frame3, h: f32, hole: Hole, frame: &Frame, four_sides: bool, pick: f32) -> Hole {
    let s = cx.surf(&frame.finish);
    let Hole { u0, u1, z0, z1 } = hole;
    let j = frame.jamb.clamp(0.0, (u1 - u0) * 0.25).min((z1 - z0) * 0.25);
    let y = h + 0.002;
    if j > 1e-4 {
        cuboid(cx.scene, wf, [u0, -y, z0], [u0 + j, y, z1], &s, pick);
        cuboid(cx.scene, wf, [u1 - j, -y, z0], [u1, y, z1], &s, pick);
        cuboid(cx.scene, wf, [u0 + j, -y, z1 - j], [u1 - j, y, z1], &s, pick);
        if four_sides {
            cuboid(cx.scene, wf, [u0 + j, -y, z0], [u1 - j, y, z0 + j], &s, pick);
        }
    }
    let c = frame.casing;
    if c > 1e-4 {
        let bottom = if four_sides { z0 - c } else { z0 };
        for sgn in [1.0f32, -1.0] {
            let (ya, yb) = (sgn * h, sgn * (h + 0.015));
            cuboid(cx.scene, wf, [u0 - c, ya, bottom], [u0, yb, z1 + c], &s, pick);
            cuboid(cx.scene, wf, [u1, ya, bottom], [u1 + c, yb, z1 + c], &s, pick);
            cuboid(cx.scene, wf, [u0, ya, z1], [u1, yb, z1 + c], &s, pick);
            if four_sides {
                cuboid(cx.scene, wf, [u0, ya, z0 - c], [u1, yb, z0], &s, pick);
            }
        }
    }
    Hole { u0: u0 + j, u1: u1 - j, z0: if four_sides { z0 + j } else { z0 }, z1: z1 - j }
}

/// A door leaf in its own frame: x across the leaf from its hinge edge, z up from its foot,
/// `y0..y1` its thickness. `handle` is the x of a lever handle.
fn leaf(cx: &mut Ctx, f: &Frame3, w: f32, hgt: f32, y0: f32, y1: f32, door: &Door, pick: f32, handle: Option<f32>) {
    if !(w > 0.01) || !(hgt > 0.01) {
        return;
    }
    let s = cx.surf(&door.leaf);
    let g = door.glazing.clamp(0.0, 1.0);
    if g < 0.02 || w < 0.25 {
        cuboid(cx.scene, f, [0.0, y0, 0.0], [w, y1, hgt], &s, pick);
    } else {
        let st = 0.1f32.min(w * 0.25);
        let top = hgt - st;
        let bottom = (top - g * (hgt - 2.0 * st)).max(st);
        cuboid(cx.scene, f, [0.0, y0, 0.0], [st, y1, hgt], &s, pick);
        cuboid(cx.scene, f, [w - st, y0, 0.0], [w, y1, hgt], &s, pick);
        cuboid(cx.scene, f, [st, y0, top], [w - st, y1, hgt], &s, pick);
        cuboid(cx.scene, f, [st, y0, 0.0], [w - st, y1, bottom], &s, pick);
        let glass = cx.surf(&Finish::of("glass"));
        let ym = (y0 + y1) * 0.5;
        cuboid(cx.scene, f, [st, ym - 0.004, bottom], [w - st, ym + 0.004, top], &glass, pick);
    }
    if let Some(x) = handle.filter(|_| hgt > 1.2) {
        let steel = cx.surf(&Finish::of("steel"));
        let toward = if x > w * 0.5 { -1.0 } else { 1.0 };
        cuboid(cx.scene, f, [x - 0.012, y0 - 0.05, 0.985], [x + 0.012, y1 + 0.05, 1.015], &steel, pick);
        for (ya, yb) in [(y0 - 0.05, y0 - 0.035), (y1 + 0.035, y1 + 0.05)] {
            cuboid(cx.scene, f, [x - 0.012, ya, 0.985], [x - 0.012 + toward * 0.11, yb, 1.015], &steel, pick);
        }
    }
}

pub fn door(cx: &mut Ctx, wf: &Frame3, h: f32, hole: Hole, door: &Door, pick: f32) {
    let clear = match &door.frame {
        Some(frame) => casing(cx, wf, h, hole, frame, false, pick),
        None => hole,
    };
    let (u0, u1) = (clear.u0, clear.u1);
    let w = u1 - u0;
    let zl = clear.z0 + 0.008;
    let hgt = clear.z1 - zl - 0.003;
    if !(w > 0.02) || !(hgt > 0.02) {
        return;
    }
    let sgn = sign(door.swing);
    let open = if door.open.is_finite() { door.open.clamp(0.0, 1.0) } else { 0.0 };
    let from_start = door.hinge == Hinge::Start;
    // A leaf hinged at `pivot` on the swing-side face, lying along `dir` (±1 along the wall)
    // when closed.
    let hinged = |cx: &mut Ctx, pivot: f32, dir: f32, width: f32, turn: f32, handle: bool| {
        let (sin, cos) = turn.sin_cos();
        let x = add(mul(wf.x, cos * dir), mul(wf.y, sin * sgn));
        let f = Frame3::upright(wf.at([pivot, sgn * h, zl]), [x[0], x[1]]);
        let (y0, y1) = if sgn * dir > 0.0 { (-LEAF, 0.0) } else { (0.0, LEAF) };
        leaf(cx, &f, width, hgt, y0, y1, door, pick, handle.then_some(width - 0.07));
    };
    let quarter = std::f32::consts::FRAC_PI_2;
    match door.style {
        DoorStyle::Hinged => {
            let (pivot, dir) = if from_start { (u0, 1.0) } else { (u1, -1.0) };
            hinged(cx, pivot, dir, w, open * quarter, true);
        }
        DoorStyle::Double => {
            hinged(cx, u0, 1.0, w * 0.5 - 0.002, open * quarter, true);
            hinged(cx, u1, -1.0, w * 0.5 - 0.002, open * quarter, true);
        }
        DoorStyle::Bifold => {
            let (pivot, dir) = if from_start { (u0, 1.0) } else { (u1, -1.0) };
            let half = w * 0.5;
            let turn = open * 1.45;
            hinged(cx, pivot, dir, half, turn, false);
            // The second half hangs off the first's free edge and folds back the other way.
            let (sin, cos) = turn.sin_cos();
            let knee = add(wf.at([pivot, sgn * h, zl]), mul(add(mul(wf.x, cos * dir), mul(wf.y, sin * sgn)), half));
            let x = add(mul(wf.x, cos * dir), mul(wf.y, -sin * sgn));
            let f = Frame3::upright(knee, [x[0], x[1]]);
            let (y0, y1) = if sgn * dir > 0.0 { (-LEAF, 0.0) } else { (0.0, LEAF) };
            leaf(cx, &f, half, hgt, y0, y1, door, pick, None);
        }
        DoorStyle::Sliding => {
            // A fixed panel on the hinge side, and a leaf on the swing side of it that slides
            // across its face.
            let panel = w * 0.5 + 0.02;
            let travel = (w - panel) * open;
            let (fixed, slider) = if from_start { (u0, u1 - panel - travel) } else { (u1 - panel, u0 + travel) };
            let f = Frame3::upright(wf.at([fixed, 0.0, zl]), [wf.x[0], wf.x[1]]);
            let (fy, sy) = if sgn > 0.0 { ((-LEAF, 0.0), (0.0, LEAF)) } else { ((0.0, LEAF), (-LEAF, 0.0)) };
            leaf(cx, &f, panel, hgt, fy.0, fy.1, door, pick, None);
            let f = Frame3::upright(wf.at([slider, 0.0, zl]), [wf.x[0], wf.x[1]]);
            let handle = if from_start { panel - 0.06 } else { 0.06 };
            leaf(cx, &f, panel, hgt, sy.0, sy.1, door, pick, Some(handle));
        }
        DoorStyle::Pocket => {
            // The leaf slides into the wall on the hinge side; what's left in the opening shows.
            let shown = (w * (1.0 - open)).max(0.03);
            let start = if from_start { u0 } else { u1 - shown };
            let f = Frame3::upright(wf.at([start, 0.0, zl]), [wf.x[0], wf.x[1]]);
            let handle = if from_start { shown - 0.06 } else { 0.06 };
            leaf(cx, &f, shown, hgt, -LEAF * 0.5, LEAF * 0.5, door, pick, (shown > 0.2).then_some(handle));
        }
    }
}

pub fn garage(cx: &mut Ctx, wf: &Frame3, h: f32, hole: Hole, door: &GarageDoor, pick: f32) {
    let s = cx.surf(&door.finish);
    let sgn = sign(door.inside);
    let open = if door.open.is_finite() { door.open.clamp(0.0, 1.0) } else { 0.0 };
    let (u0, u1) = (hole.u0 - 0.05, hole.u1 + 0.05);
    let w = u1 - u0;
    let hgt = hole.z1 - hole.z0;
    // The door hangs just behind the opening, on the inside face.
    let yd = sgn * (h + 0.03);
    let thick = 0.045;
    // A panel between two points in the wall's cross-section (y across, z up), its thickness
    // on the side that faces into the garage (down, once it's under the ceiling).
    let panel = |cx: &mut Ctx, a: [f32; 2], b: [f32; 2], thick: f32| {
        let run = [b[0] - a[0], b[1] - a[1]];
        let len = (run[0] * run[0] + run[1] * run[1]).sqrt();
        if len < 1e-4 {
            return;
        }
        let along = add(mul(wf.y, run[0]), mul(wf.z, run[1]));
        let f = Frame3::new(wf.at([u0, a[0], a[1]]), wf.x, along);
        let (z0, z1) = if sgn > 0.0 { (-thick, 0.0) } else { (0.0, thick) };
        cuboid(cx.scene, &f, [0.0, 0.0, z0], [w, len, z1], &s, pick);
    };
    match door.style {
        GarageStyle::Sectional => {
            // The track: up the opening, then back under the ceiling.
            let rise = hgt + 0.05;
            let track = |d: f32| -> [f32; 2] {
                if d <= rise { [yd, hole.z0 + d] } else { [yd + sgn * (d - rise), hole.z0 + rise] }
            };
            let n = door.panels.clamp(1, 12);
            let each = hgt / n as f32;
            let gap = 0.011f32.min(each * 0.2);
            for i in 0..n {
                let d0 = i as f32 * each + open * rise;
                panel(cx, track(d0 + gap), track(d0 + each - gap), thick);
            }
        }
        GarageStyle::Roller => {
            let foot = hole.z0 + open * (hgt - 0.05).max(0.0);
            panel(cx, [yd, foot], [yd, hole.z1 + 0.05], 0.02);
            // The drum it winds onto, fatter the more curtain is on it.
            let d = 0.26 + 0.1 * open;
            cuboid(cx.scene, wf, [u0, sgn * (h + 0.005), hole.z1 + 0.02], [u1, sgn * (h + 0.005 + d), hole.z1 + 0.02 + d], &s, pick);
        }
        GarageStyle::Tilt => {
            let turn = open * std::f32::consts::FRAC_PI_2;
            let (sin, cos) = turn.sin_cos();
            let centre = [yd + sgn * hgt * 0.25 * open, hole.z0 + hgt * 0.5 + (hgt * 0.5 - 0.04) * open];
            let half = [sgn * sin * hgt * 0.5, cos * hgt * 0.5];
            panel(cx, [centre[0] - half[0], centre[1] - half[1]], [centre[0] + half[0], centre[1] + half[1]], thick);
        }
    }
}

pub fn window(cx: &mut Ctx, wf: &Frame3, h: f32, hole: Hole, window: &Window, pick: f32) {
    let s = cx.surf(&window.frame);
    let glass = cx.surf(&Finish::of("glass"));
    let hole = match &window.casing {
        Some(frame) => casing(cx, wf, h, hole, frame, true, pick),
        None => {
            // A sill board, standing a little proud of both faces.
            cuboid(cx.scene, wf, [hole.u0 - 0.03, -h - 0.03, hole.z0 - 0.025], [hole.u1 + 0.03, h + 0.03, hole.z0 + 0.006], &s, pick);
            hole
        }
    };
    let Hole { u0, u1, z0, z1 } = hole;
    if !(u1 - u0 > 0.02) || !(z1 - z0 > 0.02) {
        return;
    }
    let fw = window.frame_width.clamp(0.005, (u1 - u0) * 0.25).min((z1 - z0) * 0.25);
    let fd = 0.045f32.min(h);
    cuboid(cx.scene, wf, [u0, -fd, z0], [u0 + fw, fd, z1], &s, pick);
    cuboid(cx.scene, wf, [u1 - fw, -fd, z0], [u1, fd, z1], &s, pick);
    cuboid(cx.scene, wf, [u0 + fw, -fd, z1 - fw], [u1 - fw, fd, z1], &s, pick);
    cuboid(cx.scene, wf, [u0 + fw, -fd, z0], [u1 - fw, fd, z0 + fw], &s, pick);
    let (iu0, iu1, iz0, iz1) = (u0 + fw, u1 - fw, z0 + fw, z1 - fw);
    let (iw, ih) = (iu1 - iu0, iz1 - iz0);

    let pane = |cx: &mut Ctx, r: Hole, y: f32| cuboid(cx.scene, wf, [r.u0, y - 0.003, r.z0], [r.u1, y + 0.003, r.z1], &glass, pick);
    // A sash: its own frame around a pane, at `y` across the wall.
    let sash = |cx: &mut Ctx, r: Hole, y: f32| {
        let m = 0.04f32.min((r.u1 - r.u0) * 0.25).min((r.z1 - r.z0) * 0.25);
        let d = 0.02;
        cuboid(cx.scene, wf, [r.u0, y - d, r.z0], [r.u0 + m, y + d, r.z1], &s, pick);
        cuboid(cx.scene, wf, [r.u1 - m, y - d, r.z0], [r.u1, y + d, r.z1], &s, pick);
        cuboid(cx.scene, wf, [r.u0 + m, y - d, r.z1 - m], [r.u1 - m, y + d, r.z1], &s, pick);
        cuboid(cx.scene, wf, [r.u0 + m, y - d, r.z0], [r.u1 - m, y + d, r.z0 + m], &s, pick);
        pane(cx, Hole { u0: r.u0 + m, u1: r.u1 - m, z0: r.z0 + m, z1: r.z1 - m }, y);
    };
    let inner = Hole { u0: iu0, u1: iu1, z0: iz0, z1: iz1 };
    let steel = cx.surf(&Finish::of("steel"));
    match window.style {
        WindowStyle::Fixed => {
            pane(cx, inner, 0.0);
            let (cols, rows) = (window.cols.clamp(1, 12), window.rows.clamp(1, 12));
            let bar = 0.0125;
            for i in 1..cols {
                let u = iu0 + iw * i as f32 / cols as f32;
                cuboid(cx.scene, wf, [u - bar, -0.015, iz0], [u + bar, 0.015, iz1], &s, pick);
            }
            for i in 1..rows {
                let z = iz0 + ih * i as f32 / rows as f32;
                cuboid(cx.scene, wf, [iu0, -0.015, z - bar], [iu1, 0.015, z + bar], &s, pick);
            }
        }
        WindowStyle::Casement => {
            sash(cx, inner, 0.0);
            // The winder, on the side away from the hinges.
            cuboid(cx.scene, wf, [iu1 - 0.03, -0.045, iz0 + ih * 0.5 - 0.05], [iu1 - 0.01, 0.045, iz0 + ih * 0.5 + 0.05], &steel, pick);
        }
        WindowStyle::Awning => {
            sash(cx, inner, 0.0);
            cuboid(cx.scene, wf, [iu0 + iw * 0.5 - 0.05, -0.045, iz0 + 0.01], [iu0 + iw * 0.5 + 0.05, 0.045, iz0 + 0.03], &steel, pick);
        }
        WindowStyle::DoubleHung => {
            let mid = iz0 + ih * 0.5;
            sash(cx, Hole { z0: mid - 0.02, ..inner }, 0.021);
            sash(cx, Hole { z1: mid + 0.02, ..inner }, -0.021);
        }
        WindowStyle::Sliding => {
            let mid = iu0 + iw * 0.5;
            sash(cx, Hole { u1: mid + 0.02, ..inner }, 0.021);
            sash(cx, Hole { u0: mid - 0.02, ..inner }, -0.021);
        }
        WindowStyle::Louvre => {
            let n = ((ih / 0.11).ceil() as u32).clamp(1, 60);
            let pitch = ih / n as f32;
            let blade = (pitch * 1.25).min(fd * 2.6);
            for i in 0..n {
                let z = iz0 + (i as f32 + 0.5) * pitch;
                let f = Frame3::new(wf.at([iu0, 0.0, z]), wf.x, add(wf.y, wf.z));
                cuboid(cx.scene, &f, [0.0, -blade * 0.5, -0.003], [iw, blade * 0.5, 0.003], &glass, pick);
            }
        }
    }
}
