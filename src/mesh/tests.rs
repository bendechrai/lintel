use std::f32::consts::FRAC_PI_2;

use super::geom::{area, triangulate};
use super::*;
use crate::catalog;
use crate::model::*;
use crate::scene::{Vertex, len};

fn wall(id: Id, a: P2, b: P2) -> Wall {
    Wall { id, a, b, thickness: 0.2, height: None, base: 0.0, left: Finish::paint(0xffffff), right: Finish::of("brick"), openings: vec![], bands: vec![], layers: vec![] }
}

fn door(style: DoorStyle, open: f32) -> OpeningKind {
    OpeningKind::Door(Door { style, hinge: Hinge::Start, swing: WallSide::Left, open, leaf: Finish::of("oak"), frame: Some(Frame::default()), glazing: 0.5 })
}

/// A 6 × 4 m room on ground that falls to the north, with one of everything in it.
fn house() -> Project {
    let mut level = Level::new(1, "Ground", 0.0, 2.6);
    let mut south = wall(10, [0.0, 0.0], [6.0, 0.0]);
    south.openings = vec![
        Opening { id: 20, at: 1.5, width: 0.9, height: 2.1, sill: 0.0, kind: door(DoorStyle::Hinged, 0.5) },
        Opening { id: 21, at: 4.0, width: 1.2, height: 1.2, sill: 0.9, kind: OpeningKind::Window(Window::new(WindowStyle::DoubleHung)) },
    ];
    let mut east = wall(11, [6.0, 0.0], [6.0, 4.0]);
    east.openings = vec![
        Opening { id: 22, at: 2.0, width: 2.6, height: 2.1, sill: 0.0, kind: OpeningKind::Garage(GarageDoor { style: GarageStyle::Sectional, inside: WallSide::Left, open: 0.4, panels: 4, finish: Finish::paint(0xeeeeee) }) },
    ];
    let mut north = wall(12, [6.0, 4.0], [0.0, 4.0]);
    north.openings = vec![
        Opening { id: 23, at: 1.0, width: 0.8, height: 1.0, sill: 1.0, kind: OpeningKind::Recess { side: WallSide::Left, depth: 0.1, finish: Finish::of("oak") } },
        Opening { id: 24, at: 3.0, width: 1.0, height: 2.1, sill: 0.0, kind: OpeningKind::Passage { frame: Some(Frame::default()) } },
        Opening { id: 25, at: 5.0, width: 0.8, height: 2.0, sill: 0.0, kind: door(DoorStyle::Bifold, 0.7) },
    ];
    let mut west = wall(13, [0.0, 4.0], [0.0, 0.0]);
    let styles = [WindowStyle::Fixed, WindowStyle::Casement, WindowStyle::Awning, WindowStyle::Sliding, WindowStyle::Louvre];
    west.openings = styles
        .iter()
        .enumerate()
        .map(|(i, style)| Opening { id: 30 + i as Id, at: 0.4 + i as f32 * 0.8, width: 0.6, height: 1.0, sill: 1.0, kind: OpeningKind::Window(Window { cols: 2, rows: 2, casing: (i == 0).then(Frame::default), ..Window::new(*style) }) })
        .collect();
    // A free-standing nib with the other door and garage styles along it.
    let mut nib = wall(14, [1.0, 2.0], [5.5, 2.0]);
    nib.openings = vec![
        Opening { id: 40, at: 0.6, width: 0.8, height: 2.0, sill: 0.0, kind: door(DoorStyle::Double, 1.0) },
        Opening { id: 41, at: 1.6, width: 0.8, height: 2.0, sill: 0.0, kind: door(DoorStyle::Sliding, 0.3) },
        Opening { id: 42, at: 2.6, width: 0.8, height: 2.0, sill: 0.0, kind: door(DoorStyle::Pocket, 0.5) },
        Opening { id: 43, at: 3.5, width: 0.7, height: 2.0, sill: 0.0, kind: OpeningKind::Garage(GarageDoor { style: GarageStyle::Roller, inside: WallSide::Right, open: 0.5, panels: 4, finish: Finish::of("roller_door") }) },
        Opening { id: 44, at: 4.2, width: 0.5, height: 2.0, sill: 0.0, kind: OpeningKind::Garage(GarageDoor { style: GarageStyle::Tilt, inside: WallSide::Left, open: 0.5, panels: 1, finish: Finish::paint(0xcccccc) }) },
    ];
    level.walls = vec![south, east, north, west, nib];
    level.rooms = vec![Room { id: 50, name: "Room".into(), outline: vec![[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]], floor: Finish::of("oak_boards"), ceiling: Finish::paint(0xffffff), ceiling_height: None, floor_offset: 0.0, has_ceiling: true, balustrade: None, ceiling_shape: CeilingShape::Flat, skirting: None, cornice: None, target_lux: None }];
    level.stairs = vec![
        Stair { id: 60, at: [1.0, 1.0], direction: 0.0, width: 0.9, run: 3.0, rise: 2.9, steps: 14, tread: Finish::of("oak"), closed: true, railing: Railing::Both },
        Stair { id: 61, at: [1.0, 3.0], direction: 0.3, width: 0.9, run: 3.0, rise: 2.9, steps: 14, tread: Finish::of("oak"), closed: false, railing: Railing::Left },
    ];
    let kinds = [
        FixtureKind::Outlet { gangs: 2 },
        FixtureKind::Switch { gangs: 3 },
        FixtureKind::SmokeDetector,
        FixtureKind::Downlight { diameter: 0.1 },
        FixtureKind::TubeLight { length: 1.2, width: 0.05 },
        FixtureKind::Surface { diameter: 0.3 },
        FixtureKind::Pendant { diameter: 0.4, drop: 0.8 },
        FixtureKind::Vent { width: 0.5, height: 0.3 },
        FixtureKind::Thermostat,
        FixtureKind::Data,
    ];
    for (i, kind) in kinds.iter().enumerate() {
        let at = [0.5 + i as f32 * 0.5, 1.0];
        for (j, mount) in [Mount::Wall { wall: 10, side: WallSide::Left, at: at[0], height: 1.1 }, Mount::Ceiling { at, turn: 0.4 }, Mount::Floor { at, turn: 0.0 }].into_iter().enumerate() {
            level.fixtures.push(Fixture::new(100 + (i * 3 + j) as Id, *kind, mount));
        }
    }
    // A fixture on a wall that isn't there is skipped.
    level.fixtures.push(Fixture::new(199, FixtureKind::Data, Mount::Wall { wall: 999, side: WallSide::Left, at: 0.0, height: 1.0 }));
    for (i, piece) in catalog::catalog().iter().enumerate() {
        let mut f = catalog::place(piece.key, 200 + i as Id, [3.0, 2.0]).unwrap();
        f.turn = i as f32 * 0.3;
        level.furniture.push(f);
    }
    level.furniture.push(Furniture { id: 299, kind: "no such thing".into(), name: String::new(), at: [1.0, 1.0], turn: 0.0, size: [1.0, 1.0, 1.0], lift: 0.0, finish: Finish::of("oak"), accent: Finish::of("oak"), hung: false });

    let mut upper = Level::new(2, "Upper", 2.9, 2.4);
    upper.walls = vec![wall(70, [0.0, 0.0], [6.0, 0.0])];
    upper.rooms = vec![Room { id: 71, name: "Loft".into(), outline: vec![[0.0, 4.0], [6.0, 4.0], [6.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]], floor: Finish::of("carpet"), ceiling: Finish::paint(0xffffff), ceiling_height: Some(2.2), floor_offset: 0.0, has_ceiling: true, balustrade: None, ceiling_shape: CeilingShape::Flat, skirting: None, cornice: None, target_lux: None }];

    let roof = |id: Id, shape: RoofShape, center: P2| Roof { id, name: "Roof".into(), shape, center, size: [7.0, 4.5], turn: 0.2, base: 5.3, pitch: 0.5, overhang: 0.4, thickness: 0.2, finish: Finish::of("shingle"), gable: Finish::of("siding"), visible: true };
    Project {
        name: "Test".into(),
        units: Units::Metric,
        site: Site {
            terrain: Terrain::sloped([-10.0, -10.0], [26.0, 24.0], 2.0, FRAC_PI_2, 0.15, [0.0, 0.0]),
            patches: vec![Patch { id: 80, name: "Drive".into(), outline: vec![[1.0, -8.0], [4.0, -8.0], [4.0, -0.5], [1.0, -0.5]], finish: Finish::of("concrete") }],
            geo: None,
            north: 0.0,
            sun: SunSetting::default(),
            barriers: vec![],
            plants: vec![],
        },
        levels: vec![level, upper],
        roofs: vec![roof(90, RoofShape::Gable, [3.0, 2.0]), roof(91, RoofShape::Hip, [3.0, 2.0]), roof(92, RoofShape::Shed, [3.0, 2.0]), roof(93, RoofShape::Flat, [3.0, 2.0]), Roof { size: [3.0, 6.0], ..roof(94, RoofShape::Hip, [3.0, 2.0]) }],
        materials: vec![],
        next_id: 1000,
        products: vec![],
        option: "Existing".into(),
        others: vec![],
    }
}

