//! The house a new project opens with: a two-level home on a lot that falls from the street
//! down to the back garden. The front door and a double garage (one wide sectional door) are at
//! street level; the basement walks out to the garden behind.
//!
//! The plan origin is the front-left corner of the house. The street is to the south (-Y).

use std::f32::consts::{FRAC_PI_2, PI};

use crate::catalog;
use crate::model::*;

const PAINT: u32 = 0xf2efe8;
const TRIM: u32 = 0xf7f5f0;
const SIDING: u32 = 0x64747c;

struct B {
    p: Project,
}

impl B {
    fn wall(&mut self, a: P2, b: P2, thickness: f32, left: Finish, right: Finish) -> Wall {
        Wall { id: self.p.id(), a, b, thickness, height: None, base: 0.0, left, right, openings: vec![], bands: vec![], layers: vec![] }
    }

    fn opening(&mut self, at: f32, width: f32, height: f32, sill: f32, kind: OpeningKind) -> Opening {
        Opening { id: self.p.id(), at, width, height, sill, kind }
    }

    fn door(&mut self, at: f32, hinge: Hinge, swing: WallSide) -> Opening {
        let door = Door { style: DoorStyle::Hinged, hinge, swing, open: 0.0, leaf: Finish::paint(TRIM), frame: Some(Frame::default()), glazing: 0.0 };
        self.opening(at, 0.82, 2.04, 0.0, OpeningKind::Door(door))
    }

    fn window(&mut self, at: f32, width: f32, height: f32, sill: f32, style: WindowStyle) -> Opening {
        let window = Window { casing: Some(Frame::default()), ..Window::new(style) };
        self.opening(at, width, height, sill, OpeningKind::Window(window))
    }

    fn room(&mut self, name: &str, outline: &[P2], floor: Finish) -> Room {
        Room { id: self.p.id(), name: name.into(), outline: outline.to_vec(), floor, ceiling: Finish::paint(0xfafaf7), ceiling_height: None, floor_offset: 0.0, has_ceiling: true, balustrade: None, ceiling_shape: CeilingShape::Flat, skirting: None, cornice: None, target_lux: None }
    }

    fn fixture(&mut self, kind: FixtureKind, mount: Mount) -> Fixture {
        Fixture::new(self.p.id(), kind, mount)
    }

