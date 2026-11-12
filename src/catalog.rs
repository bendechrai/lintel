//! The furniture and appliance catalogue: what can be placed, its default size and finishes,
//! and the boxes each piece is made of.

use crate::model::{Finish, Furniture, Id, P2, rgb};

/// A catalogue entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub key: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// Width (across the front), depth, height.
    pub size: [f32; 3],
    pub lift: f32,
    pub finish: Finish,
    pub accent: Finish,
    /// Hangs from the ceiling instead of standing on the floor.
    pub hung: bool,
}

/// Which finish a part of a piece takes.
#[derive(Clone, Debug, PartialEq)]
pub enum PartFinish {
    /// The piece's `finish`.
    Main,
    /// The piece's `accent`.
    Accent,
    /// Always this, whatever the piece's finishes (glass, a screen, a light).
    Fixed(Finish),
}

/// One box of a piece, in the piece's own frame: x across the front and centred on it, y from
/// the front (-depth / 2) to the back (+depth / 2), z up from the piece's underside.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub finish: PartFinish,
}

fn p(key: &'static str, name: &'static str, group: &'static str, size: [f32; 3], lift: f32, finish: Finish, accent: Finish) -> Piece {
    Piece { key, name, group, size, lift, finish, accent, hung: false }
}

fn dark() -> Finish {
    Finish::colored("plastic", [0.07, 0.07, 0.08])
}

fn white() -> Finish {
    Finish::colored("plastic", [0.96, 0.96, 0.95])
}

fn fabric(hex: u32) -> Finish {
    Finish::colored("fabric", rgb(hex))
}

