//! The 3D view's camera: an orbit around a target, in perspective or orthographic projection.

use crate::model::Pick;
use crate::scene::{Scene, add, cross, dot, mul, normalize, sub};

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

/// Lowest and highest the eye may sit: a little under the horizon, just short of straight down.
pub const PITCH_MIN: f32 = -0.2;
pub const PITCH_MAX: f32 = 1.553;
const DISTANCE: (f32, f32) = (0.3, 2000.0);
const ORTHO_HEIGHT: (f32, f32) = (0.2, 2000.0);
/// How far behind and beyond the target an orthographic view reaches.
const ORTHO_REACH: f32 = 1000.0;

/// The camera's axes in world space: right, up and forward (toward the target), unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
}

impl Camera {
    /// A level orthographic view for elevations and sections, looking toward `yaw + PI`.
    pub fn elevation(target: [f32; 3], yaw: f32, height: f32) -> Camera {
        Camera { target, yaw, pitch: 0.0, distance: 100.0, projection: Projection::Orthographic { height }, cut: None }
    }

    fn offset(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [cy * cp, sy * cp, sp]
    }

    pub fn eye(&self) -> [f32; 3] {
        add(self.target, mul(self.offset(), self.distance))
    }

    pub fn basis(&self) -> Basis {
        let forward = mul(self.offset(), -1.0);
        // Looking straight down, "up" on screen is the way the eye would be facing.
        let mut right = cross(forward, [0.0, 0.0, 1.0]);
        if dot(right, right) < 1e-8 {
            right = [-self.yaw.sin(), self.yaw.cos(), 0.0];
        }
        let right = normalize(right);
        let up = cross(right, forward);
        Basis { right, up, forward }
    }

    /// Half the view's extent at the target, in metres per unit of NDC: (x, y).
    fn half_extent(&self, aspect: f32) -> [f32; 2] {
        let half = match self.projection {
            Projection::Perspective { fov_y } => (fov_y * 0.5).tan() * self.distance,
            Projection::Orthographic { height } => height * 0.5,
        };
        [half * aspect, half]
    }

    /// Column-major view-projection matrix for a viewport of this aspect (width / height),
    /// with wgpu's 0..1 depth.
    pub fn view_proj(&self, aspect: f32) -> [[f32; 4]; 4] {
        let Basis { right: r, up: u, forward: f } = self.basis();
        let eye = self.eye();
        // View space: x right, y up, looking down -z.
        let view = [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let proj = match self.projection {
            Projection::Perspective { fov_y } => {
                let t = (fov_y * 0.5).tan();
                let near = (self.distance * 0.02).clamp(0.05, 2.0);
                let far = (self.distance * 4.0).max(600.0) + 400.0;
                [
                    [1.0 / (t * aspect), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / t, 0.0, 0.0],
                    [0.0, 0.0, far / (near - far), -1.0],
                    [0.0, 0.0, near * far / (near - far), 0.0],
                ]
            }
            Projection::Orthographic { height } => {
                let (near, far) = (self.distance - ORTHO_REACH, self.distance + ORTHO_REACH);
                let h = height * 0.5;
                [
                    [1.0 / (h * aspect), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / h, 0.0, 0.0],
                    [0.0, 0.0, -1.0 / (far - near), 0.0],
                    [0.0, 0.0, -near / (far - near), 1.0],
                ]
            }
        };
        mat_mul(proj, view)
    }

    /// Turn the eye around the target.
    pub fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw = (self.yaw + dyaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + dpitch).clamp(PITCH_MIN, PITCH_MAX);
    }

    /// Slide the target so the scene follows a pointer that moved `dx, dy` points in a viewport
    /// `size` points across.
    pub fn pan(&mut self, dx: f32, dy: f32, size: [f32; 2]) {
        if size[1] <= 0.0 {
            return;
        }
        let k = self.half_extent(1.0)[1] * 2.0 / size[1];
        let b = self.basis();
        self.target = add(self.target, add(mul(b.right, -dx * k), mul(b.up, dy * k)));
    }

