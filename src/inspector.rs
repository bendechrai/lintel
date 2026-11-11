//! The right-hand panel: properties of the selection, or of the project when nothing is selected.

use rune::{El, ScrollSpec, Tx, col, row, text, text_input};

use crate::app::{State, Tool};
use crate::geom;
use crate::materials::Library;
use crate::model::*;
use crate::units;
use crate::widgets::*;

fn len(s: &State, th: &Th, label: &str, key: &'static str, v: f32, set: impl Fn(&mut Project, f32) + 'static) -> El {
    let step = units::snap_step(s.project.units);
    num(s, th, label, key, v, Fmt::Length, step, on_project(key, set))
}

fn fin(s: &State, th: &Th, lib: &Library, label: &str, key: &'static str, v: &Finish, set: impl Fn(&mut Project, Finish) + 'static) -> El {
    finish(s, th, lib, label, key, v, on_project("", set))
}

fn heading(th: &Th, title: impl Into<String>, sub: impl Into<String>) -> El {
    col().w_full().gap(1.0).mb(4.0).child(text(title).font(TITLE).color(th.text).truncate()).child(text(sub).font(SMALL).color(th.dim).truncate())
}

fn actions(th: &Th, can_duplicate: bool, can_turn: bool) -> El {
    let mut r = row().w_full().gap(6.0).mt(12.0).flex_wrap();
    if can_turn {
        r = r.child(button(th, "act-turn", "Turn 90°", false).on_press(|s: &mut State| s.turn_selected(std::f32::consts::FRAC_PI_2)));
    }
    if can_duplicate {
        r = r.child(button(th, "act-duplicate", "Duplicate", false).on_press(|s: &mut State| s.duplicate_selected()));
    }
    r.child(button(th, "act-delete", "Delete", false).on_press(|s: &mut State| s.delete_selected()))
}

const SIDES: [(&str, WallSide); 2] = [("Left", WallSide::Left), ("Right", WallSide::Right)];

fn wall(s: &State, th: &Th, lib: &Library, level: &Level, w: &Wall) -> Vec<El> {
    let id = w.id;
    let full = w.height.is_none();
    let level_height = level.height;
    let mut v = vec![
        heading(th, "Wall", format!("{} · left and right are as seen walking from its first end", level.name)),
        len(s, th, "Length", "wall-length", w.length(), move |p, v| {
            if let Some(w) = p.wall_mut(id) {
                let (d, _) = w.frame();
                w.b = geom::add(w.a, geom::scale(d, v.max(0.05)));
            }
        }),
        len(s, th, "Thickness", "wall-thickness", w.thickness, move |p, v| {
            if let Some(w) = p.wall_mut(id) {
                w.thickness = v.clamp(0.02, 1.5)
            }
        }),
        toggle(th, "Full height", "wall-full", full, on_project("", move |p, on| {
            if let Some(w) = p.wall_mut(id) {
                w.height = if on { None } else { Some(level_height) }
            }
        })),
    ];
    if let Some(h) = w.height {
        v.push(len(s, th, "Height", "wall-height", h, move |p, v| {
            if let Some(w) = p.wall_mut(id) {
                w.height = Some(v.max(0.05))
            }
        }));
    }
    v.push(len(s, th, "Base", "wall-base", w.base, move |p, v| {
        if let Some(w) = p.wall_mut(id) {
            w.base = v
        }
    }));
    v.push(section(th, "Finishes"));
    v.push(fin(s, th, lib, "Left face", "wall-left", &w.left, move |p, f| {
        if let Some(w) = p.wall_mut(id) {
            w.left = f
        }
    }));
    v.push(fin(s, th, lib, "Right face", "wall-right", &w.right, move |p, f| {
        if let Some(w) = p.wall_mut(id) {
            w.right = f
        }
    }));
    v.push(fact(th, "Openings", format!("{}", w.openings.len())));
    v.push(
        row().w_full().gap(6.0).mt(12.0).flex_wrap().child(button(th, "wall-flip", "Swap faces", false).on_press(move |s: &mut State| {
            s.edit("", |p| {
                if let Some(w) = p.wall_mut(id) {
                    std::mem::swap(&mut w.left, &mut w.right);
                }
            })
        })).child(button(th, "act-delete", "Delete", false).on_press(|s: &mut State| s.delete_selected())),
    );
    v
}

