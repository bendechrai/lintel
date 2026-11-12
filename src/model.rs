//! The house model: everything a project file holds.
//!
//! Units are metres and radians. The plan is X east, Y north; Z is up. A level's `elevation` is
//! the height of its finished floor above the site datum (z = 0), so a walk-out basement on a
//! lot that falls away to the back is simply a level with a negative elevation and terrain that
//! drops below it.
//!
//! Everything here is plain data with serde derives; geometry lives in `mesh.rs`, drawing in
//! `plan.rs` and `gfx.rs`.

use serde::{Deserialize, Serialize};

pub type Id = u64;
pub type P2 = [f32; 2];

fn yes() -> bool {
    true
}

// ---------------------------------------------------------------------------------------------
// Project

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub units: Units,
    pub site: Site,
    /// Bottom to top.
    pub levels: Vec<Level>,
    #[serde(default)]
    pub roofs: Vec<Roof>,
    /// Materials the project adds to the built-in library (`materials::library`).
    #[serde(default)]
    pub materials: Vec<crate::materials::Material>,
    /// Next id to hand out. Ids are unique across the whole project.
    pub next_id: Id,
    /// The name of this version of the house: "Existing", "Open-plan kitchen" …
    #[serde(default = "Project::first_option")]
    pub option: String,
    /// The other versions kept in the same file, each a whole house of its own. Theirs are
    /// always empty: options don't nest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub others: Vec<Project>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Units {
    #[default]
    Metric,
    Imperial,
}

impl Project {
    fn first_option() -> String {
        "Existing".into()
    }

    /// Every option's name, the one being worked on first.
    pub fn option_names(&self) -> Vec<&str> {
        std::iter::once(self.option.as_str()).chain(self.others.iter().map(|o| o.option.as_str())).collect()
    }

    /// Makes `others[index]` the house being worked on; the current one takes its place in
    /// the list.
    pub fn switch_option(&mut self, index: usize) {
        if index >= self.others.len() {
            return;
        }
        let mut others = std::mem::take(&mut self.others);
        std::mem::swap(self, &mut others[index]);
        // The project's name belongs to the file, not to an option.
        self.name = others[index].name.clone();
        self.units = others[index].units;
        self.others = others;
    }

    /// Copies the house being worked on into a new option called `name` and switches to it.
    pub fn branch_option(&mut self, name: &str) {
        let mut kept = self.clone();
        kept.others.clear();
        self.others.insert(0, kept);
        self.option = name.to_string();
    }

    /// Drops the option being worked on and switches to the first of the others.
    pub fn remove_option(&mut self) {
        if self.others.is_empty() {
            return;
        }
        self.switch_option(0);
        self.others.remove(0);
    }

    pub fn id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    pub fn level(&self, id: Id) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn level_mut(&mut self, id: Id) -> Option<&mut Level> {
        self.levels.iter_mut().find(|l| l.id == id)
    }
}

// ---------------------------------------------------------------------------------------------
// Materials on surfaces

/// A surface finish: a material from the library by id, plus an optional colour.
///
/// What the colour means depends on the material (`materials::Tint`): the paint colour of a
/// painted surface, a stain over a timber, or nothing at all for a material that is the colour
/// it is (marble, concrete, brick).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Finish {
    pub material: String,
    /// sRGB, 0..1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

impl Finish {
    pub fn of(material: &str) -> Finish {
        Finish { material: material.into(), color: None }
    }

    pub fn colored(material: &str, color: [f32; 3]) -> Finish {
        Finish { material: material.into(), color: Some(color) }
    }

    /// `#rrggbb` paint.
    pub fn paint(hex: u32) -> Finish {
        Finish::colored("paint", rgb(hex))
    }
}

pub fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

// ---------------------------------------------------------------------------------------------
// Site and terrain

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Site {
    pub terrain: Terrain,
    /// Driveways, paths, patios, lawns, garden beds: polygons draped over the terrain.
    #[serde(default)]
    pub patches: Vec<Patch>,
    /// Where the plan origin is on Earth, when known (terrain fetched from an elevation service).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<Geo>,
    /// Compass bearing of the plan's +Y axis, in degrees clockwise from true north.
    #[serde(default)]
    pub north: f32,
    #[serde(default)]
    pub sun: SunSetting,
}

