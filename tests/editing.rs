//! Drives the editor the way a person does: presses, drags and keys on the real view.

use lintel::app::{State, Tool, ViewMode};
use lintel::model::*;
use lintel::{sample, ui};
use rune::{Env, Key, KeyEvent, Mods, Runtime};

const SIZE: (f32, f32) = (1440.0, 900.0);

fn app(project: Project) -> Runtime<State> {
    let mut rt = Runtime::new(State::new(project, None), ui::view, SIZE, Env { system_dark: false, scale: 1.0 });
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..4 {
        rt.frame(1.0 / 60.0);
    }
}

/// Window point of a plan model point.
fn on_plan(rt: &Runtime<State>, p: P2) -> (f32, f32) {
    let ([x, y, ..], _) = rt.presented("plan").expect("the plan is on screen");
    let s = rt.state.plan.to_screen(p, rt.state.canvas);
    (x + s[0], y + s[1])
}

fn drag(rt: &mut Runtime<State>, from: (f32, f32), to: (f32, f32)) {
    rt.pointer_move(from.0, from.1);
    rt.pointer_down(from.0, from.1);
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        rt.pointer_move(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        rt.frame(1.0 / 60.0);
    }
    rt.pointer_up(to.0, to.1);
    settle(rt);
}

fn click(rt: &mut Runtime<State>, at: (f32, f32)) {
    rt.pointer_move(at.0, at.1);
    rt.pointer_down(at.0, at.1);
    rt.pointer_up(at.0, at.1);
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let at = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
    click(rt, at);
}

fn key(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    settle(rt);
}

fn level(rt: &Runtime<State>) -> &Level {
    rt.state.active().unwrap()
}

#[test]
fn the_plan_fits_the_house_once_laid_out() {
    let rt = app(sample::house());
    let (x, y) = on_plan(&rt, [7.5, 5.5]);
    let ([px, py, pw, ph], _) = rt.presented("plan").unwrap();
    assert!(pw > 800.0 && ph > 600.0, "the plan fills the middle: {pw} × {ph}");
    assert!((x - (px + pw / 2.0)).abs() < 80.0 && (y - (py + ph / 2.0)).abs() < 120.0, "the house is centred");
}

#[test]
fn drawing_a_wall_then_putting_a_door_in_it() {
    let mut rt = app(sample::blank());
    press(&mut rt, "tool-wall");
    assert_eq!(rt.state.tool, Tool::Wall);
    let (a, b) = (on_plan(&rt, [-2.0, 1.0]), on_plan(&rt, [2.0, 1.02]));
    drag(&mut rt, a, b);
    let walls = &level(&rt).walls;
    assert_eq!(walls.len(), 1);
    let w = &walls[0];
    // Snapped to the grid, and squared up although the drag ended a little off the horizontal.
    assert_eq!((w.a, w.b), ([-2.0, 1.0], [2.0, 1.0]));

    // A second wall from the first one's end snaps to it.
    let (a, b) = (on_plan(&rt, [2.03, 0.97]), on_plan(&rt, [2.0, -2.0]));
    drag(&mut rt, a, b);
    assert_eq!(level(&rt).walls[1].a, [2.0, 1.0]);

    press(&mut rt, "tool-door");
    let at = on_plan(&rt, [0.5, 1.04]);
    click(&mut rt, at);
    let w = &level(&rt).walls[0];
    assert_eq!(w.openings.len(), 1);
    assert!((w.openings[0].at - 2.5).abs() < 0.06, "the door is where the click was: {}", w.openings[0].at);
    assert_eq!(rt.state.selected, Some(Pick::Opening(w.id, w.openings[0].id)));
    assert_eq!(rt.state.tool, Tool::Select);

    // Undo takes the door out, then the second wall.
    rt.update(|s, _| s.undo());
    assert!(level(&rt).walls[0].openings.is_empty());
    rt.update(|s, _| s.undo());
    assert_eq!(level(&rt).walls.len(), 1);
    rt.update(|s, _| s.redo());
    assert_eq!(level(&rt).walls.len(), 2);
}