fn triangles<'a>(scene: &'a Scene) -> impl Iterator<Item = [&'a Vertex; 3]> + 'a {
    scene.opaque.chunks(3).chain(scene.translucent.chunks(3)).map(|t| [&scene.vertices[t[0] as usize], &scene.vertices[t[1] as usize], &scene.vertices[t[2] as usize]])
}

fn pick_of(scene: &Scene, pick: Pick) -> f32 {
    (scene.picks.iter().position(|p| *p == pick).unwrap_or_else(|| panic!("{pick:?} was never drawn")) + 1) as f32
}

fn tri_area(t: &[P2; 3]) -> f32 {
    area(t)
}

#[test]
fn triangulation_covers_the_outline_whichever_way_it_winds() {
    let square = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
    let ell = [[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 3.0], [0.0, 3.0]];
    let comb = [[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [4.0, 3.0], [4.0, 1.0], [3.0, 1.0], [3.0, 3.0], [2.0, 3.0], [2.0, 1.0], [1.0, 1.0], [1.0, 3.0], [0.0, 3.0]];
    // Extra points along edges, and the first point repeated at the end.
    let fussy = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 2.0], [1.0, 2.0], [0.0, 2.0], [0.0, 1.0], [0.0, 0.0]];
    for outline in [&square[..], &ell[..], &comb[..], &fussy[..]] {
        let want = area(outline).abs();
        for pts in [outline.to_vec(), outline.iter().rev().copied().collect::<Vec<_>>()] {
            let tris = triangulate(&pts);
            assert!(tris.iter().all(|t| tri_area(t) > 0.0), "every triangle is counter-clockwise");
            let got: f32 = tris.iter().map(tri_area).sum();
            assert!((got - want).abs() < 1e-4, "area {got} of {want} for {pts:?}");
        }
    }
    assert!(triangulate(&[[0.0, 0.0], [1.0, 0.0]]).is_empty());
    assert!(triangulate(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]).is_empty());
}

