//! The window: top bar, tool panel, the drawing area for each view, the inspector and the
//! status line.

use rune::{Align, Cx, Drag, El, FileDialogOptions, Key, KeyEvent, LineCap, Phase, ScrollSpec, Shape, Tx, WheelEvent, canvas, col, effect, el, row, spacer, text};

use crate::app::{Side, State, Tool, ViewMode};
use crate::camera::{Projection, raycast};
use crate::catalog;
use crate::gfx;
use crate::model::*;
use crate::plan::{self, Ink};
use crate::units;
use crate::widgets::*;

// ---------------------------------------------------------------------------------------------
// Files

fn dialog(title: &str) -> FileDialogOptions {
    FileDialogOptions { title: title.into(), ..Default::default() }.filter("Lintel project", &["json"])
}

pub fn open_dialog(_: &mut State, tx: &mut Tx) {
    tx.open_file(dialog("Open a project"), |s: &mut State, result, _| match result {
        Ok(Some(path)) => s.open(path),
        Ok(None) => {}
        Err(e) => s.status = format!("Couldn't show the Open dialog ({e}). Start Lintel with a file instead: lintel my-house.lintel.json"),
    });
}

pub fn save(s: &mut State, tx: &mut Tx, ask: bool) {
    s.commit_editing();
    if s.path.is_some() && !ask {
        s.save();
        return;
    }
    let mut options = dialog("Save the project");
    options.file_name = Some(format!("{}.lintel.json", s.project.name.to_lowercase().replace(' ', "-")));
    tx.save_file(options, |s: &mut State, result, _| match result {
        Ok(Some(path)) => s.save_to(path),
        Ok(None) => {}
        // No dialog on this desktop: save beside where Lintel was started.
        Err(_) => s.save(),
    });
}