#[test]
fn dragging_furniture_moves_it_and_a_click_only_selects() {
    let mut rt = app(sample::house());
    let table = level(&rt).furniture.iter().find(|f| f.kind == "dining_table").unwrap().clone();
    // Off-centre: a pendant light hangs over the middle of the table, and it picks first.
    let at = on_plan(&rt, [table.at[0] + 0.25, table.at[1] + 0.55]);
    click(&mut rt, at);
    assert_eq!(rt.state.selected, Some(Pick::Furniture(table.id)));
    assert!(!rt.state.can_undo(), "a click changes nothing");

    let to = on_plan(&rt, [table.at[0] + 0.25 - 1.0, table.at[1] + 0.55 + 0.5]);
    drag(&mut rt, at, to);
    let moved = rt.state.project.furniture(table.id).unwrap().at;
    assert!((moved[0] - (table.at[0] - 1.0)).abs() < 0.06 && (moved[1] - (table.at[1] + 0.5)).abs() < 0.06, "{moved:?}");
    rt.update(|s, _| s.undo());
    assert_eq!(rt.state.project.furniture(table.id).unwrap().at, table.at);
}

#[test]
fn dragging_a_corner_takes_both_walls_with_it() {
    let mut rt = app(sample::house());
    // The front-right corner of the house, where the front wall ends and the east wall starts.
    let front = level(&rt).walls.iter().find(|w| w.a == [6.2, 0.0] && w.b == [15.0, 0.0]).unwrap().id;
    let east = level(&rt).walls.iter().find(|w| w.a == [15.0, 0.0] && w.b == [15.0, 11.0]).unwrap().id;
    let mid = on_plan(&rt, [10.0, -0.02]);
    click(&mut rt, mid);
    assert_eq!(rt.state.selected, Some(Pick::Wall(front)));
    let (from, to) = (on_plan(&rt, [15.0, 0.0]), on_plan(&rt, [16.0, 0.0]));
    drag(&mut rt, from, to);
    assert_eq!(rt.state.project.wall(front).unwrap().b, [16.0, 0.0]);
    assert_eq!(rt.state.project.wall(east).unwrap().a, [16.0, 0.0]);
}

#[test]
fn the_inspector_steps_and_takes_typed_lengths() {
    let mut rt = app(sample::house());
    let mut wall = level(&rt).walls[1].clone();
    // A wall with a build-up takes its thickness from its layers; this one is to be plain.
    wall.layers.clear();
    rt.update(|s, _| {
        s.edit("", |p| p.wall_mut(wall.id).unwrap().layers.clear());
        s.select(Some(Pick::Wall(wall.id)));
    });
    settle(&mut rt);

    press(&mut rt, "wall-thickness+");
    let now = rt.state.project.wall(wall.id).unwrap().thickness;
    assert!((now - (wall.thickness + 0.05)).abs() < 1e-4, "{now}");

    // Click the length, type a new one, press Return.
    press(&mut rt, "wall-length-value");
    assert!(rt.state.editing.is_some(), "the field is being edited");
    for c in ["9", ".", "5"] {
        key(&mut rt, Key::Text(c.into()));
    }
    key(&mut rt, Key::Enter);
    assert!(rt.state.editing.is_none());
    assert!((rt.state.project.wall(wall.id).unwrap().length() - 9.5).abs() < 1e-3, "{}", rt.state.project.wall(wall.id).unwrap().length());

    // Feet and inches read too.
    rt.update(|s, _| s.edit("", |p| p.units = Units::Imperial));
    settle(&mut rt);
    press(&mut rt, "wall-length-value");
    for c in ["3", "0", "'", " ", "6", "\""] {
        key(&mut rt, Key::Text(c.into()));
    }
    key(&mut rt, Key::Enter);
    assert!((rt.state.project.wall(wall.id).unwrap().length() - 9.2964).abs() < 1e-3);
}

