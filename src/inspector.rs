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
        if w.layers.is_empty() {
            len(s, th, "Thickness", "wall-thickness", w.thickness, move |p, v| {
                if let Some(w) = p.wall_mut(id) {
                    w.thickness = v.clamp(0.02, 1.5)
                }
            })
        } else {
            fact(th, "Thickness", format!("{} (from its build-up)", units::length(w.thickness, s.project.units)))
        },
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
    v.push(section(th, "Build-up, left face to right"));
    let presets = Wall::presets();
    let chips = presets.iter().map(|(name, layers)| {
        let layers = layers.clone();
        chip(th, format!("layers-{name}"), name, w.layers == layers).on_press(move |s: &mut State| {
            let layers = layers.clone();
            s.edit("", |p| {
                if let Some(w) = p.wall_mut(id) {
                    w.set_layers(layers)
                }
            })
        })
    });
    v.push(row().w_full().flex_wrap().gaps(4.0, 4.0).children(chips));
    if w.layers.is_empty() {
        v.push(text("A build-up says what the wall is made of. Its layers add up to the wall's thickness and show on the plan when you zoom in, and on top of the walls with the roof off.").font(SMALL).color(th.dim).wrap());
    }
    for (i, layer) in w.layers.iter().enumerate() {
        let kinds: Vec<(&str, LayerKind)> = LayerKind::ALL.iter().map(|k| (k.name(), *k)).collect();
        v.push(
            row()
                .w_full()
                .gap(8.0)
                .mt(4.0)
                .center_y()
                .child(rune::el().size(12.0, 12.0).radius(2.0).fill(crate::plan::srgb(layer.kind.color())).border(1.0, th.border))
                .child(text(format!("{}. {}", i + 1, layer.kind.name())).font(VALUE).color(th.text).grow(1.0))
                .child(chip(th, format!("layer-{i}-remove"), "Remove", false).on_press(move |s: &mut State| {
                    s.edit("", |p| {
                        if let Some(w) = p.wall_mut(id) {
                            let mut layers = w.layers.clone();
                            if i < layers.len() {
                                layers.remove(i);
                            }
                            w.set_layers(layers)
                        }
                    })
                })),
        );
        v.push(num(s, th, "Thickness", &format!("layer-{i}-thickness"), layer.thickness, Fmt::Length, 0.005, on_project("layer-thickness", move |p: &mut Project, x: f32| {
            if let Some(w) = p.wall_mut(id) {
                let mut layers = w.layers.clone();
                if let Some(l) = layers.get_mut(i) {
                    l.thickness = x.clamp(0.001, 1.0)
                }
                w.set_layers(layers)
            }
        })));
        v.push(choice(th, "Made of", &format!("layer-{i}-kind"), &layer.kind, &kinds, on_project("", move |p: &mut Project, k: LayerKind| {
            if let Some(l) = p.wall_mut(id).and_then(|w| w.layers.get_mut(i)) {
                l.kind = k
            }
        })));
    }
    let mut buttons = row().w_full().gap(6.0).mt(4.0).flex_wrap().child(button(th, "layer-add", "Add a layer", false).on_press(move |s: &mut State| {
        s.edit("", |p| {
            if let Some(w) = p.wall_mut(id) {
                let mut layers = w.layers.clone();
                if layers.is_empty() {
                    // Start from the wall as it is: one layer of its whole thickness.
                    layers.push(Layer { kind: LayerKind::Framing, thickness: w.thickness });
                } else {
                    layers.push(Layer { kind: LayerKind::Lining, thickness: 0.01 });
                }
                w.set_layers(layers)
            }
        })
    }));
    if !w.layers.is_empty() {
        buttons = buttons.child(button(th, "layers-clear", "No build-up", false).on_press(move |s: &mut State| {
            s.edit("", |p| {
                if let Some(w) = p.wall_mut(id) {
                    w.layers.clear()
                }
            })
        }));
    }
    v.push(buttons);
    v.push(section(th, "Part-height finishes"));
    if w.bands.is_empty() {
        v.push(text("Tiling to a height, a splashback or wainscoting: a finish over part of one face.").font(SMALL).color(th.dim).wrap());
    }
    for (i, band) in w.bands.iter().enumerate() {
        fn band_mut(p: &mut Project, wall: Id, i: usize) -> Option<&mut Band> {
            p.wall_mut(wall)?.bands.get_mut(i)
        }
        let step = units::snap_step(s.project.units);
        v.push(choice(th, "On the", &format!("band-{i}-side"), &band.side, &SIDES, on_project("", move |p: &mut Project, x: WallSide| {
            if let Some(b) = band_mut(p, id, i) {
                b.side = x
            }
        })));
        v.push(num(s, th, "From", &format!("band-{i}-from"), band.from, Fmt::Length, step, on_project("band-from", move |p: &mut Project, x: f32| {
            if let Some(b) = band_mut(p, id, i) {
                b.from = x.clamp(-5.0, b.to - 0.01)
            }
        })));
        v.push(num(s, th, "Up to", &format!("band-{i}-to"), band.to, Fmt::Length, step, on_project("band-to", move |p: &mut Project, x: f32| {
            if let Some(b) = band_mut(p, id, i) {
                b.to = x.max(b.from + 0.01)
            }
        })));
        v.push(finish(s, th, lib, "Finish", &format!("band-{i}-finish"), &band.finish, on_project("", move |p: &mut Project, f: Finish| {
            if let Some(b) = band_mut(p, id, i) {
                b.finish = f
            }
        })));
        v.push(row().w_full().pl(104.0).child(chip(th, format!("band-{i}-remove"), "Remove", false).on_press(move |s: &mut State| {
            s.edit("", |p| {
                if let Some(w) = p.wall_mut(id) {
                    if i < w.bands.len() {
                        w.bands.remove(i);
                    }
                }
            })
        })));
    }
    v.push(row().w_full().mt(2.0).child(button(th, "band-add", "Add a part-height finish", false).on_press(move |s: &mut State| {
        s.edit("", |p| {
            if let Some(w) = p.wall_mut(id) {
                w.bands.push(Band { side: WallSide::Left, from: 0.0, to: 1.2, finish: Finish::colored("subway_tile", rgb(0xf6f6f3)) });
            }
        })
    })));
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
    let mut v = vec![toggle(th, "Frame & trim", "frame-on", frame.is_some(), on_project("", move |p, on: bool| {
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
            v.push(num(s, th, "Open", "door-open", d.open, Fmt::Percent, 0.05, on_project("door-open", move |p, x: f32| {
                if let Some(d) = door(p, wall, id) {
                    d.open = x.clamp(0.0, 1.0)
                }
            })));
            v.push(num(s, th, "Glazed", "door-glazing", d.glazing, Fmt::Percent, 0.05, on_project("door-glazing", move |p, x: f32| {
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
            v.push(num(s, th, "Open", "garage-open", g.open, Fmt::Percent, 0.05, on_project("garage-open", move |p, x: f32| {
                if let Some(g) = garage(p, wall, id) {
                    g.open = x.clamp(0.0, 1.0)
                }
            })));
            if g.style == GarageStyle::Sectional {
                v.push(num(s, th, "Panels", "garage-panels", g.panels as f32, Fmt::Count, 1.0, on_project("garage-panels", move |p, x: f32| {
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
                v.push(num(s, th, "Panes across", "window-cols", win.cols as f32, Fmt::Count, 1.0, on_project("window-cols", move |p, x: f32| {
                    if let Some(w) = window(p, wall, id) {
                        w.cols = x.clamp(1.0, 12.0) as u32
                    }
                })));
                v.push(num(s, th, "Panes up", "window-rows", win.rows as f32, Fmt::Count, 1.0, on_project("window-rows", move |p, x: f32| {
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
    if !r.has_ceiling {
        v.push(toggle(th, "Balustrade", "room-balustrade", r.balustrade.is_some(), on_project("", move |p: &mut Project, on: bool| {
            if let Some(r) = p.room_mut(id) {
                r.balustrade = on.then_some(1.0)
            }
        })));
        if let Some(h) = r.balustrade {
            v.push(len(s, th, "Rail height", "room-balustrade-height", h, move |p, x| {
                if let Some(r) = p.room_mut(id) {
                    r.balustrade = Some(x.clamp(0.3, 2.0))
                }
            }));
        }
        v.push(text("A floor with nothing over it gets posts down to the ground wherever it stands clear of it, and a balustrade along the edges that have no wall.").font(SMALL).color(th.dim).wrap());
    }
    if r.has_ceiling {
        v.push(len(s, th, "Ceiling height", "room-height", r.ceiling_height.unwrap_or(level.height), move |p, x| {
            if let Some(r) = p.room_mut(id) {
                // Back at the level's own height, it follows the level again.
                r.ceiling_height = if (x - level_height).abs() < 0.001 { None } else { Some(x.max(0.5)) }
            }
        }));
        let kind = match r.ceiling_shape {
            CeilingShape::Flat => 0,
            CeilingShape::Sloped { .. } => 1,
            CeilingShape::Vaulted { .. } => 2,
        };
        v.push(choice(th, "Ceiling is", "room-shape", &kind, &[("Flat", 0), ("Sloped", 1), ("Vaulted", 2)], on_project("", move |p: &mut Project, k: i32| {
            if let Some(r) = p.room_mut(id) {
                // Keep the rise and direction when switching between the two shaped kinds.
                let (rise, angle) = match r.ceiling_shape {
                    CeilingShape::Flat => (1.0, std::f32::consts::FRAC_PI_2),
                    CeilingShape::Sloped { rise, toward } => (rise, toward),
                    CeilingShape::Vaulted { rise, along } => (rise, along),
                };
                r.ceiling_shape = match k {
                    1 => CeilingShape::Sloped { rise, toward: angle },
                    2 => CeilingShape::Vaulted { rise, along: angle },
                    _ => CeilingShape::Flat,
                };
            }
        })));
        match r.ceiling_shape {
            CeilingShape::Flat => {}
            CeilingShape::Sloped { rise, toward } => {
                v.push(len(s, th, "Rises by", "room-rise", rise, move |p, x| {
                    if let Some(CeilingShape::Sloped { rise, .. }) = p.room_mut(id).map(|r| &mut r.ceiling_shape) {
                        *rise = x.clamp(0.0, 10.0)
                    }
                }));
                v.push(num(s, th, "Toward", "room-toward", toward, Fmt::Degrees, 15f32.to_radians(), on_project("room-toward", move |p: &mut Project, x: f32| {
                    if let Some(CeilingShape::Sloped { toward, .. }) = p.room_mut(id).map(|r| &mut r.ceiling_shape) {
                        *toward = x
                    }
                })));
                v.push(text("The height above is the low edge. 0° rises to the east, 90° to the top of the plan.").font(SMALL).color(th.dim).wrap());
            }
            CeilingShape::Vaulted { rise, along } => {
                v.push(len(s, th, "Rises by", "room-rise", rise, move |p, x| {
                    if let Some(CeilingShape::Vaulted { rise, .. }) = p.room_mut(id).map(|r| &mut r.ceiling_shape) {
                        *rise = x.clamp(0.0, 10.0)
                    }
                }));
                v.push(num(s, th, "Ridge runs", "room-along", along, Fmt::Degrees, 15f32.to_radians(), on_project("room-along", move |p: &mut Project, x: f32| {
                    if let Some(CeilingShape::Vaulted { along, .. }) = p.room_mut(id).map(|r| &mut r.ceiling_shape) {
                        *along = x
                    }
                })));
                v.push(text("The height above is at the eaves; the ridge runs down the middle. 0° runs east-west, 90° up the plan.").font(SMALL).color(th.dim).wrap());
            }
        }
        v.push(fin(s, th, lib, "Ceiling", "room-ceiling", &r.ceiling, move |p, f| {
            if let Some(r) = p.room_mut(id) {
                r.ceiling = f
            }
        }));
    }
    v.push(section(th, "Trim"));
    for (label, key, trim, cornice) in [("Skirting", "room-skirting", &r.skirting, false), ("Cornice", "room-cornice", &r.cornice, true)] {
        fn slot(r: &mut Room, cornice: bool) -> &mut Option<Trim> {
            if cornice { &mut r.cornice } else { &mut r.skirting }
        }
        v.push(toggle(th, label, key, trim.is_some(), on_project("", move |p: &mut Project, on: bool| {
            if let Some(r) = p.room_mut(id) {
                *slot(r, cornice) = on.then(|| if cornice { Trim::cornice() } else { Trim::skirting() })
            }
        })));
        if let Some(t) = trim {
            let (kh, kd, kf): (&'static str, &'static str, &'static str) = if cornice { ("cornice-height", "cornice-depth", "cornice-finish") } else { ("skirting-height", "skirting-depth", "skirting-finish") };
            v.push(len(s, th, "Height", kh, t.height, move |p, x| {
                if let Some(Some(t)) = p.room_mut(id).map(|r| slot(r, cornice)) {
                    t.height = x.clamp(0.01, 0.6)
                }
            }));
            v.push(len(s, th, "Stands off", kd, t.depth, move |p, x| {
                if let Some(Some(t)) = p.room_mut(id).map(|r| slot(r, cornice)) {
                    t.depth = x.clamp(0.003, 0.4)
                }
            }));
            v.push(fin(s, th, lib, "Finish", kf, &t.finish, move |p, f| {
                if let Some(Some(t)) = p.room_mut(id).map(|r| slot(r, cornice)) {
                    t.finish = f
                }
            }));
        }
    }
    v.push(section(th, "Light"));
    let targets = [("None", None), ("150", Some(150.0f32)), ("300", Some(300.0)), ("500", Some(500.0))];
    v.push(choice(th, "Target (lux)", "room-target", &r.target_lux, &targets, on_project("", move |p: &mut Project, x: Option<f32>| {
        if let Some(r) = p.room_mut(id) {
            r.target_lux = x
        }
    })));
    if let Some(t) = r.target_lux {
        v.push(num(s, th, "Target", "room-target-lux", t, Fmt::Count, 50.0, on_project("room-target-lux", move |p: &mut Project, x: f32| {
            if let Some(r) = p.room_mut(id) {
                r.target_lux = Some(x.clamp(1.0, 100_000.0))
            }
        })));
    }
    if let Some(lit) = s.lux.levels.as_ref().and_then(|l| l.rooms.iter().find(|x| x.room == id)) {
        if let Some(t) = r.target_lux {
            let short = lit.mean < t * 0.97;
            v.push(text(if short { format!("Below its target: {} of {} lux.", round_lux(lit.mean), round_lux(t)) } else { format!("Meets its target of {} lux.", round_lux(t)) }).font(VALUE).color(if short { crate::plan::hex(0xc9772b) } else { crate::plan::hex(0x2f9e7d) }).wrap());
        }
        v.push(fact(th, "Average", format!("{} lux", round_lux(lit.mean))));
        v.push(fact(th, "Range", format!("{} to {} lux", round_lux(lit.min), round_lux(lit.max))));
        v.push(text(format!("{}{}", crate::lux::describe(lit.mean), if s.lux.stale { " (calculated before the last change)" } else { "" })).font(SMALL).color(th.dim).wrap());
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
        num(s, th, "Steps", "stair-steps", st.steps as f32, Fmt::Count, 1.0, on_project("stair-steps", move |p, x: f32| {
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
        FixtureKind::Strip { length } => v.push(num(s, th, "Length", "fx-length", length, Fmt::Length, step, set_kind("fx-length", |_, x| FixtureKind::Strip { length: x.clamp(0.1, 30.0) }))),
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
    if let Some(lamp) = f.lamp() {
        v.extend(lamp_rows(s, th, f, lamp));
    }
    if matches!(kind, FixtureKind::Switch { .. }) {
        v.extend(switch_rows(s, th, f));
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

/// Changes a light's lamp, starting from the one it has (its own or its kind's usual one).
fn set_lamp<T: 'static>(tag: &'static str, id: Id, change: impl Fn(&mut Lamp, T) + 'static) -> impl Fn(&mut State, T) + 'static {
    on_project(tag, move |p: &mut Project, x: T| {
        if let Some(f) = p.fixture_mut(id) {
            if let Some(mut lamp) = f.lamp() {
                change(&mut lamp, x);
                f.lamp = Some(lamp);
            }
        }
    })
}

fn lamp_rows(s: &State, th: &Th, f: &Fixture, lamp: Lamp) -> Vec<El> {
    let id = f.id;
    let linear = matches!(f.kind, FixtureKind::TubeLight { .. } | FixtureKind::Strip { .. });
    let sources = [("LED", Source::Led), ("Incandescent", Source::Incandescent), ("Halogen", Source::Halogen), ("Fluorescent", Source::Fluorescent)];
    let total = if lamp.on { f.output() / lamp.dim.clamp(0.0, 1.0).max(1e-6) } else { f.output() };
    let full = {
        let per = match f.kind {
            FixtureKind::TubeLight { length, .. } | FixtureKind::Strip { length } => length,
            _ => 1.0,
        };
        lamp.lumens * per
    };
    let _ = total;
    let temps = [("2700", 2700.0f32), ("3000", 3000.0), ("4000", 4000.0), ("5000", 5000.0), ("6500", 6500.0)];
    let chips = temps.iter().map(|(name, k)| {
        let k = *k;
        let set = set_lamp("", id, move |l: &mut Lamp, _: ()| l.kelvin = k);
        chip(th, format!("lamp-k-{name}"), name, (lamp.kelvin - k).abs() < 1.0).on_press(move |s: &mut State| set(s, ()))
    });
    vec![
        section(th, "Lamp"),
        toggle(th, "On", "lamp-on", lamp.on, set_lamp("", id, |l: &mut Lamp, on: bool| l.on = on)),
        choice(th, "Source", "lamp-source", &lamp.source, &sources, set_lamp("", id, |l: &mut Lamp, x: Source| {
            l.source = x;
            l.kelvin = x.kelvin();
        })),
        num(s, th, if linear { "Lumens per metre" } else { "Lumens" }, "lamp-lumens", lamp.lumens, Fmt::Count, 50.0, set_lamp("lamp-lumens", id, |l: &mut Lamp, x: f32| l.lumens = x.clamp(1.0, 100_000.0))),
        num(s, th, "Colour (K)", "lamp-kelvin", lamp.kelvin, Fmt::Count, 100.0, set_lamp("lamp-kelvin", id, |l: &mut Lamp, x: f32| l.kelvin = x.clamp(1500.0, 10_000.0))),
        row().w_full().gap(4.0).pl(104.0).flex_wrap().gaps(4.0, 4.0).children(chips),
        num(s, th, "Dimmed to", "lamp-dim", lamp.dim, Fmt::Percent, 0.05, set_lamp("lamp-dim", id, |l: &mut Lamp, x: f32| l.dim = x.clamp(0.0, 1.0))),
        fact(th, "At full", format!("{:.0} lm, about {:.0} W", full, lamp.watts(full))),
    ]
}

fn switch_rows(s: &State, th: &Th, f: &Fixture) -> Vec<El> {
    let id = f.id;
    let lights: Vec<&Fixture> = f.controls.iter().filter_map(|c| s.project.fixture(*c)).collect();
    let any_on = lights.iter().any(|l| l.lamp().is_some_and(|l| l.on));
    let picking = s.linking == Some(id);
    let mut v = vec![section(th, "Controls")];
    if lights.is_empty() {
        v.push(text("No lights yet. Pick the lights this switch turns on and off.").font(SMALL).color(th.dim).wrap());
    }
    for l in &lights {
        let light = l.id;
        let label = if l.label.is_empty() { l.kind.name().to_string() } else { l.label.clone() };
        let on = l.lamp().is_some_and(|l| l.on);
        v.push(
            row()
                .w_full()
                .h(26.0)
                .gap(8.0)
                .center_y()
                .child(text(label).font(VALUE).color(th.text).grow(1.0).min_w(0.0).truncate())
                .child(text(if on { "on" } else { "off" }).font(SMALL).color(th.dim))
                .child(chip(th, format!("unlink-{light}"), "Remove", false).on_press(move |s: &mut State| s.link(id, light))),
        );
    }
    v.push(
        row()
            .w_full()
            .gap(6.0)
            .mt(4.0)
            .flex_wrap()
            .child(button(th, "switch-flick", if any_on { "Turn off" } else { "Turn on" }, false).on_press(move |s: &mut State| s.flick(id)))
            .child(button(th, "switch-pick", if picking { "Done picking" } else { "Pick lights on the plan" }, picking).on_press(move |s: &mut State| {
                s.linking = if s.linking == Some(id) { None } else { Some(id) };
                if s.linking.is_some() && s.view != crate::app::ViewMode::Plan {
                    s.set_view(crate::app::ViewMode::Plan);
                }
            })),
    );
    v.push(text("Double-click the switch on the plan to flick it.").font(SMALL).color(th.dim).wrap());
    v
}

fn sun_rows(s: &State, th: &Th) -> Vec<El> {
    let site = &s.project.site;
    let sun = site.sun;
    let (bearing, height) = site.sun_position();
    let geo = site.geo.unwrap_or(Geo { lat: 40.0, lon: 0.0 });
    let mut v = vec![
        section(th, "Sun"),
        choice(th, "Set by", "sun-mode", &sun.by_clock, &[("Hand", false), ("Place & time", true)], on_project("", |p: &mut Project, x: bool| {
            p.site.sun.by_clock = x;
            if x && p.site.geo.is_none() {
                p.site.geo = Some(Geo { lat: 40.0, lon: 0.0 });
            }
        })),
    ];
    v.push(choice(th, "Sky", "sun-sky", &sun.overcast, &[("Clear", false), ("Overcast", true)], on_project("", |p: &mut Project, x: bool| p.site.sun.overcast = x)));
    if sun.by_clock {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        v.push(num(s, th, "Latitude", "sun-lat", geo.lat as f32, Fmt::Number, 0.5, on_project("sun-lat", |p: &mut Project, x: f32| {
            let g = p.site.geo.get_or_insert(Geo { lat: 40.0, lon: 0.0 });
            g.lat = (x as f64).clamp(-89.0, 89.0);
        })));
        v.push(num(s, th, "Longitude", "sun-lon", geo.lon as f32, Fmt::Number, 0.5, on_project("sun-lon", |p: &mut Project, x: f32| {
            let g = p.site.geo.get_or_insert(Geo { lat: 40.0, lon: 0.0 });
            g.lon = (x as f64).clamp(-180.0, 180.0);
            p.site.sun.utc_offset = crate::solar::natural_offset(g.lon);
        })));
        v.push(num(s, th, "Clock is UTC", "sun-utc", sun.utc_offset, Fmt::Number, 1.0, on_project("sun-utc", |p: &mut Project, x: f32| p.site.sun.utc_offset = x.clamp(-12.0, 14.0))));
        v.push(num(s, th, "Month", "sun-month", sun.month as f32, Fmt::Count, 1.0, on_project("sun-month", |p: &mut Project, x: f32| p.site.sun.month = ((x as i32 - 1).rem_euclid(12) + 1) as u32)));
        v.push(num(s, th, "Day", "sun-day", sun.day as f32, Fmt::Count, 1.0, on_project("sun-day", |p: &mut Project, x: f32| p.site.sun.day = x.clamp(1.0, 31.0) as u32)));
        v.push(num(s, th, "Time", "sun-hour", sun.hour, Fmt::Time, 0.25, on_project("sun-hour", |p: &mut Project, x: f32| p.site.sun.hour = x.rem_euclid(24.0))));
        let days = [("Mar 20", 3, 20), ("Jun 21", 6, 21), ("Sep 22", 9, 22), ("Dec 21", 12, 21)];
        let chips = days.iter().map(|(name, m, d)| {
            let (m, d) = (*m, *d);
            chip(th, format!("sun-date-{m}"), name, sun.month == m && sun.day == d).on_press(move |s: &mut State| {
                s.edit("", |p| {
                    p.site.sun.month = m;
                    p.site.sun.day = d;
                })
            })
        });
        v.push(row().w_full().pl(104.0).flex_wrap().gaps(4.0, 4.0).children(chips));
        let hours = [("8:00", 8.0f32), ("12:00", 12.0), ("16:00", 16.0), ("21:00", 21.0)];
        let chips = hours.iter().map(|(name, h)| {
            let h = *h;
            chip(th, format!("sun-time-{name}"), name, (sun.hour - h).abs() < 0.01).on_press(move |s: &mut State| s.edit("", |p| p.site.sun.hour = h))
        });
        v.push(row().w_full().pl(104.0).flex_wrap().gaps(4.0, 4.0).children(chips));
        let when = format!("{} {}", sun.day, MONTHS[(sun.month.clamp(1, 12) - 1) as usize]);
        v.push(fact(th, "Sun is", if height > 0.0 { format!("{:.0}° up, bearing {:.0}° on {when}", height, bearing) } else { format!("below the horizon on {when}") }));
    } else {
        v.push(num(s, th, "Bearing", "sun-azimuth", sun.azimuth.to_radians(), Fmt::Degrees, 5f32.to_radians(), on_project("sun-azimuth", |p: &mut Project, x: f32| p.site.sun.azimuth = x.to_degrees().rem_euclid(360.0))));
        v.push(num(s, th, "Height", "sun-altitude", sun.altitude.to_radians(), Fmt::Degrees, 5f32.to_radians(), on_project("sun-altitude", |p: &mut Project, x: f32| p.site.sun.altitude = x.to_degrees().clamp(-20.0, 90.0))));
        v.push(text("Bearing is a compass direction: 90° is east, 180° south, 270° west.").font(SMALL).color(th.dim).wrap());
    }
    v
}

fn underlay_mut(p: &mut Project, level: Id) -> Option<&mut Underlay> {
    p.level_mut(level)?.underlay.as_mut()
}

/// The floor plan image under the active level.
fn underlay_rows(s: &State, th: &Th) -> Vec<El> {
    let level = s.level;
    let mut v = vec![section(th, "Floor plan to trace")];
    let Some(u) = s.active().and_then(|l| l.underlay.as_ref()) else {
        v.push(text("Lay a scan or photo of existing plans under this level and draw over it.").font(SMALL).color(th.dim).wrap());
        v.push(row().w_full().mt(2.0).child(button(th, "underlay-load", "Load an image…", false).on_press_tx(|_: &mut State, tx: &mut Tx| {
            let options = rune::FileDialogOptions { title: "Choose a floor plan image".into(), ..Default::default() }.filter("Images", &["png", "jpg", "jpeg", "webp", "svg"]);
            tx.open_file(options, |s: &mut State, result, _| match result {
                Ok(Some(path)) => s.set_underlay(path.to_string_lossy().into_owned()),
                Ok(None) => {}
                Err(e) => s.status = format!("Couldn't show the Open dialog ({e})."),
            });
        })));
        return v;
    };
    let file = std::path::Path::new(&u.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| u.path.clone());
    let image = s.underlay_image(&u.path);
    let pixels = image.and_then(|i| i.size());
    v.push(fact(th, "Image", match (pixels, image.and_then(|i| i.error())) {
        (Some([w, h]), _) => format!("{file}, {w} × {h} px"),
        (None, Some(_)) => format!("{file} couldn't be read"),
        (None, None) => format!("{file} (loading)"),
    }));
    v.push(toggle(th, "Show", "underlay-show", u.visible, on_project("", move |p: &mut Project, on: bool| {
        if let Some(u) = underlay_mut(p, level) {
            u.visible = on
        }
    })));
    v.push(num(s, th, "Opacity", "underlay-opacity", u.opacity, Fmt::Percent, 0.05, on_project("underlay-opacity", move |p: &mut Project, x: f32| {
        if let Some(u) = underlay_mut(p, level) {
            u.opacity = x.clamp(0.05, 1.0)
        }
    })));
    if let Some((a, b)) = s.calibrate {
        let measured = geom::dist(a, b);
        v.push(num(s, th, "That line is really", "underlay-real", measured, Fmt::Length, units::snap_step(s.project.units), |s: &mut State, x: f32| s.apply_calibration(x)));
        v.push(text("Type the real length of the line you drew (click the number). The image is rescaled to match.").font(SMALL).color(th.dim).wrap());
    }
    if let Some([w, _]) = pixels {
        let px = w as f32;
        v.push(len(s, th, "Image width", "underlay-width", px * u.scale, move |p, x| {
            if let Some(u) = underlay_mut(p, level) {
                u.scale = (x / px).clamp(1e-5, 10.0)
            }
        }));
    }
    v.push(len(s, th, "Corner X", "underlay-x", u.at[0], move |p, x| {
        if let Some(u) = underlay_mut(p, level) {
            u.at[0] = x
        }
    }));
    v.push(len(s, th, "Corner Y", "underlay-y", u.at[1], move |p, x| {
        if let Some(u) = underlay_mut(p, level) {
            u.at[1] = x
        }
    }));
    v.push(num(s, th, "Turn", "underlay-turn", u.turn, Fmt::Degrees, 1f32.to_radians(), on_project("underlay-turn", move |p: &mut Project, x: f32| {
        if let Some(u) = underlay_mut(p, level) {
            u.turn = x
        }
    })));
    v.push(
        row()
            .w_full()
            .gap(6.0)
            .mt(4.0)
            .flex_wrap()
            .child(button(th, "underlay-calibrate", "Set its scale", s.tool == Tool::Calibrate).on_press(|s: &mut State| {
                if s.view != crate::app::ViewMode::Plan {
                    s.set_view(crate::app::ViewMode::Plan);
                }
                s.calibrate = None;
                s.set_tool(Tool::Calibrate)
            }))
            .child(button(th, "underlay-remove", "Remove", false).on_press(move |s: &mut State| {
                s.calibrate = None;
                s.edit("", |p| {
                    if let Some(l) = p.level_mut(level) {
                        l.underlay = None
                    }
                })
            })),
    );
    v
}

/// The versions of the house kept in this project.
fn option_rows(s: &State, th: &Th) -> Vec<El> {
    let p = &s.project;
    let mut v = vec![section(th, "Options")];
    let line = |key: String, name: &str, active: bool, compared: bool| {
        row()
            .key(key)
            .w_full()
            .h(28.0)
            .px(8.0)
            .gap(8.0)
            .radius(6.0)
            .center_y()
            .fill(if active { th.accent.alpha(0.14) } else { th.field })
            .border(1.0, if active { th.accent } else { th.border })
            .hover(th.hover)
            .child(text(name.to_string()).font(VALUE).color(th.text).grow(1.0).min_w(0.0).truncate())
            .child(text(if active { "editing" } else if compared { "shown in orange" } else { "" }).font(SMALL).color(th.dim))
    };
    v.push(line("option-current".into(), &p.option, true, false));
    for (i, other) in p.others.iter().enumerate() {
        let name = other.option.clone();
        let compared = s.compare.as_deref() == Some(name.as_str());
        v.push(line(format!("option-{i}"), &name, false, compared).on_press(move |s: &mut State| s.switch_option(i)));
    }
    v.push(name(s, th, "Name", "option-name", &p.option, on_project("", |p: &mut Project, t: String| {
        if !t.is_empty() && !p.others.iter().any(|o| o.option == t) {
            p.option = t
        }
    })));
    if !p.others.is_empty() {
        let mut choices: Vec<(&str, Option<String>)> = vec![("Nothing", None)];
        choices.extend(p.others.iter().map(|o| (o.option.as_str(), Some(o.option.clone()))));
        v.push(choice(th, "Compare with", "option-compare", &s.compare, &choices, |s: &mut State, x: Option<String>| s.compare = x));
    }
    let mut buttons = row().w_full().gap(6.0).mt(4.0).flex_wrap().child(button(th, "option-branch", "New option from this", false).on_press(|s: &mut State| s.branch_option()));
    if !p.others.is_empty() {
        buttons = buttons.child(button(th, "option-remove", "Remove this option", false).on_press(|s: &mut State| s.remove_option()));
    }
    v.push(buttons);
    if p.others.is_empty() {
        v.push(text("Keep the house as it is and try a change beside it: a new option is a full copy you can alter freely.").font(SMALL).color(th.dim).wrap());
    }
    v
}

fn barrier(s: &State, th: &Th, lib: &Library, b: &Barrier) -> Vec<El> {
    let id = b.id;
    let kinds = [("Fence", BarrierKind::Fence), ("Gate", BarrierKind::Gate), ("Retaining wall", BarrierKind::Retaining), ("Balustrade", BarrierKind::Balustrade)];
    let ground = s.project.site.terrain.height(b.path.first().copied().unwrap_or([0.0, 0.0]));
    let mut v = vec![
        heading(th, b.kind.name(), format!("{} long", units::length(b.length(), s.project.units))),
        name(s, th, "Name", "barrier-name", &b.name, on_project("", move |p: &mut Project, t: String| {
            if let Some(b) = p.barrier_mut(id) {
                b.name = t
            }
        })),
        choice(th, "Kind", "barrier-kind", &b.kind, &kinds, on_project("", move |p: &mut Project, k: BarrierKind| {
            if let Some(b) = p.barrier_mut(id) {
                // Take on the new kind's usual size and finish, keeping the line.
                let fresh = k.new(id, std::mem::take(&mut b.path));
                *b = Barrier { name: if b.name == b.kind.name() { fresh.name.clone() } else { b.name.clone() }, base: b.base, ..fresh };
            }
        })),
        len(s, th, "Height", "barrier-height", b.height, move |p, x| {
            if let Some(b) = p.barrier_mut(id) {
                b.height = x.clamp(0.05, 10.0)
            }
        }),
        len(s, th, "Thickness", "barrier-thickness", b.thickness, move |p, x| {
            if let Some(b) = p.barrier_mut(id) {
                b.thickness = x.clamp(0.005, 2.0)
            }
        }),
        fin(s, th, lib, "Finish", "barrier-finish", &b.finish, move |p, f| {
            if let Some(b) = p.barrier_mut(id) {
                b.finish = f
            }
        }),
        toggle(th, "Follows the ground", "barrier-ground", b.base.is_none(), on_project("", move |p: &mut Project, on: bool| {
            if let Some(b) = p.barrier_mut(id) {
                b.base = if on { None } else { Some(ground) }
            }
        })),
    ];
    if let Some(base) = b.base {
        v.push(len(s, th, "Stands at", "barrier-base", base, move |p, x| {
            if let Some(b) = p.barrier_mut(id) {
                b.base = Some(x)
            }
        }));
    }
    v.push(text(match b.kind {
        BarrierKind::Retaining => "Its top is level, set by the highest ground along it. It doesn't reshape the ground: set the ground's heights to match.",
        BarrierKind::Balustrade => "Posts, a handrail and glass. On a deck, set where it stands to the deck's floor height, or give the deck's room a balustrade.",
        _ => "Drag its ends to move them. Double-click it to add a corner.",
    }).font(SMALL).color(th.dim).wrap().mt(8.0));
    v.push(actions(th, false, false));
    v
}

fn plant(s: &State, th: &Th, t: &Plant) -> Vec<El> {
    let id = t.id;
    let kinds = [("Tree", PlantKind::Tree), ("Conifer", PlantKind::Conifer), ("Shrub", PlantKind::Shrub)];
    vec![
        heading(th, t.kind.name(), "It shades the house in the light calculation"),
        choice(th, "Kind", "plant-kind", &t.kind, &kinds, on_project("", move |p: &mut Project, k: PlantKind| {
            if let Some(t) = p.plant_mut(id) {
                let fresh = k.new(id, t.at);
                *t = fresh
            }
        })),
        len(s, th, "Height", "plant-height", t.height, move |p, x| {
            if let Some(t) = p.plant_mut(id) {
                t.height = x.clamp(0.2, 60.0)
            }
        }),
        len(s, th, "Spread", "plant-spread", t.spread, move |p, x| {
            if let Some(t) = p.plant_mut(id) {
                t.spread = x.clamp(0.2, 40.0)
            }
        }),
        len(s, th, "X", "plant-x", t.at[0], move |p, x| {
            if let Some(t) = p.plant_mut(id) {
                t.at[0] = x
            }
        }),
        len(s, th, "Y", "plant-y", t.at[1], move |p, x| {
            if let Some(t) = p.plant_mut(id) {
                t.at[1] = x
            }
        }),
        actions(th, false, false),
    ]
}

fn light_rows(s: &State, th: &Th) -> Vec<El> {
    let lux = &s.lux;
    let busy = lux.request.is_pending();
    let mut v = vec![
        section(th, "Light levels"),
        text("Lux on a level plane in every room, from the sun, the sky and the lamps that are on, with shadows and bounced light.").font(SMALL).color(th.dim).wrap(),
        num(s, th, "Measured at", "lux-height", lux.settings.height, Fmt::Length, 0.05, |s: &mut State, x| s.lux.settings.height = x.clamp(0.01, 2.5)),
        row()
            .w_full()
            .gap(6.0)
            .mt(2.0)
            .flex_wrap()
            .child(button(th, "lux-calculate", if busy { "Calculating…" } else if lux.levels.is_some() { "Recalculate" } else { "Calculate" }, false).on_press(|s: &mut State| s.calculate_light()))
            .child(button(th, "lux-show", if lux.show { "Hide the map" } else { "Show the map" }, lux.show).on_press(|s: &mut State| {
                let show = !s.lux.show;
                s.show_light(show)
            })),
    ];
    let Some(levels) = &lux.levels else { return v };
    if lux.stale {
        v.push(text("The house has changed since this was calculated.").font(SMALL).color(crate::plan::hex(0xc9772b)).wrap());
    }
    // The scale.
    let stops = [1.0f32, 10.0, 100.0, 1000.0, 10_000.0];
    v.push(row().w_full().gap(2.0).mt(4.0).children(stops.iter().map(|x| {
        col().grow(1.0).gap(2.0).child(rune::el().w_full().h(8.0).radius(2.0).fill(crate::plan::srgb(crate::lux::color(*x)))).child(text(round_lux(*x)).font(SMALL).color(th.dim))
    })));
    v.push(fact(th, "Outside", if levels.sun > 0.0 { format!("sun {} lux, sky {} lux", round_lux(levels.sun), round_lux(levels.sky)) } else { "night".to_string() }));
    v.push(fact(th, "Lamps on", format!("{}", s.scene_lamps())));
    for level in s.project.levels.iter().rev() {
        for room in levels.rooms.iter().filter(|r| r.level == level.id) {
            let id = room.room;
            v.push(
                row()
                    .key(format!("lux-room-{id}"))
                    .w_full()
                    .min_h(24.0)
                    .gap(8.0)
                    .px(4.0)
                    .radius(5.0)
                    .center_y()
                    .hover(th.hover)
                    .on_press(move |s: &mut State| s.select(Some(Pick::Room(id))))
                    .child(rune::el().size(10.0, 10.0).radius(2.0).fill(crate::plan::srgb(crate::lux::color(room.mean))))
                    .child(text(room.name.clone()).font(VALUE).color(th.text).grow(1.0).min_w(0.0).truncate())
                    .child(match s.project.room(id).and_then(|r| r.target_lux) {
                        Some(t) if room.mean < t * 0.97 => text(format!("{} of {} lux", round_lux(room.mean), round_lux(t))).font(VALUE).color(crate::plan::hex(0xc9772b)),
                        Some(t) => text(format!("{} lux ✓ {}", round_lux(room.mean), round_lux(t))).font(VALUE).color(crate::plan::hex(0x2f9e7d)),
                        None => text(format!("{} lux", round_lux(room.mean))).font(VALUE).color(th.text),
                    }),
            );
        }
    }
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
        num(s, th, "Pitch (rise/run)", "roof-pitch", r.pitch, Fmt::Number, 0.01, on_project("roof-pitch", move |p, x: f32| {
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

    v.extend(option_rows(s, th));

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

    v.extend(underlay_rows(s, th));

    // The site.
    let t = &p.site.terrain;
    let (lo, hi) = t.heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
    let size = t.size();
    v.extend(sun_rows(s, th));
    v.extend(light_rows(s, th));

    v.push(section(th, "Site & ground"));
    v.push(text(t.source.clone()).font(SMALL).color(th.dim).wrap());
    v.push(fact(th, "Extent", format!("{} × {}", units::length(size[0], p.units), units::length(size[1], p.units))));
    v.push(fact(th, "Relief", if t.heights.is_empty() { "flat".into() } else { format!("{} to {}", units::length(lo, p.units), units::length(hi, p.units)) }));
    v.push(fin(s, th, lib, "Ground cover", "site-finish", &t.finish, |p, f| p.site.terrain.finish = f));
    v.push(num(s, th, "North is", "site-north", p.site.north.to_radians(), Fmt::Degrees, 5f32.to_radians(), on_project("site-north", |p, x: f32| p.site.north = x.to_degrees())));

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
        Some(Pick::Barrier(id)) => p.barrier(id).map(|x| barrier(s, th, &lib, x)),
        Some(Pick::Plant(id)) => p.plant(id).map(|x| plant(s, th, x)),
        None => None,
    }
    .unwrap_or_else(|| project_panel(s, th, &lib));
    let body = col().w_full().gap(5.0).padding(12.0).children(rows);
    col().key("inspector").w(304.0).h_full().min_h(0.0).fill(th.panel).border_sides([0.0, 0.0, 0.0, 1.0], th.border).child(col().key("inspector-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}
