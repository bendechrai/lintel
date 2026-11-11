//! The inspector's controls: number fields you can type in, step or scrub, choices, toggles,
//! and the material picker.

use std::rc::Rc;

use rune::tok::{self, TextStyle};
use rune::{Color, El, Key, Phase, Tx, col, el, row, text, text_input};

use crate::app::State;
use crate::materials::{Library, Tint};
use crate::model::{Finish, Project, Units, rgb};
use crate::plan::{hex, srgb};
use crate::units;

pub const LABEL: TextStyle = TextStyle::new(12.0, 450.0);
pub const VALUE: TextStyle = TextStyle::new(12.0, 500.0);
pub const HEAD: TextStyle = TextStyle::new(11.0, 650.0);
pub const TITLE: TextStyle = TextStyle::new(14.0, 650.0);
pub const SMALL: TextStyle = TextStyle::new(11.0, 450.0);

/// Theme colours for the chrome.
#[derive(Clone, Copy)]
pub struct Th {
    pub bg: Color,
    pub panel: Color,
    pub field: Color,
    pub border: Color,
    pub text: Color,
    pub dim: Color,
    pub hover: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub dark: bool,
}

impl Th {
    pub fn new(dark: bool) -> Th {
        let accent = tok::accent(0, dark);
        if dark {
            Th { bg: hex(0x111214), panel: hex(0x1c1d20), field: hex(0x27292d), border: hex(0x303236), text: hex(0xe6e8eb), dim: hex(0x8e949c), hover: hex(0x2f3237), accent, on_accent: hex(0xffffff), dark }
        } else {
            Th { bg: hex(0xe9e7e2), panel: hex(0xf6f5f2), field: hex(0xffffff), border: hex(0xdddad3), text: hex(0x23262b), dim: hex(0x70767e), hover: hex(0xeae8e3), accent, on_accent: hex(0xffffff), dark }
        }
    }
}

/// How a number reads and writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fmt {
    /// Metres in the model, shown in the project's units.
    Length,
    /// Radians in the model, shown as degrees.
    Degrees,
    /// A whole number.
    Count,
    /// 0..1 in the model, shown as a percentage.
    Percent,
    /// A plain number with two decimals.
    Number,
}

pub fn show(v: f32, fmt: Fmt, units: Units) -> String {
    match fmt {
        Fmt::Length => units::length(v, units),
        Fmt::Degrees => format!("{}°", (v.to_degrees() * 10.0).round() / 10.0),
        Fmt::Count => format!("{}", v.round() as i64),
        Fmt::Percent => format!("{}%", (v * 100.0).round() as i64),
        Fmt::Number => format!("{}", (v * 100.0).round() / 100.0),
    }
}

pub fn read(text: &str, fmt: Fmt, units: Units) -> Option<f32> {
    let plain = |t: &str| t.trim().trim_end_matches(['°', '%']).trim().parse::<f32>().ok();
    match fmt {
        Fmt::Length => units::parse_length(text, units),
        Fmt::Degrees => plain(text).map(f32::to_radians),
        Fmt::Count => plain(text).map(f32::round),
        Fmt::Percent => plain(text).map(|v| v / 100.0),
        Fmt::Number => plain(text),
    }
}

/// Wraps a project edit as a state edit with an undo tag, for the widgets' setters.
pub fn on_project<T: 'static>(tag: &'static str, f: impl Fn(&mut Project, T) + 'static) -> impl Fn(&mut State, T) + 'static {
    move |s: &mut State, v: T| s.edit(tag, |p| f(p, v))
}

pub fn section(th: &Th, title: &str) -> El {
    text(title.to_uppercase()).font(HEAD.tracking(0.5)).color(th.dim).mt(10.0).mb(2.0)
}

pub fn button(th: &Th, key: impl Into<String>, label: impl Into<String>, active: bool) -> El {
    let (fill, ink) = if active { (th.accent, th.on_accent) } else { (th.field, th.text) };
    row()
        .key(key)
        .role("button")
        .h(26.0)
        .px(9.0)
        .radius(6.0)
        .center()
        .fill(fill)
        .border(1.0, if active { th.accent } else { th.border })
        .hover(if active { th.accent } else { th.hover })
        .child(text(label).font(VALUE).color(ink))
}