#[test]
fn a_whole_house_is_sound() {
    let project = house();
    let scene = build(&project, &BuildOptions::default());
    assert!(scene.opaque.len() % 3 == 0 && scene.translucent.len() % 3 == 0);
    assert!(!scene.translucent.is_empty(), "windows have glass");
    let n = scene.vertices.len() as u32;
    assert!(scene.opaque.iter().chain(&scene.translucent).all(|i| *i < n));
    for v in &scene.vertices {
        assert!(v.pos.iter().chain(&v.normal).chain(&v.color).chain(&v.uv).all(|x| x.is_finite()), "{v:?}");
        assert!((len(v.normal) - 1.0).abs() < 1e-3, "normal {:?}", v.normal);
        assert!(v.pick >= 0.0 && v.pick as usize <= scene.picks.len());
    }
    // No slivers with no area worth the name, and the normals agree with the winding.
    for t in triangles(&scene) {
        let n = crate::scene::cross(crate::scene::sub(t[1].pos, t[0].pos), crate::scene::sub(t[2].pos, t[0].pos));
        if len(n) > 1e-7 && t[0].normal == t[1].normal && t[1].normal == t[2].normal {
            assert!(crate::scene::dot(n, t[0].normal) > 0.0, "a face is wound against its normal: {t:?}");
        }
    }
    // Everything the model holds can be picked.
    let level = &project.levels[0];
    for w in &level.walls {
        pick_of(&scene, Pick::Wall(w.id));
        for o in &w.openings {
            pick_of(&scene, Pick::Opening(w.id, o.id));
        }
    }
    for f in level.fixtures.iter().filter(|f| f.id != 199) {
        pick_of(&scene, Pick::Fixture(f.id));
    }
    for f in &level.furniture {
        pick_of(&scene, Pick::Furniture(f.id));
    }
    for pick in [Pick::Room(50), Pick::Room(71), Pick::Stair(60), Pick::Stair(61), Pick::Patch(80), Pick::Roof(90), Pick::Roof(94)] {
        pick_of(&scene, pick);
    }
    assert!(!scene.picks.contains(&Pick::Fixture(199)));
    // The bounds are the building's, not the 26 m site's.
    let (lo, hi) = scene.bounds;
    assert!(lo[0] > -2.5 && lo[1] > -2.5 && hi[0] < 8.5 && hi[1] < 6.5, "{lo:?} {hi:?}");
    assert!(lo[2] <= -0.29 && hi[2] > 5.3, "{lo:?} {hi:?}");
}

/// Whether any triangle of `pick` lying in the plane `y = plane` covers the point (x, z).
fn face_covers(scene: &Scene, pick: f32, plane: f32, x: f32, z: f32) -> usize {
    triangles(scene)
        .filter(|t| t.iter().all(|v| v.pick == pick && (v.pos[1] - plane).abs() < 1e-4))
        .filter(|t| {
            let p = |i: usize| [t[i].pos[0], t[i].pos[2]];
            let side = |a: P2, b: P2| (b[0] - a[0]) * (z - a[1]) - (b[1] - a[1]) * (x - a[0]);
            let (d0, d1, d2) = (side(p(0), p(1)), side(p(1), p(2)), side(p(2), p(0)));
            (d0 > 0.0 && d1 > 0.0 && d2 > 0.0) || (d0 < 0.0 && d1 < 0.0 && d2 < 0.0)
        })
        .count()
}

