//! Pointer editing on the plan: hit testing, snapping, dragging and the drawing tools.

use std::f32::consts::FRAC_PI_2;

use rune::{Drag, MagnifyEvent, Phase, PointerEvent, Tx, WheelEvent};

use crate::app::{DragOp, State, Tool, translate};
use crate::geom::{self, add, sub};
use crate::model::*;
use crate::plan::fixture_point;
use crate::units;

// ---------------------------------------------------------------------------------------------
// Finding things under the pointer

/// The wall nearest `p` within `reach` metres of its face, and how far along it `p` projects.
pub fn nearest_wall(level: &Level, p: P2, reach: f32) -> Option<(&Wall, f32)> {
    level
        .walls
        .iter()
        .map(|w| (w, geom::segment_distance(p, w.a, w.b) - w.thickness / 2.0))
        .filter(|(_, d)| *d < reach)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(w, _)| (w, geom::project(p, w.a, w.b).0))
}

/// What's under a plan point on the active level. `slop` is the pointer's reach in metres.
pub fn hit(s: &State, p: P2, slop: f32) -> Option<Pick> {
    let level = s.active()?;
    if s.show_fixtures {
        for f in level.fixtures.iter().rev() {
            let Some((at, facing)) = fixture_point(level, f) else { continue };
            let on_wall = matches!(f.mount, Mount::Wall { .. });
            let reach = match f.kind {
                FixtureKind::TubeLight { length, .. } | FixtureKind::Strip { length } => length / 2.0,
                FixtureKind::Vent { width, .. } => width / 2.0,
                FixtureKind::Pendant { diameter, .. } | FixtureKind::Surface { diameter } => diameter / 2.0,
                _ => 0.0,
            }
            .max(slop * 1.4);
            // Wall symbols are drawn just off the face.
            let c = if on_wall { add(at, geom::scale([facing.cos(), facing.sin()], slop * 1.2)) } else { at };
            if geom::dist(p, c) < reach {
                return Some(Pick::Fixture(f.id));
            }
        }
    }
    for w in &level.walls {
        let (along, across) = geom::project(p, w.a, w.b);
        if across.abs() > w.thickness / 2.0 + slop * 0.5 {
            continue;
        }
        for o in &w.openings {
            if (along - o.at).abs() < o.width / 2.0 && geom::segment_distance(p, w.a, w.b) < w.thickness / 2.0 + slop * 0.5 {
                return Some(Pick::Opening(w.id, o.id));
            }
        }
    }
    for w in &level.walls {
        if geom::segment_distance(p, w.a, w.b) < w.thickness / 2.0 + slop * 0.4 {
            return Some(Pick::Wall(w.id));
        }
    }
    if s.show_furniture {
        // Smallest first: a coffee table on a rug wins over the rug.
        let mut pieces: Vec<&Furniture> = level.furniture.iter().filter(|f| geom::inside(p, &geom::oriented_rect(f.at, f.size[0], f.size[1], f.turn))).collect();
        pieces.sort_by(|a, b| (a.size[0] * a.size[1]).partial_cmp(&(b.size[0] * b.size[1])).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(f) = pieces.first() {
            return Some(Pick::Furniture(f.id));
        }
    }
    for st in &level.stairs {
        let center = add(st.at, geom::scale([st.direction.cos(), st.direction.sin()], st.run / 2.0));
        if geom::inside(p, &geom::oriented_rect(center, st.run, st.width, st.direction)) {
            return Some(Pick::Stair(st.id));
        }
    }
    if s.show_roofs {
        for r in &s.project.roofs {
            let outline = geom::oriented_rect(r.center, r.size[0] + r.overhang * 2.0, r.size[1] + r.overhang * 2.0, r.turn);
            if (0..4).any(|i| geom::segment_distance(p, outline[i], outline[(i + 1) % 4]) < slop) {
                return Some(Pick::Roof(r.id));
            }
        }
    }
    if s.show_site {
        for b in &s.project.site.barriers {
            if b.path.windows(2).any(|w| geom::segment_distance(p, w[0], w[1]) < b.thickness / 2.0 + slop) {
                return Some(Pick::Barrier(b.id));
            }
        }
        // A tree is picked by its trunk, so what stands under its canopy can still be reached.
        for t in &s.project.site.plants {
            if geom::dist(p, t.at) < (t.spread * 0.18).max(slop * 1.5) {
                return Some(Pick::Plant(t.id));
            }
        }
    }
    if let Some(r) = level.rooms.iter().rev().find(|r| geom::inside(p, &r.outline)) {
        return Some(Pick::Room(r.id));
    }
    if s.show_site {
        if let Some(r) = s.project.site.patches.iter().rev().find(|r| geom::inside(p, &r.outline)) {
            return Some(Pick::Patch(r.id));
        }
    }
    None
}

/// A handle of the current selection under `p`: a wall end or an outline corner.
fn hit_handle(s: &State, p: P2, slop: f32) -> Option<DragOp> {
    let level = s.active()?;
    match s.selected? {
        Pick::Wall(id) => {
            let w = level.wall(id)?;
            let end = if geom::dist(p, w.a) < slop {
                w.a
            } else if geom::dist(p, w.b) < slop {
                w.b
            } else {
                return None;
            };
            // Every wall end at that corner moves together; the selected wall's goes first.
            let mut ends: Vec<(Id, u8)> = vec![(id, if end == w.a { 0 } else { 1 })];
            for o in level.walls.iter().filter(|o| o.id != id) {
                if geom::dist(o.a, end) < 0.02 {
                    ends.push((o.id, 0));
                }
                if geom::dist(o.b, end) < 0.02 {
                    ends.push((o.id, 1));
                }
            }
            Some(DragOp::WallEnds { ends })
        }
        pick @ Pick::Room(id) => {
            let r = level.rooms.iter().find(|r| r.id == id)?;
            r.outline.iter().position(|v| geom::dist(p, *v) < slop).map(|index| DragOp::Vertex { pick, index })
        }
        pick @ Pick::Patch(id) => {
            let r = s.project.site.patches.iter().find(|r| r.id == id)?;
            r.outline.iter().position(|v| geom::dist(p, *v) < slop).map(|index| DragOp::Vertex { pick, index })
        }
        pick @ Pick::Barrier(id) => {
            let r = s.project.barrier(id)?;
            r.path.iter().position(|v| geom::dist(p, *v) < slop).map(|index| DragOp::Vertex { pick, index })
        }
        _ => None,
    }
}

fn outline_mut(project: &mut Project, pick: Pick) -> Option<&mut Vec<P2>> {
    match pick {
        Pick::Room(id) => project.levels.iter_mut().flat_map(|l| &mut l.rooms).find(|r| r.id == id).map(|r| &mut r.outline),
        Pick::Patch(id) => project.site.patches.iter_mut().find(|r| r.id == id).map(|r| &mut r.outline),
        Pick::Barrier(id) => project.barrier_mut(id).map(|r| &mut r.path),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Snapping

/// Snaps a drawing point: to a nearby wall end, else onto a nearby wall's centre line, else to
/// the grid. `skip` leaves out walls being moved.
pub fn snap_point(s: &State, p: P2, skip: &[Id]) -> P2 {
    let step = units::snap_step(s.project.units);
    let reach = 9.0 / s.plan.zoom;
    if let Some(level) = s.active() {
        let ends = level.walls.iter().filter(|w| !skip.contains(&w.id)).flat_map(|w| [w.a, w.b]);
        if let Some(end) = ends.min_by(|a, b| geom::dist(*a, p).partial_cmp(&geom::dist(*b, p)).unwrap_or(std::cmp::Ordering::Equal)) {
            if geom::dist(end, p) < reach {
                return end;
            }
        }
        for w in level.walls.iter().filter(|w| !skip.contains(&w.id)) {
            if geom::segment_distance(p, w.a, w.b) < reach * 0.7 {
                let (along, _) = geom::project(p, w.a, w.b);
                return w.point(geom::snap(along, step), 0.0);
            }
        }
    }
    geom::snap2(p, step)
}

/// Keeps `p` on the horizontal or vertical through `from` when it's within about 6° of it.
fn ortho(from: P2, p: P2) -> P2 {
    let d = sub(p, from);
    if d[0].abs() > d[1].abs() * 9.5 {
        [p[0], from[1]]
    } else if d[1].abs() > d[0].abs() * 9.5 {
        [from[0], p[1]]
    } else {
        p
    }
}

// ---------------------------------------------------------------------------------------------
// New things

/// The opening a wall tool places, sized for `wall` (id 0, at 0: the caller sets both).
pub fn default_opening(tool: &Tool, wall: &Wall) -> Option<Opening> {
    let trim = Finish::paint(0xf7f5f0);
    let (width, height, sill, kind): (f32, f32, f32, OpeningKind) = match tool {
        Tool::Door => (
            0.82,
            2.04,
            0.0,
            OpeningKind::Door(Door { style: DoorStyle::Hinged, hinge: Hinge::Start, swing: WallSide::Left, open: 0.0, leaf: trim, frame: Some(Frame::default()), glazing: 0.0 }),
        ),
        Tool::Window => (1.2, 1.2, 0.9, OpeningKind::Window(Window { casing: Some(Frame::default()), ..Window::new(WindowStyle::Casement) })),
        Tool::Passage => (1.0, 2.1, 0.0, OpeningKind::Passage { frame: Some(Frame::default()) }),
        Tool::Garage => (
            if wall.length() >= 5.6 { 4.88 } else { 2.44 },
            2.13,
            0.0,
            OpeningKind::Garage(GarageDoor { style: GarageStyle::Sectional, inside: WallSide::Left, open: 0.0, panels: 4, finish: Finish::paint(0xeceae4) }),
        ),
        Tool::Recess => (0.6, 0.9, 1.0, OpeningKind::Recess { side: WallSide::Left, depth: (wall.thickness * 0.6).min(0.1), finish: Finish::paint(0xf2efe8) }),
        _ => return None,
    };
    Some(Opening { id: 0, at: 0.0, width: width.min(wall.length() - 0.1).max(0.2), height, sill, kind })
}

/// Where a new fixture of this kind goes when the plan is clicked at `p`.
pub fn default_mount(level: &Level, kind: FixtureKind, p: P2) -> Option<Mount> {
    let on_wall = |height: f32| {
        let (w, at) = nearest_wall(level, p, 0.8)?;
        let (_, across) = geom::project(p, w.a, w.b);
        Some(Mount::Wall { wall: w.id, side: if across >= 0.0 { WallSide::Left } else { WallSide::Right }, at, height })
    };
    match kind {
        FixtureKind::Outlet { .. } | FixtureKind::Data => on_wall(0.3),
        FixtureKind::Switch { .. } => on_wall(1.1),
        FixtureKind::Thermostat => on_wall(1.5),
        // A vent goes on a wall when the click is by one, else in the ceiling.
        FixtureKind::Vent { .. } => on_wall(0.35).or(Some(Mount::Ceiling { at: p, turn: 0.0 })),
        FixtureKind::Surface { .. } => Some(Mount::Ceiling { at: p, turn: 0.0 }),
        FixtureKind::SmokeDetector | FixtureKind::Downlight { .. } | FixtureKind::TubeLight { .. } | FixtureKind::Pendant { .. } => Some(Mount::Ceiling { at: p, turn: 0.0 }),
        // A strip goes along a wall near its top when the click is by one, else on the ceiling.
        FixtureKind::Strip { .. } => on_wall(2.2).or(Some(Mount::Ceiling { at: p, turn: 0.0 })),
    }
}

fn level_mut(s: &mut State) -> Option<&mut Level> {
    let id = s.level;
    s.project.level_mut(id)
}

/// A click with a placing tool.
fn place(s: &mut State, p: P2) {
    let tool = s.tool.clone();
    let Some(level) = s.active() else { return };
    match &tool {
        Tool::Door | Tool::Window | Tool::Passage | Tool::Garage | Tool::Recess => {
            let Some((wall, at)) = nearest_wall(level, p, 0.6) else {
                s.status = "Click on a wall to place that.".into();
                return;
            };
            let Some(mut o) = default_opening(&tool, wall) else { return };
            let (_, across) = geom::project(p, wall.a, wall.b);
            let side = if across >= 0.0 { WallSide::Left } else { WallSide::Right };
            // The side you click from is the side a door swings to, a garage door retracts to,
            // or a recess is cut into.
            match &mut o.kind {
                OpeningKind::Door(d) => d.swing = side,
                OpeningKind::Garage(g) => g.inside = side,
                OpeningKind::Recess { side: s, .. } => *s = side,
                _ => {}
            }
            let step = units::snap_step(s.project.units);
            o.at = geom::snap(at, step).clamp(o.width / 2.0, (wall.length() - o.width / 2.0).max(o.width / 2.0));
            let wall_id = wall.id;
            s.checkpoint("");
            o.id = s.project.id();
            let id = o.id;
            if let Some(w) = level_mut(s).and_then(|l| l.walls.iter_mut().find(|w| w.id == wall_id)) {
                w.openings.push(o);
            }
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Opening(wall_id, id)));
        }
        Tool::Fixture(kind) => {
            let Some(mount) = default_mount(level, *kind, p) else {
                s.status = format!("{}s go on a wall: click beside one.", kind.name());
                return;
            };
            s.checkpoint("");
            let id = s.project.id();
            if let Some(l) = level_mut(s) {
                l.fixtures.push(Fixture::new(id, *kind, mount));
            }
            s.touch();
            // Stay in the tool: fixtures usually go in several at a time.
            s.selected = Some(Pick::Fixture(id));
        }
        Tool::Product(index) => {
            let Some(product) = s.project.products.get(*index).cloned() else { return };
            s.checkpoint("");
            let id = s.project.id();
            let step = units::snap_step(s.project.units);
            let piece = product.place(id, geom::snap2(p, step));
            if let Some(l) = level_mut(s) {
                l.furniture.push(piece);
            }
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Furniture(id)));
        }
        Tool::Plant(kind) => {
            s.checkpoint("");
            let id = s.project.id();
            s.project.site.plants.push(kind.new(id, p));
            s.show_site = true;
            s.touch();
            // Stay in the tool: planting usually goes several at a time.
            s.selected = Some(Pick::Plant(id));
        }
        Tool::Furniture(key) => {
            s.checkpoint("");
            let id = s.project.id();
            let step = units::snap_step(s.project.units);
            let Some(piece) = crate::catalog::place(key, id, geom::snap2(p, step)) else { return };
            if let Some(l) = level_mut(s) {
                l.furniture.push(piece);
            }
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Furniture(id)));
        }
        _ => {}
    }
}

