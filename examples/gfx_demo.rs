//! A test card for the 3D renderer: one of every material on a hand-built scene, in the
//! viewport slot under a Rune label.
//!
//! `cargo run --release --example gfx_demo -- --snap out.png --scene floor --size 1100x700`
//! Scenes: (default), `floor`, `inside`, `elevation`, `selected`, `samples`.

use std::sync::Arc;

use lintel::camera::{Camera, Projection};
use lintel::gfx::{Gfx, Host, SLOT, Sun, Viewport};
use lintel::materials::{Library, Surface};
use lintel::model::{Finish, Pick, rgb};
use lintel::scene::Scene;
use rune::tok::{self, type_};
use rune::{AppSpec, Cx, El, col, effect, el, text};

struct State {
    scene: Arc<Scene>,
    camera: Camera,
    selected: Option<Pick>,
    label: &'static str,
}

impl Host for State {
    fn viewport(&self) -> Option<Viewport> {
        Some(Viewport { scene: self.scene.clone(), revision: 1, camera: self.camera, sun: Sun::default(), selected: self.selected, hovered: Some(Pick::Furniture(2)) })
    }
}

/// A box with a surface per face: south, east, north, west, top, bottom.
fn bx(s: &mut Scene, lo: [f32; 3], hi: [f32; 3], f: [&Surface; 6], pick: f32) {
    let ([x0, y0, z0], [x1, y1, z1]) = (lo, hi);
    s.quad([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], [x0, z0], f[0], pick);
    s.quad([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], [y0, z0], f[1], pick);
    s.quad([[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]], [-x1, z0], f[2], pick);
    s.quad([[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]], [-y1, z0], f[3], pick);
    s.quad([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], [x0, y0], f[4], pick);
    s.quad([[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]], [y0, x0], f[5], pick);
}

fn solid(s: &mut Scene, lo: [f32; 3], hi: [f32; 3], f: &Surface, pick: f32) {
    bx(s, lo, hi, [f; 6], pick);
}

