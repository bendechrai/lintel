//! Roofs over a rotated rectangle: flat, shed, gable and hip.
//!
//! Built in the roof's frame: x along the ridge, y across it, z up from the eaves height. The
//! underside of every slope passes through z = 0 at the wall line.

use super::Ctx;
use super::geom::{DOWN, Frame3, UP, V3, cuboid6, planar};
use crate::model::{Finish, Pick, Roof, RoofShape};

pub fn build(cx: &mut Ctx, roof: &Roof) {
    let (a, b) = (roof.size[0] * 0.5, roof.size[1] * 0.5);
    if !(a > 1e-3) || !(b > 1e-3) {
        return;
    }
    let pick = cx.scene.pick_id(Some(Pick::Roof(roof.id)));
    let top = cx.surf(&roof.finish);
    let trim = cx.surf(&Finish::paint(0xece9e2));
    let gable = cx.surf(&roof.gable);
    let (sin, cos) = roof.turn.sin_cos();
    let rf = Frame3::upright([roof.center[0], roof.center[1], roof.base], [cos, sin]);
    let o = roof.overhang.max(0.0);
    let p = if roof.pitch.is_finite() { roof.pitch.max(0.0) } else { 0.0 };
    let t = roof.thickness.max(0.01);
    // Thickness measured vertically.
    let tv = t * (1.0 + p * p).sqrt();
    let (ea, eb) = (a + o, b + o);
    let w = |q: V3| rf.at(q);
    let lift = |q: V3| [q[0], q[1], q[2] + tv];
    // One slope: its top, its underside, and a fascia along its eave (the first two corners).
    let slope = |cx: &mut Ctx, under: &[V3]| {
        let above: Vec<V3> = under.iter().map(|q| w(lift(*q))).collect();
        let below: Vec<V3> = under.iter().map(|q| w(*q)).collect();
        planar(cx.scene, &above, UP, [0.0, 0.0], &top, pick);
        planar(cx.scene, &below, DOWN, [0.0, 0.0], &trim, pick);
        let mid = [(under[0][0] + under[1][0]) * 0.5, (under[0][1] + under[1][1]) * 0.5, 0.0];
        planar(cx.scene, &[below[0], below[1], above[1], above[0]], rf.dir(mid), [0.0, 0.0], &trim, pick);
    };
    match roof.shape {
        RoofShape::Flat => {
            cuboid6(cx.scene, &rf, [-ea, -eb, 0.0], [ea, eb, t], [&trim, &trim, &trim, &trim, &trim, &top], pick);
        }
        RoofShape::Gable => {
            let (ze, zr) = (-o * p, b * p);
            for sg in [1.0f32, -1.0] {
                slope(cx, &[[-ea, sg * eb, ze], [ea, sg * eb, ze], [ea, 0.0, zr], [-ea, 0.0, zr]]);
                for x in [-ea, ea] {
                    let rake = [[x, sg * eb, ze], [x, 0.0, zr], [x, 0.0, zr + tv], [x, sg * eb, ze + tv]];
                    planar(cx.scene, &rake.map(w), rf.dir([x, 0.0, 0.0]), [0.0, 0.0], &trim, pick);
                }
            }
            if zr > 1e-3 {
                for x in [-a, a] {
                    planar(cx.scene, &[w([x, -b, 0.0]), w([x, b, 0.0]), w([x, 0.0, zr])], rf.dir([x, 0.0, 0.0]), [-b, 0.0], &gable, pick);
                }
            }
        }
        RoofShape::Shed => {
            let (lo, hi) = (-o * p, (2.0 * b + o) * p);
            slope(cx, &[[-ea, -eb, lo], [ea, -eb, lo], [ea, eb, hi], [-ea, eb, hi]]);
            let high = [[-ea, eb, hi], [ea, eb, hi], [ea, eb, hi + tv], [-ea, eb, hi + tv]];
            planar(cx.scene, &high.map(w), rf.y, [0.0, 0.0], &trim, pick);
            for x in [-ea, ea] {
                let rake = [[x, -eb, lo], [x, eb, hi], [x, eb, hi + tv], [x, -eb, lo + tv]];
                planar(cx.scene, &rake.map(w), rf.dir([x, 0.0, 0.0]), [0.0, 0.0], &trim, pick);
            }
            let zr = 2.0 * b * p;
            if zr > 1e-3 {
                for x in [-a, a] {
                    planar(cx.scene, &[w([x, -b, 0.0]), w([x, b, 0.0]), w([x, b, zr])], rf.dir([x, 0.0, 0.0]), [-b, 0.0], &gable, pick);
                }
                planar(cx.scene, &[w([-a, b, 0.0]), w([a, b, 0.0]), w([a, b, zr]), w([-a, b, zr])], rf.y, [-a, 0.0], &gable, pick);
            }
        }
        RoofShape::Hip => {
            // Every slope rises from its eave at the same pitch, so each corner's hip runs in
            // on the diagonal until it meets the ridge.
            let m = ea.min(eb);
            let (ze, zr) = (-o * p, -o * p + m * p);
            let corners = [[-ea, -eb], [ea, -eb], [ea, eb], [-ea, eb]];
            let ridge = |c: [f32; 2]| [c[0] - c[0].signum() * m, c[1] - c[1].signum() * m, zr];
            for i in 0..4 {
                let (c, d) = (corners[i], corners[(i + 1) % 4]);
                let (rc, rd) = (ridge(c), ridge(d));
                let eave = [[c[0], c[1], ze], [d[0], d[1], ze]];
                if (rc[0] - rd[0]).abs() + (rc[1] - rd[1]).abs() < 1e-5 {
                    slope(cx, &[eave[0], eave[1], rc]);
                } else {
                    slope(cx, &[eave[0], eave[1], rd, rc]);
                }
            }
        }
    }
}
