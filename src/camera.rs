//! The 3D view's camera: an orbit around a target, in perspective or orthographic projection.

use crate::scene::Scene;
use crate::model::Pick;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// Vertical field of view, radians.
    Perspective { fov_y: f32 },
    /// World metres visible top to bottom.
    Orthographic { height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub target: [f32; 3],
    /// Compass-style angle of the eye around the target, radians from +X toward +Y.
    pub yaw: f32,
    /// Elevation of the eye above the horizon, radians: 0 looks level, PI/2 straight down.
    pub pitch: f32,
    pub distance: f32,
    pub projection: Projection,
    /// A cutting plane `[nx, ny, nz, d]`: fragments with `n · p + d < 0` are not drawn. Sections.
    pub cut: Option<[f32; 4]>,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: [0.0, 0.0, 1.5], yaw: -2.2, pitch: 0.5, distance: 30.0, projection: Projection::Perspective { fov_y: 0.7 }, cut: None }
    }
}

impl Camera {
    pub fn eye(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [self.target[0] + cy * cp * self.distance, self.target[1] + sy * cp * self.distance, self.target[2] + sp * self.distance]
    }

    /// Column-major view-projection matrix for a viewport of this aspect (width / height),
    /// with wgpu's 0..1 depth.
    pub fn view_proj(&self, _aspect: f32) -> [[f32; 4]; 4] {
        [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
    }

    /// Turn the eye around the target.
    pub fn orbit(&mut self, _dyaw: f32, _dpitch: f32) {}

    /// Slide the target so the scene follows a pointer that moved `dx, dy` points in a viewport
    /// `size` points across.
    pub fn pan(&mut self, _dx: f32, _dy: f32, _size: [f32; 2]) {}

    /// Move toward (factor < 1) or away from the target.
    pub fn zoom(&mut self, _factor: f32) {}

    /// Where a world point lands in a viewport of `size` points (origin top-left), if it's in
    /// front of the camera.
    pub fn project(&self, _p: [f32; 3], _size: [f32; 2]) -> Option<[f32; 2]> {
        None
    }

    /// The ray through a viewport point: origin and unit direction.
    pub fn ray(&self, _at: [f32; 2], _size: [f32; 2]) -> ([f32; 3], [f32; 3]) {
        ([0.0; 3], [0.0, 0.0, -1.0])
    }

    /// Aim at and back off from a box so all of it is in view.
    pub fn frame(&mut self, _bounds: ([f32; 3], [f32; 3])) {}
}

/// The nearest pickable thing a ray hits, with the distance along it.
pub fn raycast(_scene: &Scene, _origin: [f32; 3], _dir: [f32; 3], _cut: Option<[f32; 4]>) -> Option<(Pick, f32)> {
    None
}