#[test]
fn picking_a_material_and_a_colour() {
    let mut rt = app(sample::house());
    let room = level(&rt).rooms.iter().find(|r| r.name == "Living room").unwrap().id;
    rt.update(|s, _| s.select(Some(Pick::Room(room))));
    settle(&mut rt);
    press(&mut rt, "room-floor-finish");
    assert_eq!(rt.state.picker.as_deref(), Some("room-floor"));
    // A stain over the oak boards.
    press(&mut rt, "room-floor-s5a3a22");
    let floor = rt.state.project.room(room).unwrap().floor.clone();
    assert_eq!((floor.material.as_str(), floor.color), ("oak_boards", Some(rgb(0x5a3a22))));
    // Changing to a painted material drops the stain; picking a paint colour sets it.
    press(&mut rt, "room-floor-m-tile");
    assert_eq!(rt.state.project.room(room).unwrap().floor, Finish::of("tile"));
    press(&mut rt, "room-floor-c64747c");
    assert_eq!(rt.state.project.room(room).unwrap().floor, Finish::colored("tile", rgb(0x64747c)));
}

#[test]
fn the_3d_view_orbits_and_picks() {
    let mut rt = app(sample::house());
    press(&mut rt, "view-3D");
    assert_eq!(rt.state.view, ViewMode::Model);
    let ([x, y, w, h], _) = rt.presented("viewport").unwrap();
    let centre = (x + w / 2.0, y + h / 2.0);
    let yaw = rt.state.camera.yaw;
    drag(&mut rt, centre, (centre.0 + 120.0, centre.1 + 10.0));
    assert!((rt.state.camera.yaw - yaw).abs() > 0.3, "dragging turns the camera");
    assert_eq!(rt.state.selected, None, "a drag doesn't select");

    // Looking straight at the house, a click on it picks part of it.
    rt.update(|s, _| {
        let bounds = s.scene().bounds;
        s.camera.frame(bounds);
    });
    settle(&mut rt);
    click(&mut rt, centre);
    assert!(rt.state.selected.is_some(), "a click on the house selects something");
}

#[test]
fn keys_switch_tools_delete_and_nudge() {
    let mut rt = app(sample::house());
    let ev = |k: Key| KeyEvent { key: k, mods: Mods::default() };
    rt.update(|s, tx| {
        ui::on_key(s, &ev(Key::Text("w".into())), tx);
    });
    assert_eq!(rt.state.tool, Tool::Wall);
    rt.update(|s, tx| {
        ui::on_key(s, &ev(Key::Escape), tx);
    });
    assert_eq!(rt.state.tool, Tool::Select);

    let sofa = level(&rt).furniture.iter().find(|f| f.kind == "sofa").unwrap().clone();
    rt.update(|s, tx| {
        s.select(Some(Pick::Furniture(sofa.id)));
        ui::on_key(s, &ev(Key::Right), tx);
        ui::on_key(s, &ev(Key::Right), tx);
    });
    assert!((rt.state.project.furniture(sofa.id).unwrap().at[0] - (sofa.at[0] + 0.1)).abs() < 1e-4);
    rt.update(|s, tx| {
        ui::on_key(s, &ev(Key::Delete), tx);
    });
    assert!(rt.state.project.furniture(sofa.id).is_none());
    // The two nudges were one undo step, the delete another.
    rt.update(|s, _| s.undo());
    rt.update(|s, _| s.undo());
    assert_eq!(rt.state.project.furniture(sofa.id).unwrap().at, sofa.at);
}