/// A quiet button: no border until hovered.
pub fn ghost(th: &Th, key: impl Into<String>, label: impl Into<String>, active: bool) -> El {
    let (fill, ink) = if active { (th.accent.alpha(if th.dark { 0.28 } else { 0.14 }), th.accent) } else { (Color { a: 0.0, ..th.field }, th.text) };
    row().key(key).role("button").h(26.0).px(8.0).radius(6.0).center_y().fill(fill).hover(th.hover).child(text(label).font(VALUE).color(if active && th.dark { th.text } else { ink }))
}

fn field_row(th: &Th, label: &str, control: El) -> El {
    row().w_full().min_h(28.0).gap(8.0).center_y().child(text(label).font(LABEL).color(th.dim).w(96.0).truncate()).child(control)
}

fn editor(th: &Th, key: &str, s: &State) -> Option<El> {
    let e = s.editing.as_ref().filter(|e| e.key == key)?;
    Some(
        text_input(&e.edit, VALUE)
            .single_line()
            .key(key.to_string())
            .grow(1.0)
            .min_w(0.0)
            .color(th.text)
            .caret(th.text, th.accent.alpha(0.3))
            .on_edit(|s: &mut State| match &mut s.editing {
                Some(e) => &mut e.edit,
                None => &mut s.scratch,
            })
            .on_submit(|s: &mut State, _: &mut Tx| s.commit_editing())
            .on_input_key(|s: &mut State, ev: &rune::KeyEvent, _: &mut Tx| {
                if matches!(ev.key, Key::Escape) {
                    s.editing = None;
                    true
                } else {
                    false
                }
            }),
    )
}

fn field_box(th: &Th, editing: bool) -> El {
    row().grow(1.0).min_w(0.0).h(26.0).radius(6.0).fill(th.field).border(1.0, if editing { th.accent } else { th.border }).center_y()
}

/// A number: click to type, drag sideways to scrub, or step with the buttons.
pub fn num(s: &State, th: &Th, label: &str, key: &str, value: f32, fmt: Fmt, step: f32, set: impl Fn(&mut State, f32) + 'static) -> El {
    let set: Rc<dyn Fn(&mut State, f32)> = Rc::new(set);
    let units = s.project.units;
    let shown = show(value, fmt, units);
    let editing = s.editing.as_ref().is_some_and(|e| e.key == key);
    let stepper = |glyph: &str, by: f32| {
        let set = set.clone();
        row().key(format!("{key}{glyph}")).role("button").size(22.0, 24.0).center().radius(5.0).hover(th.hover).child(text(glyph).font(VALUE).color(th.dim)).on_press(move |s: &mut State| {
            s.commit_editing();
            set(s, value + by)
        })
    };
    let middle = match editor(th, key, s) {
        Some(input) => row().grow(1.0).min_w(0.0).px(6.0).child(input),
        None => {
            let (k, set) = (key.to_string(), set.clone());
            let text_now = shown.clone();
            row()
                .key(format!("{key}-value"))
                .grow(1.0)
                .min_w(0.0)
                .h(24.0)
                .center()
                .cursor(rune::Cursor::ResizeHorizontal)
                .child(text(shown).font(VALUE).color(th.text).truncate())
                .on_drag(move |s: &mut State, d: &mut rune::Drag, tx: &mut Tx| match d.phase {
                    Phase::Down => d.data = value,
                    Phase::Move => {
                        if d.dx.abs() > 3.0 {
                            set(s, d.data + ((d.dx - 3.0 * d.dx.signum()) / 5.0).round() * step);
                        }
                    }
                    Phase::Up => {
                        if d.dx.abs() <= 3.0 && !d.cancelled() {
                            let set = set.clone();
                            let apply: Rc<dyn Fn(&mut State, &str)> = Rc::new(move |s: &mut State, t: &str| match read(t, fmt, s.project.units) {
                                Some(v) => set(s, v),
                                None => s.status = format!("Couldn't read “{t}” as a number."),
                            });
                            s.begin_editing(k.clone(), text_now.clone(), apply);
                            tx.focus(k.clone(), false);
                        } else {
                            s.settle();
                        }
                    }
                })
        }
    };
    field_row(th, label, field_box(th, editing).child(stepper("−", -step)).child(middle).child(stepper("+", step)))
}

