//! Outlets, switches, lights, detectors and vents.
//!
//! Each is built in a frame on its mounting surface: z out of the surface into the room, x and
//! y in it (y up on a wall).

use super::Ctx;
use super::geom::{DOWN, Frame3, UP, cuboid, prism};
use super::rooms::{ceiling_at, floor_at};
use super::walls::sign;
use crate::model::{Finish, Fixture, FixtureKind, Level, Mount, Pick};
use crate::scene::{Lamp, add, cross};

fn mount_frame(level: &Level, mount: &Mount) -> Option<Frame3> {
    match *mount {
        Mount::Wall { wall, side, at, height } => {
            let wall = level.wall(wall)?;
            if wall.length() < 1e-4 {
                return None;
            }
            let sgn = sign(side);
            let (_, n) = wall.frame();
            let p = wall.point(at.clamp(0.0, wall.length()), sgn * wall.thickness * 0.5);
            let out = [n[0] * sgn, n[1] * sgn, 0.0];
            Some(Frame3 { o: [p[0], p[1], level.elevation + height], x: cross(UP, out), y: UP, z: out })
        }
        Mount::Ceiling { at, turn } => {
            let (sin, cos) = turn.sin_cos();
            let x = [cos, sin, 0.0];
            // Ceilings are drawn a hair low; sit on that.
            Some(Frame3 { o: [at[0], at[1], ceiling_at(level, at) - 0.005], x, y: cross(DOWN, x), z: DOWN })
        }
        Mount::Floor { at, turn } => {
            let (sin, cos) = turn.sin_cos();
            let x = [cos, sin, 0.0];
            Some(Frame3 { o: [at[0], at[1], floor_at(level, at)], x, y: cross(UP, x), z: UP })
        }
    }
}

pub fn build(cx: &mut Ctx, level: &Level) {
    for fixture in &level.fixtures {
        if let Some(f) = mount_frame(level, &fixture.mount) {
            fixture_mesh(cx, &f, fixture);
            lamps(cx, level, &f, fixture);
        }
    }
}

fn fixture_mesh(cx: &mut Ctx, f: &Frame3, fixture: &Fixture) {
    let pick = cx.scene.pick_id(Some(Pick::Fixture(fixture.id)));
    let white = cx.surf(&Finish::colored("plastic", [0.96, 0.96, 0.95]));
    let grey = cx.surf(&Finish::colored("plastic", [0.8, 0.8, 0.79]));
    let dark = cx.surf(&Finish::colored("plastic", [0.1, 0.1, 0.11]));
    // A lamp that is off is just its diffuser.
    let light = if fixture.output() > 0.0 { cx.surf(&Finish::of("light")) } else { cx.surf(&Finish::colored("plastic", [0.86, 0.86, 0.84])) };
    // A wall plate `gangs` wide; returns the centre x of each gang.
    let plate = |cx: &mut Ctx, gangs: u32| -> Vec<f32> {
        let g = gangs.clamp(1, 6);
        let w = 0.07 + 0.046 * (g - 1) as f32;
        cuboid(cx.scene, f, [-w * 0.5, -0.0575, 0.0], [w * 0.5, 0.0575, 0.008], &white, pick);
        (0..g).map(|i| (i as f32 - (g - 1) as f32 * 0.5) * 0.046).collect()
    };
    match fixture.kind {
        FixtureKind::Outlet { gangs } => {
            for x in plate(cx, gangs) {
                for y in [-0.022f32, 0.022] {
                    cuboid(cx.scene, f, [x - 0.016, y - 0.014, 0.008], [x + 0.016, y + 0.014, 0.0095], &grey, pick);
                    for dx in [-0.006f32, 0.006] {
                        cuboid(cx.scene, f, [x + dx - 0.0015, y - 0.006, 0.0095], [x + dx + 0.0015, y + 0.006, 0.0102], &dark, pick);
                    }
                }
            }
        }
        FixtureKind::Switch { gangs } => {
            for x in plate(cx, gangs) {
                cuboid(cx.scene, f, [x - 0.008, -0.02, 0.008], [x + 0.008, 0.02, 0.013], &grey, pick);
                cuboid(cx.scene, f, [x - 0.008, 0.0, 0.013], [x + 0.008, 0.02, 0.0155], &white, pick);
            }
        }
        FixtureKind::Data => {
            plate(cx, 1);
            cuboid(cx.scene, f, [-0.009, -0.009, 0.008], [0.009, 0.009, 0.0095], &dark, pick);
        }
        FixtureKind::Thermostat => {
            cuboid(cx.scene, f, [-0.045, -0.06, 0.0], [0.045, 0.06, 0.024], &white, pick);
            cuboid(cx.scene, f, [-0.032, -0.01, 0.024], [0.032, 0.04, 0.025], &dark, pick);
        }
        FixtureKind::SmokeDetector => {
            prism(cx.scene, f, 0.065, 0.0, 0.03, 8, &white, &white, pick);
            prism(cx.scene, f, 0.04, 0.03, 0.037, 8, &grey, &grey, pick);
            cuboid(cx.scene, f, [0.045, -0.004, 0.03], [0.053, 0.004, 0.031], &dark, pick);
        }
        FixtureKind::Downlight { diameter } => {
            let r = diameter.clamp(0.02, 1.0) * 0.5;
            prism(cx.scene, f, r, 0.0, 0.004, 16, &white, &white, pick);
            prism(cx.scene, f, r * 0.78, 0.004, 0.006, 16, &light, &light, pick);
        }
        FixtureKind::TubeLight { length, width } => {
            let (l, w) = (length.clamp(0.05, 20.0) * 0.5, width.clamp(0.01, 2.0) * 0.5);
            cuboid(cx.scene, f, [-l - 0.012, -w - 0.012, 0.0], [l + 0.012, w + 0.012, 0.003], &white, pick);
            cuboid(cx.scene, f, [-l, -w, 0.003], [l, w, 0.005], &light, pick);
        }
        FixtureKind::Strip { length } => {
            // An aluminium channel 17 mm wide with a diffuser standing just proud of it.
            let l = length.clamp(0.05, 30.0) * 0.5;
            let metal = cx.surf(&Finish::of("steel"));
            cuboid(cx.scene, f, [-l, -0.0085, 0.0], [l, 0.0085, 0.007], &metal, pick);
            cuboid(cx.scene, f, [-l + 0.002, -0.0065, 0.007], [l - 0.002, 0.0065, 0.0095], &light, pick);
        }
        FixtureKind::Surface { diameter } => {
            let r = diameter.clamp(0.03, 2.0) * 0.5;
            prism(cx.scene, f, r, 0.0, 0.035, 16, &white, &white, pick);
            prism(cx.scene, f, r * 0.88, 0.035, 0.06, 16, &light, &light, pick);
            prism(cx.scene, f, r * 0.6, 0.06, 0.075, 16, &light, &light, pick);
        }
        FixtureKind::Pendant { diameter, drop } => {
            let r = diameter.clamp(0.03, 2.0) * 0.5;
            let drop = drop.clamp(0.1, 10.0);
            let shade = (r * 0.9).clamp(0.08, drop * 0.6);
            prism(cx.scene, f, 0.05, 0.0, 0.02, 8, &white, &white, pick);
            cuboid(cx.scene, f, [-0.004, -0.004, 0.02], [0.004, 0.004, drop - shade], &dark, pick);
            prism(cx.scene, f, r * 0.55, drop - shade, drop - shade * 0.5, 16, &dark, &dark, pick);
            prism(cx.scene, f, r, drop - shade * 0.5, drop, 16, &dark, &dark, pick);
            prism(cx.scene, f, r * 0.85, drop, drop + 0.004, 16, &light, &light, pick);
        }
        FixtureKind::Vent { width, height } => {
            let (w, h) = (width.clamp(0.05, 3.0) * 0.5, height.clamp(0.05, 3.0) * 0.5);
            let rim = 0.018f32.min(w * 0.3).min(h * 0.3);
            cuboid(cx.scene, f, [-w, -h, 0.0], [w, h, 0.01], &white, pick);
            cuboid(cx.scene, f, [-w + rim, -h + rim, 0.01], [w - rim, h - rim, 0.0105], &dark, pick);
            let span = 2.0 * (h - rim);
            let n = ((span / 0.022).floor() as u32).clamp(1, 40);
            let pitch = span / n as f32;
            for i in 0..n {
                let y = -h + rim + (i as f32 + 0.5) * pitch;
                // Slats tip downward, as a return-air grille's do.
                let slat = Frame3::new(f.at([0.0, y, 0.0105]), f.x, add(f.y, f.z));
                cuboid(cx.scene, &slat, [-w + rim, -pitch * 0.3, -0.001], [w - rim, pitch * 0.3, 0.002], &white, pick);
            }
        }
    }
}