fn frame_rows(s: &State, th: &Th, lib: &Library, frame: &Option<Frame>, wall: Id, id: Id) -> Vec<El> {
    fn frame_of(o: &mut Opening) -> Option<&mut Option<Frame>> {
        match &mut o.kind {
            OpeningKind::Door(d) => Some(&mut d.frame),
            OpeningKind::Window(w) => Some(&mut w.casing),
            OpeningKind::Passage { frame } => Some(frame),
            _ => None,
        }
    }
    let mut v = vec![toggle(th, "Frame & trim", "frame-on", frame.is_some(), on_project("", move |p, on| {
        if let Some(f) = p.opening_mut(wall, id).and_then(frame_of) {
            *f = on.then(Frame::default)
        }
    }))];
    if let Some(f) = frame {
        v.push(len(s, th, "Jamb", "frame-jamb", f.jamb, move |p, v| {
            if let Some(Some(f)) = p.opening_mut(wall, id).and_then(frame_of) {
                f.jamb = v.clamp(0.005, 0.15)
            }
        }));
        v.push(len(s, th, "Architrave", "frame-casing", f.casing, move |p, v| {
            if let Some(Some(f)) = p.opening_mut(wall, id).and_then(frame_of) {
                f.casing = v.clamp(0.0, 0.3)
            }
        }));
        v.push(fin(s, th, lib, "Trim finish", "frame-finish", &f.finish, move |p, fi| {
            if let Some(Some(f)) = p.opening_mut(wall, id).and_then(frame_of) {
                f.finish = fi
            }
        }));
    }
    v
}

