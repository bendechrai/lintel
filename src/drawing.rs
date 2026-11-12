//! Dimensioned plans on paper: one page a level, to scale, as a PDF.
//!
//! The PDF is written directly: straight lines, filled shapes and Helvetica text are all a
//! plan needs, and that keeps drawings free of any dependency.

use crate::geom::{self, add, scale, sub};
use crate::model::*;
use crate::units;

const MM: f32 = 72.0 / 25.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    A4,
    A3,
    Letter,
    Tabloid,
}

impl Paper {
    pub const ALL: [Paper; 4] = [Paper::A4, Paper::A3, Paper::Letter, Paper::Tabloid];

    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::Letter => "Letter",
            Paper::Tabloid => "Tabloid",
        }
    }

    /// Landscape width and height in millimetres.
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (297.0, 210.0),
            Paper::A3 => (420.0, 297.0),
            Paper::Letter => (279.4, 215.9),
            Paper::Tabloid => (431.8, 279.4),
        }
    }

    pub fn usual(units: Units) -> Paper {
        match units {
            Units::Metric => Paper::A3,
            Units::Imperial => Paper::Tabloid,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    pub paper: Paper,
    /// The drawing is 1 : `scale`; `None` picks the largest standard scale that fits.
    pub scale: Option<f32>,
}

/// The standard scales for a unit system, largest drawing first, with how they are written.
pub fn scales(units: Units) -> Vec<(f32, &'static str)> {
    match units {
        Units::Metric => vec![(20.0, "1:20"), (25.0, "1:25"), (50.0, "1:50"), (75.0, "1:75"), (100.0, "1:100"), (200.0, "1:200"), (500.0, "1:500")],
        Units::Imperial => vec![(24.0, "1/2\" = 1'"), (48.0, "1/4\" = 1'"), (64.0, "3/16\" = 1'"), (96.0, "1/8\" = 1'"), (192.0, "1/16\" = 1'"), (384.0, "1/32\" = 1'")],
    }
}

fn scale_name(units: Units, s: f32) -> String {
    scales(units).iter().find(|(v, _)| (*v - s).abs() < 0.01).map(|(_, n)| n.to_string()).unwrap_or_else(|| format!("1:{s:.0}"))
}

// ---------------------------------------------------------------------------------------------
// A minimal PDF

/// Helvetica's advance widths for ASCII 32..=126, in thousandths of the font size.
const WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278,
    500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500,
    278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Width of text set in Helvetica at `size` points, in points.
pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().map(|c| if (' '..='~').contains(&c) { WIDTHS[c as usize - 32] as f32 } else { 556.0 }).sum::<f32>() * size / 1000.0
}

/// One page's drawing commands. Coordinates are millimetres from the page's bottom-left.
pub struct Page {
    size: (f32, f32),
    ops: String,
}

impl Page {
    fn new(size: (f32, f32)) -> Page {
        Page { size, ops: String::new() }
    }