/// Where the sun is: set by hand, or worked out from the site's location and a date and time.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct SunSetting {
    /// True takes the position from `month`, `day`, `hour` at the site's `geo`.
    pub by_clock: bool,
    /// By hand: compass bearing of the sun, degrees clockwise from true north.
    pub azimuth: f32,
    /// By hand: degrees above the horizon.
    pub altitude: f32,
    pub month: u32,
    pub day: u32,
    /// Local clock time in hours, 0..24.
    pub hour: f32,
    /// Hours the local clock is ahead of UTC.
    pub utc_offset: f32,
    /// A cloudy day: no direct sun, a brighter-overhead grey sky.
    #[serde(default)]
    pub overcast: bool,
}

impl Default for SunSetting {
    fn default() -> SunSetting {
        SunSetting { by_clock: false, azimuth: 153.0, altitude: 49.0, month: 6, day: 21, hour: 14.0, utc_offset: 0.0, overcast: false }
    }
}

impl Site {
    /// The unit vector toward the sun in the plan's axes, and its height in radians.
    pub fn sun_direction(&self) -> ([f32; 3], f32) {
        let (bearing, height) = self.sun_position();
        // Compass bearings run clockwise from north; the plan's +Y points `north` degrees
        // round from true north.
        let (e, n) = bearing.to_radians().sin_cos();
        let (sb, cb) = self.north.to_radians().sin_cos();
        let (x, y) = (e * cb - n * sb, e * sb + n * cb);
        let alt = height.to_radians();
        ([x * alt.cos(), y * alt.cos(), alt.sin()], alt)
    }