/// Copies a link that opens this project in the browser.
pub fn share(s: &mut State, tx: &mut Tx) {
    s.commit_editing();
    match crate::share::link(&s.project) {
        Ok(link) => {
            let size = link.len();
            tx.copy(link);
            s.status = if size > 60_000 {
                format!("Link copied. It is {} thousand characters long, which some chat apps cut short: test it before relying on it.", size / 1000)
            } else {
                "Link copied: anyone who opens it sees this house in their browser. The house travels inside the link; nothing was uploaded.".to_string()
            };
        }
        Err(e) => s.status = format!("Couldn't make a link: {e}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Keys

pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // A focused text field gets its keys first; this sees what it passes on.
    let typing = tx.focused().is_some_and(|k| k != "plan" && k != "viewport");
    let step = units::snap_step(s.project.units) * if ev.mods.shift { 10.0 } else { 1.0 };
    match &ev.key {
        Key::Command('z') if ev.mods.shift => s.redo(),
        Key::Command('z') => s.undo(),
        Key::Command('y') => s.redo(),
        Key::Command('s') => save(s, tx, ev.mods.shift),
        Key::Command('o') => open_dialog(s, tx),
        Key::Command('d') => s.duplicate_selected(),
        Key::Escape => {
            if s.editing.take().is_none() && s.picker.take().is_none() && s.linking.take().is_none() {
                if s.tool != Tool::Select {
                    s.set_tool(Tool::Select);
                } else {
                    s.select(None);
                }
            }
        }
        _ if typing || s.editing.is_some() => return false,
        Key::Delete | Key::Backspace => s.delete_selected(),
        Key::Left => s.nudge_selected([-step, 0.0]),
        Key::Right => s.nudge_selected([step, 0.0]),
        Key::Up => s.nudge_selected([0.0, step]),
        Key::Down => s.nudge_selected([0.0, -step]),
        Key::Text(t) => match t.as_str() {
            "1" => s.set_view(ViewMode::Plan),
            "2" => s.set_view(ViewMode::Elevation),
            "3" => s.set_view(ViewMode::Model),
            "v" => s.set_tool(Tool::Select),
            "w" => s.set_tool(Tool::Wall),
            "m" => s.set_tool(Tool::Room),
            "d" => s.set_tool(Tool::Door),
            "n" => s.set_tool(Tool::Window),
            "r" => s.turn_selected(std::f32::consts::FRAC_PI_2),
            "R" => s.turn_selected(-15f32.to_radians()),
            "f" => {
                s.plan_moved = false;
                s.fit_plan();
                s.camera.frame(s.scene().bounds);
                s.elevation_moved = false;
                s.aim_elevation(true);
            }
            _ => return false,
        },
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Top bar

fn top_bar(s: &State, th: &Th) -> El {
    let views = [("Plan", ViewMode::Plan), ("Elevation", ViewMode::Elevation), ("3D", ViewMode::Model)];
    let tabs = row().gap(2.0).padding(2.0).radius(8.0).fill(th.field).border(1.0, th.border).children(views.iter().map(|(name, v)| {
        let v = *v;
        let on = s.view == v;
        row()
            .key(format!("view-{name}"))
            .role("button")
            .h(24.0)
            .px(12.0)
            .radius(6.0)
            .center()
            .fill(if on { th.accent } else { rune::Color { a: 0.0, ..th.field } })
            .hover(if on { th.accent } else { th.hover })
            .child(text(*name).font(VALUE).color(if on { th.on_accent } else { th.text }))
            .on_press(move |s: &mut State| {
                s.commit_editing();
                s.set_view(v)
            })
    }));
    let levels = row().gap(4.0).children(s.project.levels.iter().map(|l| {
        let id = l.id;
        ghost(th, format!("level-{id}"), l.name.clone(), id == s.level).on_press(move |s: &mut State| {
            s.commit_editing();
            s.level = id;
            s.selected = None;
        })
    }));
    let title = row()
        .gap(8.0)
        .center_y()
        .child(text("Lintel").font(TITLE).color(th.text))
        .child(text(format!("{}{}", s.project.name, if s.dirty { " •" } else { "" })).font(VALUE).color(th.dim).truncate().max_w(220.0));
    row()
        .key("top")
        .w_full()
        .h(44.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .fill(th.panel)
        .border_sides([0.0, 0.0, 1.0, 0.0], th.border)
        .child(title)
        .child(tabs)
        .child(text("Level").font(SMALL).color(th.dim).ml(6.0))
        .child(levels)
        .child(spacer())
        .child(ghost(th, "undo", "Undo", false).opacity(if s.can_undo() { 1.0 } else { 0.4 }).on_press(|s: &mut State| s.undo()))
        .child(ghost(th, "redo", "Redo", false).opacity(if s.can_redo() { 1.0 } else { 0.4 }).on_press(|s: &mut State| s.redo()))
        .child(ghost(th, "new", "New", false).on_press(|s: &mut State| s.replace_project(crate::sample::blank(), None)))
        .child(ghost(th, "sample", "Sample", false).on_press(|s: &mut State| s.replace_project(crate::sample::house(), None)))
        .child(ghost(th, "open", "Open…", false).on_press_tx(open_dialog))
        .child(ghost(th, "share", "Share link", false).on_press_tx(share))
        .child(button(th, "save", "Save", false).on_press_tx(|s: &mut State, tx: &mut Tx| save(s, tx, false)))
        .child(ghost(th, "theme", if s.dark { "Light" } else { "Dark" }, false).on_press(|s: &mut State| s.dark = !s.dark))
}

/// The strip of switches that belong to the current view.
fn options_bar(s: &State, th: &Th) -> El {
    let mut bar = row().key("options").w_full().h(36.0).px(12.0).gap(6.0).center_y().fill(th.panel).border_sides([0.0, 0.0, 1.0, 0.0], th.border);
    let label = |t: &str| text(t.to_string()).font(SMALL).color(th.dim).ml(6.0);
    match s.view {
        ViewMode::Plan => {
            bar = bar
                .child(label("Show"))
                .child(ghost(th, "show-site", "Site", s.show_site).on_press(|s: &mut State| s.show_site = !s.show_site))
                .child(ghost(th, "show-furniture", "Furniture", s.show_furniture).on_press(|s: &mut State| s.show_furniture = !s.show_furniture))
                .child(ghost(th, "show-fixtures", "Fixtures", s.show_fixtures).on_press(|s: &mut State| s.show_fixtures = !s.show_fixtures))
                .child(ghost(th, "show-below", "Level below", s.show_below).on_press(|s: &mut State| s.show_below = !s.show_below))
                .child(ghost(th, "show-roofs", "Roofs", s.show_roofs).on_press(|s: &mut State| s.show_roofs = !s.show_roofs))
                .child(ghost(th, "show-lux", "Light levels", s.lux.show).on_press(|s: &mut State| if s.lux.levels.is_none() { s.calculate_light() } else { s.show_light(!s.lux.show) }))
                .child(spacer())
                .child(ghost(th, "fit", "Fit", false).on_press(|s: &mut State| {
                    s.plan_moved = false;
                    s.fit_plan()
                }));
        }
        ViewMode::Elevation => {
            bar = bar.child(label("From the"));
            for side in Side::ALL {
                bar = bar.child(ghost(th, format!("side-{}", side.name()), side.name(), s.side == side).on_press(move |s: &mut State| {
                    s.side = side;
                    s.elevation_moved = false;
                    s.aim_elevation(true);
                }));
            }
            bar = bar.child(label("Section")).child(ghost(th, "section", if s.section.is_some() { "Cutting" } else { "Off" }, s.section.is_some()).on_press(|s: &mut State| {
                s.section = if s.section.is_some() { None } else { Some(2.5) };
                s.aim_elevation(false);
            }));
            if let Some(depth) = s.section {
                bar = bar.child(el().w(236.0).child(num(s, th, "Cut depth", "section-depth", depth, Fmt::Length, 0.25, |s: &mut State, x| {
                    s.section = Some(x.clamp(0.0, 80.0));
                    s.aim_elevation(false);
                })));
            }
            bar = bar.child(spacer()).child(ghost(th, "fit", "Fit", false).on_press(|s: &mut State| {
                s.elevation_moved = false;
                s.aim_elevation(true)
            }));
        }
        ViewMode::Model => {
            let o = &s.options;
            bar = bar
                .child(label("Show"))
                .child(ghost(th, "opt-roofs", "Roof", o.roofs).on_press(|s: &mut State| s.set_options(|o| o.roofs = !o.roofs)))
                .child(ghost(th, "opt-ceilings", "Ceilings", o.ceilings).on_press(|s: &mut State| s.set_options(|o| o.ceilings = !o.ceilings)))
                .child(ghost(th, "opt-furniture", "Furniture", o.furniture).on_press(|s: &mut State| s.set_options(|o| o.furniture = !o.furniture)))
                .child(ghost(th, "opt-fixtures", "Fixtures", o.fixtures).on_press(|s: &mut State| s.set_options(|o| o.fixtures = !o.fixtures)))
                .child(ghost(th, "opt-terrain", "Ground", o.terrain).on_press(|s: &mut State| s.set_options(|o| o.terrain = !o.terrain)))
                .child(ghost(th, "show-lux", "Light levels", s.lux.show).on_press(|s: &mut State| if s.lux.levels.is_none() { s.calculate_light() } else { s.show_light(!s.lux.show) }))
                .child(label("Open up to"))
                .child(ghost(th, "top-all", "Whole house", o.top_level.is_none()).on_press(|s: &mut State| s.set_options(|o| o.top_level = None)));
            for l in &s.project.levels {
                let id = l.id;
                bar = bar.child(ghost(th, format!("top-{id}"), l.name.clone(), o.top_level == Some(id)).on_press(move |s: &mut State| s.set_options(|o| o.top_level = Some(id))));
            }
            let ortho = matches!(s.camera.projection, Projection::Orthographic { .. });
            bar = bar
                .child(spacer())
                .child(ghost(th, "projection", if ortho { "Parallel" } else { "Perspective" }, false).on_press(|s: &mut State| {
                    s.camera.projection = match s.camera.projection {
                        Projection::Perspective { fov_y } => Projection::Orthographic { height: 2.0 * s.camera.distance * (fov_y / 2.0).tan() },
                        Projection::Orthographic { .. } => Projection::Perspective { fov_y: 0.7 },
                    }
                }))
                .child(ghost(th, "fit", "Fit", false).on_press(|s: &mut State| {
                    let bounds = s.scene().bounds;
                    s.camera.frame(bounds)
                }));
        }
    }
    bar
}

// ---------------------------------------------------------------------------------------------
// Tools

fn tool_row(s: &State, th: &Th, key: String, label: &str, tool: Tool, hint: &str) -> El {
    let on = s.tool == tool;
    let mut r = row()
        .key(key)
        .role("button")
        .w_full()
        .h(26.0)
        .px(8.0)
        .gap(6.0)
        .radius(6.0)
        .center_y()
        .fill(if on { th.accent } else { rune::Color { a: 0.0, ..th.field } })
        .hover(if on { th.accent } else { th.hover })
        .child(text(label.to_string()).font(VALUE).color(if on { th.on_accent } else { th.text }).grow(1.0).min_w(0.0).truncate())
        .on_press(move |s: &mut State| {
            if s.view != ViewMode::Plan {
                s.set_view(ViewMode::Plan);
            }
            s.set_tool(tool.clone())
        });
    if !hint.is_empty() {
        r = r.child(text(hint.to_string()).font(SMALL).color(if on { th.on_accent } else { th.dim }));
    }
    r
}

fn group_head(s: &State, th: &Th, name: &str) -> El {
    let open = s.open_groups.iter().any(|g| g == name);
    let n = name.to_string();
    row()
        .key(format!("group-{name}"))
        .role("button")
        .w_full()
        .h(24.0)
        .px(8.0)
        .radius(6.0)
        .center_y()
        .hover(th.hover)
        .child(text(name.to_string()).font(LABEL).color(th.dim).grow(1.0))
        .child(text(if open { "–" } else { "+" }).font(VALUE).color(th.dim))
        .on_press(move |s: &mut State| match s.open_groups.iter().position(|g| *g == n) {
            Some(i) => {
                s.open_groups.remove(i);
            }
            None => s.open_groups.push(n.clone()),
        })
}

fn toolbox(s: &State, th: &Th) -> El {
    let mut list = col().w_full().gap(1.0).padding(8.0);
    let t = |key: &str, label: &str, tool: Tool, hint: &str| tool_row(s, th, format!("tool-{key}"), label, tool, hint);
    list = list
        .child(t("select", "Select", Tool::Select, "V"))
        .child(section(th, "Build"))
        .child(t("wall", "Wall", Tool::Wall, "W"))
        .child(t("room", "Room / floor", Tool::Room, "M"))
        .child(t("stair", "Stair", Tool::Stair, ""))
        .child(t("roof", "Roof", Tool::Roof, ""))
        .child(section(th, "In a wall"))
        .child(t("door", "Door", Tool::Door, "D"))
        .child(t("window", "Window", Tool::Window, "N"))
        .child(t("passage", "Opening", Tool::Passage, ""))
        .child(t("garage", "Garage door", Tool::Garage, ""))
        .child(t("recess", "Recess / niche", Tool::Recess, ""))
        .child(section(th, "Fixtures"));
    let fixtures = [
        ("Power outlet", FixtureKind::Outlet { gangs: 2 }),
        ("Light switch", FixtureKind::Switch { gangs: 1 }),
        ("Data outlet", FixtureKind::Data),
        ("Thermostat", FixtureKind::Thermostat),
        ("Smoke detector", FixtureKind::SmokeDetector),
        ("Downlight", FixtureKind::Downlight { diameter: 0.1 }),
        ("Recessed tube light", FixtureKind::TubeLight { length: 1.2, width: 0.09 }),
        ("LED strip", FixtureKind::Strip { length: 2.0 }),
        ("Surface light", FixtureKind::Surface { diameter: 0.3 }),
        ("Pendant light", FixtureKind::Pendant { diameter: 0.3, drop: 0.8 }),
        ("Air vent", FixtureKind::Vent { width: 0.6, height: 0.4 }),
    ];
    for (i, (name, kind)) in fixtures.iter().enumerate() {
        list = list.child(tool_row(s, th, format!("tool-fixture-{i}"), name, Tool::Fixture(*kind), ""));
    }
    list = list.child(section(th, "Outside")).child(t("patch", "Ground surface", Tool::Patch, ""));

    list = list.child(section(th, "Furniture & appliances"));
    let pieces = catalog::catalog();
    let mut groups: Vec<&str> = Vec::new();
    for p in &pieces {
        if !groups.contains(&p.group) {
            groups.push(p.group);
        }
    }
    for g in groups {
        list = list.child(group_head(s, th, g));
        if s.open_groups.iter().any(|o| o == g) {
            for p in pieces.iter().filter(|p| p.group == g) {
                list = list.child(row().w_full().pl(8.0).child(tool_row(s, th, format!("tool-piece-{}", p.key), p.name, Tool::Furniture(p.key.to_string()), "")));
            }
        }
    }
    col().key("tools").w(184.0).h_full().min_h(0.0).fill(th.panel).border_sides([0.0, 1.0, 0.0, 0.0], th.border).child(col().key("tools-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
}

// ---------------------------------------------------------------------------------------------
// The 3D and elevation viewport

fn viewport_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    s.canvas = [d.w, d.h];
    let size = s.canvas;
    match d.phase {
        Phase::Down => {
            s.commit_editing();
            s.picker = None;
            tx.focus("viewport", false);
            // `data` remembers how far the pointer had travelled at the last event.
            d.data = 0.0;
        }
        Phase::Move => {
            // Travel since the last event, from the total since the press.
            let (px, py) = (s.drag_last[0], s.drag_last[1]);
            let (dx, dy) = if d.data == 0.0 { (d.dx, d.dy) } else { (d.dx - px, d.dy - py) };
            d.data = 1.0;
            s.drag_last = [d.dx, d.dy];
            match s.view {
                ViewMode::Model if !tx.mods().shift => s.camera.orbit(-dx * 0.008, dy * 0.008),
                ViewMode::Model => s.camera.pan(dx, dy, size),
                _ => {
                    s.elevation_moved = true;
                    s.elevation.pan(dx, dy, size)
                }
            }
        }
        Phase::Up => {
            // A press without travel selects what's under it.
            if d.dx.abs() + d.dy.abs() < 4.0 && !d.cancelled() {
                let camera = if s.view == ViewMode::Model { s.camera } else { s.elevation };
                let (origin, dir) = camera.ray([d.x, d.y], size);
                let pick = raycast(s.scene(), origin, dir, camera.cut).map(|(p, _)| p);
                s.select(pick);
            }
            s.drag_last = [0.0, 0.0];
        }
    }
}

fn viewport_wheel(s: &mut State, e: WheelEvent, _: &mut Tx) -> bool {
    let factor = (-e.dy * 0.004).exp();
    match s.view {
        ViewMode::Model => s.camera.zoom(factor),
        _ => {
            s.elevation_moved = true;
            s.elevation.zoom(factor)
        }
    }
    true
}

/// Level lines and names over an elevation.
fn elevation_overlay(s: &State, size: [f32; 2]) -> Vec<El> {
    let ink = Ink::new(false);
    let mut shape = Shape::new();
    let mut out = Vec::new();
    let c = s.elevation.target;
    let line = plan::hex(0x1f6feb).alpha(0.55);
    let mut mark = |shape: Shape, z: f32, name: String| -> Shape {
        let Some(p) = s.elevation.project([c[0], c[1], z], size) else { return shape };
        if p[1] < 8.0 || p[1] > size[1] - 8.0 {
            return shape;
        }
        let mut shape = shape;
        let mut x = 96.0;
        while x < size[0] - 8.0 {
            shape = shape.line([x, p[1]], [(x + 7.0).min(size[0] - 8.0), p[1]], 1.0, LineCap::Butt, line);
            x += 13.0;
        }
        out.push(
            row().absolute(8.0, p[1] - 9.0).h(18.0).px(6.0).radius(4.0).center_y().fill(plan::hex(0xffffff).alpha(0.85)).child(text(name).font(SMALL).color(ink.text)).inert(),
        );
        shape
    };
    for l in &s.project.levels {
        shape = mark(shape, l.elevation, format!("{}  {}", l.name, units::length(l.elevation, s.project.units)));
    }
    if let Some(top) = s.project.levels.last() {
        shape = mark(shape, top.elevation + top.height, format!("Ceiling  {}", units::length(top.elevation + top.height, s.project.units)));
    }
    out.insert(0, canvas(shape).absolute(0.0, 0.0).w_full().h_full().inert());
    out
}

fn viewport(s: &State, th: &Th, size: [f32; 2]) -> El {
    let slot = effect(gfx::SLOT).key("slot").absolute(0.0, 0.0).w_full().h_full();
    let mut wrap = el()
        .key("viewport")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(plan::hex(if s.dark { 0x17181b } else { 0xdfe7ee }))
        .clip_rounded(0.0)
        .focusable(true)
        .no_focus_ring()
        .on_layout(|s: &mut State, r| crate::edit::canvas_resized(s, [r[2], r[3]]))
        .on_drag(viewport_drag)
        .on_wheel(viewport_wheel)
        .on_magnify(|s: &mut State, e, _| {
            match s.view {
                ViewMode::Model => s.camera.zoom(1.0 - e.delta),
                _ => s.elevation.zoom(1.0 - e.delta),
            }
            true
        })
        .child(slot);
    if s.view == ViewMode::Elevation {
        wrap = wrap.children(elevation_overlay(s, size));
        let title = format!("{} elevation{}", s.side.name(), if s.section.is_some() { " · section" } else { "" });
        wrap = wrap.child(text(title).font(TITLE).color(plan::hex(0x23262b)).align(Align::End).w(300.0).absolute(size[0] - 316.0, 12.0).inert());
    }
    let _ = th;
    wrap
}

// ---------------------------------------------------------------------------------------------
// Status line

fn status_bar(s: &State, th: &Th) -> El {
    let u = s.project.units;
    let message = if !s.status.is_empty() {
        s.status.clone()
    } else {
        match s.view {
            ViewMode::Plan if s.linking.is_some() => "Click lights to add them to this switch, or take them off it. Esc when done.".to_string(),
            ViewMode::Plan => s.tool.hint().to_string(),
            ViewMode::Elevation => "Drag to pan, scroll to zoom, click to select. Turn on Section to cut through the house.".into(),
            ViewMode::Model => "Drag to orbit, Shift-drag to pan, scroll to zoom, click to select. “Open up to” lifts the roof and upper floors off.".into(),
        }
    };
    let mut bar = row().key("status").w_full().h(26.0).px(12.0).gap(14.0).center_y().fill(th.panel).border_sides([1.0, 0.0, 0.0, 0.0], th.border).child(text(message).font(SMALL).color(th.dim).grow(1.0).min_w(0.0).truncate());
    if s.view == ViewMode::Plan {
        if let Some(c) = s.cursor {
            bar = bar.child(text(format!("{}, {}", units::length(c[0], u), units::length(c[1], u))).font(SMALL).color(th.dim));
            if let Some(v) = s.lux.levels.as_ref().filter(|_| s.lux.show).and_then(|l| l.rooms.iter().filter(|r| r.level == s.level).find_map(|r| r.at(c))) {
                bar = bar.child(text(format!("{} lux · {}", round_lux(v), crate::lux::describe(v))).font(SMALL).color(th.text));
            }
            let ground = s.project.site.terrain.height(c);
            bar = bar.child(text(format!("ground {}", units::length(ground, u))).font(SMALL).color(th.dim));
        }
        bar = bar.child(text(format!("1 m = {:.0} pt", s.plan.zoom)).font(SMALL).color(th.dim));
    }
    bar
}

// ---------------------------------------------------------------------------------------------

/// The top bar on a narrow screen: the views and the levels, nothing to edit with.
fn compact_bar(s: &State, th: &Th) -> El {
    let views = [("Plan", ViewMode::Plan), ("Side", ViewMode::Elevation), ("3D", ViewMode::Model)];
    let mut bar = row().key("compact").w_full().h(44.0).px(8.0).gap(4.0).center_y().fill(th.panel).border_sides([0.0, 0.0, 1.0, 0.0], th.border).scroll_x();
    for (name, v) in views {
        bar = bar.child(ghost(th, format!("view-{name}"), name, s.view == v).on_press(move |s: &mut State| s.set_view(v)));
    }
    bar = bar.child(rune::el().w(1.0).h(20.0).fill(th.border).mx(4.0));
    if s.view == ViewMode::Model {
        bar = bar.child(ghost(th, "top-all", "Whole", s.options.top_level.is_none()).on_press(|s: &mut State| s.set_options(|o| o.top_level = None)));
        for l in &s.project.levels {
            let id = l.id;
            bar = bar.child(ghost(th, format!("top-{id}"), l.name.clone(), s.options.top_level == Some(id)).on_press(move |s: &mut State| s.set_options(|o| o.top_level = Some(id))));
        }
    } else {
        for l in &s.project.levels {
            let id = l.id;
            bar = bar.child(ghost(th, format!("level-{id}"), l.name.clone(), id == s.level).on_press(move |s: &mut State| {
                s.level = id;
                s.selected = None;
            }));
        }
    }
    bar
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let th = Th::new(s.dark);
    let size = match s.view {
        ViewMode::Plan => cx.layout_size("plan-wrap"),
        _ => cx.layout_size("viewport"),
    }
    .filter(|z| z[0] > 1.0 && z[1] > 1.0)
    .unwrap_or(s.canvas);
    let stage = match s.view {
        ViewMode::Plan => plan::view(s, size),
        _ => viewport(s, &th, size),
    };
    if s.bare {
        return col().key("root").w_full().h_full().fill(th.bg).child(row().key("body").w_full().grow(1.0).min_h(0.0).child(stage));
    }
    // A phone gets the house to look at; editing needs the room a desktop window has.
    let narrow = cx.layout_size("root").is_some_and(|z| z[0] < 820.0);
    if narrow {
        return col().key("root").w_full().h_full().fill(th.bg).child(compact_bar(s, &th)).child(row().key("body").w_full().grow(1.0).min_h(0.0).child(stage));
    }
    col()
        .key("root")
        .w_full()
        .h_full()
        .fill(th.bg)
        .child(top_bar(s, &th))
        .child(options_bar(s, &th))
        .child(row().key("body").w_full().grow(1.0).min_h(0.0).child(toolbox(s, &th)).child(stage).child(crate::inspector::view(s, &th)))
        .child(status_bar(s, &th))
}