    fn rgb(c: u32) -> String {
        format!("{:.3} {:.3} {:.3}", ((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0)
    }

    fn path(&mut self, pts: &[[f32; 2]], close: bool) {
        for (i, p) in pts.iter().enumerate() {
            self.ops += &format!("{:.2} {:.2} {} ", p[0] * MM, p[1] * MM, if i == 0 { "m" } else { "l" });
        }
        if close {
            self.ops += "h ";
        }
    }

    pub fn fill(&mut self, pts: &[[f32; 2]], color: u32) {
        if pts.len() < 3 {
            return;
        }
        self.ops += &format!("{} rg ", Page::rgb(color));
        self.path(pts, true);
        self.ops += "f\n";
    }

    /// A line through `pts`, `width` millimetres wide; `dash` millimetres on and off if set.
    pub fn stroke(&mut self, pts: &[[f32; 2]], width: f32, color: u32, dash: Option<f32>) {
        if pts.len() < 2 {
            return;
        }
        let dash = dash.map(|d| format!("[{:.2} {:.2}] 0 d ", d * MM, d * MM * 0.7)).unwrap_or_else(|| "[] 0 d ".into());
        self.ops += &format!("{} RG {:.3} w {dash}", Page::rgb(color), width * MM);
        self.path(pts, false);
        self.ops += "S\n";
    }

    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: u32) {
        self.stroke(&[a, b], width, color, None);
    }

    /// Text of `size` points. `anchor` is 0 for left, 0.5 for centred, 1 for right aligned at
    /// `at`; `turn` rotates it (radians, counter-clockwise) about that point.
    pub fn text(&mut self, at: [f32; 2], text: &str, size: f32, color: u32, anchor: f32, turn: f32, bold: bool) {
        let shift = -text_width(text, size) * anchor;
        let (s, c) = turn.sin_cos();
        let (x, y) = (at[0] * MM + shift * c, at[1] * MM + shift * s);
        // Latin-1 covers what a drawing writes (², ×, °); anything else becomes a question mark.
        let mut encoded = String::new();
        for ch in text.chars() {
            match ch {
                '(' | ')' | '\\' => {
                    encoded.push('\\');
                    encoded.push(ch);
                }
                ' '..='~' => encoded.push(ch),
                '\u{a0}'..='\u{ff}' => encoded += &format!("\\{:03o}", ch as u32),
                _ => encoded.push('?'),
            }
        }
        self.ops += &format!("BT {} rg /{} {size:.2} Tf {c:.4} {s:.4} {:.4} {c:.4} {x:.2} {y:.2} Tm ({encoded}) Tj ET\n", Page::rgb(color), if bold { "F2" } else { "F1" }, -s);
    }
}

/// Assembles pages into a PDF file.
pub fn pdf(pages: &[Page]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };
    // 1 catalog, 2 pages, 3 and 4 fonts, then a page and its content for each page.
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", 5 + i * 2)).collect();
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(&mut out, format!("<< /Type /Pages /Count {} /Kids [{}] >>", pages.len(), kids.join(" ")).as_bytes());
    object(&mut out, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
    object(&mut out, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>");
    for (i, page) in pages.iter().enumerate() {
        let content = 6 + i * 2;
        object(
            &mut out,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content} 0 R >>", page.size.0 * MM, page.size.1 * MM).as_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", page.ops.len()).into_bytes();
        stream.extend(page.ops.as_bytes());
        stream.extend(b"\nendstream");
        object(&mut out, &stream);
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    out
}

// ---------------------------------------------------------------------------------------------
// A level on a sheet

const INK: u32 = 0x1b1d21;
const GREY: u32 = 0x8a8f96;
const PALE: u32 = 0xc9ccd1;
const BLUE: u32 = 0x2450d8;

struct Plot<'a> {
    page: &'a mut Page,
    /// Paper millimetres per model metre.
    k: f32,
    /// Where the model origin lands on the page.
    origin: [f32; 2],
    units: Units,
}

impl Plot<'_> {
    fn at(&self, p: P2) -> [f32; 2] {
        [self.origin[0] + p[0] * self.k, self.origin[1] + p[1] * self.k]
    }

    fn poly(&self, pts: &[P2]) -> Vec<[f32; 2]> {
        pts.iter().map(|p| self.at(*p)).collect()
    }

    /// A dimension line from `a` to `b` (model points on one axis), drawn `off` millimetres
    /// to the side, with ticks and the length written over it.
    fn dimension(&mut self, a: P2, b: P2, off: f32) {
        let len = geom::dist(a, b);
        if len < 0.05 {
            return;
        }
        let d = scale(sub(b, a), 1.0 / len);
        let n = [-d[1], d[0]];
        let (pa, pb) = (self.at(a), self.at(b));
        let shift = |p: [f32; 2], by: f32| [p[0] + n[0] * by, p[1] + n[1] * by];
        let (qa, qb) = (shift(pa, off), shift(pb, off));
        self.page.line(qa, qb, 0.13, BLUE);
        for (p, q) in [(pa, qa), (pb, qb)] {
            // Extension line, stopping short of the thing measured, and a 45° tick.
            self.page.line(shift(p, off.signum() * 1.5), shift(q, off.signum() * 1.2), 0.1, BLUE);
            self.page.line([q[0] - 0.9 * (d[0] + n[0]), q[1] - 0.9 * (d[1] + n[1])], [q[0] + 0.9 * (d[0] + n[0]), q[1] + 0.9 * (d[1] + n[1])], 0.25, BLUE);
        }
        let mid = shift([(qa[0] + qb[0]) / 2.0, (qa[1] + qb[1]) / 2.0], 0.9);
        let text = units::length(len, self.units);
        // Only where the figure fits between its ticks.
        if text_width(&text, 6.5) / MM < len * self.k - 1.0 {
            self.page.text(mid, &text, 6.5, BLUE, 0.5, d[1].atan2(d[0]), false);
        }
    }
}