fn opening(s: &State, th: &Th, lib: &Library, w: &Wall, o: &Opening) -> Vec<El> {
    let (wall, id) = (w.id, o.id);
    let length = w.length();
    let title = match &o.kind {
        OpeningKind::Door(_) => "Door",
        OpeningKind::Garage(_) => "Garage door",
        OpeningKind::Window(_) => "Window",
        OpeningKind::Passage { .. } => "Opening",
        OpeningKind::Recess { .. } => "Recess",
    };
    let mut v = vec![
        heading(th, title, format!("In a {} wall", units::length(length, s.project.units))),
        len(s, th, "Centre at", "op-at", o.at, move |p, v| {
            if let Some(o) = p.opening_mut(wall, id) {
                o.at = v.clamp(o.width / 2.0, (length - o.width / 2.0).max(o.width / 2.0))
            }
        }),
        len(s, th, "Width", "op-width", o.width, move |p, v| {
            if let Some(o) = p.opening_mut(wall, id) {
                o.width = v.clamp(0.1, length)
            }
        }),
        len(s, th, "Height", "op-height", o.height, move |p, v| {
            if let Some(o) = p.opening_mut(wall, id) {
                o.height = v.max(0.1)
            }
        }),
        len(s, th, "Sill height", "op-sill", o.sill, move |p, v| {
            if let Some(o) = p.opening_mut(wall, id) {
                o.sill = v.max(0.0)
            }
        }),
        fact(th, "Head height", units::length(o.sill + o.height, s.project.units)),
    ];
    match &o.kind {
        OpeningKind::Door(d) => {
            fn door(p: &mut Project, wall: Id, id: Id) -> Option<&mut Door> {
                match &mut p.opening_mut(wall, id)?.kind {
                    OpeningKind::Door(d) => Some(d),
                    _ => None,
                }
            }
            v.push(section(th, "Door"));
            let styles = [("Hinged", DoorStyle::Hinged), ("Double", DoorStyle::Double), ("Sliding", DoorStyle::Sliding), ("Pocket", DoorStyle::Pocket), ("Bifold", DoorStyle::Bifold)];
            v.push(choice(th, "Style", "door-style", &d.style, &styles, on_project("", move |p, x| {
                if let Some(d) = door(p, wall, id) {
                    d.style = x
                }
            })));
            v.push(choice(th, "Hinge at", "door-hinge", &d.hinge, &[("Start", Hinge::Start), ("End", Hinge::End)], on_project("", move |p, x| {
                if let Some(d) = door(p, wall, id) {
                    d.hinge = x
                }
            })));
            v.push(choice(th, "Opens to", "door-swing", &d.swing, &SIDES, on_project("", move |p, x| {
                if let Some(d) = door(p, wall, id) {
                    d.swing = x
                }
            })));
            v.push(num(s, th, "Open", "door-open", d.open, Fmt::Percent, 0.05, on_project("door-open", move |p, x| {
                if let Some(d) = door(p, wall, id) {
                    d.open = x.clamp(0.0, 1.0)
                }
            })));
            v.push(num(s, th, "Glazed", "door-glazing", d.glazing, Fmt::Percent, 0.05, on_project("door-glazing", move |p, x| {
                if let Some(d) = door(p, wall, id) {
                    d.glazing = x.clamp(0.0, 1.0)
                }
            })));
            v.push(fin(s, th, lib, "Leaf", "door-leaf", &d.leaf, move |p, f| {
                if let Some(d) = door(p, wall, id) {
                    d.leaf = f
                }
            }));
            v.extend(frame_rows(s, th, lib, &d.frame, wall, id));
        }
        OpeningKind::Garage(g) => {
            fn garage(p: &mut Project, wall: Id, id: Id) -> Option<&mut GarageDoor> {
                match &mut p.opening_mut(wall, id)?.kind {
                    OpeningKind::Garage(g) => Some(g),
                    _ => None,
                }
            }
            v.push(section(th, "Garage door"));
            let styles = [("Sectional", GarageStyle::Sectional), ("Roller", GarageStyle::Roller), ("Tilt-up", GarageStyle::Tilt)];
            v.push(choice(th, "Style", "garage-style", &g.style, &styles, on_project("", move |p, x| {
                if let Some(g) = garage(p, wall, id) {
                    g.style = x
                }
            })));
            v.push(choice(th, "Inside is", "garage-inside", &g.inside, &SIDES, on_project("", move |p, x| {
                if let Some(g) = garage(p, wall, id) {
                    g.inside = x
                }
            })));
            v.push(num(s, th, "Open", "garage-open", g.open, Fmt::Percent, 0.05, on_project("garage-open", move |p, x| {
                if let Some(g) = garage(p, wall, id) {
                    g.open = x.clamp(0.0, 1.0)
                }
            })));
            if g.style == GarageStyle::Sectional {
                v.push(num(s, th, "Panels", "garage-panels", g.panels as f32, Fmt::Count, 1.0, on_project("garage-panels", move |p, x| {
                    if let Some(g) = garage(p, wall, id) {
                        g.panels = x.clamp(2.0, 8.0) as u32
                    }
                })));
            }
            v.push(fin(s, th, lib, "Finish", "garage-finish", &g.finish, move |p, f| {
                if let Some(g) = garage(p, wall, id) {
                    g.finish = f
                }
            }));
        }
        OpeningKind::Window(win) => {
            fn window(p: &mut Project, wall: Id, id: Id) -> Option<&mut Window> {
                match &mut p.opening_mut(wall, id)?.kind {
                    OpeningKind::Window(w) => Some(w),
                    _ => None,
                }
            }
            v.push(section(th, "Window"));
            let styles = [
                ("Fixed", WindowStyle::Fixed),
                ("Casement", WindowStyle::Casement),
                ("Awning", WindowStyle::Awning),
                ("Double-hung", WindowStyle::DoubleHung),
                ("Sliding", WindowStyle::Sliding),
                ("Louvre", WindowStyle::Louvre),
            ];
            v.push(choice(th, "Style", "window-style", &win.style, &styles, on_project("", move |p, x| {
                if let Some(w) = window(p, wall, id) {
                    w.style = x
                }
            })));
            if win.style == WindowStyle::Fixed {
                v.push(num(s, th, "Panes across", "window-cols", win.cols as f32, Fmt::Count, 1.0, on_project("window-cols", move |p, x| {
                    if let Some(w) = window(p, wall, id) {
                        w.cols = x.clamp(1.0, 12.0) as u32
                    }
                })));
                v.push(num(s, th, "Panes up", "window-rows", win.rows as f32, Fmt::Count, 1.0, on_project("window-rows", move |p, x| {
                    if let Some(w) = window(p, wall, id) {
                        w.rows = x.clamp(1.0, 12.0) as u32
                    }
                })));
            }
            v.push(len(s, th, "Frame width", "window-frame-width", win.frame_width, move |p, x| {
                if let Some(w) = window(p, wall, id) {
                    w.frame_width = x.clamp(0.01, 0.2)
                }
            }));
            v.push(fin(s, th, lib, "Frame", "window-frame", &win.frame, move |p, f| {
                if let Some(w) = window(p, wall, id) {
                    w.frame = f
                }
            }));
            v.extend(frame_rows(s, th, lib, &win.casing, wall, id));
        }
        OpeningKind::Passage { frame } => v.extend(frame_rows(s, th, lib, frame, wall, id)),
        OpeningKind::Recess { side, depth, finish } => {
            v.push(section(th, "Recess"));
            v.push(choice(th, "Cut into", "recess-side", side, &SIDES, on_project("", move |p, x| {
                if let Some(OpeningKind::Recess { side, .. }) = p.opening_mut(wall, id).map(|o| &mut o.kind) {
                    *side = x
                }
            })));
            let max = w.thickness - 0.01;
            v.push(len(s, th, "Depth", "recess-depth", *depth, move |p, x| {
                if let Some(OpeningKind::Recess { depth, .. }) = p.opening_mut(wall, id).map(|o| &mut o.kind) {
                    *depth = x.clamp(0.01, max)
                }
            }));
            v.push(fin(s, th, lib, "Lining", "recess-finish", finish, move |p, f| {
                if let Some(OpeningKind::Recess { finish, .. }) = p.opening_mut(wall, id).map(|o| &mut o.kind) {
                    *finish = f
                }
            }));
        }
    }
    v.push(actions(th, true, false));
    v
}