    /// The sun's compass bearing and height in degrees, as the setting gives them.
    pub fn sun_position(&self) -> (f32, f32) {
        if self.sun.by_clock {
            let geo = self.geo.unwrap_or(Geo { lat: 40.0, lon: 0.0 });
            crate::solar::position(geo.lat, geo.lon, self.sun.month, self.sun.day, self.sun.hour, self.sun.utc_offset)
        } else {
            (self.sun.azimuth, self.sun.altitude)
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Geo {
    pub lat: f64,
    pub lon: f64,
}

/// A regular grid of ground heights. `heights[row * cols + col]` is the ground at
/// `origin + [col * spacing, row * spacing]`, relative to the site datum.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Terrain {
    pub origin: P2,
    pub spacing: f32,
    pub cols: usize,
    pub rows: usize,
    pub heights: Vec<f32>,
    #[serde(default = "Terrain::default_finish")]
    pub finish: Finish,
    /// Where the heights came from, shown in the inspector ("Open Topo Data ned10m", "slope").
    #[serde(default)]
    pub source: String,
}

impl Terrain {
    fn default_finish() -> Finish {
        Finish::of("grass")
    }

    /// Flat ground at the datum.
    pub fn flat(origin: P2, size: P2, spacing: f32) -> Terrain {
        Terrain::sloped(origin, size, spacing, 0.0, 0.0, [0.0, 0.0])
    }

    /// A plane through `pivot` at z = 0 that falls `grade` metres per metre toward `bearing`
    /// (radians, measured from +X toward +Y: `PI / 2` falls to the north).
    pub fn sloped(origin: P2, size: P2, spacing: f32, bearing: f32, grade: f32, pivot: P2) -> Terrain {
        let cols = (size[0] / spacing).ceil() as usize + 1;
        let rows = (size[1] / spacing).ceil() as usize + 1;
        let (s, c) = bearing.sin_cos();
        let mut heights = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for q in 0..cols {
                let x = origin[0] + q as f32 * spacing - pivot[0];
                let y = origin[1] + r as f32 * spacing - pivot[1];
                heights.push(-(x * c + y * s) * grade);
            }
        }
        Terrain { origin, spacing, cols, rows, heights, finish: Terrain::default_finish(), source: "slope".into() }
    }

    pub fn size(&self) -> P2 {
        [(self.cols.max(1) - 1) as f32 * self.spacing, (self.rows.max(1) - 1) as f32 * self.spacing]
    }

    /// Ground height at a plan point, bilinear between grid posts, clamped at the edges.
    pub fn height(&self, p: P2) -> f32 {
        if self.cols < 2 || self.rows < 2 || self.heights.len() < self.cols * self.rows {
            return 0.0;
        }
        let fx = ((p[0] - self.origin[0]) / self.spacing).clamp(0.0, (self.cols - 1) as f32);
        let fy = ((p[1] - self.origin[1]) / self.spacing).clamp(0.0, (self.rows - 1) as f32);
        let (x0, y0) = ((fx as usize).min(self.cols - 2), (fy as usize).min(self.rows - 2));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let h = |x: usize, y: usize| self.heights[y * self.cols + x];
        let a = h(x0, y0) + (h(x0 + 1, y0) - h(x0, y0)) * tx;
        let b = h(x0, y0 + 1) + (h(x0 + 1, y0 + 1) - h(x0, y0 + 1)) * tx;
        a + (b - a) * ty
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Patch {
    pub id: Id,
    pub name: String,
    pub outline: Vec<P2>,
    pub finish: Finish,
}

// ---------------------------------------------------------------------------------------------
// Levels

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Level {
    pub id: Id,
    pub name: String,
    /// Finished floor height above the site datum.
    pub elevation: f32,
    /// Floor to ceiling, the default for rooms and walls on this level.
    pub height: f32,
    /// Floor structure thickness, below the finished floor.
    pub slab: f32,
    #[serde(default)]
    pub walls: Vec<Wall>,
    #[serde(default)]
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub stairs: Vec<Stair>,
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    #[serde(default)]
    pub furniture: Vec<Furniture>,
    #[serde(default = "yes")]
    pub visible: bool,
    /// A floor plan image laid under the plan to draw over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlay: Option<Underlay>,
}

/// An image under a level's plan: a scan or photo of existing drawings to trace.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Underlay {
    /// The image file. The project holds only where it is, not the picture.
    pub path: String,
    /// Where the image's bottom-left corner sits on the plan.
    pub at: P2,
    /// Metres on the plan per pixel of the image.
    pub scale: f32,
    /// Radians, counter-clockwise about the bottom-left corner.
    #[serde(default)]
    pub turn: f32,
    pub opacity: f32,
    #[serde(default = "yes")]
    pub visible: bool,
}

impl Underlay {
    /// Rescales the image about the plan point `fixed` so that a length that measured
    /// `measured` on the plan becomes `real`.
    pub fn calibrate(&mut self, fixed: P2, measured: f32, real: f32) {
        if measured <= 1e-4 || real <= 1e-4 {
            return;
        }
        let k = real / measured;
        self.scale *= k;
        self.at = [fixed[0] + (self.at[0] - fixed[0]) * k, fixed[1] + (self.at[1] - fixed[1]) * k];
    }
}

impl Level {
    pub fn new(id: Id, name: &str, elevation: f32, height: f32) -> Level {
        Level {
            id,
            name: name.into(),
            elevation,
            height,
            slab: 0.3,
            walls: vec![],
            rooms: vec![],
            stairs: vec![],
            fixtures: vec![],
            furniture: vec![],
            visible: true,
            underlay: None,
        }
    }

    pub fn wall(&self, id: Id) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }
}

// ---------------------------------------------------------------------------------------------
// Walls and what goes in them

/// A straight wall along the centre line `a → b`. Its *left* face is on the left when walking
/// from `a` to `b`, its *right* face on the right.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Wall {
    pub id: Id,
    pub a: P2,
    pub b: P2,
    pub thickness: f32,
    /// `None` runs to the level's ceiling height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    /// Bottom of the wall above the level's floor (negative reaches below it, for a wall that
    /// runs down to a footing on falling ground).
    #[serde(default)]
    pub base: f32,
    pub left: Finish,
    pub right: Finish,
    #[serde(default)]
    pub openings: Vec<Opening>,
    /// Finishes over part of a face's height: tiling to a dado, a splashback, wainscoting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<Band>,
}

/// A finish laid over one face of a wall between two heights above the level's floor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Band {
    pub side: WallSide,
    pub from: f32,
    pub to: f32,
    pub finish: Finish,
}

impl Wall {
    pub fn length(&self) -> f32 {
        ((self.b[0] - self.a[0]).powi(2) + (self.b[1] - self.a[1]).powi(2)).sqrt()
    }