/// A line of text: click to edit.
pub fn name(s: &State, th: &Th, label: &str, key: &str, value: &str, set: impl Fn(&mut State, String) + 'static) -> El {
    let set: Rc<dyn Fn(&mut State, String)> = Rc::new(set);
    let editing = s.editing.as_ref().is_some_and(|e| e.key == key);
    let inner = match editor(th, key, s) {
        Some(input) => row().grow(1.0).min_w(0.0).px(8.0).child(input),
        None => {
            let (k, now) = (key.to_string(), value.to_string());
            let shown = if value.is_empty() { "—".to_string() } else { value.to_string() };
            row().key(format!("{key}-value")).grow(1.0).min_w(0.0).h(24.0).px(8.0).center_y().cursor(rune::Cursor::Text).child(text(shown).font(VALUE).color(th.text).truncate()).on_press_tx(
                move |s: &mut State, tx: &mut Tx| {
                    let set = set.clone();
                    s.begin_editing(k.clone(), now.clone(), Rc::new(move |s: &mut State, t: &str| set(s, t.trim().to_string())));
                    tx.focus(k.clone(), false);
                },
            )
        }
    };
    field_row(th, label, field_box(th, editing).child(inner))
}

/// One of a few options, as chips.
pub fn choice<T: Clone + PartialEq + 'static>(th: &Th, label: &str, key: &str, current: &T, options: &[(&str, T)], set: impl Fn(&mut State, T) + 'static) -> El {
    let set: Rc<dyn Fn(&mut State, T)> = Rc::new(set);
    let chips = options.iter().map(|(name, v)| {
        let (set, v2) = (set.clone(), v.clone());
        chip(th, format!("{key}-{name}"), name, v == current).on_press(move |s: &mut State| {
            s.commit_editing();
            set(s, v2.clone())
        })
    });
    field_row(th, label, row().grow(1.0).min_w(0.0).flex_wrap().gaps(4.0, 4.0).py(2.0).children(chips))
}

pub fn chip(th: &Th, key: impl Into<String>, label: &str, active: bool) -> El {
    let (fill, ink, border) = if active { (th.accent, th.on_accent, th.accent) } else { (th.field, th.text, th.border) };
    row().key(key).role("button").h(22.0).px(7.0).radius(11.0).center().fill(fill).border(1.0, border).hover(if active { th.accent } else { th.hover }).child(text(label).font(SMALL).color(ink))
}

pub fn toggle(th: &Th, label: &str, key: &str, on: bool, set: impl Fn(&mut State, bool) + 'static) -> El {
    let knob = el().size(14.0, 14.0).radius(7.0).fill(hex(0xffffff)).absolute(if on { 16.0 } else { 2.0 }, 2.0);
    let switch = el().key(key.to_string()).role("switch").checked(on).size(32.0, 18.0).radius(9.0).fill(if on { th.accent } else { th.border }).motion(tok::motion::SNAPPY).child(knob).on_press(move |s: &mut State| {
        s.commit_editing();
        set(s, !on)
    });
    field_row(th, label, switch)
}

/// A read-only line.
pub fn fact(th: &Th, label: &str, value: impl Into<String>) -> El {
    field_row(th, label, text(value).font(VALUE).color(th.dim))
}

const PAINTS: [u32; 40] = [
    0xffffff, 0xf7f5f0, 0xf2efe8, 0xe9e4d8, 0xdcd6c8, 0xcfc7b6, 0xb8b0a0, 0x9a948a, 0x6f6c66, 0x3a3a3a, 0xf3ede2, 0xead9c3, 0xd9bfa0, 0xc19a78, 0x9c6b4e, 0x6e4634, 0xf4e3d7, 0xe8b9a5, 0xc9705a,
    0x8f3b32, 0xf3ecc8, 0xe6cf7d, 0xc9a23f, 0x8a6a1f, 0xe4eddc, 0xb7cba7, 0x7d9a6d, 0x41593a, 0xdfeeee, 0xa8cfd0, 0x5d9aa3, 0x2c5f6b, 0xdee6f1, 0xa9bfdc, 0x5f82b3, 0x2b4470, 0xe7e1ee, 0xb9a8cf,
    0x64747c, 0x1f2328,
];