fn wall_box(w: &Wall, ext: (f32, f32)) -> [P2; 4] {
    let (len, h) = (w.length(), w.thickness / 2.0);
    [w.point(-ext.0, -h), w.point(len + ext.1, -h), w.point(len + ext.1, h), w.point(-ext.0, h)]
}

fn arc(center: P2, r: f32, a0: f32, a1: f32) -> Vec<P2> {
    (0..=16).map(|i| a0 + (a1 - a0) * i as f32 / 16.0).map(|a| [center[0] + a.cos() * r, center[1] + a.sin() * r]).collect()
}

fn opening(plot: &mut Plot, w: &Wall, o: &Opening) {
    let h = w.thickness / 2.0;
    let (x0, x1) = (o.at - o.width / 2.0, o.at + o.width / 2.0);
    let gap = [w.point(x0, -h), w.point(x1, -h), w.point(x1, h), w.point(x0, h)];
    let side = |s: WallSide| if s == WallSide::Left { 1.0 } else { -1.0 };
    if let OpeningKind::Recess { side: s, depth, .. } = &o.kind {
        let s = side(*s);
        let inner = h - depth.min(w.thickness);
        let pts = plot.poly(&[w.point(x0, s * inner), w.point(x1, s * inner), w.point(x1, s * h), w.point(x0, s * h)]);
        plot.page.fill(&pts, 0xffffff);
        plot.page.stroke(&[pts[3], pts[0], pts[1], pts[2]], 0.18, INK, None);
        return;
    }
    let pts = plot.poly(&gap);
    plot.page.fill(&pts, 0xffffff);
    plot.page.line(pts[0], pts[3], 0.18, INK);
    plot.page.line(pts[1], pts[2], 0.18, INK);
    let (dir, n) = w.frame();
    let wall_angle = dir[1].atan2(dir[0]);
    match &o.kind {
        OpeningKind::Window(_) => {
            for y in [-h, 0.0, h] {
                let (a, b) = (plot.at(w.point(x0, y)), plot.at(w.point(x1, y)));
                plot.page.line(a, b, if y == 0.0 { 0.3 } else { 0.13 }, INK);
            }
        }
        OpeningKind::Door(door) => {
            let s = side(door.swing);
            let mut leaf = |hinge_x: f32, toward: f32, len: f32| {
                let hinge = w.point(hinge_x, s * h);
                let tip = add(hinge, scale(n, s * len));
                let (a, b) = (plot.at(hinge), plot.at(tip));
                plot.page.line(a, b, 0.35, INK);
                let a_leaf = (n[1] * s).atan2(n[0] * s);
                let a_closed = if toward > 0.0 { wall_angle } else { wall_angle + std::f32::consts::PI };
                let mut delta = a_closed - a_leaf;
                while delta > std::f32::consts::PI {
                    delta -= std::f32::consts::TAU;
                }
                while delta < -std::f32::consts::PI {
                    delta += std::f32::consts::TAU;
                }
                let sweep = plot.poly(&arc(hinge, len, a_leaf, a_leaf + delta));
                plot.page.stroke(&sweep, 0.1, GREY, None);
            };
            match door.style {
                DoorStyle::Hinged if door.hinge == Hinge::Start => leaf(x0, 1.0, o.width),
                DoorStyle::Hinged => leaf(x1, -1.0, o.width),
                DoorStyle::Double | DoorStyle::Bifold => {
                    leaf(x0, 1.0, o.width / 2.0);
                    leaf(x1, -1.0, o.width / 2.0);
                }
                DoorStyle::Sliding | DoorStyle::Pocket => {
                    let (a, b) = (plot.at(w.point(x0, h * 0.3)), plot.at(w.point(o.at, h * 0.3)));
                    plot.page.line(a, b, 0.35, INK);
                    let (a, b) = (plot.at(w.point(o.at, -h * 0.3)), plot.at(w.point(x1, -h * 0.3)));
                    plot.page.line(a, b, 0.35, INK);
                }
            }
        }
        OpeningKind::Garage(g) => {
            let s = side(g.inside);
            let (a, b) = (plot.at(w.point(x0, -s * h * 0.5)), plot.at(w.point(x1, -s * h * 0.5)));
            plot.page.line(a, b, 0.4, INK);
            let over = plot.poly(&[w.point(x0, s * h), w.point(x0, s * (h + o.height)), w.point(x1, s * (h + o.height)), w.point(x1, s * h)]);
            plot.page.stroke(&over, 0.1, GREY, Some(1.2));
        }
        OpeningKind::Passage { .. } => {
            for y in [-h, h] {
                let (a, b) = (plot.at(w.point(x0, y)), plot.at(w.point(x1, y)));
                plot.page.stroke(&[a, b], 0.1, GREY, Some(0.8));
            }
        }
        OpeningKind::Recess { .. } => {}
    }
}