    /// Unit vector along the wall, and its left normal.
    pub fn frame(&self) -> (P2, P2) {
        let l = self.length().max(1e-6);
        let d = [(self.b[0] - self.a[0]) / l, (self.b[1] - self.a[1]) / l];
        (d, [-d[1], d[0]])
    }

    /// The plan point `along` metres from `a`, `across` metres to the left of the centre line.
    pub fn point(&self, along: f32, across: f32) -> P2 {
        let (d, n) = self.frame();
        [self.a[0] + d[0] * along + n[0] * across, self.a[1] + d[1] * along + n[1] * across]
    }
}

/// Anything that interrupts a wall: a door, a window, a cased opening, or a recess that only
/// goes part of the way through.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Opening {
    pub id: Id,
    /// Distance from the wall's `a` end to the opening's centre.
    pub at: f32,
    pub width: f32,
    pub height: f32,
    /// Bottom of the opening above the level's floor: 0 for a door, ~0.9 for a window.
    pub sill: f32,
    pub kind: OpeningKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpeningKind {
    Door(Door),
    Garage(GarageDoor),
    Window(Window),
    /// A hole with no door in it. `frame` cases it with a jamb and architrave.
    Passage { frame: Option<Frame> },
    /// A niche cut into one face, `depth` deep (less than the wall's thickness).
    Recess { side: WallSide, depth: f32, finish: Finish },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WallSide {
    Left,
    Right,
}

/// The jamb lining an opening and the architrave (casing) around it on both faces.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    /// Jamb thickness: how far the lining stands into the opening.
    pub jamb: f32,
    /// Architrave width on the wall face. 0 for none.
    pub casing: f32,
    pub finish: Finish,
}