const STAINS: [(&str, u32); 9] = [
    ("Honey", 0xd9a441),
    ("Golden oak", 0xb9792f),
    ("Chestnut", 0x8a4b25),
    ("Walnut", 0x5a3a22),
    ("Mahogany", 0x6a2a1c),
    ("Jarrah", 0x7a2e24),
    ("Ebony", 0x2a2320),
    ("Grey wash", 0xa7a39b),
    ("White wash", 0xe9e4d8),
];

fn swatch(color: [f32; 3], size: f32, th: &Th) -> El {
    el().size(size, size).radius(4.0).fill(srgb(color)).border(1.0, th.border)
}

/// A surface finish: its material and colour. Opens a picker in place.
pub fn finish(s: &State, th: &Th, lib: &Library, label: &str, key: &str, value: &Finish, set: impl Fn(&mut State, Finish) + 'static) -> El {
    let set: Rc<dyn Fn(&mut State, Finish)> = Rc::new(set);
    let material = lib.get(&value.material);
    let name = material.map(|m| m.name.clone()).unwrap_or_else(|| format!("{}?", value.material));
    let open = s.picker.as_deref() == Some(key);
    let k = key.to_string();
    let head = field_row(
        th,
        label,
        row()
            .key(format!("{key}-finish"))
            .role("button")
            .grow(1.0)
            .min_w(0.0)
            .h(26.0)
            .px(5.0)
            .gap(7.0)
            .radius(6.0)
            .center_y()
            .fill(th.field)
            .border(1.0, if open { th.accent } else { th.border })
            .hover(th.hover)
            .child(swatch(lib.swatch(value), 16.0, th))
            .child(text(name).font(VALUE).color(th.text).truncate().grow(1.0).min_w(0.0))
            .child(text(if open { "▴" } else { "▾" }).font(SMALL).color(th.dim))
            .on_press(move |s: &mut State| {
                s.commit_editing();
                s.picker = if s.picker.as_deref() == Some(k.as_str()) { None } else { Some(k.clone()) };
            }),
    );
    if !open {
        return head;
    }
    let mut panel = col().w_full().gap(4.0).padding(8.0).radius(8.0).fill(th.field).border(1.0, th.border);

    // Colour: paint takes any, timber takes a stain, fixed materials none.
    match material.map(|m| m.tint) {
        Some(Tint::Paint) => {
            let chips = PAINTS.iter().map(|c| {
                let (set, material, color) = (set.clone(), value.material.clone(), rgb(*c));
                let on = value.color.is_some_and(|v| close(v, color));
                el().key(format!("{key}-c{c:06x}")).role("button").size(22.0, 22.0).radius(5.0).fill(hex(*c)).border(if on { 2.0 } else { 1.0 }, if on { th.accent } else { th.border }).on_press(
                    move |s: &mut State| set(s, Finish { material: material.clone(), color: Some(color) }),
                )
            });
            panel = panel.child(text("Colour").font(SMALL).color(th.dim)).child(row().w_full().flex_wrap().gaps(4.0, 4.0).children(chips));
            let (set, material) = (set.clone(), value.material.clone());
            let now = to_hex(lib.swatch(value));
            panel = panel.child(name_inline(s, th, "Hex", &format!("{key}-hex"), &now, move |s: &mut State, t: String| match parse_hex(&t) {
                Some(color) => set(s, Finish { material: material.clone(), color: Some(color) }),
                None => s.status = format!("“{t}” isn't a colour like #c8b89a."),
            }));
        }
        Some(Tint::Stain) => {
            let natural = {
                let (set, material) = (set.clone(), value.material.clone());
                chip(th, format!("{key}-natural"), "Natural", value.color.is_none()).on_press(move |s: &mut State| set(s, Finish { material: material.clone(), color: None }))
            };
            let chips = STAINS.iter().map(|(name, c)| {
                let (set, material, color) = (set.clone(), value.material.clone(), rgb(*c));
                let on = value.color.is_some_and(|v| close(v, color));
                row()
                    .key(format!("{key}-s{c:06x}"))
                    .role("button")
                    .h(22.0)
                    .pl(3.0)
                    .pr(7.0)
                    .gap(5.0)
                    .radius(11.0)
                    .center_y()
                    .fill(th.field)
                    .border(1.0, if on { th.accent } else { th.border })
                    .hover(th.hover)
                    .child(el().size(14.0, 14.0).radius(7.0).fill(srgb(lib.swatch(&Finish { material: material.clone(), color: Some(color) }))))
                    .child(text(*name).font(SMALL).color(th.text))
                    .on_press(move |s: &mut State| set(s, Finish { material: material.clone(), color: Some(color) }))
            });
            panel = panel.child(text("Stain").font(SMALL).color(th.dim)).child(row().w_full().flex_wrap().gaps(4.0, 4.0).child(natural).children(chips));
        }
        _ => panel = panel.child(text("This material is its own colour.").font(SMALL).color(th.dim)),
    }

    // Materials, by group.
    let mut group = "";
    let mut cells: Vec<El> = Vec::new();
    let flush = |panel: El, cells: &mut Vec<El>| if cells.is_empty() { panel } else { panel.child(row().w_full().flex_wrap().gaps(3.0, 3.0).children(std::mem::take(cells))) };
    for m in lib.all() {
        if m.group != group {
            panel = flush(panel, &mut cells);
            group = &m.group;
            panel = panel.child(text(m.group.clone()).font(SMALL).color(th.dim).mt(6.0));
        }
        let on = m.id == value.material;
        let (set, id, keep) = (set.clone(), m.id.clone(), value.color);
        let same_tint = material.is_some_and(|cur| cur.tint == m.tint);
        cells.push(
            row()
                .key(format!("{key}-m-{}", m.id))
                .role("button")
                .w(126.0)
                .h(24.0)
                .px(4.0)
                .gap(6.0)
                .radius(5.0)
                .center_y()
                .fill(if on { th.accent.alpha(0.16) } else { Color { a: 0.0, ..th.field } })
                .hover(th.hover)
                .child(swatch(m.color, 14.0, th))
                .child(text(m.name.clone()).font(SMALL).color(th.text).truncate().grow(1.0).min_w(0.0))
                // A colour carries over between materials that use it the same way.
                .on_press(move |s: &mut State| set(s, Finish { material: id.clone(), color: if same_tint { keep } else { None } })),
        );
    }
    panel = flush(panel, &mut cells);
    col().w_full().gap(4.0).child(head).child(panel)
}