/// The end of a drawing drag from `a` to `b`.
fn finish_draw(s: &mut State, a: P2, b: P2) {
    let (lo, hi) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
    let rect = vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
    let big = hi[0] - lo[0] > 0.2 && hi[1] - lo[1] > 0.2;
    let tool = s.tool.clone();
    match tool {
        Tool::Calibrate if geom::dist(a, b) * s.plan.zoom > 8.0 => {
            s.calibrate = Some((a, b));
            s.tool = Tool::Select;
            s.select(None);
            s.status = "Now type what that length really is, under “That line is really” on the right.".into();
        }
        Tool::Wall if geom::dist(a, b) > 0.1 => {
            s.checkpoint("");
            let id = s.project.id();
            let thickness = s.wall_width;
            let paint = Finish::paint(0xf2efe8);
            if let Some(l) = level_mut(s) {
                l.walls.push(Wall { id, a, b, thickness, height: None, base: 0.0, left: paint.clone(), right: paint, openings: vec![], bands: vec![], layers: vec![] });
            }
            s.touch();
            s.selected = Some(Pick::Wall(id));
        }
        Tool::Room if big => {
            s.checkpoint("");
            let id = s.project.id();
            if let Some(l) = level_mut(s) {
                let name = format!("Room {}", l.rooms.len() + 1);
                l.rooms.push(Room { id, name, outline: rect, floor: Finish::of("oak_boards"), ceiling: Finish::paint(0xfafaf7), ceiling_height: None, floor_offset: 0.0, has_ceiling: true, balustrade: None, ceiling_shape: CeilingShape::Flat, skirting: None, cornice: None, target_lux: None });
            }
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Room(id)));
        }
        Tool::Stair if geom::dist(a, b) > 0.5 => {
            s.checkpoint("");
            let id = s.project.id();
            let index = s.project.levels.iter().position(|l| l.id == s.level).unwrap_or(0);
            let here = &s.project.levels[index];
            // Up to the next level's floor, or one storey when this is the top level.
            let rise = s.project.levels.get(index + 1).map(|up| up.elevation - here.elevation).unwrap_or(here.height + 0.3).max(0.3);
            let steps = (rise / 0.18).round().max(2.0) as u32;
            let d = sub(b, a);
            if let Some(l) = level_mut(s) {
                l.stairs.push(Stair { id, at: a, direction: d[1].atan2(d[0]), width: 0.95, run: geom::dist(a, b), rise, steps, tread: Finish::of("oak"), closed: true, railing: Railing::None });
            }
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Stair(id)));
        }
        Tool::Barrier(kind) if geom::dist(a, b) > 0.2 => {
            s.checkpoint("");
            let id = s.project.id();
            // A new retaining wall holds a terrace behind it.
            let level_behind = if kind == BarrierKind::Retaining { 3.0 } else { 0.0 };
            s.project.site.barriers.push(Barrier { level_behind, ..kind.new(id, vec![a, b]) });
            s.show_site = true;
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Barrier(id)));
        }
        Tool::Patch if big => {
            s.checkpoint("");
            let id = s.project.id();
            let name = format!("Ground {}", s.project.site.patches.len() + 1);
            s.project.site.patches.push(Patch { id, name, outline: rect, finish: Finish::of("concrete") });
            s.show_site = true;
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Patch(id)));
        }
        Tool::Roof if big => {
            s.checkpoint("");
            let id = s.project.id();
            let top = s.project.levels.last().map(|l| l.elevation + l.height).unwrap_or(2.7);
            let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
            // The ridge runs the long way.
            let (size, turn) = if w >= d { ([w, d], 0.0) } else { ([d, w], FRAC_PI_2) };
            let name = format!("Roof {}", s.project.roofs.len() + 1);
            s.project.roofs.push(Roof {
                id,
                name,
                shape: RoofShape::Gable,
                center: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
                size,
                turn,
                base: top,
                pitch: 0.42,
                overhang: 0.5,
                thickness: 0.2,
                finish: Finish::of("shingle"),
                gable: Finish::colored("siding", rgb(0xdfe3e6)),
                visible: true,
            });
            s.show_roofs = true;
            s.touch();
            s.tool = Tool::Select;
            s.select(Some(Pick::Roof(id)));
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Handlers

pub fn canvas_resized(s: &mut State, size: [f32; 2]) {
    if size[0] < 1.0 || size[1] < 1.0 || size == s.canvas {
        return;
    }
    s.canvas = size;
    if !s.plan_moved {
        s.fit_plan();
    }
    s.aim_elevation(!s.elevation_moved);
}

pub fn plan_hover(s: &mut State, e: PointerEvent, _: &mut Tx) {
    let p = s.plan.to_model([e.x, e.y], s.canvas);
    s.cursor = Some(match s.tool {
        Tool::Select | Tool::Door | Tool::Window | Tool::Passage | Tool::Garage | Tool::Recess | Tool::Fixture(_) | Tool::Plant(_) => p,
        _ => snap_point(s, p, &[]),
    });
}

pub fn plan_wheel(s: &mut State, e: WheelEvent, _: &mut Tx) -> bool {
    s.plan_moved = true;
    if e.precise && !e.mods.cmd && !e.mods.ctrl {
        // A trackpad's two-finger scroll pans.
        s.plan.center = add(s.plan.center, [-e.dx / s.plan.zoom, e.dy / s.plan.zoom]);
    } else {
        s.plan.zoom_about([e.x, e.y], s.canvas, (e.dy * 0.004).exp());
    }
    true
}

pub fn plan_magnify(s: &mut State, e: MagnifyEvent, _: &mut Tx) -> bool {
    s.plan_moved = true;
    s.plan.zoom_about([e.x, e.y], s.canvas, 1.0 + e.delta);
    true
}

pub fn plan_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    s.canvas = [d.w, d.h];
    let raw = s.plan.to_model([d.x, d.y], s.canvas);
    let slop = 7.0 / s.plan.zoom;
    let step = units::snap_step(s.project.units);
    let free = tx.mods().shift;
    match d.phase {
        Phase::Down => {
            s.commit_editing();
            s.picker = None;
            s.status.clear();
            tx.focus("plan", false);
            match s.tool.clone() {
                Tool::Select => {
                    if let Some(op) = hit_handle(s, raw, slop * 1.3) {
                        s.checkpoint("");
                        s.drag = op;
                        return;
                    }
                    // A double click on the edge of the selected outline adds a corner there.
                    if d.clicks == 2 {
                        if let Some(pick @ (Pick::Room(_) | Pick::Patch(_) | Pick::Barrier(_))) = s.selected {
                            s.checkpoint("");
                            let at = geom::snap2(raw, step);
                            if let Some(outline) = outline_mut(&mut s.project, pick) {
                                let n = outline.len();
                                let edges = if matches!(pick, Pick::Barrier(_)) { n.saturating_sub(1) } else { n };
                                let edge = (0..edges).min_by(|a, b| {
                                    let da = geom::segment_distance(raw, outline[*a], outline[(*a + 1) % n]);
                                    let db = geom::segment_distance(raw, outline[*b], outline[(*b + 1) % n]);
                                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                                });
                                if let Some(i) = edge.filter(|i| geom::segment_distance(raw, outline[*i], outline[(*i + 1) % n]) < slop * 2.0) {
                                    outline.insert(i + 1, at);
                                    s.drag = DragOp::Vertex { pick, index: i + 1 };
                                    s.touch();
                                    return;
                                }
                            }
                            s.forget_checkpoint();
                        }
                    }
                    let under = hit(s, raw, slop);
                    // Picking lights for a switch: a click on a light links or unlinks it.
                    if let (Some(switch), Some(Pick::Fixture(light))) = (s.linking, under) {
                        if light != switch {
                            s.link(switch, light);
                            return;
                        }
                    }
                    // A double click on a switch flicks it.
                    if let (2, Some(Pick::Fixture(id))) = (d.clicks, under) {
                        if s.project.fixture(id).is_some_and(|f| matches!(f.kind, FixtureKind::Switch { .. })) {
                            s.flick(id);
                            return;
                        }
                    }
                    match under {
                        Some(pick) => {
                            s.select(Some(pick));
                            s.checkpoint("");
                            s.drag = DragOp::Move { pick, grab: raw };
                        }
                        None => {
                            s.select(None);
                            s.drag = DragOp::Pan { center: s.plan.center };
                        }
                    }
                }
                Tool::Calibrate => s.drag = DragOp::Draw { a: raw, b: raw },
                Tool::Wall | Tool::Room | Tool::Stair | Tool::Patch | Tool::Roof | Tool::Barrier(_) => {
                    let a = snap_point(s, raw, &[]);
                    s.drag = DragOp::Draw { a, b: a };
                }
                _ => {
                    place(s, raw);
                    s.drag = DragOp::Idle;
                }
            }
        }
        Phase::Move => match s.drag.clone() {
            DragOp::Idle => {}
            DragOp::Pan { center } => {
                s.plan_moved = true;
                s.plan.center = add(center, [-d.dx / s.plan.zoom, d.dy / s.plan.zoom]);
            }
            DragOp::Draw { a, .. } if s.tool == Tool::Calibrate => s.drag = DragOp::Draw { a, b: raw },
            DragOp::Draw { a, .. } => {
                let mut b = snap_point(s, raw, &[]);
                if s.tool == Tool::Wall && !free {
                    b = ortho(a, b);
                }
                s.drag = DragOp::Draw { a, b };
            }
            DragOp::WallEnds { ends } => {
                let ids: Vec<Id> = ends.iter().map(|e| e.0).collect();
                let mut p = snap_point(s, raw, &ids);
                // Keep the first wall square to its other end when it's nearly so.
                if !free {
                    if let Some(w) = s.active().and_then(|l| l.wall(ends[0].0)) {
                        p = ortho(if ends[0].1 == 0 { w.b } else { w.a }, p);
                    }
                }
                if let Some(l) = level_mut(s) {
                    for (id, end) in &ends {
                        if let Some(w) = l.walls.iter_mut().find(|w| w.id == *id) {
                            if *end == 0 { w.a = p } else { w.b = p }
                        }
                    }
                }
            }
            DragOp::Vertex { pick, index } => {
                let p = snap_point(s, raw, &[]);
                if let Some(v) = outline_mut(&mut s.project, pick).and_then(|o| o.get_mut(index)) {
                    *v = p;
                }
            }
            DragOp::Move { pick, grab } => {
                if d.dx.abs() + d.dy.abs() < 3.0 {
                    return;
                }
                move_pick(s, pick, grab, raw, step);
            }
        },
        Phase::Up => {
            let op = std::mem::replace(&mut s.drag, DragOp::Idle);
            match op {
                DragOp::Draw { a, b } if !d.cancelled() => finish_draw(s, a, b),
                DragOp::Move { .. } | DragOp::WallEnds { .. } | DragOp::Vertex { .. } => {
                    if d.cancelled() {
                        s.undo();
                    } else if s.before().is_some_and(|b| *b == s.project) {
                        s.forget_checkpoint();
                    } else {
                        // A corner dropped onto its neighbour removes it.
                        if let DragOp::Vertex { pick, index } = op {
                            if let Some(o) = outline_mut(&mut s.project, pick) {
                                let n = o.len();
                                if n > 3 && (geom::dist(o[index], o[(index + 1) % n]) < 0.03 || geom::dist(o[index], o[(index + n - 1) % n]) < 0.03) {
                                    o.remove(index);
                                }
                            }
                        }
                        s.touch();
                        s.settle();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Moves `pick` so the point grabbed at `grab` follows the pointer at `raw`.
fn move_pick(s: &mut State, pick: Pick, grab: P2, raw: P2, step: f32) {
    let Some(before) = s.before().cloned() else { return };
    match pick {
        Pick::Opening(wall, opening) => {
            if let Some(w) = level_mut(s).and_then(|l| l.walls.iter_mut().find(|w| w.id == wall)) {
                let (along, _) = geom::project(raw, w.a, w.b);
                let len = w.length();
                if let Some(o) = w.openings.iter_mut().find(|o| o.id == opening) {
                    o.at = geom::snap(along, step).clamp(o.width / 2.0, (len - o.width / 2.0).max(o.width / 2.0));
                }
            }
        }
        Pick::Fixture(id) => {
            let Some(level) = s.active() else { return };
            let Some(f) = level.fixtures.iter().find(|f| f.id == id) else { return };
            match f.mount {
                // A wall fixture slides along its wall, and hops to another wall the pointer reaches.
                Mount::Wall { height, .. } => {
                    let Some((w, at)) = nearest_wall(level, raw, 1.0) else { return };
                    let (_, across) = geom::project(raw, w.a, w.b);
                    let mount = Mount::Wall { wall: w.id, side: if across >= 0.0 { WallSide::Left } else { WallSide::Right }, at: geom::snap(at, step), height };
                    if let Some(f) = level_mut(s).and_then(|l| l.fixtures.iter_mut().find(|f| f.id == id)) {
                        f.mount = mount;
                    }
                }
                _ => translate(&mut s.project, &before, pick, geom::snap2(sub(raw, grab), step)),
            }
        }
        _ => translate(&mut s.project, &before, pick, geom::snap2(sub(raw, grab), step)),
    }
}