impl Default for Frame {
    fn default() -> Frame {
        Frame { jamb: 0.03, casing: 0.07, finish: Finish::paint(0xf4f2ec) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Door {
    pub style: DoorStyle,
    /// Which jamb the hinges (or the fixed panel) are on, seen from the wall's left side.
    pub hinge: Hinge,
    /// Which face the leaf swings toward.
    pub swing: WallSide,
    /// 0 closed … 1 fully open (90° for a hinged leaf), as drawn in 3D.
    #[serde(default)]
    pub open: f32,
    pub leaf: Finish,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
    /// Fraction of the leaf that is glazed, 0..1 (a French or patio door).
    #[serde(default)]
    pub glazing: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoorStyle {
    Hinged,
    /// Two hinged leaves meeting in the middle.
    Double,
    /// A leaf that slides across the face of a fixed panel (patio door).
    Sliding,
    /// Slides into the wall.
    Pocket,
    Bifold,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Hinge {
    /// Nearer the wall's `a` end.
    Start,
    /// Nearer the wall's `b` end.
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GarageDoor {
    pub style: GarageStyle,
    /// The face the door retracts toward (the inside of the garage).
    pub inside: WallSide,
    /// 0 closed … 1 fully open.
    #[serde(default)]
    pub open: f32,
    /// Horizontal panels in a sectional door.
    #[serde(default = "GarageDoor::default_panels")]
    pub panels: u32,
    pub finish: Finish,
}

impl GarageDoor {
    fn default_panels() -> u32 {
        4
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GarageStyle {
    /// Hinged panels that ride tracks up and back under the ceiling.
    Sectional,
    /// A corrugated curtain that winds onto a drum above the opening.
    Roller,
    /// One rigid panel that tilts up and back.
    Tilt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub style: WindowStyle,
    pub frame: Finish,
    /// Frame member width on the face.
    #[serde(default = "Window::default_frame_width")]
    pub frame_width: f32,
    /// Panes across and up, for fixed and picture windows divided by mullions or glazing bars.
    #[serde(default = "Window::one")]
    pub cols: u32,
    #[serde(default = "Window::one")]
    pub rows: u32,
    /// Architrave around the opening, inside and out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub casing: Option<Frame>,
}

impl Window {
    fn default_frame_width() -> f32 {
        0.05
    }

    fn one() -> u32 {
        1
    }

    pub fn new(style: WindowStyle) -> Window {
        Window { style, frame: Finish::paint(0xf4f2ec), frame_width: 0.05, cols: 1, rows: 1, casing: None }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowStyle {
    Fixed,
    /// Hinged at the side.
    Casement,
    /// Hinged at the top.
    Awning,
    /// Two sashes, one above the other; the lower slides up.
    DoubleHung,
    /// Two sashes side by side.
    Sliding,
    /// Horizontal glass blades.
    Louvre,
}

// ---------------------------------------------------------------------------------------------
// Rooms, stairs

/// A floor area with its own floor and ceiling. Rooms make the slabs: where a level has no room
/// there is no floor, which is how a stairwell or a double-height void is left open.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Room {
    pub id: Id,
    pub name: String,
    pub outline: Vec<P2>,
    pub floor: Finish,
    pub ceiling: Finish,
    /// `None` uses the level's height (a corridor with a dropped ceiling sets its own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling_height: Option<f32>,
    /// Finished floor above the level's (a sunken lounge is negative, a garage slab often is).
    #[serde(default)]
    pub floor_offset: f32,
    /// False for a deck, a porch or a carport: a floor with nothing over it.
    #[serde(default = "yes")]
    pub has_ceiling: bool,
    /// A board along the foot of the room's walls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skirting: Option<Trim>,
    /// A moulding where the walls meet the ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cornice: Option<Trim>,
    /// The light level this room should reach, in lux, if it has a target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lux: Option<f32>,
}

/// A strip of trim that runs round a room: a skirting board or a cornice.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Trim {
    /// How tall it is on the wall.
    pub height: f32,
    /// How far it stands off the wall.
    pub depth: f32,
    pub finish: Finish,
}

impl Trim {
    pub fn skirting() -> Trim {
        Trim { height: 0.09, depth: 0.015, finish: Finish::paint(0xf7f5f0) }
    }

    pub fn cornice() -> Trim {
        Trim { height: 0.075, depth: 0.075, finish: Finish::paint(0xfafaf7) }
    }
}

/// A straight flight. It starts at `at` on this level's floor and climbs `rise` metres toward
/// `direction` over `run` metres.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stair {
    pub id: Id,
    /// Centre of the bottom edge.
    pub at: P2,
    /// Radians from +X toward +Y.
    pub direction: f32,
    pub width: f32,
    pub run: f32,
    pub rise: f32,
    pub steps: u32,
    pub tread: Finish,
    /// Closed risers and a solid underside, or open treads.
    #[serde(default = "yes")]
    pub closed: bool,
    #[serde(default)]
    pub railing: Railing,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Railing {
    #[default]
    None,
    Left,
    Right,
    Both,
}

// ---------------------------------------------------------------------------------------------
// Fixtures: electrical, lighting, safety, air

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fixture {
    pub id: Id,
    pub kind: FixtureKind,
    pub mount: Mount,
    #[serde(default)]
    pub label: String,
    /// For a light: its lamp, when it differs from the kind's usual one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lamp: Option<Lamp>,
    /// For a switch: the lights it turns on and off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<Id>,
}

/// What a light fitting gives out.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lamp {
    pub source: Source,
    /// Total light output. For a strip or tube, per metre of its length.
    pub lumens: f32,
    /// Colour temperature in kelvin: 2700 warm white, 4000 neutral, 6500 daylight.
    pub kelvin: f32,
    #[serde(default = "yes")]
    pub on: bool,
    /// 0..1.
    #[serde(default = "Lamp::full")]
    pub dim: f32,
}

impl Lamp {
    fn full() -> f32 {
        1.0
    }

    /// Electrical power at full brightness for `lumens` of light.
    pub fn watts(&self, lumens: f32) -> f32 {
        lumens / self.source.efficacy()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Led,
    Incandescent,
    Halogen,
    Fluorescent,
}

impl Source {
    /// Typical lumens per watt.
    pub fn efficacy(self) -> f32 {
        match self {
            Source::Led => 100.0,
            Source::Incandescent => 14.0,
            Source::Halogen => 20.0,
            Source::Fluorescent => 70.0,
        }
    }

    /// The colour temperature such a lamp usually has.
    pub fn kelvin(self) -> f32 {
        match self {
            Source::Led => 3000.0,
            Source::Incandescent => 2700.0,
            Source::Halogen => 2900.0,
            Source::Fluorescent => 4000.0,
        }
    }
}

impl Fixture {
    pub fn new(id: Id, kind: FixtureKind, mount: Mount) -> Fixture {
        Fixture { id, kind, mount, label: String::new(), lamp: None, controls: vec![] }
    }

    /// The lamp in a light fitting: its own, or the usual one for its kind. `None` for
    /// anything that isn't a light.
    pub fn lamp(&self) -> Option<Lamp> {
        let usual = |lumens: f32| Lamp { source: Source::Led, lumens, kelvin: 3000.0, on: true, dim: 1.0 };
        let usual = match self.kind {
            FixtureKind::Downlight { .. } => usual(700.0),
            FixtureKind::Surface { .. } => usual(1400.0),
            FixtureKind::Pendant { .. } => usual(800.0),
            FixtureKind::TubeLight { .. } => usual(2200.0),
            FixtureKind::Strip { .. } => usual(1000.0),
            _ => return None,
        };
        Some(self.lamp.unwrap_or(usual))
    }

    /// Light output in lumens, as dimmed, 0 when off.
    pub fn output(&self) -> f32 {
        let Some(lamp) = self.lamp() else { return 0.0 };
        if !lamp.on {
            return 0.0;
        }
        let per_metre = match self.kind {
            FixtureKind::TubeLight { length, .. } | FixtureKind::Strip { length } => length,
            _ => 1.0,
        };
        lamp.lumens * per_metre * lamp.dim.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum Mount {
    /// On a wall face: `at` metres from the wall's `a` end, `height` to the fixture's centre
    /// above the level's floor.
    Wall { wall: Id, side: WallSide, at: f32, height: f32 },
    /// On the ceiling of whatever room is at `at`; `turn` rotates it (radians).
    Ceiling { at: P2, turn: f32 },
    /// On the floor.
    Floor { at: P2, turn: f32 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixtureKind {
    /// A power outlet plate with this many sockets.
    Outlet { gangs: u32 },
    /// A light switch plate with this many rockers.
    Switch { gangs: u32 },
    SmokeDetector,
    /// A round recessed downlight.
    Downlight { diameter: f32 },
    /// A linear light recessed into the ceiling.
    TubeLight { length: f32, width: f32 },
    /// An LED strip in a diffuser channel, on a wall, ceiling or floor.
    Strip { length: f32 },
    /// A surface light on a wall or ceiling.
    Surface { diameter: f32 },
    /// A hanging light, `drop` metres below the ceiling.
    Pendant { diameter: f32, drop: f32 },
    /// A register or grille: HVAC supply or return, on a wall, ceiling or floor.
    Vent { width: f32, height: f32 },
    Thermostat,
    /// Data, TV or phone outlet.
    Data,
}

impl FixtureKind {
    pub fn name(&self) -> &'static str {
        match self {
            FixtureKind::Outlet { .. } => "Power outlet",
            FixtureKind::Switch { .. } => "Light switch",
            FixtureKind::SmokeDetector => "Smoke detector",
            FixtureKind::Downlight { .. } => "Downlight",
            FixtureKind::TubeLight { .. } => "Recessed tube light",
            FixtureKind::Strip { .. } => "LED strip",
            FixtureKind::Surface { .. } => "Surface light",
            FixtureKind::Pendant { .. } => "Pendant light",
            FixtureKind::Vent { .. } => "Air vent",
            FixtureKind::Thermostat => "Thermostat",
            FixtureKind::Data => "Data outlet",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Furniture and appliances

/// A piece from the catalogue (`catalog.rs`), placed on a level's floor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Furniture {
    pub id: Id,
    /// Catalogue key: `"sofa"`, `"fridge"`, `"kitchen_island"` …
    pub kind: String,
    #[serde(default)]
    pub name: String,
    /// Centre of the footprint.
    pub at: P2,
    /// Radians, counter-clockwise from the piece facing -Y (its front toward the bottom of the plan).
    pub turn: f32,
    /// Width (across the front), depth, height.
    pub size: [f32; 3],
    /// Height of its underside above the floor (a wall cabinet, a wall-mounted TV).
    #[serde(default)]
    pub lift: f32,
    /// The main finish (upholstery, carcass, body) and the secondary one (legs, benchtop, trim).
    pub finish: Finish,
    pub accent: Finish,
}

// ---------------------------------------------------------------------------------------------
// Roofs

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Roof {
    pub id: Id,
    pub name: String,
    pub shape: RoofShape,
    /// Centre of the rectangle the roof covers, at the wall line (overhang is added outside it).
    pub center: P2,
    /// Size along the ridge and across it.
    pub size: P2,
    /// Rotation of the ridge from +X, radians.
    pub turn: f32,
    /// Height of the eaves (top of the wall plate) above the site datum.
    pub base: f32,
    /// Rise over run: 0.5 is a 6:12 roof (26.6°).
    pub pitch: f32,
    pub overhang: f32,
    pub thickness: f32,
    pub finish: Finish,
    /// The gable-end walls a gable or shed roof leaves open.
    pub gable: Finish,
    #[serde(default = "yes")]
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoofShape {
    Flat,
    /// One plane, low along the -across edge and high along the +across edge.
    Shed,
    Gable,
    Hip,
}

// ---------------------------------------------------------------------------------------------
// Selection

/// Something the editor can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
    Wall(Id),
    /// Wall id, opening id.
    Opening(Id, Id),
    Room(Id),
    Stair(Id),
    Fixture(Id),
    Furniture(Id),
    Roof(Id),
    Patch(Id),
}

// ---------------------------------------------------------------------------------------------
// Finding things by id, on whichever level they live

impl Project {
    pub fn wall_mut(&mut self, id: Id) -> Option<&mut Wall> {
        self.levels.iter_mut().flat_map(|l| &mut l.walls).find(|w| w.id == id)
    }

    pub fn wall(&self, id: Id) -> Option<&Wall> {
        self.levels.iter().flat_map(|l| &l.walls).find(|w| w.id == id)
    }

    pub fn opening_mut(&mut self, wall: Id, id: Id) -> Option<&mut Opening> {
        self.wall_mut(wall)?.openings.iter_mut().find(|o| o.id == id)
    }

    pub fn opening(&self, wall: Id, id: Id) -> Option<&Opening> {
        self.wall(wall)?.openings.iter().find(|o| o.id == id)
    }

    pub fn room_mut(&mut self, id: Id) -> Option<&mut Room> {
        self.levels.iter_mut().flat_map(|l| &mut l.rooms).find(|r| r.id == id)
    }

    pub fn room(&self, id: Id) -> Option<&Room> {
        self.levels.iter().flat_map(|l| &l.rooms).find(|r| r.id == id)
    }

    pub fn stair_mut(&mut self, id: Id) -> Option<&mut Stair> {
        self.levels.iter_mut().flat_map(|l| &mut l.stairs).find(|r| r.id == id)
    }

    pub fn stair(&self, id: Id) -> Option<&Stair> {
        self.levels.iter().flat_map(|l| &l.stairs).find(|r| r.id == id)
    }

    pub fn fixture_mut(&mut self, id: Id) -> Option<&mut Fixture> {
        self.levels.iter_mut().flat_map(|l| &mut l.fixtures).find(|r| r.id == id)
    }

    pub fn fixture(&self, id: Id) -> Option<&Fixture> {
        self.levels.iter().flat_map(|l| &l.fixtures).find(|r| r.id == id)
    }

    pub fn furniture_mut(&mut self, id: Id) -> Option<&mut Furniture> {
        self.levels.iter_mut().flat_map(|l| &mut l.furniture).find(|r| r.id == id)
    }

    pub fn furniture(&self, id: Id) -> Option<&Furniture> {
        self.levels.iter().flat_map(|l| &l.furniture).find(|r| r.id == id)
    }

    pub fn roof_mut(&mut self, id: Id) -> Option<&mut Roof> {
        self.roofs.iter_mut().find(|r| r.id == id)
    }

    pub fn roof(&self, id: Id) -> Option<&Roof> {
        self.roofs.iter().find(|r| r.id == id)
    }

    pub fn patch_mut(&mut self, id: Id) -> Option<&mut Patch> {
        self.site.patches.iter_mut().find(|r| r.id == id)
    }

    pub fn patch(&self, id: Id) -> Option<&Patch> {
        self.site.patches.iter().find(|r| r.id == id)
    }
}