fn build() -> Scene {
    let lib = Library::builtin();
    let m = |id: &str| lib.surface(&Finish::of(id));
    let paint = |hex: u32| lib.surface(&Finish::paint(hex));
    let mut s = Scene::default();
    let white = paint(0xf2efe8);

    // Ground, with a driveway, a path, a garden bed and a gravel strip.
    s.quad([[-20.0, -20.0, 0.0], [20.0, -20.0, 0.0], [20.0, 20.0, 0.0], [-20.0, 20.0, 0.0]], [0.0, 0.0], &m("grass"), 0.0);
    let flat = |s: &mut Scene, x0: f32, y0: f32, x1: f32, y1: f32, f: &Surface| s.quad([[x0, y0, 0.02], [x1, y0, 0.02], [x1, y1, 0.02], [x0, y1, 0.02]], [x0, y0], f, 0.0);
    flat(&mut s, -6.0, -9.0, -2.0, 0.0, &m("asphalt"));
    flat(&mut s, 2.0, -9.0, 3.2, 0.0, &m("concrete"));
    flat(&mut s, 6.4, 0.0, 8.0, 4.0, &m("soil"));
    flat(&mut s, 3.4, -4.0, 6.0, -0.4, &m("pavers"));
    flat(&mut s, -1.6, -6.0, 1.6, -3.0, &m("gravel"));

    // A 6 × 4 × 2.7 m house: brick south, siding east, painted north, render west.
    let (t, h) = (0.2, 2.7);
    let wall = s.pick_id(Some(Pick::Wall(1)));
    bx(&mut s, [t, 0.0, 0.0], [6.0 - t, t, h], [&m("brick"), &m("brick"), &white, &m("brick"), &white, &white], wall);
    bx(&mut s, [6.0 - t, 0.0, 0.0], [6.0, 4.0, h], [&white, &m("siding"), &white, &paint(0xc9d6d2), &white, &white], 0.0);
    bx(&mut s, [t, 4.0 - t, 0.0], [6.0 - t, 4.0, h], [&white, &m("siding"), &white, &white, &white, &white], 0.0);
    bx(&mut s, [0.0, 0.0, 0.0], [t, 4.0, h], [&white, &paint(0xd9c7a8), &white, &m("render"), &white, &white], 0.0);
    // Floors: oak boards to the west, tile to the east, a rug on the boards.
    s.quad([[t, t, 0.03], [3.2, t, 0.03], [3.2, 4.0 - t, 0.03], [t, 4.0 - t, 0.03]], [0.0, 0.0], &m("oak_boards"), 0.0);
    s.quad([[3.2, t, 0.03], [6.0 - t, t, 0.03], [6.0 - t, 4.0 - t, 0.03], [3.2, 4.0 - t, 0.03]], [0.0, 0.0], &m("tile"), 0.0);
    solid(&mut s, [0.6, 1.0, 0.03], [2.6, 2.6, 0.045], &m("rug"), 0.0);
    // Furniture: a marble block, a fabric sofa, a pine table, a steel fridge, a stone bench.
    let block = s.pick_id(Some(Pick::Furniture(1)));
    solid(&mut s, [1.1, 1.4, 0.045], [2.1, 2.0, 0.45], &m("marble"), block);
    let sofa = s.pick_id(Some(Pick::Furniture(2)));
    solid(&mut s, [0.5, 2.9, 0.03], [2.7, 3.7, 0.45], &m("fabric"), sofa);
    solid(&mut s, [0.5, 3.5, 0.45], [2.7, 3.7, 0.85], &m("fabric"), sofa);
    solid(&mut s, [3.6, 0.5, 0.72], [4.8, 1.3, 0.76], &m("pine"), 0.0);
    for (x, y) in [(3.65, 0.55), (4.7, 0.55), (3.65, 1.2), (4.7, 1.2)] {
        solid(&mut s, [x, y, 0.03], [x + 0.05, y + 0.05, 0.72], &m("walnut"), 0.0);
    }
    solid(&mut s, [5.0, 2.9, 0.03], [5.75, 3.7, 1.8], &m("steel"), 0.0);
    solid(&mut s, [3.4, 3.1, 0.03], [4.9, 3.7, 0.9], &paint(0x2f4858), 0.0);
    solid(&mut s, [3.38, 3.08, 0.9], [4.92, 3.72, 0.94], &m("granite"), 0.0);
    solid(&mut s, [5.2, 1.6, 0.03], [5.7, 2.2, 0.5], &m("marble_dark"), 0.0);
    // A light strip on the north wall, and a glass table top over black steel.
    solid(&mut s, [1.0, 3.76, 2.3], [2.4, 3.8, 2.36], &m("light"), 0.0);
    solid(&mut s, [3.5, 1.9, 0.42], [4.5, 2.6, 0.44], &m("glass"), 0.0);
    solid(&mut s, [3.55, 1.95, 0.03], [3.6, 2.0, 0.42], &m("black_steel"), 0.0);
    solid(&mut s, [4.4, 2.5, 0.03], [4.45, 2.55, 0.42], &m("black_steel"), 0.0);
    // A shingle roof plane over the east half, on a brass fascia.
    let roof = m("shingle");
    s.quad([[2.9, -0.4, 2.7], [6.4, -0.4, 2.7], [6.4, 4.4, 3.9], [2.9, 4.4, 3.9]], [0.0, 0.0], &roof, 0.0);
    solid(&mut s, [2.9, -0.44, 2.6], [6.4, -0.4, 2.72], &m("brass"), 0.0);

    // Outside: a glazed screen, a roller door panel, roofing ramps, samples on plinths.
    solid(&mut s, [-3.0, 1.0, 0.0], [-2.95, 3.4, 2.1], &m("glass_tinted"), 0.0);
    solid(&mut s, [-3.05, 0.9, 0.0], [-2.9, 1.0, 2.1], &m("aluminium"), 0.0);
    solid(&mut s, [-3.05, 3.4, 0.0], [-2.9, 3.5, 2.1], &m("aluminium"), 0.0);
    bx(&mut s, [-5.6, 5.0, 0.0], [-2.4, 5.1, 2.2], [&m("roller_door"), &white, &m("cedar_siding"), &white, &white, &white], 0.0);
    s.quad([[7.0, -5.0, 0.3], [10.0, -5.0, 0.3], [10.0, -2.5, 1.5], [7.0, -2.5, 1.5]], [0.0, 0.0], &m("roof_tile"), 0.0);
    s.quad([[7.0, -8.0, 0.3], [10.0, -8.0, 0.3], [10.0, -5.5, 1.5], [7.0, -5.5, 1.5]], [0.0, 0.0], &m("metal_roof"), 0.0);
    for (i, id) in ["carpet", "polished_concrete", "walnut_boards", "blockwork", "quartz", "slate", "decking", "mirror"].iter().enumerate() {
        let x = -9.0 + i as f32 * 1.5;
        solid(&mut s, [x, -12.0, 0.0], [x + 1.2, -10.8, 0.6], &m(id), 0.0);
    }
    let stain = lib.surface(&Finish::colored("pine", rgb(0x5a3a22)));
    solid(&mut s, [3.2, -12.0, 0.0], [4.4, -10.8, 0.6], &stain, 0.0);
    solid(&mut s, [4.7, -12.0, 0.0], [5.9, -10.8, 0.6], &m("leather"), 0.0);

    s.bounds = ([-6.0, -9.0, 0.0], [8.0, 5.1, 3.9]);
    s
}