fn room(s: &State, th: &Th, lib: &Library, level: &Level, r: &Room) -> Vec<El> {
    let id = r.id;
    let level_height = level.height;
    let mut v = vec![
        heading(th, "Room", format!("{} · {}", level.name, units::area(geom::area(&r.outline).abs(), s.project.units))),
        name(s, th, "Name", "room-name", &r.name, on_project("", move |p, t: String| {
            if let Some(r) = p.room_mut(id) {
                r.name = t
            }
        })),
        fin(s, th, lib, "Floor", "room-floor", &r.floor, move |p, f| {
            if let Some(r) = p.room_mut(id) {
                r.floor = f
            }
        }),
        len(s, th, "Floor offset", "room-offset", r.floor_offset, move |p, x| {
            if let Some(r) = p.room_mut(id) {
                r.floor_offset = x
            }
        }),
        toggle(th, "Has a ceiling", "room-ceiling-on", r.has_ceiling, on_project("", move |p, on| {
            if let Some(r) = p.room_mut(id) {
                r.has_ceiling = on
            }
        })),
    ];
    if r.has_ceiling {
        v.push(len(s, th, "Ceiling height", "room-height", r.ceiling_height.unwrap_or(level.height), move |p, x| {
            if let Some(r) = p.room_mut(id) {
                // Back at the level's own height, it follows the level again.
                r.ceiling_height = if (x - level_height).abs() < 0.001 { None } else { Some(x.max(0.5)) }
            }
        }));
        v.push(fin(s, th, lib, "Ceiling", "room-ceiling", &r.ceiling, move |p, f| {
            if let Some(r) = p.room_mut(id) {
                r.ceiling = f
            }
        }));
    }
    v.push(text("Drag a corner to reshape. Double-click an edge to add a corner; drop a corner on its neighbour to remove it.").font(SMALL).color(th.dim).wrap().mt(8.0));
    v.push(actions(th, true, false));
    v
}

fn stair(s: &State, th: &Th, lib: &Library, st: &Stair) -> Vec<El> {
    let id = st.id;
    let u = s.project.units;
    let riser = st.rise / st.steps.max(1) as f32;
    let going = st.run / st.steps.max(1) as f32;
    let railings = [("None", Railing::None), ("Left", Railing::Left), ("Right", Railing::Right), ("Both", Railing::Both)];
    vec![
        heading(th, "Stair", format!("{} risers of {}, treads of {}", st.steps, units::length(riser, u), units::length(going, u))),
        len(s, th, "Width", "stair-width", st.width, move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.width = x.max(0.3)
            }
        }),
        len(s, th, "Run", "stair-run", st.run, move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.run = x.max(0.3)
            }
        }),
        len(s, th, "Rise", "stair-rise", st.rise, move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.rise = x.max(0.1)
            }
        }),
        num(s, th, "Steps", "stair-steps", st.steps as f32, Fmt::Count, 1.0, on_project("stair-steps", move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.steps = x.clamp(1.0, 40.0) as u32
            }
        })),
        num(s, th, "Direction", "stair-direction", st.direction, Fmt::Degrees, 5f32.to_radians(), on_project("stair-direction", move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.direction = x
            }
        })),
        toggle(th, "Closed risers", "stair-closed", st.closed, on_project("", move |p, on| {
            if let Some(r) = p.stair_mut(id) {
                r.closed = on
            }
        })),
        choice(th, "Railing", "stair-railing", &st.railing, &railings, on_project("", move |p, x| {
            if let Some(r) = p.stair_mut(id) {
                r.railing = x
            }
        })),
        fin(s, th, lib, "Treads", "stair-tread", &st.tread, move |p, f| {
            if let Some(r) = p.stair_mut(id) {
                r.tread = f
            }
        }),
        actions(th, false, true),
    ]
}

