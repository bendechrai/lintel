//! Builds the `Scene` (triangles) for a `Project`.

mod fixtures;
mod geom;
mod openings;
mod roofs;
mod rooms;
mod terrain;
#[cfg(test)]
mod tests;
mod walls;

use crate::catalog::{self, PartFinish};
use crate::materials::{Library, Surface};
use crate::model::{Finish, Id, Level, P2, Pick, Project};
use crate::scene::Scene;

pub use geom::triangulate;
pub use terrain::Ground;

/// What to leave out, so the 3D view can open the house up like a doll's house.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildOptions {
    pub terrain: bool,
    pub roofs: bool,
    pub ceilings: bool,
    pub fixtures: bool,
    pub furniture: bool,
    /// Draw only levels up to and including this one (by id); `None` draws all visible levels.
    pub top_level: Option<Id>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions { terrain: true, roofs: true, ceilings: true, fixtures: true, furniture: true, top_level: None }
    }
}

/// What the builders write into.
pub(crate) struct Ctx<'a> {
    pub scene: &'a mut Scene,
    pub lib: &'a Library,
}

impl Ctx<'_> {
    pub fn surf(&self, finish: &Finish) -> Surface {
        self.lib.surface(finish)
    }
}

/// Ground height at a plan point as drawn: the terrain, dug out under the building.
/// Builds the whole ground each call; keep a `Ground` to ask about many points.
pub fn terrain_height(project: &Project, p: P2) -> f32 {
    Ground::new(project).height(p)
}

pub fn build(project: &Project, options: &BuildOptions) -> Scene {
    let lib = Library::new(project);
    let mut scene = Scene::default();
    let mut cx = Ctx { scene: &mut scene, lib: &lib };

    // With a top level chosen, the roof comes off and so does that
    // level's ceiling, to look down into it.
    let top = options.top_level.and_then(|id| project.levels.iter().position(|l| l.id == id));
    let cut = top.is_some();
    for (i, level) in project.levels.iter().enumerate() {
        if !level.visible || top.is_some_and(|t| i > t) {
            continue;
        }
        let ceilings = options.ceilings && !(cut && top == Some(i));
        walls::build(&mut cx, level);
        rooms::build(&mut cx, level, ceilings);
        if options.fixtures {
            fixtures::build(&mut cx, level);
        }
        if options.furniture {
            furniture(&mut cx, level);
        }
    }
    if options.roofs && !cut {
        for roof in project.roofs.iter().filter(|r| r.visible) {
            roofs::build(&mut cx, roof);
        }
    }
    let building = cx.scene.vertices.len();
    if options.terrain {
        let ground = Ground::new(project);
        terrain::build(&mut cx, project, &ground);
    }

    if building > 0 {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &scene.vertices[..building] {
            for i in 0..3 {
                lo[i] = lo[i].min(v.pos[i]);
                hi[i] = hi[i].max(v.pos[i]);
            }
        }
        scene.bounds = (lo, hi);
    }
    scene
}

fn furniture(cx: &mut Ctx, level: &Level) {
    for piece in &level.furniture {
        let pick = cx.scene.pick_id(Some(Pick::Furniture(piece.id)));
        let (sin, cos) = piece.turn.sin_cos();
        let floor = rooms::floor_at(level, piece.at);
        // A hung piece's top is at the ceiling above its centre.
        let base = if piece.hung { rooms::ceiling_at(level, piece.at) - piece.size[2].max(0.005) - 0.005 } else { floor + piece.lift };
        let frame = geom::Frame3::upright([piece.at[0], piece.at[1], base], [cos, sin]);
        let (main, accent) = (cx.surf(&piece.finish), cx.surf(&piece.accent));
        for part in catalog::parts(piece) {
            let s = match &part.finish {
                PartFinish::Main => main,
                PartFinish::Accent => accent,
                PartFinish::Fixed(finish) => cx.surf(finish),
            };
            geom::cuboid(cx.scene, &frame, part.min, part.max, &s, pick);
        }
    }
}
