//! Builds the `Scene` (triangles) for a `Project`.

use crate::model::{Id, Project};
use crate::scene::Scene;

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

pub fn build(_project: &Project, _options: &BuildOptions) -> Scene {
    Scene::default()
}
