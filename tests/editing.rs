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
    let wall = level(&rt).walls[1].clone();
    rt.update(|s, _| s.select(Some(Pick::Wall(wall.id))));
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
