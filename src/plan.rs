//! The plan view: the active level drawn from above as vector shapes.

use std::f32::consts::{FRAC_PI_2, PI};

use rune::{Align, Color, El, LineCap, Shape, TextStyle, canvas, el, text};

use crate::app::{DragOp, PlanCam, State, Tool};
use crate::geom::{self, add, scale, sub};
use crate::materials::Library;
use crate::model::*;
use crate::units;

/// A colour from `0xrrggbb` (sRGB).
pub fn hex(h: u32) -> Color {
    let c = |s: u32| rune::color::srgb_decode(((h >> s) & 255) as f32 / 255.0);
    Color { r: c(16), g: c(8), b: c(0), a: 1.0 }
}

pub fn srgb(c: [f32; 3]) -> Color {
    Color { r: rune::color::srgb_decode(c[0]), g: rune::color::srgb_decode(c[1]), b: rune::color::srgb_decode(c[2]), a: 1.0 }
}

/// The plan's colours.
#[derive(Clone, Copy)]
pub struct Ink {
    pub paper: Color,
    pub grid: Color,
    pub grid_major: Color,
    pub wall: Color,
    pub ghost: Color,
    pub line: Color,
    pub text: Color,
    pub faint: Color,
    pub accent: Color,
    pub glass: Color,
    pub power: Color,
    pub light: Color,
    pub safety: Color,
    pub air: Color,
    pub contour: Color,
}

impl Ink {
    pub fn new(dark: bool) -> Ink {
        if dark {
            Ink {
                paper: hex(0x17181b),
                grid: hex(0x1f2125),
                grid_major: hex(0x2a2d32),
                wall: hex(0xd9dce1),
                ghost: hex(0x34373d),
                line: hex(0x9aa0a8),
                text: hex(0xc9cdd3),
                faint: hex(0x7d838c),
                accent: hex(0x4c9dff),
                glass: hex(0x6fc3e8),
                power: hex(0xf0a04b),
                light: hex(0xf2d24a),
                safety: hex(0xf06a6a),
                air: hex(0x6fd3b0),
                contour: hex(0x3f5a3c),
            }
        } else {
            Ink {
                paper: hex(0xfbfaf7),
                grid: hex(0xefede8),
                grid_major: hex(0xe2dfd8),
                wall: hex(0x2b2e33),
                ghost: hex(0xd9d6cf),
                line: hex(0x5d636b),
                text: hex(0x3c4046),
                faint: hex(0x8c9198),
                accent: hex(0x1f6feb),
                glass: hex(0x3c9bd0),
                power: hex(0xd9772b),
                light: hex(0xcf9d12),
                safety: hex(0xd64545),
                air: hex(0x2f9e7d),
                contour: hex(0xc3d6bd),
            }
        }
    }
}

struct Label {
    at: [f32; 2],
    text: String,
    style: TextStyle,
    color: Color,
}

struct Draw<'a> {
    cam: PlanCam,
    size: [f32; 2],
    ink: Ink,
    shape: Shape,
    labels: Vec<Label>,
    units: Units,
    lib: &'a Library,
}

const SMALL: TextStyle = TextStyle::new(10.5, 500.0);
const NAME: TextStyle = TextStyle::new(12.0, 600.0);