fn fixture(s: &State, th: &Th, f: &Fixture) -> Vec<El> {
    let id = f.id;
    let kind = f.kind;
    let on_what = match f.mount {
        Mount::Wall { .. } => "On a wall",
        Mount::Ceiling { .. } => "On the ceiling",
        Mount::Floor { .. } => "On the floor",
    };
    let mut v = vec![
        heading(th, kind.name(), on_what),
        name(s, th, "Label", "fixture-label", &f.label, on_project("", move |p, t: String| {
            if let Some(f) = p.fixture_mut(id) {
                f.label = t
            }
        })),
    ];
    let set_kind = move |tag: &'static str, make: fn(FixtureKind, f32) -> FixtureKind| on_project(tag, move |p: &mut Project, x: f32| {
        if let Some(f) = p.fixture_mut(id) {
            f.kind = make(f.kind, x)
        }
    });
    let step = units::snap_step(s.project.units);
    match kind {
        FixtureKind::Outlet { gangs } => v.push(num(s, th, "Sockets", "fx-gangs", gangs as f32, Fmt::Count, 1.0, set_kind("fx-gangs", |_, x| FixtureKind::Outlet { gangs: x.clamp(1.0, 6.0) as u32 }))),
        FixtureKind::Switch { gangs } => v.push(num(s, th, "Switches", "fx-gangs", gangs as f32, Fmt::Count, 1.0, set_kind("fx-gangs", |_, x| FixtureKind::Switch { gangs: x.clamp(1.0, 6.0) as u32 }))),
        FixtureKind::Downlight { diameter } => v.push(num(s, th, "Diameter", "fx-diameter", diameter, Fmt::Length, 0.01, set_kind("fx-diameter", |_, x| FixtureKind::Downlight { diameter: x.clamp(0.03, 0.5) }))),
        FixtureKind::Surface { diameter } => v.push(num(s, th, "Diameter", "fx-diameter", diameter, Fmt::Length, 0.01, set_kind("fx-diameter", |_, x| FixtureKind::Surface { diameter: x.clamp(0.05, 1.0) }))),
        FixtureKind::TubeLight { length, width } => {
            v.push(num(s, th, "Length", "fx-length", length, Fmt::Length, step, set_kind("fx-length", |k, x| match k {
                FixtureKind::TubeLight { width, .. } => FixtureKind::TubeLight { length: x.clamp(0.2, 6.0), width },
                k => k,
            })));
            v.push(num(s, th, "Width", "fx-width", width, Fmt::Length, 0.01, set_kind("fx-width", |k, x| match k {
                FixtureKind::TubeLight { length, .. } => FixtureKind::TubeLight { length, width: x.clamp(0.02, 0.6) },
                k => k,
            })));
        }
        FixtureKind::Pendant { diameter, drop } => {
            v.push(num(s, th, "Diameter", "fx-diameter", diameter, Fmt::Length, 0.01, set_kind("fx-diameter", |k, x| match k {
                FixtureKind::Pendant { drop, .. } => FixtureKind::Pendant { diameter: x.clamp(0.05, 1.5), drop },
                k => k,
            })));
            v.push(num(s, th, "Drop", "fx-drop", drop, Fmt::Length, step, set_kind("fx-drop", |k, x| match k {
                FixtureKind::Pendant { diameter, .. } => FixtureKind::Pendant { diameter, drop: x.clamp(0.1, 4.0) },
                k => k,
            })));
        }
        FixtureKind::Vent { width, height } => {
            v.push(num(s, th, "Width", "fx-width", width, Fmt::Length, step, set_kind("fx-width", |k, x| match k {
                FixtureKind::Vent { height, .. } => FixtureKind::Vent { width: x.clamp(0.1, 2.0), height },
                k => k,
            })));
            v.push(num(s, th, "Height", "fx-height", height, Fmt::Length, step, set_kind("fx-height", |k, x| match k {
                FixtureKind::Vent { width, .. } => FixtureKind::Vent { width, height: x.clamp(0.05, 2.0) },
                k => k,
            })));
        }
        FixtureKind::SmokeDetector | FixtureKind::Thermostat | FixtureKind::Data => {}
    }
    v.push(section(th, "Position"));
    match f.mount {
        Mount::Wall { side, at, height, .. } => {
            v.push(len(s, th, "Along wall", "fx-at", at, move |p, x| {
                if let Some(Mount::Wall { at, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    *at = x.max(0.0)
                }
            }));
            v.push(len(s, th, "Height", "fx-mount-height", height, move |p, x| {
                if let Some(Mount::Wall { height, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    *height = x.max(0.0)
                }
            }));
            v.push(choice(th, "Face", "fx-side", &side, &SIDES, on_project("", move |p, x| {
                if let Some(Mount::Wall { side, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    *side = x
                }
            })));
        }
        Mount::Ceiling { at, turn } | Mount::Floor { at, turn } => {
            let ceiling = matches!(f.mount, Mount::Ceiling { .. });
            v.push(len(s, th, "X", "fx-x", at[0], move |p, x| {
                if let Some(Mount::Ceiling { at, .. } | Mount::Floor { at, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    at[0] = x
                }
            }));
            v.push(len(s, th, "Y", "fx-y", at[1], move |p, x| {
                if let Some(Mount::Ceiling { at, .. } | Mount::Floor { at, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    at[1] = x
                }
            }));
            v.push(num(s, th, "Turn", "fx-turn", turn, Fmt::Degrees, 15f32.to_radians(), on_project("fx-turn", move |p, x| {
                if let Some(Mount::Ceiling { turn, .. } | Mount::Floor { turn, .. }) = p.fixture_mut(id).map(|f| &mut f.mount) {
                    *turn = x
                }
            })));
            v.push(choice(th, "Mounted on", "fx-surface", &ceiling, &[("Ceiling", true), ("Floor", false)], on_project("", move |p, up| {
                if let Some(f) = p.fixture_mut(id) {
                    if let Mount::Ceiling { at, turn } | Mount::Floor { at, turn } = f.mount {
                        f.mount = if up { Mount::Ceiling { at, turn } } else { Mount::Floor { at, turn } };
                    }
                }
            })));
        }
    }
    v.push(actions(th, true, !matches!(f.mount, Mount::Wall { .. })));
    v
}

fn furniture(s: &State, th: &Th, lib: &Library, f: &Furniture) -> Vec<El> {
    let id = f.id;
    let kind = crate::catalog::piece(&f.kind).map(|p| p.name.to_string()).unwrap_or_else(|| f.kind.clone());
    vec![
        heading(th, f.name.clone(), kind),
        name(s, th, "Name", "furn-name", &f.name, on_project("", move |p, t: String| {
            if let Some(f) = p.furniture_mut(id) {
                f.name = t
            }
        })),
        section(th, "Size"),
        len(s, th, "Width", "furn-w", f.size[0], move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.size[0] = x.max(0.05)
            }
        }),
        len(s, th, "Depth", "furn-d", f.size[1], move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.size[1] = x.max(0.05)
            }
        }),
        len(s, th, "Height", "furn-h", f.size[2], move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.size[2] = x.max(0.005)
            }
        }),
        section(th, "Position"),
        len(s, th, "X", "furn-x", f.at[0], move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.at[0] = x
            }
        }),
        len(s, th, "Y", "furn-y", f.at[1], move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.at[1] = x
            }
        }),
        len(s, th, "Off the floor", "furn-lift", f.lift, move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.lift = x.max(0.0)
            }
        }),
        num(s, th, "Turn", "furn-turn", f.turn, Fmt::Degrees, 15f32.to_radians(), on_project("furn-turn", move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.turn = x
            }
        })),
        section(th, "Finishes"),
        fin(s, th, lib, "Main", "furn-finish", &f.finish, move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.finish = x
            }
        }),
        fin(s, th, lib, "Second", "furn-accent", &f.accent, move |p, x| {
            if let Some(f) = p.furniture_mut(id) {
                f.accent = x
            }
        }),
        actions(th, true, true),
    ]
}