#[test]
fn openings_are_cut_out_of_both_faces_of_a_wall() {
    let project = house();
    let scene = build(&project, &BuildOptions { furniture: false, fixtures: false, ..Default::default() });
    let pick = pick_of(&scene, Pick::Wall(10));
    for plane in [0.1, -0.1] {
        // Solid wall, beside and between the openings, under the window and over the door.
        for (x, z) in [(0.5, 1.0), (2.7, 1.3), (5.5, 2.5), (4.2, 0.3), (4.2, 2.4), (1.6, 2.35)] {
            assert_eq!(face_covers(&scene, pick, plane, x, z), 1, "wall face at {x}, {z} on y = {plane}");
        }
        // The door (0.9 wide at 1.5) and the window (1.2 wide at 4.0, sill 0.9, head 2.1).
        for (x, z) in [(1.5, 1.0), (1.1, 0.05), (1.9, 2.05), (4.0, 1.5), (3.45, 0.95), (4.55, 2.05)] {
            assert_eq!(face_covers(&scene, pick, plane, x, z), 0, "opening at {x}, {z} on y = {plane}");
        }
    }
    // A recess is cut from its own face only, and has a back.
    let north = pick_of(&scene, Pick::Wall(12));
    assert_eq!(face_covers(&scene, north, 3.9, 5.1, 1.3), 0, "the recess is open on the wall's left face");
    assert_eq!(face_covers(&scene, north, 4.1, 5.1, 1.3), 1, "and solid on the right");
    let recess = pick_of(&scene, Pick::Opening(12, 23));
    assert_eq!(face_covers(&scene, recess, 4.0, 5.1, 1.3), 1, "its back is 0.1 m in");
}

#[test]
fn overlapping_and_overhanging_openings_do_not_break_a_wall() {
    let mut project = house();
    let w = &mut project.levels[0].walls[0];
    let window = || OpeningKind::Window(Window::new(WindowStyle::Fixed));
    w.openings = vec![
        Opening { id: 1, at: 0.1, width: 1.0, height: 5.0, sill: -1.0, kind: window() },
        Opening { id: 2, at: 0.3, width: 1.0, height: 1.0, sill: 1.0, kind: window() },
        Opening { id: 3, at: 9.0, width: 1.0, height: 1.0, sill: 1.0, kind: window() },
        Opening { id: 4, at: 3.0, width: f32::NAN, height: 1.0, sill: 1.0, kind: window() },
        Opening { id: 5, at: 5.0, width: 0.0, height: 1.0, sill: 1.0, kind: door(DoorStyle::Hinged, 2.0) },
    ];
    let scene = build(&project, &BuildOptions::default());
    assert!(scene.vertices.iter().all(|v| v.pos.iter().all(|x| x.is_finite())));
    assert!(scene.picks.contains(&Pick::Opening(10, 1)));
    for id in 2..=5 {
        assert!(!scene.picks.contains(&Pick::Opening(10, id)), "opening {id} should be dropped");
    }
}

#[test]
fn an_empty_project_builds_to_nothing() {
    let project = Project {
        name: String::new(),
        units: Units::Metric,
        site: Site { terrain: Terrain { origin: [0.0, 0.0], spacing: 1.0, cols: 0, rows: 0, heights: vec![], finish: Finish::of("grass"), source: String::new() }, patches: vec![], geo: None, north: 0.0, sun: SunSetting::default(), barriers: vec![], plants: vec![] },
        levels: vec![],
        roofs: vec![],
        materials: vec![],
        next_id: 0,
        products: vec![],
        option: "Existing".into(),
        others: vec![],
    };
    let scene = build(&project, &BuildOptions::default());
    assert!(scene.vertices.is_empty() && scene.opaque.is_empty());
    assert_eq!(scene.bounds, ([0.0; 3], [0.0; 3]));
    assert_eq!(terrain_height(&project, [3.0, 3.0]), 0.0);
    // Degenerate things on a level are skipped rather than drawn as nonsense.
    let mut project = project;
    let mut level = Level::new(1, "L", 0.0, 2.4);
    level.walls = vec![wall(1, [1.0, 1.0], [1.0, 1.0])];
    level.rooms = vec![Room { id: 2, name: String::new(), outline: vec![[0.0, 0.0], [1.0, 1.0]], floor: Finish::of("oak"), ceiling: Finish::of("paint"), ceiling_height: None, floor_offset: 0.0, has_ceiling: true, balustrade: None, ceiling_shape: CeilingShape::Flat, skirting: None, cornice: None, target_lux: None }];
    level.stairs = vec![Stair { id: 3, at: [0.0, 0.0], direction: 0.0, width: 1.0, run: 0.0, rise: 3.0, steps: 0, tread: Finish::of("oak"), closed: true, railing: Railing::Both }];
    project.levels = vec![level];
    assert!(build(&project, &BuildOptions::default()).vertices.is_empty());
}