pub fn catalog() -> Vec<Piece> {
    let of = Finish::of;
    let paint = Finish::paint;
    let cabinet = || Finish::colored("laminate", rgb(0xf1efe9));
    let appliance = || Finish::colored("paint_satin", rgb(0xf4f4f2));
    let porcelain = || Finish::colored("paint_gloss", rgb(0xfafaf8));
    vec![
        // Living.
        p("sofa", "Sofa, 3-seat", "Living", [2.2, 0.95, 0.85], 0.0, fabric(0x8f9499), of("oak")),
        p("sofa_2", "Sofa, 2-seat", "Living", [1.6, 0.95, 0.85], 0.0, fabric(0x8f9499), of("oak")),
        p("sectional_sofa", "Sectional sofa", "Living", [2.8, 1.9, 0.85], 0.0, fabric(0x7d8388), of("oak")),
        p("armchair", "Armchair", "Living", [0.9, 0.9, 0.85], 0.0, fabric(0x6f7f8c), of("oak")),
        p("coffee_table", "Coffee table, timber", "Living", [1.2, 0.6, 0.42], 0.0, of("oak"), of("oak")),
        p("coffee_table_glass", "Coffee table, glass", "Living", [1.1, 0.6, 0.4], 0.0, of("glass"), of("steel")),
        p("coffee_table_marble", "Coffee table, marble", "Living", [1.0, 1.0, 0.35], 0.0, of("marble"), of("marble")),
        p("rug", "Rug", "Living", [2.4, 1.7, 0.012], 0.0, Finish::colored("rug", rgb(0x8c5a4a)), Finish::colored("rug", rgb(0xd9cdb8))),
        p("tv", "Television", "Living", [1.45, 0.06, 0.83], 0.75, dark(), Finish::colored("paint_gloss", [0.02, 0.02, 0.03])),
        p("tv_unit", "TV unit", "Living", [1.8, 0.42, 0.5], 0.0, of("walnut"), of("walnut")),
        p("bookshelf", "Bookshelf", "Living", [0.9, 0.3, 1.9], 0.0, of("birch"), of("birch")),
        // Dining.
        p("dining_table", "Dining table", "Dining", [1.8, 0.95, 0.75], 0.0, of("oak"), of("oak")),
        p("dining_chair", "Dining chair", "Dining", [0.45, 0.5, 0.9], 0.0, of("oak"), of("oak")),
        p("bar_stool", "Bar stool", "Dining", [0.42, 0.42, 1.0], 0.0, of("birch"), of("birch")),
        // Kitchen.
        p("kitchen_island", "Kitchen island", "Kitchen", [2.2, 1.0, 0.92], 0.0, cabinet(), of("quartz")),
        p("base_cabinet", "Base cabinet", "Kitchen", [0.6, 0.62, 0.9], 0.0, cabinet(), of("quartz")),
        p("bench_run", "Bench with cabinets", "Kitchen", [2.4, 0.62, 0.9], 0.0, cabinet(), of("quartz")),
        p("sink_cabinet", "Sink cabinet", "Kitchen", [0.9, 0.62, 0.9], 0.0, cabinet(), of("quartz")),
        p("wall_cabinet", "Wall cabinet", "Kitchen", [0.8, 0.35, 0.75], 1.45, cabinet(), cabinet()),
        p("tall_cabinet", "Tall cabinet", "Kitchen", [0.6, 0.6, 2.2], 0.0, cabinet(), cabinet()),
        p("pantry", "Pantry", "Kitchen", [0.9, 0.6, 2.2], 0.0, cabinet(), cabinet()),
        p("fridge", "Fridge, French door", "Kitchen", [0.91, 0.75, 1.78], 0.0, of("steel"), of("black_steel")),
        p("range", "Range (stove and oven)", "Kitchen", [0.76, 0.68, 0.92], 0.0, of("steel"), of("black_steel")),
        p("microwave_otr", "Microwave, over the range", "Kitchen", [0.76, 0.4, 0.42], 1.37, of("steel"), of("black_steel")),
        p("dishwasher", "Dishwasher", "Kitchen", [0.6, 0.6, 0.86], 0.0, of("steel"), of("black_steel")),
        p("coffee_machine", "Coffee machine", "Kitchen", [0.3, 0.42, 0.4], 0.9, of("steel"), of("black_steel")),
        // Laundry and plant.
        p("wash_tower", "Wash tower (washer and dryer)", "Laundry", [0.69, 0.77, 1.89], 0.0, appliance(), of("steel")),
        p("washer", "Washing machine", "Laundry", [0.69, 0.75, 0.98], 0.0, appliance(), of("steel")),
        p("dryer", "Dryer", "Laundry", [0.69, 0.75, 0.98], 0.0, appliance(), of("steel")),
        p("water_heater", "Water heater", "Laundry", [0.55, 0.55, 1.5], 0.0, appliance(), of("steel")),
        p("hvac_unit", "Furnace / air handler", "Laundry", [0.6, 0.75, 1.4], 0.0, Finish::colored("paint_satin", rgb(0xc9cdd0)), of("steel")),
        // Bedroom and study.
        p("bed_queen", "Bed, queen", "Bedroom", [1.6, 2.1, 1.0], 0.0, fabric(0xb9c2c9), of("oak")),
        p("bed_king", "Bed, king", "Bedroom", [1.95, 2.1, 1.0], 0.0, fabric(0xb9c2c9), of("oak")),
        p("bed_single", "Bed, single", "Bedroom", [1.0, 2.0, 0.9], 0.0, fabric(0x9fb3c4), of("pine")),
        p("bedside_table", "Bedside table", "Bedroom", [0.5, 0.4, 0.55], 0.0, of("oak"), of("oak")),
        p("wardrobe", "Wardrobe", "Bedroom", [1.8, 0.6, 2.1], 0.0, paint(0xf1efe9), paint(0xf1efe9)),
        p("desk", "Desk", "Bedroom", [1.5, 0.7, 0.74], 0.0, of("birch"), paint(0xf1efe9)),
        p("office_chair", "Office chair", "Bedroom", [0.62, 0.62, 1.05], 0.0, fabric(0x34383c), dark()),
        // Bathroom.
        p("toilet", "Toilet", "Bathroom", [0.4, 0.7, 0.8], 0.0, porcelain(), porcelain()),
        p("vanity", "Vanity", "Bathroom", [0.9, 0.5, 0.86], 0.0, of("oak"), of("quartz")),
        p("bathtub", "Bathtub", "Bathroom", [1.7, 0.76, 0.56], 0.0, porcelain(), of("steel")),
        p("shower", "Shower", "Bathroom", [0.9, 0.9, 2.0], 0.0, porcelain(), of("steel")),
        // Garage.
        p("car", "Car", "Garage", [1.85, 4.6, 1.45], 0.0, Finish::colored("paint_gloss", rgb(0x5a6b7a)), dark()),
        Piece { hung: true, ..p("bulkhead", "Bulkhead", "Ceiling", [2.4, 0.6, 0.3], 0.0, paint(0xfafaf7), paint(0xfafaf7)) },
        Piece { hung: true, ..p("beam", "Exposed beam", "Ceiling", [4.0, 0.14, 0.24], 0.0, of("oak"), of("oak")) },
    ]
}

pub fn piece(key: &str) -> Option<Piece> {
    catalog().into_iter().find(|p| p.key == key)
}

/// A new piece of furniture of this kind at `at`, at its catalogue size and finishes.
pub fn place(key: &str, id: Id, at: P2) -> Option<Furniture> {
    let p = piece(key)?;
    Some(Furniture { id, kind: p.key.into(), name: p.name.into(), at, turn: 0.0, size: p.size, lift: p.lift, finish: p.finish, accent: p.accent, hung: p.hung })
}

/// Where a cabinet door's handle goes.
#[derive(Clone, Copy)]
enum Pull {
    /// A bar across the top of the door (floor cabinets, drawers).
    Top,
    /// Upright at mid-height (tall doors).
    Tall,
    /// Upright at the bottom edge (wall cabinets).
    Bottom,
}

/// Collects a piece's boxes. Coordinates here run from the piece's front-left-bottom corner:
/// x 0..w, y 0 (front) ..d (back), z 0..h.
struct Build {
    w: f32,
    d: f32,
    parts: Vec<Part>,
}

impl Build {
    fn add(&mut self, finish: PartFinish, x: [f32; 2], y: [f32; 2], z: [f32; 2]) {
        let (hw, hd) = (self.w * 0.5, self.d * 0.5);
        let min = [x[0].min(x[1]) - hw, y[0].min(y[1]) - hd, z[0].min(z[1])];
        let max = [x[0].max(x[1]) - hw, y[0].max(y[1]) - hd, z[0].max(z[1])];
        if (0..3).all(|i| max[i] - min[i] > 1e-5) {
            self.parts.push(Part { min, max, finish });
        }
    }