impl Draw<'_> {
    fn sc(&self, p: P2) -> [f32; 2] {
        self.cam.to_screen(p, self.size)
    }

    fn fill(&mut self, pts: &[P2], color: Color) {
        let pts: Vec<[f32; 2]> = pts.iter().map(|p| self.sc(*p)).collect();
        let shape = std::mem::take(&mut self.shape);
        self.shape = shape.fill(pts, color);
    }

    /// A line between model points, `width` in screen points.
    fn line(&mut self, a: P2, b: P2, width: f32, color: Color) {
        let (a, b) = (self.sc(a), self.sc(b));
        let shape = std::mem::take(&mut self.shape);
        self.shape = shape.line(a, b, width, LineCap::Butt, color);
    }

    fn outline(&mut self, pts: &[P2], width: f32, color: Color) {
        for i in 0..pts.len() {
            self.line(pts[i], pts[(i + 1) % pts.len()], width, color);
        }
    }

    fn dashed(&mut self, a: P2, b: P2, width: f32, color: Color) {
        let (a, b) = (self.sc(a), self.sc(b));
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let n = (len / 9.0).floor().max(1.0) as usize;
        let mut shape = std::mem::take(&mut self.shape);
        for i in 0..n {
            let (t0, t1) = (i as f32 / n as f32, (i as f32 + 0.55) / n as f32);
            let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            shape = shape.line(at(t0), at(t1), width, LineCap::Butt, color);
        }
        self.shape = shape;
    }

    fn dashed_outline(&mut self, pts: &[P2], width: f32, color: Color) {
        for i in 0..pts.len() {
            self.dashed(pts[i], pts[(i + 1) % pts.len()], width, color);
        }
    }

    /// A circle of `r` screen points at a model point.
    fn dot(&mut self, at: P2, r: f32, fill: Color, ring: Option<Color>) {
        let c = self.sc(at);
        let pts: Vec<[f32; 2]> = (0..16).map(|i| (i as f32 / 16.0 * 2.0 * PI).sin_cos()).map(|(s, co)| [c[0] + co * r, c[1] + s * r]).collect();
        let mut shape = std::mem::take(&mut self.shape);
        if fill.a > 0.0 {
            shape = shape.fill(pts.clone(), fill);
        }
        if let Some(ring) = ring {
            let mut closed = pts;
            closed.push(closed[0]);
            shape = shape.stroke(closed, 1.25, ring);
        }
        self.shape = shape;
    }

    /// An arc of radius `r` metres about `center` from angle `a0` to `a1` (model radians).
    fn arc(&mut self, center: P2, r: f32, a0: f32, a1: f32, width: f32, color: Color) {
        let n = 14;
        let pts: Vec<[f32; 2]> = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                self.sc([center[0] + a.cos() * r, center[1] + a.sin() * r])
            })
            .collect();
        let shape = std::mem::take(&mut self.shape);
        self.shape = shape.stroke(pts, width, color);
    }

    fn label(&mut self, at: P2, text: impl Into<String>, style: TextStyle, color: Color) {
        let at = self.sc(at);
        self.labels.push(Label { at, text: text.into(), style, color });
    }

    fn px(&self, points: f32) -> f32 {
        points / self.cam.zoom
    }
}

/// How far each end of a wall runs past its end point so corners close: half the thickness of
/// whatever wall it meets there.
pub fn wall_extensions(level: &Level, wall: &Wall) -> (f32, f32) {
    let ext = |end: P2| {
        level.walls.iter().filter(|o| o.id != wall.id).filter(|o| geom::segment_distance(end, o.a, o.b) < o.thickness / 2.0 + 0.02).map(|o| o.thickness / 2.0).fold(0.0, f32::max)
    };
    (ext(wall.a), ext(wall.b))
}

fn wall_outline(level: &Level, w: &Wall) -> [P2; 4] {
    let (ea, eb) = wall_extensions(level, w);
    let len = w.length();
    let h = w.thickness / 2.0;
    [w.point(-ea, -h), w.point(len + eb, -h), w.point(len + eb, h), w.point(-ea, h)]
}

fn side_sign(side: WallSide) -> f32 {
    match side {
        WallSide::Left => 1.0,
        WallSide::Right => -1.0,
    }
}

/// Where a fixture sits on the plan, and the direction it faces (for wall fixtures, out of the wall).
pub fn fixture_point(level: &Level, f: &Fixture) -> Option<(P2, f32)> {
    match f.mount {
        Mount::Wall { wall, side, at, .. } => {
            let w = level.wall(wall)?;
            let s = side_sign(side);
            let (d, n) = w.frame();
            let _ = d;
            Some((w.point(at, s * w.thickness / 2.0), (n[1] * s).atan2(n[0] * s)))
        }
        Mount::Ceiling { at, turn } | Mount::Floor { at, turn } => Some((at, turn)),
    }
}

