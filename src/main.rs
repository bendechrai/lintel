//! Lintel: model a home, in plan, elevation and 3D.
//!
//! `lintel [project.lintel.json]` opens a project, or the sample house.
//! `lintel --snap out.png --scene model` renders a view headless (see `shell::SCENES`).

fn main() {
    lintel::shell::run();
}