/// The distinct positions, along one axis, of the walls that cross a side of the building:
/// what a chain of dimensions steps between.
fn stations(level: &Level, axis: usize, lo: f32, hi: f32) -> Vec<f32> {
    let mut at: Vec<f32> = vec![lo, hi];
    for w in &level.walls {
        // A wall running across the axis contributes its position on it.
        if (w.a[axis] - w.b[axis]).abs() < 0.02 && w.length() > 0.6 {
            at.push(w.a[axis]);
        }
    }
    at.sort_by(|a, b| a.total_cmp(b));
    at.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    at.into_iter().filter(|v| *v >= lo - 1e-3 && *v <= hi + 1e-3).collect()
}

/// The plan extent of a level: its walls and rooms.
fn extent(level: &Level) -> Option<(P2, P2)> {
    let pts = level.walls.iter().flat_map(|w| [w.a, w.b]).chain(level.rooms.iter().flat_map(|r| r.outline.iter().copied()));
    let (lo, hi) = pts.fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    (lo[0] <= hi[0]).then_some((lo, hi))
}

/// Space kept clear round the drawing, in paper millimetres: margins, dimension chains, and
/// the title strip along the bottom.
const EDGE: f32 = 12.0;
const DIMS: f32 = 22.0;
const TITLE: f32 = 16.0;

/// The largest standard scale at which `level` fits on `paper`.
pub fn fit(level: &Level, paper: Paper, units: Units) -> f32 {
    let Some((lo, hi)) = extent(level) else { return scales(units)[2].0 };
    let (w, h) = paper.size();
    let room = [w - 2.0 * EDGE - DIMS - 6.0, h - 2.0 * EDGE - DIMS - TITLE - 8.0];
    scales(units).iter().map(|(s, _)| *s).find(|s| (hi[0] - lo[0]) * 1000.0 / s <= room[0] && (hi[1] - lo[1]) * 1000.0 / s <= room[1]).unwrap_or(scales(units).last().map(|s| s.0).unwrap_or(500.0))
}