fn draw_opening(d: &mut Draw, w: &Wall, o: &Opening, selected: bool) {
    let ink = d.ink;
    let h = w.thickness / 2.0;
    let (x0, x1) = (o.at - o.width / 2.0, o.at + o.width / 2.0);
    let line = if selected { ink.accent } else { ink.line };
    let gap = [w.point(x0, -h), w.point(x1, -h), w.point(x1, h), w.point(x0, h)];
    match &o.kind {
        OpeningKind::Recess { side, depth, .. } => {
            let s = side_sign(*side);
            let inner = h - depth.min(w.thickness);
            d.fill(&[w.point(x0, s * inner), w.point(x1, s * inner), w.point(x1, s * h), w.point(x0, s * h)], ink.paper);
            d.outline(&[w.point(x0, s * inner), w.point(x1, s * inner), w.point(x1, s * h), w.point(x0, s * h)], 1.0, line);
            return;
        }
        _ => d.fill(&gap, ink.paper),
    }
    // Jambs.
    d.line(w.point(x0, -h), w.point(x0, h), 1.0, line);
    d.line(w.point(x1, -h), w.point(x1, h), 1.0, line);
    let (dir, n) = w.frame();
    let wall_angle = dir[1].atan2(dir[0]);
    match &o.kind {
        OpeningKind::Window(win) => {
            d.fill(&gap, ink.glass.alpha(0.16));
            let glass = if selected { ink.accent } else { ink.glass };
            d.line(w.point(x0, -h), w.point(x1, -h), 1.0, glass);
            d.line(w.point(x0, h), w.point(x1, h), 1.0, glass);
            d.line(w.point(x0, 0.0), w.point(x1, 0.0), 1.5, glass);
            if matches!(win.style, WindowStyle::Sliding | WindowStyle::DoubleHung) {
                d.line(w.point(o.at, -h * 0.6), w.point(o.at, h * 0.6), 1.5, glass);
            }
        }
        OpeningKind::Door(door) => {
            let s = side_sign(door.swing);
            let swing_leaf = |d: &mut Draw, hinge_x: f32, toward: f32, len: f32| {
                // The leaf drawn open at 90°, and the arc its edge sweeps.
                let hinge = w.point(hinge_x, s * h);
                let tip = add(hinge, scale(n, s * len));
                d.line(hinge, tip, 2.0, line);
                let a_leaf = (n[1] * s).atan2(n[0] * s);
                let a_closed = if toward > 0.0 { wall_angle } else { wall_angle + PI };
                let mut delta = a_closed - a_leaf;
                while delta > PI {
                    delta -= 2.0 * PI;
                }
                while delta < -PI {
                    delta += 2.0 * PI;
                }
                d.arc(hinge, len, a_leaf, a_leaf + delta, 0.75, line.alpha(0.6));
            };
            match door.style {
                DoorStyle::Hinged => match door.hinge {
                    Hinge::Start => swing_leaf(d, x0, 1.0, o.width),
                    Hinge::End => swing_leaf(d, x1, -1.0, o.width),
                },
                DoorStyle::Double => {
                    swing_leaf(d, x0, 1.0, o.width / 2.0);
                    swing_leaf(d, x1, -1.0, o.width / 2.0);
                }
                DoorStyle::Sliding => {
                    let glass = if door.glazing > 0.0 { ink.glass } else { line };
                    d.line(w.point(x0, h * 0.3), w.point(o.at + 0.03, h * 0.3), 2.0, glass);
                    d.line(w.point(o.at - 0.03, -h * 0.3), w.point(x1, -h * 0.3), 2.0, glass);
                }
                DoorStyle::Pocket => {
                    let (from, to) = match door.hinge {
                        Hinge::Start => (x0, x0 - o.width),
                        Hinge::End => (x1, x1 + o.width),
                    };
                    d.dashed(w.point(from, 0.0), w.point(to, 0.0), 1.5, ink.paper);
                    d.line(w.point(x0 + o.width * 0.25, 0.0), w.point(x1 - o.width * 0.25, 0.0), 2.0, line);
                }
                DoorStyle::Bifold => {
                    let q = o.width / 4.0;
                    let pts = [w.point(x0, 0.0), w.point(x0 + q, s * q), w.point(o.at, 0.0), w.point(o.at + q, s * q), w.point(x1, 0.0)];
                    for i in 0..4 {
                        d.line(pts[i], pts[i + 1], 1.5, line);
                    }
                }
            }
        }
        OpeningKind::Garage(g) => {
            let s = side_sign(g.inside);
            d.line(w.point(x0, -s * h * 0.5), w.point(x1, -s * h * 0.5), 2.5, line);
            // Where the door lies under the ceiling when it's open.
            let reach = match g.style {
                GarageStyle::Roller => 0.35,
                _ => o.height,
            };
            let over = [w.point(x0, s * h), w.point(x1, s * h), w.point(x1, s * (h + reach)), w.point(x0, s * (h + reach))];
            d.dashed_outline(&over, 0.75, line.alpha(0.55));
        }
        OpeningKind::Passage { .. } => {
            d.dashed(w.point(x0, -h), w.point(x1, -h), 0.75, line.alpha(0.6));
            d.dashed(w.point(x0, h), w.point(x1, h), 0.75, line.alpha(0.6));
        }
        OpeningKind::Recess { .. } => {}
    }
    if selected {
        d.outline(&gap, 1.5, ink.accent);
    }
}

