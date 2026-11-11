//! The 3D renderer: a `rune::Compositor` that draws the `Scene` into the `rune::effect(SLOT)`
//! element of the view.

use std::sync::Arc;

use crate::camera::Camera;
use crate::model::Pick;
use crate::scene::Scene;

/// The `El::effect` mark of the 3D viewport element.
pub const SLOT: u32 = 0x3d;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Radians from +X toward +Y, where the sun is.
    pub azimuth: f32,
    /// Radians above the horizon.
    pub altitude: f32,
}

impl Default for Sun {
    fn default() -> Self {
        Sun { azimuth: -1.1, altitude: 0.85 }
    }
}

/// What the 3D viewport shows this frame.
#[derive(Clone)]
pub struct Viewport {
    pub scene: Arc<Scene>,
    /// Changes whenever `scene` does, so the renderer knows to upload it again.
    pub revision: u64,
    pub camera: Camera,
    pub sun: Sun,
    pub selected: Option<Pick>,
    pub hovered: Option<Pick>,
}

/// App state the renderer can draw from.
pub trait Host: 'static {
    /// The viewport to draw, when the view has a `SLOT` element on screen.
    fn viewport(&self) -> Option<Viewport>;
}

pub struct Gfx;

impl Gfx {
    pub fn new() -> Gfx {
        Gfx
    }
}

impl<S: Host> rune::Compositor<S> for Gfx {
    fn render(&mut self, f: &mut rune::Frame<'_, S>) {
        f.draw_ui();
    }
}