    /// Move toward (factor < 1) or away from the target.
    pub fn zoom(&mut self, factor: f32) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        match &mut self.projection {
            Projection::Perspective { .. } => self.distance = (self.distance * factor).clamp(DISTANCE.0, DISTANCE.1),
            Projection::Orthographic { height } => *height = (*height * factor).clamp(ORTHO_HEIGHT.0, ORTHO_HEIGHT.1),
        }
    }

    /// Where a world point lands in a viewport of `size` points (origin top-left), if it's in
    /// front of the camera.
    pub fn project(&self, p: [f32; 3], size: [f32; 2]) -> Option<[f32; 2]> {
        if size[1] <= 0.0 {
            return None;
        }
        let m = self.view_proj(size[0] / size[1]);
        let c: [f32; 4] = std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r]);
        if c[3] <= 1e-6 || c[2] < 0.0 {
            return None;
        }
        Some([(c[0] / c[3] * 0.5 + 0.5) * size[0], (0.5 - c[1] / c[3] * 0.5) * size[1]])
    }

    /// The ray through a viewport point: origin and unit direction.
    pub fn ray(&self, at: [f32; 2], size: [f32; 2]) -> ([f32; 3], [f32; 3]) {
        let b = self.basis();
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return (self.eye(), b.forward);
        }
        let ndc = [at[0] / size[0] * 2.0 - 1.0, 1.0 - at[1] / size[1] * 2.0];
        let half = self.half_extent(size[0] / size[1]);
        let across = add(mul(b.right, ndc[0] * half[0]), mul(b.up, ndc[1] * half[1]));
        match self.projection {
            Projection::Perspective { .. } => (self.eye(), normalize(add(mul(b.forward, self.distance), across))),
            // Parallel rays, starting behind anything the view can show.
            Projection::Orthographic { .. } => (add(add(self.target, across), mul(b.forward, -ORTHO_REACH)), b.forward),
        }
    }

    /// Aim at and back off from a box so all of it is in view.
    pub fn frame(&mut self, bounds: ([f32; 3], [f32; 3])) {
        let (lo, hi) = bounds;
        if (0..3).any(|i| !(lo[i].is_finite() && hi[i].is_finite()) || hi[i] < lo[i]) {
            return;
        }
        self.target = mul(add(lo, hi), 0.5);
        let radius = (len3(sub(hi, lo)) * 0.5).max(0.5);
        match &mut self.projection {
            Projection::Perspective { fov_y } => {
                // Fit the bounding sphere in the narrower of the two fields of view, assuming a
                // viewport no taller than it is wide.
                self.distance = (radius / (*fov_y * 0.5).sin() * 1.05).clamp(DISTANCE.0, DISTANCE.1);
            }
            Projection::Orthographic { height } => {
                *height = (radius * 2.0 * 1.1).clamp(ORTHO_HEIGHT.0, ORTHO_HEIGHT.1);
                self.distance = self.distance.max(radius * 2.0);
            }
        }
    }
}

fn len3(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// `a * b` for column-major matrices.
pub fn mat_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}