fn draw_fixture(d: &mut Draw, level: &Level, f: &Fixture, selected: bool) {
    let Some((at, facing)) = fixture_point(level, f) else { return };
    let ink = d.ink;
    let r = (0.085 * d.cam.zoom).clamp(4.0, 9.0);
    let out = [facing.cos(), facing.sin()];
    let along = [-out[1], out[0]];
    let on_wall = matches!(f.mount, Mount::Wall { .. });
    // Wall symbols sit just off the face so they don't hide in the wall.
    let c = if on_wall { add(at, scale(out, d.px(r + 1.0))) } else { at };
    let rect = |d: &mut Draw, w: f32, h: f32, turn: f32, fill: Color, ring: Color| {
        let pts = geom::oriented_rect(c, w, h, turn);
        d.fill(&pts, fill);
        d.outline(&pts, 1.0, ring);
    };
    match f.kind {
        FixtureKind::Outlet { gangs } => {
            d.dot(c, r, ink.power.alpha(0.18), Some(ink.power));
            for i in 0..gangs.clamp(1, 3) {
                let off = (i as f32 - (gangs.clamp(1, 3) - 1) as f32 / 2.0) * d.px(r * 0.7);
                let p = add(c, scale(along, off));
                d.line(add(p, scale(out, -d.px(r * 0.45))), add(p, scale(out, d.px(r * 0.45))), 1.25, ink.power);
            }
        }
        FixtureKind::Switch { .. } => {
            d.dot(c, r * 0.55, ink.power, None);
            d.line(c, add(c, add(scale(out, d.px(r * 1.5)), scale(along, d.px(r * 0.9)))), 1.5, ink.power);
        }
        FixtureKind::Data => {
            let k = d.px(r);
            d.fill(&[add(c, scale(out, k)), add(c, add(scale(out, -k * 0.7), scale(along, k * 0.9))), add(c, add(scale(out, -k * 0.7), scale(along, -k * 0.9)))], ink.accent.alpha(0.8));
        }
        FixtureKind::Thermostat => rect(d, d.px(r * 1.6), d.px(r * 1.6), facing, ink.safety.alpha(0.2), ink.safety),
        FixtureKind::SmokeDetector => {
            d.dot(c, r, ink.safety.alpha(0.12), Some(ink.safety));
            d.dot(c, r * 0.35, ink.safety, None);
        }
        FixtureKind::Downlight { diameter } => {
            let rr = (diameter / 2.0 * d.cam.zoom).max(r * 0.7);
            d.dot(c, rr, ink.light.alpha(0.3), Some(ink.light));
            let k = d.px(rr * 0.7);
            d.line(add(c, [-k, -k]), add(c, [k, k]), 1.0, ink.light);
            d.line(add(c, [-k, k]), add(c, [k, -k]), 1.0, ink.light);
        }
        FixtureKind::TubeLight { length, width } => rect(d, length, width.max(d.px(4.0)), facing, ink.light.alpha(0.45), ink.light),
        // A strip runs along its wall, or along its own turn on a ceiling.
        FixtureKind::Strip { length } => rect(d, length, d.px(4.0), if on_wall { facing + FRAC_PI_2 } else { facing }, ink.light, ink.light),
        FixtureKind::Surface { diameter } => d.dot(c, (diameter / 2.0 * d.cam.zoom).max(r * 0.8), ink.light.alpha(0.2), Some(ink.light)),
        FixtureKind::Pendant { diameter, .. } => {
            let rr = (diameter / 2.0 * d.cam.zoom).max(r * 0.8);
            d.dot(c, rr, ink.light.alpha(0.2), Some(ink.light));
            d.dot(c, rr * 0.4, ink.light, None);
        }
        FixtureKind::Vent { width, height } => {
            let (w, h) = if on_wall { (d.px(r * 1.1), width) } else { (width, height) };
            let turn = if on_wall { facing } else { facing };
            let pts = geom::oriented_rect(c, w, h, turn);
            d.fill(&pts, ink.air.alpha(0.2));
            d.outline(&pts, 1.0, ink.air);
            for i in 1..4 {
                let t = i as f32 / 4.0;
                let a = add(pts[0], scale(sub(pts[3], pts[0]), t));
                let b = add(pts[1], scale(sub(pts[2], pts[1]), t));
                d.line(a, b, 0.75, ink.air);
            }
        }
    }
    if selected {
        d.dot(c, r + 5.0, Color { a: 0.0, ..ink.accent }, Some(ink.accent));
    }
}