fn roof(s: &State, th: &Th, lib: &Library, r: &Roof) -> Vec<El> {
    let id = r.id;
    let shapes = [("Gable", RoofShape::Gable), ("Hip", RoofShape::Hip), ("Shed", RoofShape::Shed), ("Flat", RoofShape::Flat)];
    let ridge = r.base + if matches!(r.shape, RoofShape::Gable | RoofShape::Hip) { r.size[1] / 2.0 * r.pitch } else if r.shape == RoofShape::Shed { r.size[1] * r.pitch } else { 0.0 };
    vec![
        heading(th, "Roof", format!("{:.1}° pitch, top at {}", r.pitch.atan().to_degrees(), units::length(ridge, s.project.units))),
        name(s, th, "Name", "roof-name", &r.name, on_project("", move |p, t: String| {
            if let Some(r) = p.roof_mut(id) {
                r.name = t
            }
        })),
        choice(th, "Shape", "roof-shape", &r.shape, &shapes, on_project("", move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.shape = x
            }
        })),
        len(s, th, "Along ridge", "roof-length", r.size[0], move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.size[0] = x.max(0.5)
            }
        }),
        len(s, th, "Across", "roof-width", r.size[1], move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.size[1] = x.max(0.5)
            }
        }),
        num(s, th, "Pitch (rise/run)", "roof-pitch", r.pitch, Fmt::Number, 0.01, on_project("roof-pitch", move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.pitch = x.clamp(0.0, 3.0)
            }
        })),
        len(s, th, "Eaves height", "roof-base", r.base, move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.base = x
            }
        }),
        len(s, th, "Overhang", "roof-overhang", r.overhang, move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.overhang = x.clamp(0.0, 3.0)
            }
        }),
        len(s, th, "Thickness", "roof-thickness", r.thickness, move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.thickness = x.clamp(0.02, 1.0)
            }
        }),
        len(s, th, "Centre X", "roof-x", r.center[0], move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.center[0] = x
            }
        }),
        len(s, th, "Centre Y", "roof-y", r.center[1], move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.center[1] = x
            }
        }),
        num(s, th, "Turn", "roof-turn", r.turn, Fmt::Degrees, 15f32.to_radians(), on_project("roof-turn", move |p, x| {
            if let Some(r) = p.roof_mut(id) {
                r.turn = x
            }
        })),
        toggle(th, "Visible", "roof-visible", r.visible, on_project("", move |p, on| {
            if let Some(r) = p.roof_mut(id) {
                r.visible = on
            }
        })),
        fin(s, th, lib, "Roofing", "roof-finish", &r.finish, move |p, f| {
            if let Some(r) = p.roof_mut(id) {
                r.finish = f
            }
        }),
        fin(s, th, lib, "Gable ends", "roof-gable", &r.gable, move |p, f| {
            if let Some(r) = p.roof_mut(id) {
                r.gable = f
            }
        }),
        actions(th, false, true),
    ]
}

fn patch(s: &State, th: &Th, lib: &Library, r: &Patch) -> Vec<El> {
    let id = r.id;
    vec![
        heading(th, "Ground surface", units::area(geom::area(&r.outline).abs(), s.project.units)),
        name(s, th, "Name", "patch-name", &r.name, on_project("", move |p, t: String| {
            if let Some(r) = p.patch_mut(id) {
                r.name = t
            }
        })),
        fin(s, th, lib, "Surface", "patch-finish", &r.finish, move |p, f| {
            if let Some(r) = p.patch_mut(id) {
                r.finish = f
            }
        }),
        text("It drapes over the ground. Drag a corner to reshape; double-click an edge to add a corner.").font(SMALL).color(th.dim).wrap().mt(8.0),
        actions(th, false, false),
    ]
}

fn small_input(th: &Th, key: &'static str, edit: &rune::TextEdit, hint: &str, which: fn(&mut State) -> &mut rune::TextEdit) -> El {
    row().grow(1.0).min_w(0.0).h(26.0).px(8.0).radius(6.0).fill(th.field).border(1.0, th.border).center_y().child(
        text_input(edit, VALUE).single_line().key(key).grow(1.0).min_w(0.0).color(th.text).placeholder(hint, th.dim).caret(th.text, th.accent.alpha(0.3)).on_edit(which).on_submit(|s: &mut State, _: &mut Tx| fetch_terrain(s)),
    )
}