#[test]
fn the_ground_is_dug_out_under_rooms_and_left_alone_elsewhere() {
    let project = house();
    let ground = Ground::new(&project);
    let terrain = &project.site.terrain;
    // The lot falls to the north, so the south of the room is below natural ground.
    assert!(terrain.height([3.0, -5.0]) > 0.5);
    let slab = project.levels[0].elevation - project.levels[0].slab;
    for x in 0..=12 {
        for y in 0..=8 {
            let p = [x as f32 * 0.5, y as f32 * 0.5];
            assert!(ground.height(p) <= slab.max(terrain.height(p)) + 1e-4, "ground {} at {p:?} is above the slab", ground.height(p));
            if terrain.height(p) > slab {
                assert!(ground.height(p) < slab, "ground at {p:?} was not dug");
            }
        }
    }
    for p in [[3.0, -5.0], [-6.0, 2.0], [12.0, 9.0]] {
        assert!((ground.height(p) - terrain.height(p)).abs() < 1e-4, "ground away from the house at {p:?}");
    }
    assert_eq!(terrain_height(&project, [3.0, 2.0]), ground.height([3.0, 2.0]));
    // And the patch lies on it.
    let scene = build(&project, &BuildOptions::default());
    let pick = pick_of(&scene, Pick::Patch(80));
    let on: Vec<&Vertex> = scene.vertices.iter().filter(|v| v.pick == pick).collect();
    assert!(on.len() > 30, "the patch is subdivided to follow the ground");
    assert!(on.iter().all(|v| (v.pos[2] - ground.height([v.pos[0], v.pos[1]]) - 0.03).abs() < 1e-4));
    let cells = scene.opaque.len() / 3;
    assert!(cells < 150_000, "{cells} triangles");
}

#[test]
fn options_leave_things_out() {
    let project = house();
    let all = build(&project, &BuildOptions::default());
    let count = |o: BuildOptions| build(&project, &o).vertices.len();
    for o in [
        BuildOptions { terrain: false, ..Default::default() },
        BuildOptions { roofs: false, ..Default::default() },
        BuildOptions { ceilings: false, ..Default::default() },
        BuildOptions { fixtures: false, ..Default::default() },
        BuildOptions { furniture: false, ..Default::default() },
    ] {
        assert!(count(o.clone()) < all.vertices.len(), "{o:?}");
    }
    // Cutting at the lower level takes off the upper level, the roofs and the lower ceiling.
    let cut = build(&project, &BuildOptions { top_level: Some(1), terrain: false, ..Default::default() });
    assert!(!cut.picks.iter().any(|p| matches!(p, Pick::Roof(_) | Pick::Room(71) | Pick::Wall(70))));
    let room = pick_of(&cut, Pick::Room(50));
    assert!(cut.vertices.iter().filter(|v| v.pick == room).all(|v| v.pos[2] < 1.0), "the ceiling is off");
    // Cutting at the top level opens that one up too: no roof, no ceiling.
    let top = build(&project, &BuildOptions { top_level: Some(2), terrain: false, ..Default::default() });
    assert!(!top.picks.contains(&Pick::Roof(90)));
    let loft = pick_of(&top, Pick::Room(71));
    assert!(top.vertices.iter().any(|v| v.pick == loft), "its floor is still there");
    assert!(top.vertices.iter().filter(|v| v.pick == loft).all(|v| v.pos[2] < 5.0), "but not its ceiling at 2.2 m over a floor at 2.9");
    // A hidden level is not drawn.
    let mut hidden = project.clone();
    hidden.levels[1].visible = false;
    assert!(!build(&hidden, &BuildOptions::default()).picks.contains(&Pick::Room(71)));
}

#[test]
fn a_garage_door_rides_up_and_back_under_the_ceiling() {
    let mut project = house();
    let span = |project: &Project| {
        let scene = build(project, &BuildOptions { terrain: false, furniture: false, fixtures: false, ..Default::default() });
        let pick = pick_of(&scene, Pick::Opening(11, 22));
        let of: Vec<[f32; 3]> = scene.vertices.iter().filter(|v| v.pick == pick).map(|v| v.pos).collect();
        let fold = |i: usize, f: fn(f32, f32) -> f32, start: f32| of.iter().map(|p| p[i]).fold(start, f);
        (fold(2, f32::min, f32::MAX), fold(2, f32::max, f32::MIN), fold(0, f32::min, f32::MAX))
    };
    let set = |project: &mut Project, open: f32| {
        if let OpeningKind::Garage(g) = &mut project.levels[0].walls[1].openings[0].kind {
            g.open = open;
        }
    };
    // The wall runs north along x = 6 and its left side, the inside, is toward -x.
    set(&mut project, 0.0);
    let (lo, hi, reach) = span(&project);
    assert!(lo < 0.02 && hi < 2.2 && reach > 5.8, "closed: {lo} {hi} {reach}");
    set(&mut project, 1.0);
    let (lo, hi, reach) = span(&project);
    assert!(lo > 2.05 && hi < 2.25 && reach < 3.9, "open: {lo} {hi} {reach}");
}