fn name_inline(s: &State, th: &Th, label: &str, key: &str, value: &str, set: impl Fn(&mut State, String) + 'static) -> El {
    name(s, th, label, key, value, set)
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.004)
}

fn to_hex(c: [f32; 3]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

pub fn parse_hex(t: &str) -> Option<[f32; 3]> {
    let h = t.trim().trim_start_matches('#');
    let h = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect::<String>() } else { h.to_string() };
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(&h, 16).ok().map(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_back_what_they_show() {
        for (v, fmt) in [(2.4, Fmt::Length), (0.5, Fmt::Percent), (4.0, Fmt::Count), (std::f32::consts::FRAC_PI_2, Fmt::Degrees), (0.42, Fmt::Number)] {
            for units in [Units::Metric, Units::Imperial] {
                let back = read(&show(v, fmt, units), fmt, units).unwrap();
                assert!((back - v).abs() < 0.01, "{fmt:?} {units:?}: {v} → {} → {back}", show(v, fmt, units));
            }
        }
        assert_eq!(parse_hex("#fff"), Some([1.0, 1.0, 1.0]));
        assert_eq!(parse_hex("c8b89a").map(to_hex).as_deref(), Some("#c8b89a"));
        assert_eq!(parse_hex("nope"), None);
    }
}