pub fn fetch_terrain(s: &mut State) {
    let (lat, lon) = (s.geo.lat.text.trim().parse::<f64>(), s.geo.lon.text.trim().parse::<f64>());
    let (Ok(lat), Ok(lon)) = (lat, lon) else {
        s.geo.status = "Enter a latitude and longitude in decimal degrees, like 39.7392 and -104.9903.".into();
        return;
    };
    let span = s.geo.span;
    s.geo.status = "Asking for the ground heights…".into();
    s.geo.request.request((Geo { lat, lon }, span), |(geo, span)| crate::geo::fetch(geo, span));
}

fn project_panel(s: &State, th: &Th, lib: &Library) -> Vec<El> {
    let p = &s.project;
    let mut v = vec![
        heading(th, "Project", s.file_name()),
        name(s, th, "Name", "project-name", &p.name, on_project("", |p, t: String| p.name = t)),
        choice(th, "Units", "project-units", &p.units, &[("Metric", Units::Metric), ("Feet & inches", Units::Imperial)], on_project("", |p, u| p.units = u)),
    ];

    if s.tool == Tool::Wall {
        v.push(section(th, "New walls"));
        v.push(num(s, th, "Thickness", "tool-wall-width", s.wall_width, Fmt::Length, units::snap_step(p.units), |s: &mut State, x| s.wall_width = x.clamp(0.02, 1.0)));
        let presets = [("Interior", 0.115), ("Exterior", 0.2), ("Masonry", 0.25)];
        let chips = presets.iter().map(|(name, t)| {
            let t = *t;
            chip(th, format!("wall-preset-{name}"), name, (s.wall_width - t).abs() < 0.001).on_press(move |s: &mut State| s.wall_width = t)
        });
        v.push(row().w_full().gap(4.0).pl(104.0).children(chips));
    }

    v.push(section(th, "Levels"));
    for l in p.levels.iter().rev() {
        let id = l.id;
        let active = id == s.level;
        v.push(
            row()
                .key(format!("level-row-{id}"))
                .w_full()
                .h(28.0)
                .px(8.0)
                .gap(8.0)
                .radius(6.0)
                .center_y()
                .fill(if active { th.accent.alpha(0.14) } else { th.field })
                .border(1.0, if active { th.accent } else { th.border })
                .hover(th.hover)
                .on_press(move |s: &mut State| {
                    s.commit_editing();
                    s.level = id;
                    s.selected = None;
                })
                .child(text(l.name.clone()).font(VALUE).color(th.text).grow(1.0).min_w(0.0).truncate())
                .child(text(format!("floor at {}", units::length(l.elevation, p.units))).font(SMALL).color(th.dim)),
        );
    }
    if let Some(l) = s.active() {
        let id = l.id;
        v.push(name(s, th, "Name", "level-name", &l.name, on_project("", move |p, t: String| {
            if let Some(l) = p.level_mut(id) {
                l.name = t
            }
        })));
        v.push(len(s, th, "Floor at", "level-elevation", l.elevation, move |p, x| {
            if let Some(l) = p.level_mut(id) {
                l.elevation = x
            }
        }));
        v.push(len(s, th, "Ceiling height", "level-height", l.height, move |p, x| {
            if let Some(l) = p.level_mut(id) {
                l.height = x.max(0.5)
            }
        }));
        v.push(len(s, th, "Floor thickness", "level-slab", l.slab, move |p, x| {
            if let Some(l) = p.level_mut(id) {
                l.slab = x.clamp(0.02, 1.5)
            }
        }));
        v.push(toggle(th, "Show in 3D", "level-visible", l.visible, on_project("", move |p, on| {
            if let Some(l) = p.level_mut(id) {
                l.visible = on
            }
        })));
        v.push(fact(
            th,
            "Contents",
            format!("{} walls, {} rooms, {} fixtures, {} pieces", l.walls.len(), l.rooms.len(), l.fixtures.len(), l.furniture.len()),
        ));
    }
    let level = s.level;
    v.push(
        row()
            .w_full()
            .gap(6.0)
            .mt(6.0)
            .flex_wrap()
            .child(button(th, "level-add-above", "Add level above", false).on_press(|s: &mut State| s.add_level(true)))
            .child(button(th, "level-add-below", "Add below", false).on_press(|s: &mut State| s.add_level(false)))
            .child(button(th, "level-remove", "Remove", false).on_press(move |s: &mut State| s.remove_level(level))),
    );

    // The site.
    let t = &p.site.terrain;
    let (lo, hi) = t.heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
    let size = t.size();
    v.push(section(th, "Site & ground"));
    v.push(text(t.source.clone()).font(SMALL).color(th.dim).wrap());
    v.push(fact(th, "Extent", format!("{} × {}", units::length(size[0], p.units), units::length(size[1], p.units))));
    v.push(fact(th, "Relief", if t.heights.is_empty() { "flat".into() } else { format!("{} to {}", units::length(lo, p.units), units::length(hi, p.units)) }));
    v.push(fin(s, th, lib, "Ground cover", "site-finish", &t.finish, |p, f| p.site.terrain.finish = f));
    v.push(num(s, th, "North is", "site-north", p.site.north.to_radians(), Fmt::Degrees, 5f32.to_radians(), on_project("site-north", |p, x| p.site.north = x.to_degrees())));

    v.push(text("Shape it as a plane").font(LABEL).color(th.text).mt(8.0));
    v.push(num(s, th, "Falls toward", "slope-bearing", s.slope.0.to_radians(), Fmt::Degrees, 5f32.to_radians(), |s: &mut State, x| s.slope.0 = x.to_degrees()));
    v.push(num(s, th, "Fall per metre", "slope-grade", s.slope.1, Fmt::Number, 0.01, |s: &mut State, x| s.slope.1 = x.clamp(0.0, 1.0)));
    v.push(text("0° falls to the east, 90° to the north (the top of the plan), 270° to the south.").font(SMALL).color(th.dim).wrap());
    v.push(
        row()
            .w_full()
            .gap(6.0)
            .mt(4.0)
            .child(button(th, "slope-apply", "Make this slope", false).on_press(|s: &mut State| {
                let (bearing, grade) = s.slope;
                s.edit("", |p| {
                    let (origin, size, finish) = (p.site.terrain.origin, p.site.terrain.size(), p.site.terrain.finish.clone());
                    p.site.terrain = Terrain { finish, ..Terrain::sloped(origin, size, 1.0, bearing.to_radians(), grade, [0.0, 0.0]) };
                    p.site.terrain.source = format!("A plane falling {grade:.2} m per metre toward {bearing:.0}°");
                })
            }))
            .child(button(th, "slope-flat", "Flatten", false).on_press(|s: &mut State| {
                s.edit("", |p| {
                    let (origin, size, finish) = (p.site.terrain.origin, p.site.terrain.size(), p.site.terrain.finish.clone());
                    p.site.terrain = Terrain { finish, source: "Flat ground".into(), ..Terrain::flat(origin, size, 1.0) };
                })
            })),
    );

    v.push(text("Or fetch the real ground").font(LABEL).color(th.text).mt(10.0));
    v.push(row().w_full().gap(6.0).child(small_input(th, "geo-lat", &s.geo.lat, "Latitude", |s| &mut s.geo.lat)).child(small_input(th, "geo-lon", &s.geo.lon, "Longitude", |s| &mut s.geo.lon)));
    v.push(num(s, th, "Area across", "geo-span", s.geo.span, Fmt::Length, 5.0, |s: &mut State, x| s.geo.span = x.clamp(20.0, 200.0)));
    let busy = s.geo.request.is_pending();
    v.push(row().w_full().mt(2.0).child(button(th, "geo-fetch", if busy { "Fetching…" } else { "Fetch ground heights" }, false).on_press(fetch_terrain)));
    v.push(
        text(if s.geo.status.is_empty() {
            "Sends a grid of coordinates around that point to opentopodata.org (or open-meteo.com) and replaces the ground with the answer. Elevation models are coarse (10–90 m), so expect the lot's overall fall, not its fine grading. The point becomes the plan's origin."
        } else {
            s.geo.status.as_str()
        })
        .font(SMALL)
        .color(th.dim)
        .wrap(),
    );
    v
}