fn init() -> State {
    State { scene: Arc::new(build()), camera: Camera { target: [2.5, 1.0, 1.0], yaw: -2.15, pitch: 0.48, distance: 24.0, ..Camera::default() }, selected: None, label: "Perspective" }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    let label = col().absolute(16.0, 16.0).padding(10.0).gap(2.0).radius(8.0).fill(t.bg.alpha(0.82))
        .child(text("Lintel renderer test card").font(type_::TITLE).color(t.text))
        .child(text(s.label).font(type_::BODY).color(t.text_secondary));
    el().key("root").w_full().h_full().fill(t.bg)
        .child(effect(SLOT).key("viewport").absolute(0.0, 0.0).w_full().h_full())
        .child(label)
}

fn main() {
    rune::App::new(AppSpec::<State> {
        scenes: &[
            ("floor", |s| {
                s.camera = Camera { target: [2.9, 2.0, 0.1], yaw: 2.2, pitch: 0.95, distance: 4.6, ..Camera::default() };
                s.label = "Close-up: boards, tile, rug, marble";
            }),
            ("inside", |s| {
                s.camera = Camera { target: [3.0, 2.4, 0.9], yaw: -2.6, pitch: 0.2, distance: 3.2, projection: Projection::Perspective { fov_y: 1.0 }, cut: None };
                s.label = "Inside, under the roof";
            }),
            ("elevation", |s| {
                // Looking north at a section cut through the middle of the house.
                s.camera = Camera::elevation([3.0, 2.0, 1.6], -std::f32::consts::FRAC_PI_2, 7.0);
                s.camera.cut = Some([0.0, 1.0, 0.0, -2.2]);
                s.label = "Orthographic section, looking north, cut at y = 2.2";
            }),
            ("selected", |s| {
                s.selected = Some(Pick::Wall(1));
                s.camera = Camera { target: [3.0, 1.5, 1.0], yaw: -1.2, pitch: 0.4, distance: 14.0, ..Camera::default() };
                s.label = "South wall selected, sofa hovered";
            }),
            ("samples", |s| {
                s.camera = Camera { target: [-1.5, -11.4, 0.3], yaw: -1.57, pitch: 0.7, distance: 9.0, ..Camera::default() };
                s.label = "Samples";
            }),
        ],
        ..AppSpec::new("Lintel gfx demo", (1100.0, 700.0), init, view)
    })
    .compositor(Gfx::new())
    .run();
}