/// The point sources a lit fitting stands for.
fn lamps(cx: &mut Ctx, level: &Level, f: &Frame3, fixture: &Fixture) {
    let lumens = fixture.output();
    let Some(lamp) = fixture.lamp().filter(|_| lumens > 0.0) else { return };
    // Where along the fitting the light comes from, how far off the surface, and how much
    // of it goes forward.
    let (length, off, spread) = match fixture.kind {
        FixtureKind::Downlight { .. } => (0.0, 0.03, 1.0),
        FixtureKind::TubeLight { length, .. } => (length, 0.03, 0.9),
        FixtureKind::Strip { length } => (length, 0.03, 0.85),
        FixtureKind::Surface { .. } => (0.0, 0.1, 0.6),
        FixtureKind::Pendant { drop, .. } => (0.0, drop + 0.03, 0.5),
        _ => return,
    };
    let n = ((length / 0.4).ceil() as usize).clamp(1, 10);
    // Eyes and cameras adapt most of the way to a lamp's colour; show the rest.
    let color = crate::materials::kelvin(lamp.kelvin).map(|c| c + (1.0 - c) * 0.55);
    for i in 0..n {
        let x = if n == 1 { 0.0 } else { (i as f32 + 0.5) / n as f32 * length - length / 2.0 };
        let pos = f.at([x, 0.0, off]);
        cx.scene.lamps.push(Lamp { pos, axis: f.z, spread, color, lumens: lumens / n as f32, room: room_box(level, pos) });
    }
}

/// The box of the room a point is in: its outline's extent, floor to ceiling.
fn room_box(level: &Level, p: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let at = [p[0], p[1]];
    let pad = 0.06;
    match level.rooms.iter().find(|r| crate::geom::inside(at, &r.outline)) {
        Some(r) => {
            let (lo, hi) = r.outline.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]));
            let floor = level.elevation + r.floor_offset;
            let top = floor + r.ceiling_height.unwrap_or(level.height);
            ([lo[0] - pad, lo[1] - pad, floor - pad], [hi[0] + pad, hi[1] + pad, top + pad])
        }
        None => ([p[0] - 4.0, p[1] - 4.0, level.elevation - pad], [p[0] + 4.0, p[1] + 4.0, level.elevation + level.height + pad]),
    }
}