pub fn view(s: &State, th: &Th) -> El {
    let lib = Library::new(&s.project);
    let p = &s.project;
    let rows = match s.selected {
        Some(Pick::Wall(id)) => p.levels.iter().find_map(|l| l.wall(id).map(|w| wall(s, th, &lib, l, w))),
        Some(Pick::Opening(w, o)) => p.wall(w).and_then(|wall| wall.openings.iter().find(|x| x.id == o).map(|o| opening(s, th, &lib, wall, o))),
        Some(Pick::Room(id)) => p.levels.iter().find_map(|l| l.rooms.iter().find(|r| r.id == id).map(|r| room(s, th, &lib, l, r))),
        Some(Pick::Stair(id)) => p.stair(id).map(|x| stair(s, th, &lib, x)),
        Some(Pick::Fixture(id)) => p.fixture(id).map(|x| fixture(s, th, x)),
        Some(Pick::Furniture(id)) => p.furniture(id).map(|x| furniture(s, th, &lib, x)),
        Some(Pick::Roof(id)) => p.roof(id).map(|x| roof(s, th, &lib, x)),
        Some(Pick::Patch(id)) => p.patch(id).map(|x| patch(s, th, &lib, x)),
        None => None,
    }
    .unwrap_or_else(|| project_panel(s, th, &lib));
    let body = col().w_full().gap(5.0).padding(12.0).children(rows);
    col().key("inspector").w(304.0).h_full().min_h(0.0).fill(th.panel).border_sides([0.0, 0.0, 0.0, 1.0], th.border).child(col().key("inspector-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}