#[test]
fn skirting_runs_along_a_rooms_walls_and_stops_at_doors() {
    use crate::model::Trim;
    let mut project = house();
    let plain = build(&project, &BuildOptions { terrain: false, ..Default::default() }).vertices.len();
    let level = &mut project.levels[0];
    let room = level.rooms[0].id;
    level.rooms[0].skirting = Some(Trim::skirting());
    level.rooms[0].cornice = Some(Trim::cornice());
    let scene = build(&project, &BuildOptions { terrain: false, ..Default::default() });
    assert!(scene.vertices.len() > plain, "trim adds geometry");
    // Nothing of the room's stands in a doorway at skirting height: sample the door's width
    // on the wall with the door, just above the floor and just off the face.
    let level = &project.levels[0];
    let pick = pick_of(&scene, Pick::Room(room));
    let floor = level.elevation + level.rooms[0].floor_offset;
    let low: Vec<_> = scene.vertices.iter().filter(|v| v.pick == pick && v.pos[2] > floor + 0.005 && v.pos[2] < floor + 0.2).collect();
    assert!(!low.is_empty(), "there is skirting");
    for wall in &level.walls {
        for o in wall.openings.iter().filter(|o| o.sill < 0.05) {
            let (d, _) = wall.frame();
            for v in &low {
                let along = (v.pos[0] - wall.a[0]) * d[0] + (v.pos[1] - wall.a[1]) * d[1];
                let off = crate::geom::segment_distance([v.pos[0], v.pos[1]], wall.a, wall.b);
                if off < wall.thickness / 2.0 + 0.05 {
                    assert!((along - o.at).abs() > o.width / 2.0 - 0.001, "skirting crosses a door at {along}");
                }
            }
        }
    }
    // The cornice is up at the ceiling.
    let top = floor + level.rooms[0].ceiling_height.unwrap_or(level.height);
    assert!(scene.vertices.iter().any(|v| v.pick == pick && v.pos[2] > top - 0.09 && v.pos[2] < top && (v.normal[2]).abs() < 0.5));
}

#[test]
fn a_part_height_finish_covers_its_band_of_one_face_and_goes_round_openings() {
    use crate::model::{Band, WallSide};
    let mut project = house();
    let wall = project.levels[0].walls.iter().position(|w| !w.openings.is_empty()).unwrap();
    let before = build(&project, &BuildOptions { terrain: false, ..Default::default() });
    project.levels[0].walls[wall].bands.push(Band { side: WallSide::Left, from: 0.0, to: 1.2, finish: Finish::of("slate") });
    let after = build(&project, &BuildOptions { terrain: false, ..Default::default() });
    let added = &after.vertices[..];
    let slate = crate::materials::Library::builtin().surface(&Finish::of("slate"));
    let tiles: Vec<_> = added.iter().filter(|v| v.pattern == slate.pattern as u32 as f32 && (v.color[0] - slate.color[0]).abs() < 1e-4).collect();
    assert!(after.vertices.len() > before.vertices.len() && !tiles.is_empty());
    let floor = project.levels[0].elevation;
    assert!(tiles.iter().all(|v| v.pos[2] >= floor - 1e-3 && v.pos[2] <= floor + 1.2 + 1e-3), "it stays within its heights");
    // No tile inside a door's width below its head.
    let w = &project.levels[0].walls[wall];
    let (d, _) = w.frame();
    for o in &w.openings {
        for v in &tiles {
            let along = (v.pos[0] - w.a[0]) * d[0] + (v.pos[1] - w.a[1]) * d[1];
            let z = v.pos[2] - floor;
            assert!(!((along - o.at).abs() < o.width / 2.0 - 1e-3 && z > o.sill + 1e-3 && z < (o.sill + o.height).min(1.2) - 1e-3), "tile inside an opening");
        }
    }
}