fn level_page(project: &Project, level: &Level, sheet: Sheet) -> Page {
    let (pw, ph) = sheet.paper.size();
    let mut page = Page::new((pw, ph));
    let s = sheet.scale.unwrap_or_else(|| fit(level, sheet.paper, project.units));
    let k = 1000.0 / s;

    // Border and title strip.
    page.stroke(&[[EDGE, EDGE], [pw - EDGE, EDGE], [pw - EDGE, ph - EDGE], [EDGE, ph - EDGE], [EDGE, EDGE]], 0.35, INK, None);
    page.line([EDGE, EDGE + TITLE], [pw - EDGE, EDGE + TITLE], 0.35, INK);
    page.text([EDGE + 5.0, EDGE + 9.0], &project.name, 13.0, INK, 0.0, 0.0, true);
    let option = if project.others.is_empty() { String::new() } else { format!("  ·  {}", project.option) };
    page.text([EDGE + 5.0, EDGE + 3.5], &format!("{} plan{option}", level.name), 9.0, GREY, 0.0, 0.0, false);
    page.text([pw - EDGE - 5.0, EDGE + 9.0], &format!("Scale {}", scale_name(project.units, s)), 11.0, INK, 1.0, 0.0, true);
    page.text([pw - EDGE - 5.0, EDGE + 3.5], &format!("{} landscape  ·  drawn with Lintel", sheet.paper.name()), 7.5, GREY, 1.0, 0.0, false);

    let Some((lo, hi)) = extent(level) else {
        page.text([pw / 2.0, ph / 2.0], "Nothing is drawn on this level yet.", 11.0, GREY, 0.5, 0.0, false);
        return page;
    };
    // Centre the building in the space above the title strip, leaving room for dimensions
    // below and to the left.
    let space = ([EDGE + DIMS, EDGE + TITLE + DIMS], [pw - EDGE - 6.0, ph - EDGE - 8.0]);
    let size = [(hi[0] - lo[0]) * k, (hi[1] - lo[1]) * k];
    let origin = [space.0[0] + (space.1[0] - space.0[0] - size[0]) / 2.0 - lo[0] * k, space.0[1] + (space.1[1] - space.0[1] - size[1]) / 2.0 - lo[1] * k];
    let mut plot = Plot { page: &mut page, k, origin, units: project.units };

    // Rooms: a name, an area and inside sizes.
    for r in &level.rooms {
        let pts = plot.poly(&r.outline);
        if !r.has_ceiling {
            let mut closed = pts.clone();
            closed.push(pts[0]);
            plot.page.stroke(&closed, 0.13, GREY, Some(1.4));
        }
        let c = plot.at(geom::centroid(&r.outline));
        let (rlo, rhi) = r.outline.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let wide = (rhi[0] - rlo[0]) * k;
        if wide > text_width(&r.name, 8.5) / MM + 2.0 {
            plot.page.text([c[0], c[1] + 1.2], &r.name, 8.5, INK, 0.5, 0.0, true);
            let detail = format!("{}  ·  {} × {}", units::area(geom::area(&r.outline).abs(), project.units), units::length(rhi[0] - rlo[0], project.units), units::length(rhi[1] - rlo[1], project.units));
            if wide > text_width(&detail, 6.0) / MM + 2.0 {
                plot.page.text([c[0], c[1] - 2.2], &detail, 6.0, GREY, 0.5, 0.0, false);
            } else {
                plot.page.text([c[0], c[1] - 2.2], &units::area(geom::area(&r.outline).abs(), project.units), 6.0, GREY, 0.5, 0.0, false);
            }
        }
    }

    // Stairs.
    for st in &level.stairs {
        let dir = [st.direction.cos(), st.direction.sin()];
        let side = [-dir[1], dir[0]];
        let c = |along: f32, across: f32| add(st.at, add(scale(dir, along), scale(side, across)));
        let hw = st.width / 2.0;
        let mut outline = plot.poly(&[c(0.0, -hw), c(st.run, -hw), c(st.run, hw), c(0.0, hw)]);
        outline.push(outline[0]);
        plot.page.stroke(&outline, 0.18, INK, None);
        for i in 1..st.steps.max(1) {
            let t = st.run * i as f32 / st.steps as f32;
            let (a, b) = (plot.at(c(t, -hw)), plot.at(c(t, hw)));
            plot.page.line(a, b, 0.1, GREY);
        }
        let (a, b) = (plot.at(c(st.run * 0.1, 0.0)), plot.at(c(st.run * 0.9, 0.0)));
        plot.page.line(a, b, 0.18, INK);
        let head = plot.poly(&[c(st.run * 0.9, 0.0), c(st.run * 0.9 - 0.18, 0.09), c(st.run * 0.9 - 0.18, -0.09)]);
        plot.page.fill(&head, INK);
        let mid = plot.at(c(st.run * 0.5, hw + 0.12));
        plot.page.text(mid, "UP", 5.5, GREY, 0.5, st.direction, false);
    }

    // Furniture, lightly.
    for f in &level.furniture {
        let mut pts = plot.poly(&geom::oriented_rect(f.at, f.size[0], f.size[1], f.turn));
        pts.push(pts[0]);
        plot.page.stroke(&pts, 0.1, PALE, if f.lift > 0.5 || f.hung { Some(0.8) } else { None });
    }

    // Walls, solid; then the openings cut out of them.
    for w in &level.walls {
        let pts = plot.poly(&wall_box(w, crate::plan::wall_extensions(level, w)));
        plot.page.fill(&pts, 0x3a3d42);
    }
    for w in &level.walls {
        for o in &w.openings {
            opening(&mut plot, w, o);
        }
    }

    // Fixtures: power as a filled dot, switches as a ring, lights as a cross in a circle.
    for f in &level.fixtures {
        let Some((at, facing)) = crate::plan::fixture_point(level, f) else { continue };
        let out = [facing.cos(), facing.sin()];
        let on_wall = matches!(f.mount, Mount::Wall { .. });
        let c = plot.at(if on_wall { add(at, scale(out, 1.6 / k)) } else { at });
        let ring = |r: f32| -> Vec<[f32; 2]> { (0..=12).map(|i| i as f32 / 12.0 * std::f32::consts::TAU).map(|a| [c[0] + a.cos() * r, c[1] + a.sin() * r]).collect() };
        match f.kind {
            FixtureKind::Outlet { .. } | FixtureKind::Data => plot.page.fill(&ring(0.8), if matches!(f.kind, FixtureKind::Data) { BLUE } else { INK }),
            FixtureKind::Switch { .. } => {
                plot.page.stroke(&ring(0.8), 0.18, INK, None);
                plot.page.text([c[0] + 1.1, c[1] - 0.8], "S", 5.0, INK, 0.0, 0.0, false);
            }
            FixtureKind::Thermostat => plot.page.text([c[0], c[1] - 0.8], "T", 5.0, INK, 0.5, 0.0, true),
            FixtureKind::SmokeDetector => {
                plot.page.stroke(&ring(1.3), 0.18, INK, None);
                plot.page.text([c[0], c[1] - 0.75], "SD", 3.6, INK, 0.5, 0.0, false);
            }
            FixtureKind::Downlight { .. } | FixtureKind::Surface { .. } | FixtureKind::Pendant { .. } => {
                plot.page.stroke(&ring(1.1), 0.13, INK, None);
                plot.page.line([c[0] - 0.78, c[1] - 0.78], [c[0] + 0.78, c[1] + 0.78], 0.1, INK);
                plot.page.line([c[0] - 0.78, c[1] + 0.78], [c[0] + 0.78, c[1] - 0.78], 0.1, INK);
            }
            FixtureKind::TubeLight { length, .. } | FixtureKind::Strip { length } => {
                let turn = if on_wall { facing + std::f32::consts::FRAC_PI_2 } else { facing };
                let half = scale([turn.cos(), turn.sin()], length / 2.0 * k);
                plot.page.line([c[0] - half[0], c[1] - half[1]], [c[0] + half[0], c[1] + half[1]], 0.5, GREY);
            }
            FixtureKind::Vent { .. } => plot.page.text([c[0], c[1] - 0.8], "V", 5.0, GREY, 0.5, 0.0, false),
        }
    }

    // Dimensions: a chain between the walls and an overall figure, below and to the left.
    let xs = stations(level, 0, lo[0], hi[0]);
    for pair in xs.windows(2) {
        plot.dimension([pair[0], lo[1]], [pair[1], lo[1]], -8.0);
    }
    plot.dimension([lo[0], lo[1]], [hi[0], lo[1]], -15.0);
    let ys = stations(level, 1, lo[1], hi[1]);
    for pair in ys.windows(2) {
        plot.dimension([lo[0], pair[0]], [lo[0], pair[1]], 8.0);
    }
    plot.dimension([lo[0], lo[1]], [lo[0], hi[1]], 15.0);

    // North arrow and a scale bar.
    let north = project.site.north.to_radians();
    let c = [pw - EDGE - 12.0, ph - EDGE - 14.0];
    let tip = [c[0] - north.sin() * 7.0, c[1] + north.cos() * 7.0];
    let tail = [c[0] + north.sin() * 5.0, c[1] - north.cos() * 5.0];
    let wing = |side: f32| [tail[0] + north.cos() * 3.0 * side, tail[1] + north.sin() * 3.0 * side];
    page.fill(&[tip, wing(1.0), c, wing(-1.0)], INK);
    page.text([tip[0], tip[1] + 1.5], "N", 8.0, INK, 0.5, 0.0, true);
    let unit = if project.units == Units::Metric { 1.0 } else { 0.3048 * 5.0 };
    let (bx, by) = (pw / 2.0 - 2.5 * unit * k, EDGE + 4.0);
    for i in 0..5 {
        let x = bx + i as f32 * unit * k;
        let cell = [[x, by], [x + unit * k, by], [x + unit * k, by + 1.6], [x, by + 1.6]];
        if i % 2 == 0 {
            page.fill(&cell, INK);
        }
        page.stroke(&[cell[0], cell[1], cell[2], cell[3], cell[0]], 0.1, INK, None);
    }
    page.text([bx, by + 2.6], "0", 6.0, INK, 0.5, 0.0, false);
    page.text([bx + 5.0 * unit * k, by + 2.6], &units::length(5.0 * unit, project.units), 6.0, INK, 0.5, 0.0, false);
    page
}