/// The nearest thing a ray hits, with the distance along it: its pick, if it has one. A surface
/// with no pick (terrain, a slab) in front of a pickable one blocks it. Points on the cut-away
/// side of `cut` don't count.
pub fn raycast(scene: &Scene, origin: [f32; 3], dir: [f32; 3], cut: Option<[f32; 4]>) -> Option<(Pick, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for tri in scene.opaque.chunks_exact(3).chain(scene.translucent.chunks_exact(3)) {
        let (Some(a), Some(b), Some(c)) = (scene.vertices.get(tri[0] as usize), scene.vertices.get(tri[1] as usize), scene.vertices.get(tri[2] as usize)) else { continue };
        // Möller–Trumbore.
        let e1 = sub(b.pos, a.pos);
        let e2 = sub(c.pos, a.pos);
        let p = cross(dir, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-10 {
            continue;
        }
        let inv = 1.0 / det;
        let s = sub(origin, a.pos);
        let u = dot(s, p) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(s, e1);
        let v = dot(dir, q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = dot(e2, q) * inv;
        if t <= 1e-5 || best.is_some_and(|(bt, _)| bt <= t) {
            continue;
        }
        if let Some(plane) = cut {
            let hit = add(origin, mul(dir, t));
            if dot([plane[0], plane[1], plane[2]], hit) + plane[3] < 0.0 {
                continue;
            }
        }
        best = Some((t, a.pick));
    }
    let (t, pick) = best?;
    let index = pick.round() as usize;
    (index >= 1).then(|| scene.picks.get(index - 1).map(|p| (*p, t))).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::Library;
    use crate::model::Finish;

    fn near(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < eps)
    }

    #[test]
    fn the_target_projects_to_the_centre_and_east_is_right_when_looking_north() {
        // Eye south of the target, looking north.
        let cam = Camera { target: [0.0, 0.0, 0.0], yaw: -std::f32::consts::FRAC_PI_2, pitch: 0.3, distance: 20.0, ..Camera::default() };
        let size = [800.0, 600.0];
        let c = cam.project([0.0, 0.0, 0.0], size).unwrap();
        assert!((c[0] - 400.0).abs() < 0.01 && (c[1] - 300.0).abs() < 0.01, "{c:?}");
        let east = cam.project([2.0, 0.0, 0.0], size).unwrap();
        assert!(east[0] > c[0] + 10.0, "east is to the right: {east:?}");
        let up = cam.project([0.0, 0.0, 2.0], size).unwrap();
        assert!(up[1] < c[1] - 10.0, "up is up: {up:?}");
        assert!(cam.project([0.0, -40.0, 0.0], size).is_none(), "behind the eye");
    }

    #[test]
    fn a_ray_through_a_projected_point_passes_through_it() {
        let size = [1100.0, 700.0];
        for projection in [Projection::Perspective { fov_y: 0.7 }, Projection::Orthographic { height: 12.0 }] {
            let cam = Camera { target: [1.0, 2.0, 1.0], yaw: 0.9, pitch: 0.4, distance: 25.0, projection, cut: None };
            for p in [[3.0, 1.0, 0.5], [-2.0, 4.0, 2.5], [1.0, 2.0, 1.0]] {
                let at = cam.project(p, size).unwrap();
                let (o, d) = cam.ray(at, size);
                let t = dot(sub(p, o), d);
                assert!(t > 0.0);
                assert!(near(add(o, mul(d, t)), p, 2e-2), "{projection:?} {p:?} → {:?}", add(o, mul(d, t)));
            }
        }
    }

    #[test]
    fn panning_keeps_the_point_under_the_pointer() {
        let size = [1000.0, 800.0];
        for projection in [Projection::Perspective { fov_y: 0.7 }, Projection::Orthographic { height: 12.0 }] {
            let mut cam = Camera { target: [0.0, 0.0, 0.0], yaw: 1.0, pitch: 0.6, distance: 18.0, projection, cut: None };
            let world = cam.target;
            cam.pan(40.0, -25.0, size);
            let p = cam.project(world, size).unwrap();
            assert!((p[0] - 540.0).abs() < 0.5 && (p[1] - 375.0).abs() < 0.5, "{projection:?} {p:?}");
        }
    }

    #[test]
    fn framing_puts_the_whole_box_in_view() {
        let size = [900.0, 600.0];
        for projection in [Projection::Perspective { fov_y: 0.7 }, Projection::Orthographic { height: 1.0 }] {
            let mut cam = Camera { projection, ..Camera::default() };
            let (lo, hi) = ([-6.0, -4.0, -3.0], [8.0, 5.0, 7.0]);
            cam.frame((lo, hi));
            for i in 0..8 {
                let p = [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }];
                let at = cam.project(p, size).unwrap();
                assert!(at[0] >= 0.0 && at[0] <= size[0] && at[1] >= 0.0 && at[1] <= size[1], "{projection:?} {p:?} → {at:?}");
            }
        }
    }

    #[test]
    fn a_ray_picks_the_nearest_quad_and_an_unpickable_one_blocks() {
        let lib = Library::builtin();
        let s = lib.surface(&Finish::of("paint"));
        let mut scene = Scene::default();
        let wall = scene.pick_id(Some(Pick::Wall(7)));
        let far = scene.pick_id(Some(Pick::Wall(9)));
        let quad = |y: f32| [[-1.0, y, 0.0], [1.0, y, 0.0], [1.0, y, 2.0], [-1.0, y, 2.0]];
        scene.quad(quad(5.0), [0.0, 0.0], &s, wall);
        scene.quad(quad(8.0), [0.0, 0.0], &s, far);
        let (o, d) = ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
        let (pick, t) = raycast(&scene, o, d, None).unwrap();
        assert_eq!(pick, Pick::Wall(7));
        assert!((t - 5.0).abs() < 1e-4);
        // Miss to the side.
        assert!(raycast(&scene, [3.0, 0.0, 1.0], d, None).is_none());
        // Cut away everything nearer than y = 6: the far wall is what's hit.
        let (pick, t) = raycast(&scene, o, d, Some([0.0, 1.0, 0.0, -6.0])).unwrap();
        assert_eq!(pick, Pick::Wall(9));
        assert!((t - 8.0).abs() < 1e-4);
        // An unpickable quad in front blocks.
        scene.quad(quad(2.0), [0.0, 0.0], &s, 0.0);
        assert!(raycast(&scene, o, d, None).is_none());
    }
}