    fn furniture(&mut self, key: &str, at: P2, turn: f32) -> Furniture {
        let id = self.p.id();
        catalog::place(key, id, at).map(|f| Furniture { turn, ..f }).unwrap_or(Furniture {
            id,
            kind: key.into(),
            name: key.replace('_', " "),
            at,
            turn,
            size: [0.8, 0.8, 0.8],
            lift: 0.0,
            finish: Finish::of("oak"),
            accent: Finish::of("steel"),
            hung: false,
        })
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> [P2; 4] {
    [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// Ground height for the sample lot: level at the street and under the garage apron, then
/// falling 2.9 m across the depth of the house, easing out into the back garden.
fn ground(x: f32, y: f32) -> f32 {
    let t = ((y - 1.0) / 10.0).clamp(0.0, 1.0);
    let fall = t * t * (3.0 - 2.0 * t) * 2.95;
    let beyond = (y - 11.0).max(0.0) * 0.04;
    let cross = (x - 7.5) * 0.012 * t;
    let kerb = if y < -9.2 { -0.12 } else { 0.0 };
    -(fall + beyond) - cross + kerb
}

pub fn house() -> Project {
    let mut terrain = Terrain::flat([-9.0, -13.0], [33.0, 40.0], 1.0);
    for r in 0..terrain.rows {
        for c in 0..terrain.cols {
            terrain.heights[r * terrain.cols + c] = ground(terrain.origin[0] + c as f32, terrain.origin[1] + r as f32);
        }
    }
    terrain.source = "Sample lot: falls 2.9 m from the street to the back garden".into();

    let mut b = B {
        p: Project {
            name: "Sample house".into(),
            units: Units::Metric,
            site: Site { terrain, patches: vec![], geo: None, north: 0.0, sun: SunSetting::default(), barriers: vec![], plants: vec![] },
            levels: vec![],
            roofs: vec![],
            materials: vec![],
            next_id: 0,
            products: vec![],
            option: "Existing".into(),
            others: vec![],
        },
    };

    let inside = || Finish::paint(PAINT);
    let siding = || Finish::colored("siding", rgb(SIDING));
    let render = || Finish::colored("render", rgb(0xb9b6ae));

    // -----------------------------------------------------------------------------------------
    // Basement: under the main part of the house only, open to the garden at the back.
    let mut lower = Level::new(b.p.id(), "Basement", -2.9, 2.55);
    lower.slab = 0.15;
    let conc = |b: &mut B, a: P2, c: P2| b.wall(a, c, 0.25, Finish::paint(PAINT), Finish::colored("render", rgb(0xb9b6ae)));
    let mut front = conc(&mut b, [6.2, 0.0], [15.0, 0.0]);
    front.height = Some(2.6);
    let mut east = conc(&mut b, [15.0, 0.0], [15.0, 11.0]);
    east.height = Some(2.6);
    east.openings.push(b.window(9.3, 1.2, 0.9, 1.25, WindowStyle::Sliding));
    let mut back = b.wall([15.0, 11.0], [6.2, 11.0], 0.25, inside(), siding());
    back.height = Some(2.6);
    back.openings.push(b.window(1.9, 1.5, 1.2, 0.9, WindowStyle::DoubleHung));
    let patio_door = Door { style: DoorStyle::Sliding, hinge: Hinge::Start, swing: WallSide::Left, open: 0.35, leaf: Finish::colored("aluminium", rgb(0x2f3134)), frame: None, glazing: 0.92 };
    back.openings.push(b.opening(6.2, 2.4, 2.1, 0.0, OpeningKind::Door(patio_door.clone())));
    let mut west = conc(&mut b, [6.2, 11.0], [6.2, 0.0]);
    west.height = Some(2.6);
    let mut rec_south = b.wall([6.2, 4.6], [11.2, 4.6], 0.115, inside(), inside());
    rec_south.openings.push(b.door(3.6, Hinge::Start, WallSide::Right));
    let mut spine = b.wall([11.2, 3.4], [11.2, 11.0], 0.115, inside(), inside());
    spine.openings.push(b.door(1.9, Hinge::End, WallSide::Left));
    spine.openings.push(b.door(3.6, Hinge::Start, WallSide::Left));
    let bed_south = b.wall([11.2, 6.0], [15.0, 6.0], 0.115, inside(), inside());
    let mut bath_south = b.wall([11.2, 3.4], [15.0, 3.4], 0.115, inside(), Finish::paint(0xe4e2dc));
    bath_south.bands.push(Band { side: WallSide::Left, from: 0.0, to: 1.2, finish: Finish::colored("tile_small", rgb(0xe9eef0)) });
    lower.walls = vec![front, east, back, west, rec_south, spine, bed_south, bath_south];

    lower.rooms.push(b.room("Rec room", &rect(6.2, 4.6, 11.2, 11.0), Finish::colored("carpet", rgb(0x8d8a84))));
    lower.rooms.push(b.room("Bedroom", &rect(11.2, 6.0, 15.0, 11.0), Finish::colored("carpet", rgb(0xb5ab9c))));
    lower.rooms.push(b.room("Bathroom", &rect(11.2, 3.4, 15.0, 6.0), Finish::colored("tile_small", rgb(0xdfe5e6))));
    lower.rooms.push(b.room("Utility", &[[6.2, 0.0], [15.0, 0.0], [15.0, 3.4], [11.2, 3.4], [11.2, 4.6], [6.2, 4.6]], Finish::of("concrete")));

    lower.stairs.push(Stair {
        id: b.p.id(),
        at: [6.85, 8.5],
        direction: -FRAC_PI_2,
        width: 0.95,
        run: 3.9,
        rise: 2.9,
        steps: 16,
        tread: Finish::of("oak"),
        closed: true,
        railing: Railing::Left,
    });

    // Steps from the patio up to the deck.
    lower.stairs.push(Stair {
        id: b.p.id(),
        at: [9.3, 17.7],
        direction: -FRAC_PI_2,
        width: 1.1,
        run: 3.5,
        rise: 2.87,
        steps: 16,
        tread: Finish::of("decking"),
        closed: false,
        railing: Railing::Both,
    });

    for (key, at, turn) in [
        ("sectional_sofa", [8.9, 9.6], PI),
        ("coffee_table_marble", [8.9, 8.2], 0.0),
        ("tv_unit", [9.2, 4.95], 0.0),
        ("tv", [9.2, 4.75], 0.0),
        ("bed_queen", [13.1, 7.15], 0.0),
        ("bedside_table", [12.0, 6.35], 0.0),
        ("bedside_table", [14.2, 6.35], 0.0),
        ("wardrobe", [11.6, 9.6], -FRAC_PI_2),
        ("toilet", [14.5, 3.85], 0.0),
        ("vanity", [13.2, 3.75], 0.0),
        ("shower", [14.45, 5.45], PI),
        ("water_heater", [6.85, 0.6], 0.0),
        ("hvac_unit", [7.9, 0.7], 0.0),
    ] {
        let f = b.furniture(key, at, turn);
        lower.furniture.push(f);
    }
    let rec_south_id = lower.walls[4].id;
    let spine_id = lower.walls[5].id;
    for mount in [
        Mount::Wall { wall: rec_south_id, side: WallSide::Left, at: 1.2, height: 0.3 },
        Mount::Wall { wall: rec_south_id, side: WallSide::Left, at: 4.6, height: 0.3 },
        Mount::Wall { wall: spine_id, side: WallSide::Right, at: 6.2, height: 0.3 },
    ] {
        let f = b.fixture(FixtureKind::Outlet { gangs: 2 }, mount);
        lower.fixtures.push(f);
    }
    let f = b.fixture(FixtureKind::Switch { gangs: 2 }, Mount::Wall { wall: rec_south_id, side: WallSide::Left, at: 4.25, height: 1.1 });
    lower.fixtures.push(f);
    let f = b.fixture(FixtureKind::Vent { width: 0.6, height: 0.4 }, Mount::Wall { wall: rec_south_id, side: WallSide::Left, at: 2.3, height: 0.35 });
    lower.fixtures.push(f);
    for at in [[8.0, 6.3], [10.2, 6.3], [8.0, 9.6], [10.2, 9.6], [13.1, 8.5]] {
        let f = b.fixture(FixtureKind::Downlight { diameter: 0.1 }, Mount::Ceiling { at, turn: 0.0 });
        lower.fixtures.push(f);
    }
    for at in [[9.0, 7.9], [13.1, 9.9]] {
        let f = b.fixture(FixtureKind::SmokeDetector, Mount::Ceiling { at, turn: 0.0 });
        lower.fixtures.push(f);
    }

    // -----------------------------------------------------------------------------------------
    // Main level: garage on the left, the house on the right, at street level.
    let mut main = Level::new(b.p.id(), "Main", 0.0, 2.7);
    let garage_paint = || Finish::paint(0xe2e0da);

    // Outside walls, counter-clockwise, so the left face is always the inside.
    let mut front_garage = b.wall([0.0, 0.0], [6.2, 0.0], 0.2, garage_paint(), siding());
    let garage_door = GarageDoor { style: GarageStyle::Sectional, inside: WallSide::Left, open: 0.0, panels: 4, finish: Finish::paint(0xeceae4) };
    front_garage.openings.push(b.opening(3.1, 4.9, 2.13, 0.0, OpeningKind::Garage(garage_door)));
    let mut front_house = b.wall([6.2, 0.0], [15.0, 0.0], 0.2, inside(), siding());
    let front_door = Door { style: DoorStyle::Hinged, hinge: Hinge::Start, swing: WallSide::Left, open: 0.0, leaf: Finish::colored("oak", rgb(0x7a4a2c)), frame: Some(Frame::default()), glazing: 0.0 };
    front_house.openings.push(b.opening(1.15, 0.92, 2.1, 0.0, OpeningKind::Door(front_door)));
    let mut picture = b.window(5.5, 2.6, 1.5, 0.75, WindowStyle::Fixed);
    if let OpeningKind::Window(w) = &mut picture.kind {
        w.cols = 3;
    }
    front_house.openings.push(picture);
    let mut east = b.wall([15.0, 0.0], [15.0, 11.0], 0.2, inside(), siding());
    east.openings.push(b.window(2.6, 0.9, 1.5, 0.75, WindowStyle::DoubleHung));
    east.openings.push(b.window(8.2, 1.1, 1.2, 1.0, WindowStyle::Casement));
    let mut back_house = b.wall([15.0, 11.0], [6.2, 11.0], 0.2, inside(), siding());
    back_house.openings.push(b.opening(2.4, 2.4, 2.1, 0.0, OpeningKind::Door(patio_door)));
    back_house.openings.push(b.window(5.9, 1.8, 1.0, 1.05, WindowStyle::Sliding));
    let mut back_garage = b.wall([6.2, 11.0], [0.0, 11.0], 0.2, inside(), siding());
    back_garage.base = -3.1;
    back_garage.openings.push(b.window(1.7, 0.9, 0.6, 1.3, WindowStyle::Awning));
    let mut west = b.wall([0.0, 11.0], [0.0, 0.0], 0.2, garage_paint(), siding());
    west.base = -3.1;
    west.openings.push(b.window(7.6, 1.2, 0.6, 1.4, WindowStyle::Louvre));

    // Inside.
    let mut garage_east = b.wall([6.2, 0.0], [6.2, 6.4], 0.115, garage_paint(), inside());
    let mut garage_back = b.wall([0.0, 6.4], [6.2, 6.4], 0.115, inside(), garage_paint());
    garage_back.openings.push(b.door(1.5, Hinge::Start, WallSide::Left));
    garage_back.openings.push(b.door(4.6, Hinge::End, WallSide::Left));
    let mut rear_east = b.wall([6.2, 6.4], [6.2, 11.0], 0.115, inside(), inside());
    rear_east.openings.push(b.door(3.5, Hinge::Start, WallSide::Left));
    let store = b.wall([3.0, 6.4], [3.0, 11.0], 0.115, inside(), inside());
    let mut hall_east = b.wall([8.4, 0.0], [8.4, 4.6], 0.115, inside(), inside());
    hall_east.openings.push(b.opening(2.5, 1.8, 2.2, 0.0, OpeningKind::Passage { frame: Some(Frame::default()) }));
    hall_east.openings.push(b.opening(0.6, 0.5, 0.9, 1.0, OpeningKind::Recess { side: WallSide::Left, depth: 0.08, finish: Finish::of("oak") }));
    let mut stair_east = b.wall([7.4, 4.6], [7.4, 8.6], 0.115, inside(), inside());
    // A tiled splashback between the bench and the wall cabinets.
    stair_east.bands.push(Band { side: WallSide::Right, from: 0.9, to: 1.45, finish: Finish::colored("subway_tile", rgb(0xdfe9e6)) });
    let stair_north = b.wall([6.2, 8.6], [7.4, 8.6], 0.115, inside(), inside());
    let mut pantry_east = b.wall([7.4, 8.6], [7.4, 11.0], 0.115, inside(), inside());
    pantry_east.openings.push(b.door(1.2, Hinge::End, WallSide::Right));
    garage_east.height = None;

    let (hall_east_id, stair_east_id, garage_back_id, east_id, front_house_id) = (hall_east.id, stair_east.id, garage_back.id, east.id, front_house.id);
    main.walls = vec![
        front_garage, front_house, east, back_house, back_garage, west, garage_east, garage_back, rear_east, store, hall_east, stair_east, stair_north, pantry_east,
    ];

    let mut garage = b.room("Garage", &rect(0.0, 0.0, 6.2, 6.4), Finish::of("concrete"));
    garage.floor_offset = -0.12;
    main.rooms.push(garage);
    let mut hall = b.room("Entry hall", &rect(6.2, 0.0, 8.4, 4.6), Finish::colored("tile", rgb(0xd8d3c8)));
    hall.ceiling_height = Some(2.4);
    main.rooms.push(hall);
    let mut living = b.room("Living room", &rect(8.4, 0.0, 15.0, 5.2), Finish::of("oak_boards"));
    living.target_lux = Some(150.0);
    // The living room's ceiling follows the roof up toward the middle of the house.
    living.ceiling_shape = CeilingShape::Sloped { rise: 1.6, toward: FRAC_PI_2 };
    main.rooms.push(living);
    let mut kitchen = b.room("Kitchen & dining", &[[7.4, 4.6], [8.4, 4.6], [8.4, 5.2], [15.0, 5.2], [15.0, 11.0], [7.4, 11.0]], Finish::of("laminate_boards"));
    kitchen.target_lux = Some(300.0);
    main.rooms.push(kitchen);
    main.rooms.push(b.room("Pantry", &rect(6.2, 8.6, 7.4, 11.0), Finish::of("laminate_boards")));
    main.rooms.push(b.room("Laundry", &rect(3.0, 6.4, 6.2, 11.0), Finish::colored("tile", rgb(0xcfd4d6))));
    main.rooms.push(b.room("Workshop", &rect(0.0, 6.4, 3.0, 11.0), Finish::of("polished_concrete")));
    let mut deck = b.room("Deck", &rect(8.6, 11.0, 15.0, 14.2), Finish::of("decking"));
    deck.has_ceiling = false;
    deck.floor_offset = -0.03;
    deck.balustrade = Some(1.0);
    main.rooms.push(deck);
    // Skirting and cornices in the finished rooms.
    for room in main.rooms.iter_mut().filter(|r| ["Entry hall", "Living room", "Kitchen & dining", "Pantry"].contains(&r.name.as_str())) {
        room.skirting = Some(Trim::skirting());
        room.cornice = Some(Trim::cornice());
    }

    for (key, at, turn) in [
        // Garage.
        ("car", [1.65, 3.1], 0.0),
        ("car", [4.5, 3.1], 0.0),
        // Living room.
        ("rug", [11.6, 2.5], 0.0),
        ("sofa", [11.6, 4.2], PI),
        ("armchair", [9.5, 2.5], FRAC_PI_2),
        ("armchair", [13.8, 2.5], -FRAC_PI_2),
        ("coffee_table_glass", [11.6, 2.5], 0.0),
        ("tv_unit", [11.6, 0.45], 0.0),
        ("tv", [11.6, 0.25], 0.0),
        ("beam", [11.7, 1.7], 0.0),
        ("beam", [11.7, 3.5], 0.0),
        // Kitchen along the stair wall, island in front of it.
        ("fridge", [7.85, 5.2], FRAC_PI_2),
        ("base_cabinet", [7.75, 6.05], FRAC_PI_2),
        ("range", [7.8, 6.85], FRAC_PI_2),
        ("microwave_otr", [7.68, 6.85], FRAC_PI_2),
        ("base_cabinet", [7.75, 7.65], FRAC_PI_2),
        ("wall_cabinet", [7.65, 6.05], FRAC_PI_2),
        ("wall_cabinet", [7.65, 7.65], FRAC_PI_2),
        ("coffee_machine", [7.75, 7.65], FRAC_PI_2),
        ("sink_cabinet", [9.3, 10.6], PI),
        ("dishwasher", [10.2, 10.6], PI),
        ("kitchen_island", [9.9, 7.0], FRAC_PI_2),
        ("bar_stool", [10.75, 6.3], -FRAC_PI_2),
        ("bar_stool", [10.75, 7.0], -FRAC_PI_2),
        ("bar_stool", [10.75, 7.7], -FRAC_PI_2),
        // Dining.
        ("dining_table", [13.0, 8.3], FRAC_PI_2),
        ("dining_chair", [12.25, 7.6], -FRAC_PI_2),
        ("dining_chair", [12.25, 8.3], -FRAC_PI_2),
        ("dining_chair", [12.25, 9.0], -FRAC_PI_2),
        ("dining_chair", [13.75, 7.6], FRAC_PI_2),
        ("dining_chair", [13.75, 8.3], FRAC_PI_2),
        ("dining_chair", [13.75, 9.0], FRAC_PI_2),
        // Pantry and laundry.
        ("tall_cabinet", [6.55, 9.1], -FRAC_PI_2),
        ("wash_tower", [3.5, 10.45], PI),
        ("base_cabinet", [4.4, 10.6], PI),
    ] {
        let f = b.furniture(key, at, turn);
        main.furniture.push(f);
    }

    // Power, switches, air.
    for mount in [
        Mount::Wall { wall: front_house_id, side: WallSide::Left, at: 3.2, height: 0.3 },
        Mount::Wall { wall: front_house_id, side: WallSide::Left, at: 7.6, height: 0.3 },
        Mount::Wall { wall: east_id, side: WallSide::Left, at: 1.2, height: 0.3 },
        Mount::Wall { wall: east_id, side: WallSide::Left, at: 4.6, height: 0.3 },
        Mount::Wall { wall: east_id, side: WallSide::Left, at: 9.6, height: 0.3 },
        Mount::Wall { wall: stair_east_id, side: WallSide::Right, at: 1.4, height: 1.1 },
        Mount::Wall { wall: stair_east_id, side: WallSide::Right, at: 3.5, height: 1.1 },
        Mount::Wall { wall: garage_back_id, side: WallSide::Right, at: 3.0, height: 1.2 },
    ] {
        let f = b.fixture(FixtureKind::Outlet { gangs: 2 }, mount);
        main.fixtures.push(f);
    }
    for (mount, gangs) in [
        (Mount::Wall { wall: front_house_id, side: WallSide::Left, at: 1.85, height: 1.1 }, 3),
        (Mount::Wall { wall: hall_east_id, side: WallSide::Right, at: 3.65, height: 1.1 }, 2),
        (Mount::Wall { wall: garage_back_id, side: WallSide::Right, at: 5.3, height: 1.1 }, 1),
    ] {
        let f = b.fixture(FixtureKind::Switch { gangs }, mount);
        main.fixtures.push(f);
    }
    let f = b.fixture(FixtureKind::Thermostat, Mount::Wall { wall: hall_east_id, side: WallSide::Left, at: 4.1, height: 1.5 });
    main.fixtures.push(f);
    let f = b.fixture(FixtureKind::Vent { width: 0.6, height: 0.4 }, Mount::Wall { wall: hall_east_id, side: WallSide::Left, at: 1.1, height: 0.35 });
    main.fixtures.push(f);
    let f = b.fixture(FixtureKind::Data, Mount::Wall { wall: front_house_id, side: WallSide::Left, at: 5.9, height: 0.3 });
    main.fixtures.push(f);
    for at in [[9.8, 1.3], [11.7, 1.3], [13.6, 1.3], [9.8, 3.9], [11.7, 3.9], [13.6, 3.9]] {
        let f = b.fixture(FixtureKind::Downlight { diameter: 0.1 }, Mount::Ceiling { at, turn: 0.0 });
        main.fixtures.push(f);
    }
    for (at, turn) in [([7.3, 1.2], FRAC_PI_2), ([7.3, 3.3], FRAC_PI_2), ([9.9, 9.3], 0.0), ([8.6, 7.0], FRAC_PI_2)] {
        let f = b.fixture(FixtureKind::TubeLight { length: 1.2, width: 0.09 }, Mount::Ceiling { at, turn });
        main.fixtures.push(f);
    }
    for at in [[9.9, 6.3], [9.9, 7.0], [9.9, 7.7]] {
        let f = b.fixture(FixtureKind::Pendant { diameter: 0.22, drop: 0.9 }, Mount::Ceiling { at, turn: 0.0 });
        main.fixtures.push(f);
    }
    let f = b.fixture(FixtureKind::Pendant { diameter: 0.5, drop: 1.0 }, Mount::Ceiling { at: [13.0, 8.3], turn: 0.0 });
    main.fixtures.push(f);
    for at in [[7.3, 2.3], [11.0, 6.0], [3.1, 3.2]] {
        let f = b.fixture(FixtureKind::SmokeDetector, Mount::Ceiling { at, turn: 0.0 });
        main.fixtures.push(f);
    }
    for at in [[1.65, 3.2], [4.5, 3.2]] {
        let f = b.fixture(FixtureKind::TubeLight { length: 1.5, width: 0.12 }, Mount::Ceiling { at, turn: FRAC_PI_2 });
        main.fixtures.push(f);
    }

    // An LED strip under the wall cabinets, and switches wired to their lights.
    let strip = b.fixture(FixtureKind::Strip { length: 2.4 }, Mount::Wall { wall: stair_east_id, side: WallSide::Right, at: 2.25, height: 1.43 });
    let strip_id = strip.id;
    main.fixtures.push(strip);
    let lights = |main: &Level, pick: &dyn Fn(&Fixture) -> bool| -> Vec<Id> { main.fixtures.iter().filter(|f| f.lamp().is_some() && pick(f)).map(|f| f.id).collect() };
    let at_of = |f: &Fixture| match f.mount {
        Mount::Ceiling { at, .. } | Mount::Floor { at, .. } => at,
        Mount::Wall { .. } => [7.5, 6.5],
    };
    let living = lights(&main, &|f| at_of(f)[0] > 8.4 && at_of(f)[1] < 5.2);
    let hall = lights(&main, &|f| at_of(f)[0] > 6.2 && at_of(f)[0] < 8.4 && at_of(f)[1] < 4.6);
    let kitchen = lights(&main, &|f| f.id == strip_id || (at_of(f)[0] > 7.4 && at_of(f)[1] > 5.2));
    let garage = lights(&main, &|f| at_of(f)[0] < 6.2 && at_of(f)[1] < 6.4);
    let mut switches = main.fixtures.iter_mut().filter(|f| matches!(f.kind, FixtureKind::Switch { .. }));
    for (controls, label) in [(living, "Living room lights"), (kitchen, "Kitchen & dining lights"), (garage, "Garage lights")] {
        if let Some(sw) = switches.next() {
            sw.controls = controls;
            sw.label = label.into();
        }
    }
    drop(switches);
    let id = b.p.id();
    main.fixtures.push(Fixture { controls: hall, label: "Hall lights".into(), ..Fixture::new(id, FixtureKind::Switch { gangs: 1 }, Mount::Wall { wall: hall_east_id, side: WallSide::Left, at: 3.75, height: 1.1 }) });

    // What the walls are made of: clad, insulated studs outside; stud partitions inside;
    // lined concrete below ground.
    let presets = Wall::presets();
    let build = |name: &str| presets.iter().find(|(n, _)| *n == name).map(|(_, l)| l.clone()).unwrap_or_default();
    for w in &mut main.walls {
        w.set_layers(build(if w.thickness > 0.15 { "Insulated stud, clad" } else { "Stud partition" }));
    }
    for w in &mut lower.walls {
        w.set_layers(build(if w.thickness > 0.15 { "Concrete, lined" } else { "Stud partition" }));
    }

    b.p.levels = vec![lower, main];

    // -----------------------------------------------------------------------------------------
    // Roof and site.
    let id = b.p.id();
    b.p.roofs.push(Roof {
        id,
        name: "Main roof".into(),
        shape: RoofShape::Gable,
        center: [7.5, 5.5],
        size: [15.0, 11.0],
        turn: 0.0,
        base: 2.7,
        pitch: 0.42,
        overhang: 0.5,
        thickness: 0.2,
        finish: Finish::colored("shingle", rgb(0x45474c)),
        gable: siding(),
        visible: true,
    });

    let patches: [(&str, Vec<P2>, Finish); 6] = [
        ("Street", rect(-9.0, -13.0, 24.0, -9.2).to_vec(), Finish::of("asphalt")),
        ("Driveway", vec![[0.5, -0.1], [5.7, -0.1], [6.3, -9.2], [-0.1, -9.2]], Finish::of("concrete")),
        ("Front path", vec![[6.85, -0.1], [7.85, -0.1], [7.85, -4.6], [6.3, -4.6], [6.3, -3.6], [6.85, -3.6]], Finish::colored("pavers", rgb(0xb3a999))),
        ("Garden bed", rect(8.5, -1.3, 14.9, -0.15).to_vec(), Finish::of("soil")),
        ("Patio", rect(6.2, 11.15, 15.2, 14.6).to_vec(), Finish::colored("pavers", rgb(0xa9a196))),
        ("Side path", rect(15.3, 0.5, 16.3, 11.0).to_vec(), Finish::of("gravel")),
    ];
    for (name, outline, finish) in patches {
        let id = b.p.id();
        b.p.site.patches.push(Patch { id, name: name.into(), outline, finish });
    }
    // A fence round the back garden with a gate across the side path, a low retaining wall
    // where the ground drops beside the house, and some planting.
    for (kind, path) in [
        (BarrierKind::Fence, vec![[16.3, 0.5], [22.5, 0.5], [22.5, 26.0], [-7.5, 26.0], [-7.5, 3.0], [-0.1, 3.0]]),
        (BarrierKind::Gate, vec![[15.25, 0.5], [16.3, 0.5]]),
        (BarrierKind::Retaining, vec![[-1.2, 5.5], [-1.2, 12.5]]),
    ] {
        let id = b.p.id();
        b.p.site.barriers.push(kind.new(id, path));
    }
    for (kind, at) in [
        (PlantKind::Tree, [-4.5, -5.0]),
        (PlantKind::Tree, [19.5, -4.5]),
        (PlantKind::Tree, [18.5, 20.5]),
        (PlantKind::Conifer, [-4.5, 21.0]),
        (PlantKind::Conifer, [-2.0, 23.0]),
        (PlantKind::Shrub, [9.6, -0.75]),
        (PlantKind::Shrub, [11.7, -0.75]),
        (PlantKind::Shrub, [13.8, -0.75]),
    ] {
        let id = b.p.id();
        b.p.site.plants.push(kind.new(id, at));
    }
    let _ = render;
    b.p
}

/// An empty project: one level on flat ground.
pub fn blank() -> Project {
    let mut p = Project {
        name: "Untitled".into(),
        units: Units::Metric,
        site: Site { terrain: Terrain::flat([-15.0, -15.0], [30.0, 30.0], 1.0), patches: vec![], geo: None, north: 0.0, sun: SunSetting::default(), barriers: vec![], plants: vec![] },
        levels: vec![],
        roofs: vec![],
        materials: vec![],
        next_id: 0,
        products: vec![],
        option: "Existing".into(),
        others: vec![],
    };
    let id = p.id();
    p.levels.push(Level::new(id, "Ground", 0.0, 2.7));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_house_round_trips_through_json_and_ids_are_unique() {
        let p = house();
        let json = serde_json::to_string_pretty(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        let mut ids: Vec<Id> = Vec::new();
        for l in &p.levels {
            ids.push(l.id);
            ids.extend(l.walls.iter().flat_map(|w| std::iter::once(w.id).chain(w.openings.iter().map(|o| o.id))));
            ids.extend(l.rooms.iter().map(|r| r.id));
            ids.extend(l.fixtures.iter().map(|f| f.id));
            ids.extend(l.furniture.iter().map(|f| f.id));
        }
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn the_lot_is_level_at_the_street_and_a_storey_lower_at_the_back() {
        let p = house();
        assert!(p.site.terrain.height([7.0, -3.0]).abs() < 0.05);
        assert!(p.site.terrain.height([10.0, 12.0]) < -2.8);
    }
}