#[test]
fn a_switch_turns_its_lights_off_and_picks_new_ones_on_the_plan() {
    let mut rt = app(sample::house());
    let switch = level(&rt).fixtures.iter().find(|f| f.label == "Living room lights").unwrap().clone();
    assert_eq!(switch.controls.len(), 6);
    let lit = |rt: &Runtime<State>| rt.state.scene().lamps.len();
    let before = lit(&rt);
    rt.update(|s, _| {
        s.select(Some(Pick::Fixture(switch.id)));
        s.set_view(ViewMode::Model);
    });
    settle(&mut rt);
    press(&mut rt, "switch-flick");
    assert!(switch.controls.iter().all(|id| !rt.state.project.fixture(*id).unwrap().lamp().unwrap().on));
    assert_eq!(lit(&rt), before - 6, "six fewer lamps light the scene");
    press(&mut rt, "switch-flick");
    assert_eq!(lit(&rt), before);

    // Picking on the plan: a click on a light that isn't on the switch adds it, again removes it.
    press(&mut rt, "switch-pick");
    assert_eq!((rt.state.view, rt.state.linking), (ViewMode::Plan, Some(switch.id)));
    let pendant = level(&rt).fixtures.iter().find(|f| matches!(f.kind, FixtureKind::Pendant { diameter, .. } if diameter > 0.4)).unwrap().id;
    let at = on_plan(&rt, [13.0, 8.3]);
    click(&mut rt, at);
    assert!(rt.state.project.fixture(switch.id).unwrap().controls.contains(&pendant));
    assert_eq!(rt.state.selected, Some(Pick::Fixture(switch.id)), "the switch stays selected while picking");
    click(&mut rt, at);
    assert!(!rt.state.project.fixture(switch.id).unwrap().controls.contains(&pendant));
}

#[test]
fn the_sun_follows_the_clock_and_the_plans_north() {
    let mut rt = app(sample::house());
    rt.update(|s, _| {
        s.edit("", |p| {
            p.site.geo = Some(Geo { lat: 40.0, lon: -105.0 });
            p.site.sun = SunSetting { by_clock: true, month: 6, day: 21, hour: 12.0, utc_offset: -7.0, ..p.site.sun };
        })
    });
    // Noon in June at 40° N: high in the south, which is -Y on this plan.
    let sun = rt.state.sun();
    assert!(sun.altitude.to_degrees() > 65.0, "{}", sun.altitude.to_degrees());
    assert!((sun.azimuth.to_degrees() + 90.0).abs() < 25.0, "plan angle {}", sun.azimuth.to_degrees());
    // Turn the plan so +Y points east: the southern sun is now along +X.
    rt.update(|s, _| s.edit("", |p| p.site.north = 90.0));
    assert!(rt.state.sun().azimuth.to_degrees().abs() < 25.0, "{}", rt.state.sun().azimuth.to_degrees());
    // At night it is below the horizon.
    rt.update(|s, _| s.edit("", |p| p.site.sun.hour = 23.0));
    assert!(rt.state.sun().altitude < 0.0);
}

#[test]
fn a_phone_sized_window_shows_the_house_without_the_editing_panels() {
    let mut rt = Runtime::new(State::new(sample::house(), None), ui::view, (390.0, 800.0), Env { system_dark: false, scale: 2.0 });
    settle(&mut rt);
    assert!(rt.find("tools").is_none() && rt.find("inspector").is_none(), "no tool panel or inspector");
    press(&mut rt, "view-3D");
    assert_eq!(rt.state.view, ViewMode::Model);
    let ([_, _, w, h], _) = rt.presented("viewport").unwrap();
    assert!(w > 380.0 && h > 700.0, "the view fills the screen: {w} × {h}");
    // Lifting the roof off is one tap.
    let main = rt.state.project.levels.last().unwrap().id;
    press(&mut rt, &format!("top-{main}"));
    assert_eq!(rt.state.options.top_level, Some(main));
}