fn draw_stair(d: &mut Draw, s: &Stair, selected: bool) {
    let ink = d.ink;
    let dir = [s.direction.cos(), s.direction.sin()];
    let side = [-dir[1], dir[0]];
    let corner = |along: f32, across: f32| add(s.at, add(scale(dir, along), scale(side, across)));
    let hw = s.width / 2.0;
    let pts = [corner(0.0, -hw), corner(s.run, -hw), corner(s.run, hw), corner(0.0, hw)];
    let line = if selected { ink.accent } else { ink.line };
    d.fill(&pts, ink.paper.alpha(0.7));
    d.outline(&pts, if selected { 1.75 } else { 1.0 }, line);
    for i in 1..s.steps.max(1) {
        let t = s.run * i as f32 / s.steps as f32;
        d.line(corner(t, -hw), corner(t, hw), 0.75, line.alpha(0.7));
    }
    // An arrow pointing up the flight.
    let (a, b) = (corner(s.run * 0.12, 0.0), corner(s.run * 0.88, 0.0));
    d.line(a, b, 1.25, line);
    let k = d.px(7.0).max(0.08);
    d.line(b, add(b, add(scale(dir, -k), scale(side, k * 0.6))), 1.25, line);
    d.line(b, add(b, add(scale(dir, -k), scale(side, -k * 0.6))), 1.25, line);
    d.label(corner(s.run * 0.5, 0.0), "UP", SMALL, ink.faint);
}

fn draw_furniture(d: &mut Draw, f: &Furniture, selected: bool) {
    let ink = d.ink;
    let pts = geom::oriented_rect(f.at, f.size[0], f.size[1], f.turn);
    let swatch = srgb(d.lib.swatch(&f.finish));
    let line = if selected { ink.accent } else { ink.line.alpha(0.85) };
    d.fill(&pts, swatch.mix(ink.paper, 0.45).alpha(if f.lift > 0.5 { 0.35 } else { 0.8 }));
    if f.lift > 0.5 {
        d.dashed_outline(&pts, 1.0, line);
    } else {
        d.outline(&pts, if selected { 1.75 } else { 1.0 }, line);
    }
    // The front edge, heavier.
    d.line(pts[0], pts[1], if selected { 3.0 } else { 2.0 }, line);
    if d.cam.zoom > 52.0 && f.size[0] * d.cam.zoom > 58.0 && f.size[1] * d.cam.zoom > 22.0 && f.lift < 0.5 {
        d.label(f.at, f.name.clone(), SMALL, ink.faint);
    }
}

/// Contour lines of the terrain every `step` metres, by marching squares.
fn draw_contours(d: &mut Draw, t: &Terrain, step: f32) {
    if t.cols < 2 || t.rows < 2 || t.heights.len() < t.cols * t.rows {
        return;
    }
    let (lo, hi) = t.heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
    if hi - lo < step * 0.5 || (hi - lo) / step > 60.0 {
        return;
    }
    let color = d.ink.contour;
    let h = |c: usize, r: usize| t.heights[r * t.cols + c];
    let p = |c: f32, r: f32| [t.origin[0] + c * t.spacing, t.origin[1] + r * t.spacing];
    let mut level = (lo / step).ceil() * step;
    while level <= hi {
        let major = ((level / step).round() as i32) % 2 == 0;
        for r in 0..t.rows - 1 {
            for c in 0..t.cols - 1 {
                let v = [h(c, r), h(c + 1, r), h(c + 1, r + 1), h(c, r + 1)];
                let corners = [(c as f32, r as f32), (c as f32 + 1.0, r as f32), (c as f32 + 1.0, r as f32 + 1.0), (c as f32, r as f32 + 1.0)];
                let mut cuts: Vec<P2> = Vec::with_capacity(4);
                for e in 0..4 {
                    let (a, b) = (e, (e + 1) % 4);
                    if (v[a] < level) != (v[b] < level) {
                        let k = (level - v[a]) / (v[b] - v[a]);
                        cuts.push(p(corners[a].0 + (corners[b].0 - corners[a].0) * k, corners[a].1 + (corners[b].1 - corners[a].1) * k));
                    }
                }
                if cuts.len() >= 2 {
                    d.line(cuts[0], cuts[1], if major { 1.0 } else { 0.5 }, color);
                }
                if cuts.len() == 4 {
                    d.line(cuts[2], cuts[3], if major { 1.0 } else { 0.5 }, color);
                }
            }
        }
        level += step;
    }
}

fn roof_outline(r: &Roof, overhang: f32) -> [P2; 4] {
    geom::oriented_rect(r.center, r.size[0] + overhang * 2.0, r.size[1] + overhang * 2.0, r.turn)
}