#[test]
fn a_sloped_ceiling_rises_across_the_room_and_the_walls_fill_in_under_it() {
    use crate::model::CeilingShape;
    let mut project = house();
    let options = BuildOptions { terrain: false, roofs: false, furniture: false, fixtures: false, ..Default::default() };
    let flat = build(&project, &options);
    let level = &mut project.levels[0];
    let (floor, height) = (level.elevation + level.rooms[0].floor_offset, level.rooms[0].ceiling_height.unwrap_or(level.height));
    let id = level.rooms[0].id;
    level.rooms[0].ceiling_shape = CeilingShape::Sloped { rise: 1.5, toward: 0.0 };
    let (lo, hi) = level.rooms[0].outline.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let sloped = build(&project, &options);
    let pick = pick_of(&sloped, Pick::Room(id));
    // The ceiling's underside: low on the west side, 1.5 m higher on the east.
    let under: Vec<_> = sloped.vertices.iter().filter(|v| v.pick == pick && v.normal[2] < -0.5 && v.pos[2] > floor + 1.0).collect();
    let at = |x: f32| under.iter().filter(|v| (v.pos[0] - x).abs() < 1e-3).map(|v| v.pos[2]).fold(f32::MIN, f32::max);
    assert!((at(lo) - (floor + height)).abs() < 0.02, "{}", at(lo));
    assert!((at(hi) - (floor + height + 1.5)).abs() < 0.02, "{}", at(hi));
    // Walls gained a piece above their flat tops, up to the slope and no higher.
    let top_of = |scene: &crate::scene::Scene| {
        let walls: Vec<f32> = project.levels[0].walls.iter().map(|w| pick_of(scene, Pick::Wall(w.id))).collect();
        scene.vertices.iter().filter(|v| walls.contains(&v.pick)).map(|v| v.pos[2]).fold(f32::MIN, f32::max)
    };
    let (wall_top, flat_top) = (top_of(&sloped), top_of(&flat));
    assert!(wall_top > flat_top + 1.0 && wall_top <= floor + height + 1.5 + 0.01, "{wall_top} vs {flat_top}");
    // A vaulted ceiling peaks in the middle and is back down at both sides.
    project.levels[0].rooms[0].ceiling_shape = CeilingShape::Vaulted { rise: 1.2, along: std::f32::consts::FRAC_PI_2 };
    let r = &project.levels[0].rooms[0];
    assert!(r.ceiling_rise([lo, 1.0]).abs() < 1e-4 && r.ceiling_rise([hi, 1.0]).abs() < 1e-4);
    assert!((r.ceiling_rise([(lo + hi) / 2.0, 1.0]) - 1.2).abs() < 1e-4);
    let vaulted = build(&project, &options);
    let peak = vaulted.vertices.iter().filter(|v| v.pick == pick_of(&vaulted, Pick::Room(id)) && v.normal[2] < -0.3).map(|v| v.pos[2]).fold(f32::MIN, f32::max);
    assert!((peak - (floor + height + 1.2)).abs() < 0.02, "{peak}");
}

#[test]
fn a_beam_hangs_from_the_ceiling_wherever_that_is() {
    let mut project = house();
    let level = &mut project.levels[0];
    let room = &level.rooms[0];
    let c = crate::geom::centroid(&room.outline);
    let ceiling = level.elevation + room.floor_offset + room.ceiling_height.unwrap_or(level.height);
    let beam = crate::catalog::place("beam", 900, c).unwrap();
    assert!(beam.hung);
    level.furniture.push(beam);
    let scene = build(&project, &BuildOptions { terrain: false, roofs: false, fixtures: false, ..Default::default() });
    let pick = pick_of(&scene, Pick::Furniture(900));
    let (lo, hi) = scene.vertices.iter().filter(|v| v.pick == pick).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.pos[2]), hi.max(v.pos[2])));
    assert!((hi - ceiling).abs() < 0.02 && (hi - lo - 0.24).abs() < 0.01, "{lo}..{hi} under a ceiling at {ceiling}");
}

#[test]
fn a_fence_follows_the_ground_a_retaining_wall_has_a_level_top_and_a_tree_stands_on_the_ground() {
    use crate::model::{BarrierKind, PlantKind};
    let mut project = house();
    let ground = |p: [f32; 2]| project.site.terrain.height(p);
    // A line running down the slope, clear of the building.
    let (a, b) = ([-8.0, -8.0], [-8.0, 8.0]);
    assert!((ground(a) - ground(b)).abs() > 1.0, "the test site slopes along this line");
    project.site.barriers.push(BarrierKind::Fence.new(700, vec![a, b]));
    project.site.barriers.push(BarrierKind::Retaining.new(701, vec![[-9.0, -8.0], [-9.0, 8.0]]));
    project.site.plants.push(PlantKind::Tree.new(702, [-6.0, 0.0]));
    let scene = build(&project, &BuildOptions::default());
    let of = |pick: Pick| -> Vec<[f32; 3]> { let id = pick_of(&scene, pick); scene.vertices.iter().filter(|v| v.pick == id).map(|v| v.pos).collect() };

    // The fence's top is 1.8 m over the ground at each end.
    let fence = of(Pick::Barrier(700));
    let top_near = |y: f32| fence.iter().filter(|p| (p[1] - y).abs() < 0.2).map(|p| p[2]).fold(f32::MIN, f32::max);
    assert!((top_near(a[1]) - (ground(a) + 1.86)).abs() < 0.08, "{} vs ground {}", top_near(a[1]), ground(a));
    assert!((top_near(b[1]) - (ground(b) + 1.86)).abs() < 0.08, "{} vs ground {}", top_near(b[1]), ground(b));

    // The retaining wall's top is one height all along, 0.9 m over the highest ground.
    let wall = of(Pick::Barrier(701));
    let top = wall.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
    let high = ground([-9.0, -8.0]).max(ground([-9.0, 8.0]));
    assert!((top - (high + 0.9)).abs() < 0.05, "{top} vs {high}");
    assert!(wall.iter().filter(|p| p[2] > top - 1e-3).count() >= 8, "level along its length");

    // The tree: foot on the ground, 6 m tall, 4.5 m across.
    let tree = of(Pick::Plant(702));
    let (lo, hi) = tree.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[2]), hi.max(p[2])));
    assert!((lo - ground([-6.0, 0.0])).abs() < 0.05 && (hi - lo - 6.0).abs() < 0.05, "{lo}..{hi}");
    let wide = tree.iter().map(|p| (p[0] + 6.0).abs()).fold(0.0, f32::max);
    assert!((wide - 2.25).abs() < 0.15, "{wide}");
}