    fn m(&mut self, x: [f32; 2], y: [f32; 2], z: [f32; 2]) {
        self.add(PartFinish::Main, x, y, z);
    }

    fn a(&mut self, x: [f32; 2], y: [f32; 2], z: [f32; 2]) {
        self.add(PartFinish::Accent, x, y, z);
    }

    fn f(&mut self, finish: Finish, x: [f32; 2], y: [f32; 2], z: [f32; 2]) {
        self.add(PartFinish::Fixed(finish), x, y, z);
    }

    /// Four legs under a top, inset from its corners.
    fn legs(&mut self, finish: PartFinish, inset: f32, t: f32, z: [f32; 2]) {
        let (w, d) = (self.w, self.d);
        let t = t.min(w * 0.3).min(d * 0.3);
        for x in [inset, w - inset - t] {
            for y in [inset, d - inset - t] {
                self.add(finish.clone(), [x, x + t], [y, y + t], z);
            }
        }
    }

    /// Cabinet fronts across `x`: the shadow gaps between `n` doors, and a bar handle on each.
    /// `front` is the y of the door faces, facing `out` (-1 toward the front, 1 toward the back).
    fn fronts(&mut self, x: [f32; 2], z: [f32; 2], front: f32, out: f32, n: u32, pull: Pull) {
        let n = n.max(1);
        let each = (x[1] - x[0]) / n as f32;
        let y = [front + out * 0.018, front + out * 0.03];
        for i in 0..n {
            let x0 = x[0] + i as f32 * each;
            if i > 0 {
                self.f(dark(), [x0 - 0.0015, x0 + 0.0015], [front - out * 0.004, front + out * 0.001], z);
            }
            // Upright handles pair up toward the gap between two doors.
            let hx = if n == 1 || i % 2 == 0 { x0 + each - 0.05 } else { x0 + 0.05 };
            let steel = Finish::of("steel");
            match pull {
                Pull::Top => self.f(steel, [x0 + each * 0.5 - 0.07, x0 + each * 0.5 + 0.07], y, [z[1] - 0.07, z[1] - 0.058]),
                Pull::Tall => {
                    let mid = (z[0] + z[1]) * 0.5;
                    self.f(steel, [hx - 0.006, hx + 0.006], y, [mid - 0.15, mid + 0.15]);
                }
                Pull::Bottom => self.f(steel, [hx - 0.006, hx + 0.006], y, [z[0] + 0.04, (z[0] + 0.16).min(z[1])]),
            }
        }
    }

    /// A floor cabinet: kick, carcass, door fronts, and a benchtop in the accent finish.
    fn base_cabinet(&mut self, h: f32, doors: u32, overhang: f32) {
        let (w, d) = (self.w, self.d);
        let top = 0.035f32.min(h * 0.2);
        let kick = 0.1f32.min(h * 0.2);
        self.f(dark(), [0.02, w - 0.02], [overhang + 0.06, d - 0.02], [0.0, kick]);
        self.m([0.0, w], [overhang + 0.02, d], [kick, h - top]);
        self.a([0.0, w], [0.0, d], [h - top, h]);
        if doors > 0 {
            self.fronts([0.0, w], [kick, h - top], overhang + 0.02, -1.0, doors, Pull::Top);
        }
    }

    fn sofa(&mut self, h: f32, seats: u32) {
        let (w, d) = (self.w, self.d);
        let arm = 0.18f32.min(w * 0.2);
        let back = 0.24f32.min(d * 0.35);
        let seat = 0.42f32.min(h * 0.6);
        self.legs(PartFinish::Accent, 0.04, 0.05, [0.0, 0.1]);
        self.m([0.0, w], [0.0, d], [0.1, seat - 0.1]);
        self.m([0.0, arm], [0.0, d], [seat - 0.1, (seat + 0.2).min(h)]);
        self.m([w - arm, w], [0.0, d], [seat - 0.1, (seat + 0.2).min(h)]);
        self.m([arm, w - arm], [d - back, d], [seat - 0.1, h]);
        let each = (w - 2.0 * arm) / seats.max(1) as f32;
        for i in 0..seats.max(1) {
            let x = arm + i as f32 * each;
            self.m([x + 0.008, x + each - 0.008], [0.0, d - back], [seat - 0.1, seat + 0.03]);
            self.m([x + 0.02, x + each - 0.02], [d - back - 0.14, d - back], [seat + 0.03, h - 0.06]);
        }
    }

    fn bed(&mut self, h: f32, pillows: u32) {
        let (w, d) = (self.w, self.d);
        let head = 0.07;
        let top = 0.52f32.min(h * 0.7);
        self.a([0.0, w], [d - head, d], [0.0, h]);
        self.legs(PartFinish::Accent, 0.03, 0.06, [0.0, 0.1]);
        self.a([0.0, w], [0.0, d - head], [0.1, 0.28]);
        self.f(white(), [0.02, w - 0.02], [0.02, d - head - 0.01], [0.28, top]);
        self.m([0.0, w], [0.0, d * 0.68], [top - 0.12, top + 0.04]);
        let each = (w - 0.1) / pillows.max(1) as f32;
        for i in 0..pillows.max(1) {
            let x = 0.05 + i as f32 * each;
            self.f(white(), [x + 0.04, x + each - 0.04], [d - head - 0.46, d - head - 0.06], [top, top + 0.13]);
        }
    }