fn draw_grid(d: &mut Draw) {
    let zoom = d.cam.zoom;
    let minor = if zoom > 90.0 {
        0.1
    } else if zoom > 28.0 {
        0.5
    } else if zoom > 9.0 {
        1.0
    } else {
        5.0
    };
    let major = if minor < 0.5 { 1.0 } else { minor * 5.0 };
    let lo = d.cam.to_model([0.0, d.size[1]], d.size);
    let hi = d.cam.to_model([d.size[0], 0.0], d.size);
    let ink = d.ink;
    for (step, color, w) in [(minor, ink.grid, 1.0), (major, ink.grid_major, 1.0)] {
        let mut x = (lo[0] / step).floor() * step;
        while x <= hi[0] {
            d.line([x, lo[1]], [x, hi[1]], w, color);
            x += step;
        }
        let mut y = (lo[1] / step).floor() * step;
        while y <= hi[1] {
            d.line([lo[0], y], [hi[0], y], w, color);
            y += step;
        }
    }
    // The origin.
    d.line([-0.3, 0.0], [0.3, 0.0], 1.0, ink.faint.alpha(0.6));
    d.line([0.0, -0.3], [0.0, 0.3], 1.0, ink.faint.alpha(0.6));
}

fn handle(d: &mut Draw, at: P2) {
    let (accent, paper) = (d.ink.accent, d.ink.paper);
    d.dot(at, 5.0, paper, Some(accent));
}

fn draw_selection(d: &mut Draw, s: &State, level: &Level) {
    let ink = d.ink;
    match s.selected {
        Some(Pick::Wall(id)) => {
            if let Some(w) = level.wall(id) {
                d.outline(&wall_outline(level, w), 2.0, ink.accent);
                handle(d, w.a);
                handle(d, w.b);
                let (_, n) = w.frame();
                let off = scale(n, w.thickness / 2.0 + d.px(16.0));
                let mid = add(scale(add(w.a, w.b), 0.5), off);
                d.label(mid, units::length(w.length(), d.units), SMALL, ink.accent);
            }
        }
        Some(Pick::Room(id)) => {
            if let Some(r) = level.rooms.iter().find(|r| r.id == id) {
                d.outline(&r.outline, 2.0, ink.accent);
                for i in 0..r.outline.len() {
                    handle(d, r.outline[i]);
                    let (a, b) = (r.outline[i], r.outline[(i + 1) % r.outline.len()]);
                    if geom::dist(a, b) * d.cam.zoom > 50.0 {
                        d.label(scale(add(a, b), 0.5), units::length(geom::dist(a, b), d.units), SMALL, ink.accent);
                    }
                }
            }
        }
        Some(Pick::Patch(id)) => {
            if let Some(r) = s.project.site.patches.iter().find(|r| r.id == id) {
                d.outline(&r.outline, 2.0, ink.accent);
                for p in &r.outline {
                    handle(d, *p);
                }
            }
        }
        Some(Pick::Roof(id)) => {
            if let Some(r) = s.project.roofs.iter().find(|r| r.id == id) {
                d.outline(&roof_outline(r, 0.0), 2.0, ink.accent);
            }
        }
        _ => {}
    }
}

fn draw_tool_preview(d: &mut Draw, s: &State, level: &Level) {
    let ink = d.ink;
    if let DragOp::Draw { a, b } = &s.drag {
        let (a, b) = (*a, *b);
        match s.tool {
            Tool::Wall => {
                d.line(a, b, (s.wall_thickness() * d.cam.zoom).max(2.0), ink.accent.alpha(0.5));
                d.label(add(scale(add(a, b), 0.5), [0.0, d.px(16.0)]), units::length(geom::dist(a, b), d.units), SMALL, ink.accent);
            }
            Tool::Stair => {
                d.line(a, b, 2.0, ink.accent);
                d.label(add(scale(add(a, b), 0.5), [0.0, d.px(14.0)]), units::length(geom::dist(a, b), d.units), SMALL, ink.accent);
            }
            _ => {
                let pts = [a, [b[0], a[1]], b, [a[0], b[1]]];
                d.fill(&pts, ink.accent.alpha(0.12));
                d.outline(&pts, 1.5, ink.accent);
                let text = format!("{} × {}", units::length((b[0] - a[0]).abs(), d.units), units::length((b[1] - a[1]).abs(), d.units));
                d.label(scale(add(a, b), 0.5), text, SMALL, ink.accent);
            }
        }
        return;
    }
    let Some(cursor) = s.cursor else { return };
    match &s.tool {
        Tool::Select => {}
        Tool::Door | Tool::Window | Tool::Passage | Tool::Garage | Tool::Recess => {
            if let Some((w, at)) = crate::edit::nearest_wall(level, cursor, 0.6) {
                let width = crate::edit::default_opening(&s.tool, w).map(|o| o.width).unwrap_or(0.9).min(w.length());
                let at = at.clamp(width / 2.0, (w.length() - width / 2.0).max(width / 2.0));
                let h = w.thickness / 2.0 + d.px(2.0);
                let pts = [w.point(at - width / 2.0, -h), w.point(at + width / 2.0, -h), w.point(at + width / 2.0, h), w.point(at - width / 2.0, h)];
                d.fill(&pts, ink.accent.alpha(0.35));
                d.outline(&pts, 1.5, ink.accent);
            }
        }
        Tool::Furniture(key) => {
            if let Some(p) = crate::catalog::piece(key) {
                d.outline(&geom::oriented_rect(cursor, p.size[0], p.size[1], 0.0), 1.5, ink.accent);
            }
        }
        _ => {
            let k = d.px(6.0);
            d.line(add(cursor, [-k, 0.0]), add(cursor, [k, 0.0]), 1.5, ink.accent);
            d.line(add(cursor, [0.0, -k]), add(cursor, [0.0, k]), 1.5, ink.accent);
        }
    }
}