#[test]
fn a_floor_plan_image_is_laid_under_the_plan_and_scaled_from_a_known_length() {
    let mut rt = app(sample::blank());
    // A 400 × 200 px image stands in for a scanned plan.
    let waker: rune::runtime::Waker = std::sync::Arc::new(|| {});
    let cache = rune::ImageCache::new(waker);
    let image = cache.rgba(400, 200, vec![200u8; 400 * 200 * 4]);
    image.wait().unwrap();
    rt.update(|s, _| {
        s.underlays.push(("plan.png".into(), image.clone()));
        let level = s.level;
        s.edit("", |p| p.level_mut(level).unwrap().underlay = Some(Underlay { path: "plan.png".into(), at: [-2.0, -1.0], scale: 0.01, turn: 0.0, opacity: 0.6, visible: true }));
    });
    settle(&mut rt);
    // At 1 cm a pixel it is 4 m × 2 m on the plan, with its bottom-left corner at (-2, -1).
    let ([x, y, w, h], _) = rt.presented("underlay").expect("the image is on the plan");
    let zoom = rt.state.plan.zoom;
    assert!((w - 4.0 * zoom).abs() < 1.0 && (h - 2.0 * zoom).abs() < 1.0, "{w} × {h} at {zoom} pt/m");
    let corner = on_plan(&rt, [-2.0, -1.0]);
    assert!((x - corner.0).abs() < 1.0 && (y + h - corner.1).abs() < 1.0);

    // Set its scale: drag along the image's bottom edge (4 m as it stands), and say it is 10 m.
    // (The button is far down the project panel; choose the tool as it does.)
    rt.update(|s, _| s.set_tool(Tool::Calibrate));
    settle(&mut rt);
    let (a, b) = (on_plan(&rt, [-2.0, -1.0]), on_plan(&rt, [2.0, -1.0]));
    drag(&mut rt, a, b);
    let (p, q) = rt.state.calibrate.expect("the line is waiting for its length");
    assert!((lintel::geom::dist(p, q) - 4.0).abs() < 0.05);
    rt.update(|s, _| s.apply_calibration(10.0));
    let u = rt.state.active().unwrap().underlay.clone().unwrap();
    assert!((u.scale - 0.025).abs() < 4e-4, "{}", u.scale);
    // The end the line started from stays where it was.
    assert!((u.at[0] + 2.0).abs() < 0.05 && (u.at[1] + 1.0).abs() < 0.05, "{:?}", u.at);
    assert!(rt.state.calibrate.is_none());
}

#[test]
fn a_new_option_is_a_copy_to_change_while_the_original_stays_as_it_was() {
    let mut rt = app(sample::house());
    let walls = level(&rt).walls.len();
    let east = level(&rt).walls.iter().find(|w| w.a == [15.0, 0.0] && w.b == [15.0, 11.0]).unwrap().id;
    press(&mut rt, "option-branch");
    assert_eq!(rt.state.project.option_names(), vec!["Option 1", "Existing"]);
    assert_eq!(rt.state.compare.as_deref(), Some("Existing"));
    // Take a wall out of the new option only.
    rt.update(|s, _| {
        s.select(Some(Pick::Wall(east)));
        s.delete_selected();
    });
    assert_eq!(level(&rt).walls.len(), walls - 1);
    assert_eq!(rt.state.project.others[0].levels.last().unwrap().walls.len(), walls, "Existing keeps its wall");

    // Switch back: the wall is there, and the same storey is still in view.
    rt.update(|s, _| {
        s.select(None);
    });
    settle(&mut rt);
    press(&mut rt, "option-0");
    assert_eq!(rt.state.project.option, "Existing");
    assert_eq!((level(&rt).name.as_str(), level(&rt).walls.len()), ("Main", walls));
    assert_eq!(rt.state.project.others[0].option, "Option 1");

    // Both survive a save and reopen, and undo steps back across the switch.
    let json = serde_json::to_string(&rt.state.project).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back, rt.state.project);
    rt.update(|s, _| s.undo());
    assert_eq!(rt.state.project.option, "Option 1");

    // Removing the option being edited goes back to the other.
    rt.update(|s, _| s.remove_option());
    assert_eq!(rt.state.project.option_names(), vec!["Existing"]);
}