    /// A front-loading washer or dryer body from `z0` to `z1`, controls along its top.
    fn laundry(&mut self, z0: f32, z1: f32) {
        let (w, d) = (self.w, self.d);
        let hgt = z1 - z0;
        self.m([0.0, w], [0.03, d], [z0, z1]);
        self.m([0.0, w], [0.0, 0.03], [z0 + 0.01, z1 - hgt * 0.16]);
        self.f(dark(), [0.0, w], [0.005, 0.03], [z1 - hgt * 0.16, z1 - 0.005]);
        self.f(Finish::colored("light", [0.75, 0.9, 1.0]), [w * 0.62, w * 0.86], [0.003, 0.005], [z1 - hgt * 0.12, z1 - hgt * 0.06]);
        // The door: a ring in the accent finish around a dark window.
        let r = (w * 0.3).min(hgt * 0.3);
        let (cx, cz) = (w * 0.5, z0 + hgt * 0.42);
        self.a([cx - r, cx + r], [-0.02, 0.0], [cz - r, cz + r]);
        self.f(Finish::of("glass_tinted"), [cx - r * 0.72, cx + r * 0.72], [-0.03, -0.02], [cz - r * 0.72, cz + r * 0.72]);
        self.f(dark(), [cx - r * 0.72, cx + r * 0.72], [-0.021, -0.0195], [cz - r * 0.72, cz + r * 0.72]);
    }
}