/// The plan as a shape plus its text labels, for a canvas of `size` points.
fn draw(s: &State, size: [f32; 2]) -> (Shape, Vec<Label>) {
    let lib = Library::new(&s.project);
    let ink = Ink::new(s.dark);
    let mut d = Draw { cam: s.plan, size, ink, shape: Shape::new(), labels: vec![], units: s.project.units, lib: &lib };
    draw_grid(&mut d);

    if s.show_site {
        let t = &s.project.site.terrain;
        let sz = t.size();
        d.dashed_outline(&[t.origin, [t.origin[0] + sz[0], t.origin[1]], [t.origin[0] + sz[0], t.origin[1] + sz[1]], [t.origin[0], t.origin[1] + sz[1]]], 1.0, ink.contour);
        for p in &s.project.site.patches {
            let selected = s.selected == Some(Pick::Patch(p.id));
            d.fill(&p.outline, srgb(lib.swatch(&p.finish)).mix(ink.paper, 0.35).alpha(0.75));
            d.outline(&p.outline, 1.0, if selected { ink.accent } else { ink.faint.alpha(0.6) });
            if (geom::area(&p.outline).abs()).sqrt() * d.cam.zoom > 60.0 {
                d.label(geom::centroid(&p.outline), p.name.clone(), SMALL, ink.faint);
            }
        }
        draw_contours(&mut d, t, 0.5);
    }

    let Some(level) = s.active() else { return (d.shape, d.labels) };
    let index = s.project.levels.iter().position(|l| l.id == level.id).unwrap_or(0);

    if s.show_below && index > 0 {
        let below = &s.project.levels[index - 1];
        for w in &below.walls {
            d.fill(&wall_outline(below, w), ink.ghost);
        }
    }

    for r in &level.rooms {
        let selected = s.selected == Some(Pick::Room(r.id));
        let floor = srgb(lib.swatch(&r.floor));
        d.fill(&r.outline, floor.mix(ink.paper, 0.5).alpha(if r.has_ceiling { 0.9 } else { 0.55 }));
        if !r.has_ceiling {
            d.dashed_outline(&r.outline, 1.0, ink.line.alpha(0.7));
        }
        let c = geom::centroid(&r.outline);
        let _ = selected;
        d.label(add(c, [0.0, d.px(8.0)]), r.name.clone(), NAME, ink.text);
        d.label(add(c, [0.0, -d.px(8.0)]), units::area(geom::area(&r.outline).abs(), d.units), SMALL, ink.faint);
    }

    for st in &level.stairs {
        draw_stair(&mut d, st, s.selected == Some(Pick::Stair(st.id)));
    }
    // Stairs arriving from the level below show where the floor opens.
    if index > 0 {
        for st in &s.project.levels[index - 1].stairs {
            let dir = [st.direction.cos(), st.direction.sin()];
            let side = [-dir[1], dir[0]];
            let c = |along: f32, across: f32| add(st.at, add(scale(dir, along), scale(side, across)));
            let hw = st.width / 2.0;
            d.dashed_outline(&[c(0.0, -hw), c(st.run, -hw), c(st.run, hw), c(0.0, hw)], 1.0, ink.line.alpha(0.7));
            d.label(c(st.run * 0.5, 0.0), "DN", SMALL, ink.faint);
        }
    }

    if s.show_furniture {
        // Rugs first, so what stands on them draws over.
        let mut order: Vec<&Furniture> = level.furniture.iter().collect();
        order.sort_by(|a, b| (a.size[2] + a.lift).partial_cmp(&(b.size[2] + b.lift)).unwrap_or(std::cmp::Ordering::Equal));
        for f in order {
            draw_furniture(&mut d, f, s.selected == Some(Pick::Furniture(f.id)));
        }
    }

    for w in &level.walls {
        let hovered = s.hovered == Some(Pick::Wall(w.id));
        d.fill(&wall_outline(level, w), if hovered { ink.wall.mix(ink.accent, 0.35) } else { ink.wall });
    }
    for w in &level.walls {
        for o in &w.openings {
            draw_opening(&mut d, w, o, s.selected == Some(Pick::Opening(w.id, o.id)));
        }
    }

    if s.show_fixtures {
        for f in &level.fixtures {
            draw_fixture(&mut d, level, f, s.selected == Some(Pick::Fixture(f.id)));
        }
    }

    if s.show_roofs {
        for r in &s.project.roofs {
            let selected = s.selected == Some(Pick::Roof(r.id));
            let color = if selected { ink.accent } else { ink.line.alpha(0.8) };
            d.dashed_outline(&roof_outline(r, r.overhang), 1.25, color);
            if matches!(r.shape, RoofShape::Gable | RoofShape::Hip) {
                let inset = if r.shape == RoofShape::Hip { (r.size[1] / 2.0).min(r.size[0] / 2.0) } else { -r.overhang };
                let half = r.size[0] / 2.0 - inset;
                let ridge = [geom::rotate([-half, 0.0], r.turn), geom::rotate([half, 0.0], r.turn)];
                d.dashed(add(r.center, ridge[0]), add(r.center, ridge[1]), 1.25, color);
            }
        }
    }

    // A selected switch shows what it controls.
    if let Some(Pick::Fixture(id)) = s.selected {
        if let Some((sw, (from, _))) = level.fixtures.iter().find(|f| f.id == id).and_then(|f| fixture_point(level, f).map(|p| (f, p))) {
            for light in sw.controls.iter().filter_map(|c| level.fixtures.iter().find(|f| f.id == *c)) {
                if let Some((to, _)) = fixture_point(level, light) {
                    d.dashed(from, to, 1.25, ink.accent);
                    d.dot(to, 9.0, Color { a: 0.0, ..ink.accent }, Some(ink.accent));
                }
            }
        }
    }

    draw_selection(&mut d, s, level);
    draw_tool_preview(&mut d, s, level);

    // North arrow, top right.
    let north = s.project.site.north.to_radians();
    let c = [size[0] - 34.0, 40.0];
    let tip = [c[0] - north.sin() * 16.0, c[1] - north.cos() * 16.0];
    let tail = [c[0] + north.sin() * 12.0, c[1] + north.cos() * 12.0];
    let wing = |k: f32| [tail[0] + north.cos() * 7.0 * k, tail[1] - north.sin() * 7.0 * k];
    let mut shape = std::mem::take(&mut d.shape);
    shape = shape.fill(vec![tip, wing(1.0), [c[0], c[1] + 4.0 * north.cos().signum()], wing(-1.0)], ink.faint);
    d.shape = shape;
    d.labels.push(Label { at: [c[0], c[1] - 28.0], text: "N".into(), style: SMALL, color: ink.faint });
    let _ = FRAC_PI_2;
    (d.shape, d.labels)
}

/// The plan canvas with its labels and handlers.
pub fn view(s: &State, size: [f32; 2]) -> El {
    let ink = Ink::new(s.dark);
    let (shape, labels) = draw(s, size);
    let drawing = canvas(shape)
        .key("plan")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(ink.paper)
        .focusable(true)
        .no_focus_ring()
        .on_layout(|s: &mut State, r| crate::edit::canvas_resized(s, [r[2], r[3]]))
        .on_drag(crate::edit::plan_drag)
        .on_wheel(crate::edit::plan_wheel)
        .on_magnify(crate::edit::plan_magnify)
        .on_pointer_move_local(crate::edit::plan_hover);
    let labels = labels.into_iter().map(|l| {
        let w = 180.0;
        text(l.text).font(l.style).color(l.color).align(Align::Center).w(w).absolute(l.at[0] - w / 2.0, l.at[1] - l.style.size * 0.65).inert()
    });
    el().key("plan-wrap").grow(1.0).h_full().min_w(0.0).clip_rounded(0.0).child(drawing).children(labels)
}