#[test]
fn a_walls_build_up_sets_its_thickness_and_can_be_edited_layer_by_layer() {
    let mut rt = app(sample::blank());
    press(&mut rt, "tool-wall");
    let (a, b) = (on_plan(&rt, [-2.0, 0.0]), on_plan(&rt, [2.0, 0.0]));
    drag(&mut rt, a, b);
    let id = level(&rt).walls[0].id;
    assert!(rt.state.selected == Some(Pick::Wall(id)) && level(&rt).walls[0].layers.is_empty());
    rt.update(|s, _| s.set_tool(Tool::Select));
    rt.update(|s, _| s.select(Some(Pick::Wall(id))));
    settle(&mut rt);
    press(&mut rt, "layers-Brick veneer");
    let w = rt.state.project.wall(id).unwrap().clone();
    assert_eq!(w.layers.len(), 4);
    assert!((w.thickness - 0.25).abs() < 1e-4, "10 + 90 + 40 + 110 mm: {}", w.thickness);
    // Thicken the cavity by one step; the wall grows with it.
    press(&mut rt, "layer-2-thickness+");
    assert!((rt.state.project.wall(id).unwrap().thickness - 0.255).abs() < 1e-4);
    // Take the brick off.
    // (Its Remove button is below the fold of the panel; do what it does.)
    rt.update(|s, _| {
        s.edit("", |p| {
            let w = p.wall_mut(id).unwrap();
            let mut layers = w.layers.clone();
            layers.remove(3);
            w.set_layers(layers)
        })
    });
    let w = rt.state.project.wall(id).unwrap();
    assert_eq!(w.layers.len(), 3);
    assert!((w.thickness - 0.145).abs() < 1e-4, "{}", w.thickness);
    // The top of the wall is drawn in stripes, one a layer.
    let scene = lintel::mesh::build(&rt.state.project, &lintel::mesh::BuildOptions { terrain: false, ..Default::default() });
    let top = 2.7;
    let caps = scene.vertices.iter().filter(|v| (v.pos[2] - top).abs() < 1e-4 && v.normal[2] > 0.9).count();
    assert_eq!(caps, 3 * 4, "three strips of four corners");
}

#[test]
fn drawing_a_fence_and_planting_a_tree() {
    let mut rt = app(sample::blank());
    // (The outside tools are below the fold of the tool panel; choose them as the buttons do.)
    rt.update(|s, _| s.set_tool(Tool::Barrier(BarrierKind::Fence)));
    settle(&mut rt);
    let (a, b) = (on_plan(&rt, [-3.0, -2.0]), on_plan(&rt, [3.0, -2.0]));
    drag(&mut rt, a, b);
    let fence = rt.state.project.site.barriers[0].clone();
    assert_eq!((fence.kind, fence.path.clone()), (BarrierKind::Fence, vec![[-3.0, -2.0], [3.0, -2.0]]));
    assert_eq!(rt.state.selected, Some(Pick::Barrier(fence.id)));
    // Drag its end; the other stays.
    let (from, to) = (on_plan(&rt, [3.0, -2.0]), on_plan(&rt, [3.0, 1.0]));
    drag(&mut rt, from, to);
    assert_eq!(rt.state.project.barrier(fence.id).unwrap().path, vec![[-3.0, -2.0], [3.0, 1.0]]);

    rt.update(|s, _| s.set_tool(Tool::Plant(PlantKind::Tree)));
    settle(&mut rt);
    let at = on_plan(&rt, [0.0, 2.5]);
    click(&mut rt, at);
    let tree = rt.state.project.site.plants[0];
    assert!((tree.at[0]).abs() < 0.05 && (tree.at[1] - 2.5).abs() < 0.05);
    // Delete the tree; the fence remains.
    rt.update(|s, _| {
        s.select(Some(Pick::Plant(tree.id)));
        s.delete_selected();
    });
    assert!(rt.state.project.site.plants.is_empty() && rt.state.project.site.barriers.len() == 1);
}