/// The boxes a piece is made of, scaled to its `size`.
pub fn parts(f: &Furniture) -> Vec<Part> {
    let [w, d, h] = f.size.map(|v| if v.is_finite() { v.max(0.005) } else { 0.005 });
    let mut b = Build { w, d, parts: Vec::new() };
    let steel = || Finish::of("steel");
    let glass = || Finish::of("glass");
    match f.kind.as_str() {
        "sofa" => b.sofa(h, 3),
        "sofa_2" => b.sofa(h, 2),
        "armchair" => b.sofa(h, 1),
        "sectional_sofa" => {
            // A run along the back and a chaise coming forward on the left.
            let run = 0.95f32.min(d * 0.55);
            let chaise = 0.95f32.min(w * 0.45);
            let (arm, back, seat) = (0.18f32.min(w * 0.1), 0.24f32.min(run * 0.35), 0.42f32.min(h * 0.6));
            b.m([0.0, w], [d - run, d], [0.08, seat - 0.1]);
            b.m([0.0, chaise], [0.0, d - run], [0.08, seat - 0.1]);
            b.m([0.0, w], [d - back, d], [seat - 0.1, h]);
            b.m([w - arm, w], [d - run, d - back], [seat - 0.1, (seat + 0.2).min(h)]);
            b.m([0.0, arm], [0.0, d - back], [seat - 0.1, (seat + 0.2).min(h)]);
            b.m([arm + 0.008, chaise - 0.008], [0.0, d - back], [seat - 0.1, seat + 0.03]);
            let each = (w - arm - chaise) / 2.0;
            for i in 0..2 {
                let x = chaise + i as f32 * each;
                b.m([x + 0.008, x + each - 0.008], [d - run, d - back], [seat - 0.1, seat + 0.03]);
                b.m([x + 0.02, x + each - 0.02], [d - back - 0.14, d - back], [seat + 0.03, h - 0.06]);
            }
            b.m([arm + 0.02, chaise - 0.02], [d - back - 0.14, d - back], [seat + 0.03, h - 0.06]);
            b.legs(PartFinish::Accent, 0.04, 0.05, [0.0, 0.08]);
        }
        "coffee_table" | "dining_table" => {
            let top = 0.04f32.min(h * 0.3);
            b.m([0.0, w], [0.0, d], [h - top, h]);
            b.legs(PartFinish::Accent, 0.05, if f.kind == "dining_table" { 0.07 } else { 0.05 }, [0.0, h - top]);
        }
        "coffee_table_glass" => {
            b.m([0.0, w], [0.0, d], [h - 0.012, h]);
            b.legs(PartFinish::Accent, 0.04, 0.025, [0.0, h - 0.012]);
            for y in [0.04, d - 0.065] {
                b.a([0.065, w - 0.065], [y, y + 0.025], [h - 0.037, h - 0.012]);
            }
            for x in [0.04, w - 0.065] {
                b.a([x, x + 0.025], [0.065, d - 0.065], [h - 0.037, h - 0.012]);
            }
        }
        "coffee_table_marble" => b.m([0.0, w], [0.0, d], [0.0, h]),
        "rug" => {
            b.a([0.0, w], [0.0, d], [0.0, h * 0.8]);
            let edge = 0.12f32.min(w * 0.1).min(d * 0.1);
            b.m([edge, w - edge], [edge, d - edge], [h * 0.8, h]);
        }
        "dining_chair" => {
            let seat = 0.45f32.min(h * 0.55);
            b.legs(PartFinish::Accent, 0.0, 0.035, [0.0, seat - 0.03]);
            b.m([0.0, w], [0.0, d], [seat - 0.03, seat]);
            b.a([0.0, 0.035], [d - 0.035, d], [seat, h]);
            b.a([w - 0.035, w], [d - 0.035, d], [seat, h]);
            b.m([0.035, w - 0.035], [d - 0.03, d - 0.01], [h - 0.24, h]);
        }
        "bar_stool" => {
            let seat = h * 0.76;
            b.legs(PartFinish::Accent, 0.02, 0.03, [0.0, seat - 0.035]);
            b.m([0.0, w], [0.0, d], [seat - 0.035, seat]);
            for y in [0.02, d - 0.045] {
                b.a([0.05, w - 0.05], [y, y + 0.025], [0.28, 0.305]);
            }
            for x in [0.02, w - 0.045] {
                b.a([x, x + 0.025], [0.05, d - 0.05], [0.28, 0.305]);
            }
            b.a([0.02, 0.05], [d - 0.05, d - 0.02], [seat, h]);
            b.a([w - 0.05, w - 0.02], [d - 0.05, d - 0.02], [seat, h]);
            b.m([0.05, w - 0.05], [d - 0.045, d - 0.025], [h - 0.1, h]);
        }
        "kitchen_island" => {
            // The benchtop overhangs the front, for stools.
            let over = 0.28f32.min(d * 0.3);
            let n = (w / 0.6).round().max(1.0) as u32;
            b.base_cabinet(h, 0, over);
            // Doors open from the back (the working side).
            b.fronts([0.0, w], [0.1, h - 0.035], d, 1.0, n, Pull::Top);
        }
        "base_cabinet" => b.base_cabinet(h, 1, 0.0),
        "bench_run" => b.base_cabinet(h, (w / 0.6).round().max(1.0) as u32, 0.0),
        "sink_cabinet" => {
            b.base_cabinet(h, 2, 0.0);
            let rim = 0.12f32.min(w * 0.2);
            b.f(steel(), [rim, w - rim], [0.09, d - 0.13], [h, h + 0.003]);
            b.f(Finish::of("black_steel"), [rim + 0.03, w - rim - 0.03], [0.12, d - 0.16], [h + 0.003, h + 0.004]);
            b.f(steel(), [w * 0.5 - 0.012, w * 0.5 + 0.012], [d - 0.1, d - 0.076], [h, h + 0.26]);
            b.f(steel(), [w * 0.5 - 0.01, w * 0.5 + 0.01], [d - 0.27, d - 0.076], [h + 0.24, h + 0.26]);
        }
        "wall_cabinet" => {
            b.m([0.0, w], [0.02, d], [0.0, h]);
            b.m([0.0, w], [0.0, 0.02], [0.003, h - 0.003]);
            b.fronts([0.0, w], [0.003, h - 0.003], 0.0, -1.0, (w / 0.45).round().max(1.0) as u32, Pull::Bottom);
        }
        "tall_cabinet" | "pantry" | "wardrobe" => {
            let kick = 0.1f32.min(h * 0.1);
            b.f(dark(), [0.02, w - 0.02], [0.06, d - 0.02], [0.0, kick]);
            b.m([0.0, w], [0.02, d], [kick, h]);
            b.a([0.0, w], [0.0, 0.02], [kick + 0.003, h - 0.003]);
            b.fronts([0.0, w], [kick + 0.003, h - 0.003], 0.0, -1.0, (w / 0.5).round().max(1.0) as u32, Pull::Tall);
        }
        "fridge" => {
            let split = h * 0.4;
            b.m([0.0, w], [0.05, d], [0.0, h]);
            b.f(dark(), [0.004, w - 0.004], [0.04, 0.05], [0.01, h - 0.004]);
            // Two doors over a freezer drawer.
            b.m([0.004, w * 0.5 - 0.003], [0.0, 0.04], [split + 0.004, h - 0.006]);
            b.m([w * 0.5 + 0.003, w - 0.004], [0.0, 0.04], [split + 0.004, h - 0.006]);
            b.m([0.004, w - 0.004], [0.0, 0.04], [0.03, split - 0.004]);
            for x in [w * 0.5 - 0.055, w * 0.5 + 0.035] {
                b.a([x, x + 0.02], [-0.045, -0.02], [split + 0.12, h - 0.25]);
                for z in [split + 0.12, h - 0.27] {
                    b.a([x, x + 0.02], [-0.02, 0.0], [z, z + 0.02]);
                }
            }
            b.a([0.12, w - 0.12], [-0.045, -0.02], [split - 0.1, split - 0.08]);
            for x in [0.12, w - 0.14] {
                b.a([x, x + 0.02], [-0.02, 0.0], [split - 0.1, split - 0.08]);
            }
        }
        "range" => {
            let top = h - 0.02;
            b.m([0.0, w], [0.03, d], [0.0, top]);
            b.f(dark(), [0.0, w], [0.0, d], [top, h]);
            b.m([0.015, w - 0.015], [0.0, 0.03], [0.14, top - 0.2]);
            b.f(Finish::of("glass_tinted"), [0.1, w - 0.1], [-0.004, 0.0], [0.28, top - 0.32]);
            b.f(dark(), [0.1, w - 0.1], [-0.0005, 0.0005], [0.28, top - 0.32]);
            b.a([0.06, w - 0.06], [-0.05, -0.03], [top - 0.25, top - 0.23]);
            for x in [0.06, w - 0.08] {
                b.a([x, x + 0.02], [-0.03, 0.0], [top - 0.25, top - 0.23]);
            }
            b.m([0.015, w - 0.015], [0.0, 0.03], [0.02, 0.12]);
            // Controls and knobs across the front.
            b.a([0.0, w], [0.005, 0.03], [top - 0.17, top - 0.01]);
            for i in 0..5 {
                let x = w * (0.14 + 0.18 * i as f32);
                b.m([x - 0.02, x + 0.02], [-0.015, 0.005], [top - 0.11, top - 0.07]);
            }
            // Burners.
            for (x, y, r) in [(0.27, 0.27, 0.1), (0.73, 0.27, 0.085), (0.27, 0.72, 0.085), (0.73, 0.72, 0.1)] {
                b.a([w * x - r, w * x + r], [d * y - r, d * y + r], [h, h + 0.002]);
            }
        }
        "microwave_otr" => {
            b.m([0.0, w], [0.02, d], [0.0, h]);
            b.m([0.0, w - 0.16], [0.0, 0.02], [0.03, h]);
            b.f(Finish::of("glass_tinted"), [0.04, w - 0.2], [-0.004, 0.0], [0.08, h - 0.05]);
            b.f(dark(), [0.04, w - 0.2], [-0.0005, 0.0005], [0.08, h - 0.05]);
            b.a([w - 0.16, w], [0.0, 0.02], [0.03, h]);
            b.a([w - 0.2, w - 0.18], [-0.04, -0.02], [0.07, h - 0.04]);
            b.f(dark(), [0.0, w], [0.0, 0.02], [0.0, 0.03]);
            b.f(Finish::colored("light", [0.7, 1.0, 0.8]), [w - 0.13, w - 0.03], [-0.002, 0.0], [h - 0.09, h - 0.05]);
        }
        "dishwasher" => {
            b.f(dark(), [0.02, w - 0.02], [0.06, d], [0.0, 0.1]);
            b.m([0.0, w], [0.03, d], [0.1, h]);
            b.m([0.004, w - 0.004], [0.0, 0.03], [0.11, h - 0.12]);
            b.a([0.004, w - 0.004], [0.0, 0.03], [h - 0.115, h - 0.004]);
            b.a([0.08, w - 0.08], [-0.045, -0.025], [h - 0.19, h - 0.17]);
            for x in [0.08, w - 0.1] {
                b.a([x, x + 0.02], [-0.025, 0.0], [h - 0.19, h - 0.17]);
            }
        }
        "coffee_machine" => {
            let neck = h * 0.62;
            b.m([0.0, w], [d * 0.4, d], [0.0, h]);
            b.m([0.0, w], [0.0, d * 0.4], [neck, h]);
            b.a([0.02, w - 0.02], [0.0, d * 0.4], [0.0, 0.035]);
            b.a([w * 0.5 - 0.035, w * 0.5 + 0.035], [d * 0.1, d * 0.3], [neck - 0.06, neck]);
            b.a([w * 0.5 - 0.012, w * 0.5 + 0.012], [-0.08, d * 0.12], [neck - 0.05, neck - 0.03]);
            b.f(white(), [w * 0.5 - 0.035, w * 0.5 + 0.035], [d * 0.12, d * 0.12 + 0.07], [0.035, 0.1]);
            b.f(dark(), [0.03, w - 0.03], [-0.002, 0.0], [neck + 0.03, h - 0.03]);
        }
        "wash_tower" => {
            b.laundry(0.0, h * 0.5);
            b.laundry(h * 0.5, h);
        }
        "washer" | "dryer" => b.laundry(0.0, h),
        "water_heater" => {
            let inset = w.min(d) * 0.08;
            b.m([inset, w - inset], [inset, d - inset], [0.05, h - 0.12]);
            b.m([0.0, w], [0.0, d], [0.12, h - 0.2]);
            b.f(dark(), [0.03, w - 0.03], [0.03, d - 0.03], [0.0, 0.05]);
            for x in [w * 0.3, w * 0.7] {
                b.a([x - 0.015, x + 0.015], [d * 0.5 - 0.015, d * 0.5 + 0.015], [h - 0.12, h]);
            }
            b.a([w * 0.35, w * 0.65], [-0.02, 0.0], [0.2, 0.36]);
        }
        "hvac_unit" => {
            let body = h * 0.8;
            b.m([0.0, w], [0.0, d], [0.0, body]);
            b.f(dark(), [0.0, w], [-0.002, 0.0], [body * 0.48, body * 0.49]);
            b.f(dark(), [0.05, w - 0.05], [-0.003, 0.0], [body * 0.08, body * 0.3]);
            b.a([w * 0.12, w * 0.88], [d * 0.15, d * 0.85], [body, h]);
            b.a([w * 0.75, w * 0.87], [-0.01, 0.0], [body * 0.6, body * 0.7]);
        }
        "tv" => {
            b.m([0.0, w], [d * 0.25, d], [0.0, h]);
            b.a([0.008, w - 0.008], [0.0, d * 0.25], [0.008, h - 0.008]);
        }
        "tv_unit" => {
            b.legs(PartFinish::Accent, 0.04, 0.04, [0.0, 0.12]);
            b.m([0.0, w], [0.02, d], [0.12, h - 0.025]);
            b.a([0.0, w], [0.0, d], [h - 0.025, h]);
            b.fronts([0.0, w], [0.12, h - 0.025], 0.02, -1.0, (w / 0.6).round().max(1.0) as u32, Pull::Top);
        }
        "bookshelf" => {
            let t = 0.02f32.min(w * 0.1);
            b.m([0.0, t], [0.0, d], [0.0, h]);
            b.m([w - t, w], [0.0, d], [0.0, h]);
            b.a([t, w - t], [d - 0.008, d], [0.0, h]);
            let n = (h / 0.36).round().max(1.0) as u32;
            for i in 0..=n {
                let z = (h - t) * i as f32 / n as f32;
                b.m([t, w - t], [0.0, d - 0.008], [z, z + t]);
            }
        }
        "bed_queen" | "bed_king" => b.bed(h, 2),
        "bed_single" => b.bed(h, 1),
        "bedside_table" => {
            b.legs(PartFinish::Accent, 0.02, 0.03, [0.0, 0.12]);
            b.m([0.0, w], [0.015, d], [0.12, h]);
            for z in [[0.13, (0.12 + h) * 0.5 - 0.003], [(0.12 + h) * 0.5 + 0.003, h - 0.01]] {
                b.m([0.008, w - 0.008], [0.0, 0.015], z);
                b.f(steel(), [w * 0.5 - 0.05, w * 0.5 + 0.05], [-0.02, -0.01], [(z[0] + z[1]) * 0.5 - 0.005, (z[0] + z[1]) * 0.5 + 0.005]);
            }
        }
        "desk" => {
            let top = 0.03f32.min(h * 0.3);
            b.m([0.0, w], [0.0, d], [h - top, h]);
            b.a([0.02, 0.05], [0.04, d - 0.04], [0.0, h - top]);
            // A drawer pedestal on the right.
            let ped = 0.42f32.min(w * 0.35);
            b.a([w - ped - 0.02, w - 0.02], [0.05, d - 0.04], [0.0, h - top]);
            for i in 0..3 {
                let z = (h - top) * (i as f32 + 0.5) / 3.0;
                b.f(steel(), [w - ped * 0.5 - 0.07, w - ped * 0.5 + 0.03], [0.03, 0.05], [z - 0.005, z + 0.005]);
            }
        }
        "office_chair" => {
            let seat = 0.47f32.min(h * 0.5);
            let (cx, cy) = (w * 0.5, d * 0.5);
            b.a([0.02, w - 0.02], [cy - 0.025, cy + 0.025], [0.03, 0.07]);
            b.a([cx - 0.025, cx + 0.025], [0.02, d - 0.02], [0.03, 0.07]);
            b.f(steel(), [cx - 0.025, cx + 0.025], [cy - 0.025, cy + 0.025], [0.07, seat - 0.07]);
            b.m([0.07, w - 0.07], [0.04, d - 0.1], [seat - 0.07, seat]);
            b.a([cx - 0.03, cx + 0.03], [d - 0.12, d - 0.09], [seat - 0.07, seat + 0.2]);
            b.m([0.1, w - 0.1], [d - 0.14, d - 0.08], [seat + 0.12, h]);
            for x in [0.04, w - 0.09] {
                b.a([x, x + 0.05], [0.14, d - 0.18], [seat + 0.17, seat + 0.2]);
                b.a([x + 0.015, x + 0.035], [d * 0.5, d * 0.5 + 0.02], [seat - 0.03, seat + 0.17]);
            }
        }
        "toilet" => {
            let tank = 0.2f32.min(d * 0.35);
            let rim = 0.4f32.min(h * 0.55);
            b.m([0.02, w - 0.02], [d - tank, d], [rim - 0.04, h]);
            b.m([0.05, w - 0.05], [0.1, d - tank], [0.0, rim - 0.06]);
            b.m([0.0, w], [0.0, d - tank], [rim - 0.06, rim]);
            b.a([0.01, w - 0.01], [0.01, d - tank - 0.01], [rim, rim + 0.025]);
            b.f(steel(), [w * 0.5 - 0.025, w * 0.5 + 0.025], [d - tank * 0.6, d - tank * 0.4], [h, h + 0.008]);
        }
        "vanity" => {
            b.base_cabinet(h, (w / 0.45).round().max(1.0) as u32, 0.0);
            let rim = 0.2f32.min(w * 0.25);
            b.f(white(), [rim, w - rim], [0.07, d - 0.12], [h, h + 0.02]);
            b.f(Finish::colored("plastic", [0.8, 0.82, 0.83]), [rim + 0.03, w - rim - 0.03], [0.1, d - 0.15], [h + 0.02, h + 0.021]);
            b.f(steel(), [w * 0.5 - 0.012, w * 0.5 + 0.012], [d - 0.09, d - 0.066], [h, h + 0.15]);
            b.f(steel(), [w * 0.5 - 0.01, w * 0.5 + 0.01], [d - 0.2, d - 0.066], [h + 0.13, h + 0.15]);
        }
        "bathtub" => {
            let wall = 0.06f32.min(w * 0.1).min(d * 0.2);
            b.m([0.0, w], [0.0, d], [0.0, 0.12f32.min(h * 0.4)]);
            b.m([0.0, w], [0.0, wall], [0.0, h]);
            b.m([0.0, w], [d - wall, d], [0.0, h]);
            b.m([0.0, wall], [wall, d - wall], [0.0, h]);
            b.m([w - wall, w], [wall, d - wall], [0.0, h]);
            b.a([w - wall - 0.02, w - wall + 0.02], [d * 0.5 - 0.015, d * 0.5 + 0.015], [h, h + 0.12]);
            b.a([w - wall - 0.14, w - wall + 0.02], [d * 0.5 - 0.012, d * 0.5 + 0.012], [h + 0.1, h + 0.12]);
        }
        "shower" => {
            b.m([0.0, w], [0.0, d], [0.0, 0.06]);
            b.f(glass(), [0.0, w], [0.0, 0.01], [0.06, h]);
            b.f(glass(), [w - 0.01, w], [0.01, d], [0.06, h]);
            b.a([0.08, 0.1], [d - 0.04, d - 0.02], [1.0f32.min(h * 0.5), h - 0.05]);
            b.a([0.08, 0.1], [d - 0.3, d - 0.02], [h - 0.07, h - 0.05]);
            b.a([0.02, 0.16], [d - 0.36, d - 0.22], [h - 0.085, h - 0.07]);
            b.a([w - 0.035, w - 0.01], [-0.03, 0.0], [h * 0.45, h * 0.55]);
        }
        "car" => {
            let belt = h * 0.55;
            let sill = h * 0.2;
            let wheel = (h * 0.46).min(d * 0.16);
            let tyre = 0.22f32.min(w * 0.14);
            for x in [0.0, w - tyre] {
                for y in [d * 0.13, d * 0.87 - wheel] {
                    b.a([x, x + tyre], [y, y + wheel], [0.0, wheel]);
                }
            }
            b.m([0.03, w - 0.03], [0.0, d], [sill, belt]);
            b.m([0.06, w - 0.06], [0.02, d * 0.3], [belt, belt + h * 0.04]);
            // The cabin: glass all round under a roof.
            b.f(Finish::of("glass_tinted"), [0.1, w - 0.1], [d * 0.3, d * 0.84], [belt, h - 0.05]);
            b.f(dark(), [0.12, w - 0.12], [d * 0.32, d * 0.82], [belt, h - 0.06]);
            b.m([0.08, w - 0.08], [d * 0.34, d * 0.8], [h - 0.06, h]);
            for y in [d * 0.3, d * 0.56, d * 0.82] {
                for x in [0.085, w - 0.125] {
                    b.m([x, x + 0.04], [y, y + 0.04], [belt, h - 0.05]);
                }
            }
            for x in [0.1, w - 0.42] {
                b.f(Finish::of("light"), [x, x + 0.32], [-0.004, 0.0], [belt - h * 0.14, belt - h * 0.05]);
                b.f(Finish::colored("light", [0.75, 0.05, 0.04]), [x, x + 0.32], [d, d + 0.004], [belt - h * 0.12, belt - h * 0.04]);
            }
            b.a([0.25, w - 0.25], [-0.004, 0.0], [sill + 0.03, sill + h * 0.1]);
        }
        _ => b.m([0.0, w], [0.0, d], [0.0, h]),
    }
    b.parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_has_a_unique_key_and_parts_inside_a_sensible_envelope() {
        let all = catalog();
        for (i, a) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|b| b.key != a.key), "duplicate key {}", a.key);
            let f = place(a.key, 1, [0.0, 0.0]).unwrap();
            let parts = parts(&f);
            assert!(!parts.is_empty(), "{} has no parts", a.key);
            assert!(parts.len() > 1 || a.hung || matches!(a.key, "coffee_table_marble"), "{} is only a box", a.key);
            for part in &parts {
                for i in 0..3 {
                    assert!(part.max[i] > part.min[i], "{}: empty part {part:?}", a.key);
                    let half = if i < 2 { a.size[i] * 0.5 + 0.12 } else { a.size[2] + 0.3 };
                    assert!(part.max[i] <= half && part.min[i] >= if i < 2 { -half } else { -1e-4 }, "{}: part outside its piece {part:?}", a.key);
                }
            }
        }
    }

    #[test]
    fn parts_follow_the_size() {
        let mut f = place("sofa", 1, [0.0, 0.0]).unwrap();
        f.size = [3.0, 1.2, 1.0];
        let widest = parts(&f).iter().map(|p| p.max[0] - p.min[0]).fold(0.0, f32::max);
        assert!((widest - 3.0).abs() < 1e-4, "{widest}");
    }
}