#[test]
fn a_raised_deck_gets_posts_to_the_ground_and_a_balustrade_on_its_open_edges() {
    let project = crate::sample::house();
    let main = project.levels.last().unwrap();
    let deck = main.rooms.iter().find(|r| r.name == "Deck").unwrap();
    let scene = build(&project, &BuildOptions::default());
    let pick = pick_of(&scene, Pick::Room(deck.id));
    let floor = main.elevation + deck.floor_offset;
    let pts: Vec<[f32; 3]> = scene.vertices.iter().filter(|v| v.pick == pick).map(|v| v.pos).collect();
    // Posts reach well below the deck, toward ground nearly a storey down.
    let lowest = pts.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
    assert!(lowest < floor - 2.0, "{lowest}");
    // The balustrade's rail is a metre over the deck, and none of it is along the house wall.
    let rail: Vec<&[f32; 3]> = pts.iter().filter(|p| (p[2] - (floor + 1.0)).abs() < 1e-3).collect();
    assert!(!rail.is_empty());
    // Near the house there are only the ends of the two side rails, not a run along the wall.
    assert!(rail.iter().filter(|p| p[1] < 11.3).all(|p| (p[0] - 8.6).abs() < 0.3 || (p[0] - 15.0).abs() < 0.3), "a balustrade runs along the house wall");
}

#[test]
fn a_retaining_wall_holds_level_ground_behind_it_and_leaves_the_ground_in_front_alone() {
    use crate::model::{Barrier, BarrierKind};
    let mut project = house();
    let terrain = project.site.terrain.clone();
    // A wall across the fall of the site, clear of the building.
    let wall = Barrier { level_behind: 3.0, ..BarrierKind::Retaining.new(710, vec![[-8.0, -2.0], [-3.0, -2.0]]) };
    let fall = terrain.height([-5.5, -4.0]) - terrain.height([-5.5, 0.0]);
    assert!(fall.abs() > 0.2, "the test site falls across this wall: {fall}");
    // Left of a wall running east is north.
    let up = wall.uphill(&terrain);
    assert_eq!(up, if fall < 0.0 { 1.0 } else { -1.0 });
    let top = wall.level_top(&terrain);
    let before = Ground::new(&project);
    project.site.barriers.push(wall);
    let ground = Ground::new(&project);
    let at = |behind: f32| [-5.5, -2.0 + up * behind];
    // Behind it the ground is level just under the top, though that side was the higher one already.
    for d in [0.5, 1.5, 2.5] {
        let h = ground.natural_height(at(d));
        assert!((h - (top - 0.05)).abs() < 0.03 || h > top - 0.05, "{d} m behind: {h} vs top {top}");
    }
    assert!(ground.natural_height(at(1.0)) > terrain.height(at(1.0)) + 0.3, "ground was filled");
    // In front, past the bank, and beyond its ends it is as it was.
    for p in [at(-1.0), at(6.0), [-1.5, -2.0 + up], [-8.8, -2.0 + up]] {
        assert!((ground.natural_height(p) - before.natural_height(p)).abs() < 1e-4, "{p:?} changed: {} was {}, top {top}, up {up}, fill {:?}", ground.natural_height(p), before.natural_height(p), project.site.barriers.last().unwrap().fill(&terrain, p));
    }
    // The wall still stands on the ground it was built on, with the same level top.
    let scene = build(&project, &BuildOptions::default());
    let id = pick_of(&scene, Pick::Barrier(710));
    let z: Vec<f32> = scene.vertices.iter().filter(|v| v.pick == id).map(|v| v.pos[2]).collect();
    assert!((z.iter().cloned().fold(f32::MIN, f32::max) - top).abs() < 1e-3);
    // Without a width it reshapes nothing, as files saved before this had it.
    project.site.barriers[0..].iter_mut().for_each(|b| b.level_behind = 0.0);
    assert!((Ground::new(&project).natural_height(at(1.0)) - before.natural_height(at(1.0))).abs() < 1e-4);
    assert!(!serde_json::to_string(&project.site.barriers.last()).unwrap().contains("level_behind"));
}