/// Dimensioned plans of every level, one a page, top level first.
pub fn plans(project: &Project, sheet: Sheet) -> Vec<u8> {
    let pages: Vec<Page> = project.levels.iter().rev().map(|l| level_page(project, l, sheet)).collect();
    pdf(&pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn the_house_draws_as_a_well_formed_pdf_with_a_page_a_level() {
        let project = crate::sample::house();
        let bytes = plans(&project, Sheet { paper: Paper::A3, scale: None });
        let doc = text(&bytes);
        assert!(doc.starts_with("%PDF-1.4"));
        assert_eq!(doc.matches("/Type /Page ").count(), 2, "one page a level");
        // Every offset in the cross-reference table points at the object it names.
        let xref = doc.rfind("startxref\n").map(|i| doc[i + 10..].lines().next().unwrap().parse::<usize>().unwrap()).unwrap();
        assert!(bytes[xref..].starts_with(b"xref\n"));
        let table = text(&bytes[xref..]);
        for (i, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(bytes[offset..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "object {} is not at {offset}", i + 1);
        }
        // Stream lengths are what they say.
        let find = |needle: &[u8], from: usize| bytes[from..].windows(needle.len()).position(|w| w == needle).map(|i| i + from);
        let mut at = 0;
        let mut streams = 0;
        while let Some(i) = find(b"/Length ", at) {
            let n: usize = text(&bytes[i + 8..i + 20]).split(' ').next().unwrap().parse().unwrap();
            let start = find(b"stream\n", i).unwrap() + 7;
            assert_eq!(&bytes[start + n..start + n + 10], b"\nendstream");
            at = start + n;
            streams += 1;
        }
        assert_eq!(streams, 2);
        // What a builder reads off it: rooms, the scale, and the overall size.
        for needle in ["(Living room)", "(Kitchen & dining)", "(Scale 1:75)", "(15 m)", "(11 m)", "(Main plan)", "(Basement plan)"] {
            assert!(doc.contains(needle), "the drawing lacks {needle}");
        }
    }

    #[test]
    fn the_scale_is_the_largest_standard_one_that_fits_the_paper() {
        let project = crate::sample::house();
        let main = project.levels.last().unwrap();
        // 15 m × 14.2 m: 1:75 on A3, 1:200 on A4; a small room gets a bigger scale.
        assert_eq!(fit(main, Paper::A3, Units::Metric), 75.0);
        assert_eq!(fit(main, Paper::A4, Units::Metric), 200.0);
        let mut small = crate::sample::blank();
        small.levels[0].rooms.push(Room { outline: vec![[0.0, 0.0], [3.0, 0.0], [3.0, 2.4], [0.0, 2.4]], ..project.levels[0].rooms[0].clone() });
        assert_eq!(fit(&small.levels[0], Paper::A3, Units::Metric), 20.0);
        assert_eq!(scale_name(Units::Imperial, fit(main, Paper::Tabloid, Units::Imperial)), "1/8\" = 1'");
        // Chain stations: the outside corners and the walls between them, in order.
        let xs = stations(main, 0, 0.0, 15.0);
        assert_eq!((xs.first(), xs.last()), (Some(&0.0), Some(&15.0)));
        assert!(xs.contains(&6.2) && xs.windows(2).all(|w| w[0] < w[1]));
        assert!((text_width("Lintel", 10.0) - 23.9).abs() < 0.01);
    }
}
