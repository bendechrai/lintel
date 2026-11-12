//! Editor state and the operations on it. Views read `State`; handlers call these methods.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use rune::TextEdit;

use crate::camera::{Camera, Projection};
use crate::geom;
use crate::gfx::{Host, Sun, Viewport};
use crate::mesh::{self, BuildOptions};
use crate::model::*;
use crate::scene::{Scene, Vertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Plan,
    Elevation,
    Model,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Select,
    Wall,
    Room,
    Door,
    Window,
    Passage,
    Garage,
    Recess,
    Stair,
    Fixture(FixtureKind),
    Furniture(String),
    /// One of the project's saved products, by its place in the list.
    Product(usize),
    Patch,
    Roof,
    Barrier(BarrierKind),
    Plant(PlantKind),
    /// Drawing a line along a known length on the floor plan image, to set its scale.
    Calibrate,
}

impl Tool {
    pub fn hint(&self) -> &'static str {
        match self {
            Tool::Select => "Click to select. Drag things to move them, drag empty space to pan, scroll to zoom.",
            Tool::Wall => "Drag to draw a wall. Ends snap to other walls and to the grid; hold Shift to draw off-axis.",
            Tool::Room => "Drag a rectangle for the room's floor. Drag its corners afterwards; double-click an edge to add a corner.",
            Tool::Door | Tool::Window | Tool::Passage | Tool::Garage | Tool::Recess => "Click on a wall to place it there.",
            Tool::Stair => "Drag from the foot of the stair toward the top.",
            Tool::Fixture(_) => "Click to place: wall fixtures go on the nearest wall face, ceiling fixtures where you click.",
            Tool::Furniture(_) | Tool::Product(_) => "Click to place it. Press R to turn the selected piece.",
            Tool::Patch => "Drag a rectangle of ground to surface (a driveway, path or patio).",
            Tool::Roof => "Drag a rectangle over the walls the roof covers.",
            Tool::Barrier(_) => "Drag along the line it follows. Afterwards, drag its ends, or double-click it to add a corner.",
            Tool::Plant(_) => "Click where it grows.",
            Tool::Calibrate => "Drag a line along something on the image whose real length you know, then type that length.",
        }
    }
}

/// The plan view's camera: the model point at the centre of the canvas, and points per metre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanCam {
    pub center: P2,
    pub zoom: f32,
}

impl PlanCam {
    pub fn to_screen(&self, p: P2, size: [f32; 2]) -> [f32; 2] {
        [(p[0] - self.center[0]) * self.zoom + size[0] / 2.0, (self.center[1] - p[1]) * self.zoom + size[1] / 2.0]
    }

    pub fn to_model(&self, s: [f32; 2], size: [f32; 2]) -> P2 {
        [(s[0] - size[0] / 2.0) / self.zoom + self.center[0], self.center[1] - (s[1] - size[1] / 2.0) / self.zoom]
    }

    /// Zoom by `factor`, keeping the model point under `at` (canvas points) where it is.
    pub fn zoom_about(&mut self, at: [f32; 2], size: [f32; 2], factor: f32) {
        let before = self.to_model(at, size);
        self.zoom = (self.zoom * factor).clamp(4.0, 600.0);
        let after = self.to_model(at, size);
        self.center = geom::add(self.center, geom::sub(before, after));
    }
}

/// Which way an elevation looks at the house from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    South,
    East,
    North,
    West,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::South, Side::East, Side::North, Side::West];

    pub fn name(self) -> &'static str {
        match self {
            Side::South => "South",
            Side::East => "East",
            Side::North => "North",
            Side::West => "West",
        }
    }

    /// Direction from the house toward the viewer.
    pub fn yaw(self) -> f32 {
        match self {
            Side::South => -std::f32::consts::FRAC_PI_2,
            Side::East => 0.0,
            Side::North => std::f32::consts::FRAC_PI_2,
            Side::West => std::f32::consts::PI,
        }
    }
}

/// A gesture in progress on the plan canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum DragOp {
    Idle,
    Pan { center: P2 },
    /// Moving the selection: `grab` is where the press landed, in model space.
    Move { pick: Pick, grab: P2 },
    /// Moving walls' ends that meet at a corner: (wall, 0 for `a` or 1 for `b`).
    WallEnds { ends: Vec<(Id, u8)> },
    /// Moving one corner of a room or patch outline.
    Vertex { pick: Pick, index: usize },
    /// Drawing: from `a` to `b`, both snapped.
    Draw { a: P2, b: P2 },
}

/// A value being typed into an inspector field.
pub struct Editing {
    pub key: String,
    pub edit: TextEdit,
    pub apply: Rc<dyn Fn(&mut State, &str)>,
}

/// The terrain fetch form.
pub struct GeoForm {
    pub lat: TextEdit,
    pub lon: TextEdit,
    pub span: f32,
    pub status: String,
    pub request: rune::Latest<Result<crate::geo::Fetched, String>>,
}

/// Two options standing side by side in the 3D view: the other's name, and where each is.
pub struct Beside {
    pub name: String,
    pub ours: ([f32; 3], [f32; 3]),
    pub theirs: ([f32; 3], [f32; 3]),
}

/// The conversation with the measuring assistant.
pub struct Assistant {
    pub open: bool,
    /// Your own API key, typed in for this session. Never saved, never part of a project.
    pub key: TextEdit,
    pub input: TextEdit,
    pub lines: Vec<crate::assistant::Line>,
    /// The conversation as the API takes it.
    pub messages: Vec<serde_json::Value>,
    pub request: rune::Latest<Result<serde_json::Value, String>>,
    pub status: String,
    /// Read replies aloud with the computer's voice.
    pub speak: bool,
    /// Requests made for the message being answered, to stop a runaway exchange.
    rounds: usize,
    /// Messages you have sent: each one's changes are a single undo step.
    turns: usize,
}

impl Assistant {
    fn new() -> Assistant {
        Assistant { open: false, key: TextEdit::new("").secure(), input: TextEdit::new(""), lines: vec![], messages: vec![], request: rune::Latest::default(), status: String::new(), speak: false, rounds: 0, turns: 0 }
    }

    /// The key to use: the one typed in, or else the environment's.
    pub fn api_key(&self) -> Option<String> {
        let typed = self.key.text.trim();
        if !typed.is_empty() {
            return Some(typed.to_string());
        }
        std::env::var("ANTHROPIC_API_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
    }

    pub fn busy(&self) -> bool {
        self.request.is_pending()
    }
}

/// Calculated light levels and how they are shown.
pub struct LuxView {
    pub settings: crate::lux::Settings,
    pub request: rune::Latest<crate::lux::Levels>,
    pub levels: Option<crate::lux::Levels>,
    /// True when the project changed after `levels` was calculated.
    pub stale: bool,
    pub show: bool,
}

pub struct State {
    pub project: Project,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// What the last checkpoint was for, so a run of the same edit (a scrub, a nudge) is one undo.
    last_edit: String,

    pub view: ViewMode,
    /// The level the plan shows and edits.
    pub level: Id,
    pub tool: Tool,
    pub selected: Option<Pick>,
    pub hovered: Option<Pick>,

    pub plan: PlanCam,
    /// Size of the drawing area, in points.
    pub canvas: [f32; 2],
    pub drag: DragOp,
    /// The pointer on the plan, in model space, snapped: where a tool would act.
    pub cursor: Option<P2>,
    pub show_roofs: bool,
    pub show_site: bool,
    pub show_furniture: bool,
    pub show_fixtures: bool,
    pub show_below: bool,
    /// Thickness of the next wall the wall tool draws.
    pub wall_width: f32,
    /// True once the plan (or the elevation) has been panned or zoomed by hand, so a window
    /// resize stops refitting it.
    pub plan_moved: bool,
    pub elevation_moved: bool,
    /// Scratch field for a text input with nothing to edit.
    pub scratch: TextEdit,
    /// The slope the site panel's "make a slope" builds: bearing in degrees, fall in metres
    /// per metre.
    pub slope: (f32, f32),
    /// Which catalogue groups are folded open in the tool panel.
    pub open_groups: Vec<String>,
    pub lux: LuxView,
    pub assistant: Assistant,
    /// Another option's walls drawn faintly on the plan for comparison, by its name.
    pub compare: Option<String>,
    /// Set while the 3D view shows the compared option next to this one.
    pub beside: Option<Beside>,
    /// The paper and scale drawings are made for.
    pub sheet: crate::drawing::Sheet,
    /// Where to save a picture of the 3D view, while one is being taken.
    pub capture: Option<PathBuf>,
    /// Draw only the view, with no panels: for pictures of the house.
    pub bare: bool,
    /// Decodes floor plan images; set once the window is up.
    pub images: Option<rune::ImageCache>,
    /// The floor plan images in use, by path.
    pub underlays: Vec<(String, rune::Image)>,
    /// A line drawn along a known length on the floor plan image, waiting for that length.
    pub calibrate: Option<(P2, P2)>,
    /// The switch whose lights are being picked on the plan.
    pub linking: Option<Id>,
    /// Pointer travel at the last event of a viewport drag.
    pub drag_last: [f32; 2],

    pub camera: Camera,
    pub side: Side,
    pub elevation: Camera,
    /// Distance of the section plane behind the front of the house, when cutting.
    pub section: Option<f32>,
    pub options: BuildOptions,
    scene: Arc<Scene>,
    scene_revision: u64,
    /// True when the project changed since the scene was built.
    stale: bool,

    pub editing: Option<Editing>,
    /// The inspector row whose material picker is open.
    pub picker: Option<String>,
    pub name_edit: TextEdit,
    pub geo: GeoForm,
    pub status: String,
    pub dark: bool,
}

impl State {
    pub fn new(project: Project, path: Option<PathBuf>) -> State {
        let level = project.levels.last().map(|l| l.id).unwrap_or(0);
        let geo = GeoForm {
            lat: TextEdit::new(project.site.geo.map(|g| g.lat.to_string()).unwrap_or_default()),
            lon: TextEdit::new(project.site.geo.map(|g| g.lon.to_string()).unwrap_or_default()),
            span: 40.0,
            status: String::new(),
            request: rune::Latest::default(),
        };
        let mut s = State {
            name_edit: TextEdit::new(project.name.clone()),
            project,
            path,
            dirty: false,
            undo: vec![],
            redo: vec![],
            last_edit: String::new(),
            view: ViewMode::Plan,
            level,
            tool: Tool::Select,
            selected: None,
            hovered: None,
            plan: PlanCam { center: [0.0, 0.0], zoom: 40.0 },
            canvas: [900.0, 700.0],
            drag: DragOp::Idle,
            cursor: None,
            show_roofs: false,
            show_site: true,
            show_furniture: true,
            show_fixtures: true,
            show_below: true,
            wall_width: 0.115,
            plan_moved: false,
            elevation_moved: false,
            scratch: TextEdit::new(""),
            slope: (90.0, 0.1),
            open_groups: vec!["Living".into()],
            drag_last: [0.0, 0.0],
            linking: None,
            bare: false,
            capture: None,
            sheet: crate::drawing::Sheet { paper: crate::drawing::Paper::A3, scale: None },
            compare: None,
            beside: None,
            images: None,
            underlays: vec![],
            calibrate: None,
            lux: LuxView { settings: crate::lux::Settings::default(), request: rune::Latest::default(), levels: None, stale: false, show: false },
            assistant: Assistant::new(),
            camera: Camera::default(),
            side: Side::South,
            elevation: Camera::default(),
            section: None,
            options: BuildOptions::default(),
            scene: Arc::new(Scene::default()),
            scene_revision: 0,
            stale: true,
            editing: None,
            picker: None,
            geo,
            status: String::new(),
            dark: false,
        };
        s.rebuild_scene();
        s.fit_plan();
        s.camera.frame(s.scene.bounds);
        s.aim_elevation(true);
        s
    }

    // -----------------------------------------------------------------------------------------
    // History

    /// Call before changing the project. Consecutive calls with the same non-empty `tag` share
    /// one undo step.
    pub fn checkpoint(&mut self, tag: &str) {
        if tag.is_empty() || tag != self.last_edit {
            self.undo.push(self.project.clone());
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = tag.to_string();
    }

    /// Change the project as one undoable step.
    pub fn edit(&mut self, tag: &str, f: impl FnOnce(&mut Project)) {
        self.checkpoint(tag);
        f(&mut self.project);
        self.touch();
    }

    /// The project changed.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.stale = true;
        self.lux.stale = self.lux.levels.is_some();
        if self.view != ViewMode::Plan {
            self.rebuild_scene();
        }
    }

    /// Ends a run of same-tag edits, so the next one is its own undo step.
    pub fn wall_thickness(&self) -> f32 {
        self.wall_width
    }

    pub fn settle(&mut self) {
        self.last_edit.clear();
    }

    /// Drops the newest checkpoint: the gesture it was for changed nothing.
    pub fn forget_checkpoint(&mut self) {
        self.undo.pop();
        self.last_edit.clear();
    }

    /// The project as it was at the newest checkpoint (the start of the current gesture).
    pub fn before(&self) -> Option<&Project> {
        self.undo.last()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.after_history();
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.after_history();
        }
    }

    fn after_history(&mut self) {
        self.last_edit.clear();
        self.editing = None;
        if self.selected.is_some_and(|p| !self.exists(p)) {
            self.selected = None;
        }
        if self.project.level(self.level).is_none() {
            self.level = self.project.levels.last().map(|l| l.id).unwrap_or(0);
        }
        self.name_edit = TextEdit::new(self.project.name.clone());
        self.touch();
    }

    // -----------------------------------------------------------------------------------------
    // Scene and cameras

    /// Starts loading every floor plan image the project names that isn't loaded yet.
    pub fn load_underlays(&mut self) {
        let Some(cache) = &self.images else { return };
        for path in self.project.levels.iter().filter_map(|l| l.underlay.as_ref()).map(|u| u.path.clone()) {
            if !self.underlays.iter().any(|(p, _)| *p == path) {
                let image = cache.file(&path, 1024.0, 1.0);
                self.underlays.push((path, image));
            }
        }
    }

    pub fn underlay_image(&self, path: &str) -> Option<&rune::Image> {
        self.underlays.iter().find(|(p, _)| p == path).map(|(_, i)| i)
    }

    /// Lays an image under the active level, a guess at 1 cm a pixel until it is calibrated.
    pub fn set_underlay(&mut self, path: String) {
        let level = self.level;
        let corner = self.plan.to_model([40.0, self.canvas[1] - 40.0], self.canvas);
        self.edit("", |p| {
            if let Some(l) = p.level_mut(level) {
                l.underlay = Some(Underlay { path, at: corner, scale: 0.01, turn: 0.0, opacity: 0.6, visible: true });
            }
        });
        self.load_underlays();
        self.status = "Image placed. Use “Set its scale” and drag along a length you know.".into();
    }

    /// The line drawn for calibration is really `real` metres long: rescale the image to fit.
    pub fn apply_calibration(&mut self, real: f32) {
        let Some((a, b)) = self.calibrate.take() else { return };
        let level = self.level;
        let measured = geom::dist(a, b);
        self.edit("", |p| {
            if let Some(u) = p.level_mut(level).and_then(|l| l.underlay.as_mut()) {
                u.calibrate(a, measured, real);
            }
        });
    }

    /// Starts calculating light levels for the project as it stands.
    pub fn calculate_light(&mut self) {
        self.commit_editing();
        let (project, settings) = (self.project.clone(), self.lux.settings);
        self.lux.request.request((project, settings), |(project, settings)| crate::lux::compute(&project, &settings));
        self.lux.show = true;
    }

    /// Takes a finished light calculation, if one has landed.
    pub fn land_light(&mut self) -> bool {
        let Some(levels) = self.lux.request.take() else { return false };
        self.status = format!("Light levels calculated in {:.1} s.", levels.seconds);
        self.lux.levels = Some(levels);
        self.lux.stale = false;
        if self.view != ViewMode::Plan {
            self.rebuild_scene();
        }
        true
    }

    // -----------------------------------------------------------------------------------------
    // The measuring assistant

    /// Sends what is in the assistant's box.
    pub fn ask(&mut self) {
        let said = self.assistant.input.text.trim().to_string();
        if said.is_empty() || self.assistant.busy() {
            return;
        }
        if self.assistant.api_key().is_none() {
            self.assistant.status = "Enter your Anthropic API key first.".into();
            return;
        }
        self.assistant.input.clear();
        self.assistant.lines.push(crate::assistant::Line { from_you: true, text: said.clone() });
        self.assistant.messages.push(serde_json::json!({ "role": "user", "content": said }));
        self.assistant.rounds = 0;
        self.assistant.turns += 1;
        self.assistant_send();
    }

    fn assistant_send(&mut self) {
        let Some(key) = self.assistant.api_key() else { return };
        let body = crate::assistant::request(&self.assistant.messages);
        self.assistant.rounds += 1;
        self.assistant.status = "Thinking…".into();
        self.assistant.request.request((key, body), |(key, body)| crate::assistant::send(&key, &body));
    }

    /// Takes the assistant's answer, if one has landed.
    pub fn poll_assistant(&mut self) -> bool {
        let Some(answer) = self.assistant.request.take() else { return false };
        self.land_assistant(answer);
        true
    }

    /// Acts on one response from the API: shows a reply, or runs the tools it asks for and
    /// sends their results back.
    pub fn land_assistant(&mut self, answer: Result<serde_json::Value, String>) {
        use crate::assistant::{Line, Turn};
        let response = match answer {
            Ok(r) => r,
            Err(e) => {
                self.assistant.status = e;
                return;
            }
        };
        self.assistant.status.clear();
        match crate::assistant::read(&response) {
            Turn::Reply(text) => {
                // The whole content goes back next time, thinking included, as it came.
                self.assistant.messages.push(serde_json::json!({ "role": "assistant", "content": response["content"] }));
                if !text.is_empty() {
                    if self.assistant.speak {
                        crate::assistant::speak(&text);
                    }
                    self.assistant.lines.push(Line { from_you: false, text });
                }
            }
            Turn::Tools(text, calls) => {
                self.assistant.messages.push(serde_json::json!({ "role": "assistant", "content": response["content"] }));
                if !text.is_empty() {
                    self.assistant.lines.push(Line { from_you: false, text });
                }
                let changes = calls.iter().any(|(_, name, _)| name != "get_house");
                if changes {
                    self.checkpoint(&format!("assistant-{}", self.assistant.turns));
                }
                let mut level = self.level;
                let results: Vec<serde_json::Value> = calls
                    .iter()
                    .map(|(id, name, input)| match crate::assistant::apply(&mut self.project, &mut level, name, input) {
                        Ok(v) => serde_json::json!({ "type": "tool_result", "tool_use_id": id, "content": v.to_string() }),
                        Err(e) => serde_json::json!({ "type": "tool_result", "tool_use_id": id, "content": e, "is_error": true }),
                    })
                    .collect();
                if changes {
                    self.level = level;
                    self.selected = None;
                    self.touch();
                }
                // Every result goes back in one message.
                self.assistant.messages.push(serde_json::json!({ "role": "user", "content": results }));
                if self.assistant.rounds >= 16 {
                    self.assistant.status = "Stopped after many steps without an answer. Say what to do next.".into();
                } else {
                    self.assistant_send();
                }
            }
            Turn::Stopped(why) => self.assistant.lines.push(Line { from_you: false, text: why }),
        }
    }

    /// Starts the conversation afresh. The house stays as it is.
    pub fn forget_conversation(&mut self) {
        self.assistant.request.cancel();
        self.assistant.lines.clear();
        self.assistant.messages.clear();
        self.assistant.status.clear();
    }

    /// How many point sources the lamps that are on amount to.
    pub fn scene_lamps(&self) -> usize {
        self.project.levels.iter().flat_map(|l| &l.fixtures).filter(|f| f.output() > 0.0).count()
    }

    pub fn show_light(&mut self, show: bool) {
        self.lux.show = show;
        if self.view != ViewMode::Plan {
            self.rebuild_scene();
        }
    }

    /// The light-level map as coloured tiles on each room's measuring plane.
    fn light_tiles(&self, scene: &mut Scene) {
        let Some(levels) = self.lux.levels.as_ref().filter(|_| self.lux.show) else { return };
        let top = self.options.top_level.and_then(|id| self.project.levels.iter().position(|l| l.id == id));
        for room in &levels.rooms {
            let index = self.project.levels.iter().position(|l| l.id == room.level);
            if index.is_none() || top.is_some_and(|t| index.is_some_and(|i| i > t)) {
                continue;
            }
            let h = room.spacing / 2.0;
            for r in 0..room.rows {
                for c in 0..room.cols {
                    let v = room.lux[r * room.cols + c];
                    if !v.is_finite() {
                        continue;
                    }
                    let (x, y) = (room.origin[0] + c as f32 * room.spacing, room.origin[1] + r as f32 * room.spacing);
                    let surface = crate::materials::Surface { pattern: crate::materials::Pattern::Flat, color: crate::materials::linear(crate::lux::color(v)), scale: 0.0, roughness: 1.0, opacity: 0.82 };
                    scene.quad([[x - h, y - h, room.z], [x + h, y - h, room.z], [x + h, y + h, room.z], [x - h, y + h, room.z]], [0.0, 0.0], &surface, 0.0);
                }
            }
        }
    }

    pub fn rebuild_scene(&mut self) {
        let mut scene = mesh::build(&self.project, &self.options);
        self.light_tiles(&mut scene);
        self.beside = None;
        // In 3D, the option being compared stands next to this one, to the east.
        let other = self.compare.as_ref().filter(|_| self.view == ViewMode::Model).and_then(|name| self.project.others.iter().find(|o| o.option == *name));
        if let Some(other) = other {
            let theirs = mesh::build(other, &self.options);
            let ours = scene.bounds;
            let ground = if self.options.terrain { self.project.site.terrain.size()[0].max(other.site.terrain.size()[0]) } else { 0.0 };
            let dx = ground.max(ours.1[0] - ours.0[0]).max(theirs.bounds.1[0] - theirs.bounds.0[0]) + 3.0;
            let base = scene.vertices.len() as u32;
            // Only the house being edited can be picked.
            scene.vertices.extend(theirs.vertices.iter().map(|v| Vertex { pos: [v.pos[0] + dx, v.pos[1], v.pos[2]], pick: 0.0, ..*v }));
            scene.opaque.extend(theirs.opaque.iter().map(|i| i + base));
            scene.translucent.extend(theirs.translucent.iter().map(|i| i + base));
            let east = |p: [f32; 3]| [p[0] + dx, p[1], p[2]];
            scene.lamps.extend(theirs.lamps.iter().map(|l| crate::scene::Lamp { pos: east(l.pos), room: (east(l.room.0), east(l.room.1)), ..*l }));
            let there = (east(theirs.bounds.0), east(theirs.bounds.1));
            if there.0[0] <= there.1[0] {
                scene.bounds = (std::array::from_fn(|i| ours.0[i].min(there.0[i])), std::array::from_fn(|i| ours.1[i].max(there.1[i])));
                self.beside = Some(Beside { name: other.option.clone(), ours, theirs: there });
            }
        }
        self.scene = Arc::new(scene);
        self.scene_revision += 1;
        self.stale = false;
    }

    /// The sun as the renderer wants it: its bearing turned into the plan's axes.
    pub fn sun(&self) -> Sun {
        let (d, altitude) = self.project.site.sun_direction();
        Sun { azimuth: d[1].atan2(d[0]), altitude, overcast: self.project.site.sun.overcast }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
        self.drag = DragOp::Idle;
        self.cursor = None;
        if view != ViewMode::Plan && (self.stale || self.compare.is_some()) {
            self.rebuild_scene();
            if self.beside.is_some() {
                self.camera.frame(self.scene.bounds);
            }
        }
        if view == ViewMode::Elevation {
            self.aim_elevation(false);
        }
    }

    pub fn set_options(&mut self, f: impl FnOnce(&mut BuildOptions)) {
        f(&mut self.options);
        self.rebuild_scene();
    }

    /// Points the elevation camera at the house from `self.side`. `refit` also resets its zoom.
    pub fn aim_elevation(&mut self, refit: bool) {
        let (lo, hi) = self.scene.bounds;
        let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let yaw = self.side.yaw();
        let (sy, cy) = yaw.sin_cos();
        // Half the house's depth along the view direction: where its near face is.
        let half = ((hi[0] - lo[0]) * cy.abs() + (hi[1] - lo[1]) * sy.abs()) / 2.0;
        let width = (hi[0] - lo[0]) * sy.abs() + (hi[1] - lo[1]) * cy.abs();
        let c = &mut self.elevation;
        c.yaw = yaw;
        c.pitch = 0.0;
        c.distance = half + 60.0;
        if refit || !matches!(c.projection, Projection::Orthographic { .. }) {
            c.target = center;
            let aspect = (self.canvas[0] / self.canvas[1].max(1.0)).max(0.2);
            let height = ((hi[2] - lo[2]) + 3.0).max((width + 4.0) / aspect).max(4.0);
            c.projection = Projection::Orthographic { height };
        }
        c.cut = self.section.map(|depth| {
            let front = cy * center[0] + sy * center[1] + half;
            [-cy, -sy, 0.0, front - depth]
        });
    }

    /// Centres and zooms the plan on the active level (or the whole building).
    pub fn fit_plan(&mut self) {
        let mut lo = [f32::MAX; 2];
        let mut hi = [f32::MIN; 2];
        let mut grow = |p: P2| {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        };
        for l in &self.project.levels {
            l.walls.iter().for_each(|w| {
                grow(w.a);
                grow(w.b);
            });
            l.rooms.iter().flat_map(|r| &r.outline).for_each(|p| grow(*p));
        }
        if lo[0] > hi[0] {
            lo = [-6.0, -4.0];
            hi = [6.0, 4.0];
        }
        self.plan.center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let zx = self.canvas[0] / (hi[0] - lo[0] + 5.0);
        let zy = self.canvas[1] / (hi[1] - lo[1] + 5.0);
        self.plan.zoom = zx.min(zy).clamp(8.0, 200.0);
    }

    // -----------------------------------------------------------------------------------------
    // Finding things

    pub fn active(&self) -> Option<&Level> {
        self.project.level(self.level)
    }

    pub fn exists(&self, pick: Pick) -> bool {
        let p = &self.project;
        match pick {
            Pick::Wall(id) => p.levels.iter().any(|l| l.walls.iter().any(|w| w.id == id)),
            Pick::Opening(w, o) => p.levels.iter().any(|l| l.walls.iter().any(|x| x.id == w && x.openings.iter().any(|y| y.id == o))),
            Pick::Room(id) => p.levels.iter().any(|l| l.rooms.iter().any(|r| r.id == id)),
            Pick::Stair(id) => p.levels.iter().any(|l| l.stairs.iter().any(|r| r.id == id)),
            Pick::Fixture(id) => p.levels.iter().any(|l| l.fixtures.iter().any(|r| r.id == id)),
            Pick::Furniture(id) => p.levels.iter().any(|l| l.furniture.iter().any(|r| r.id == id)),
            Pick::Roof(id) => p.roofs.iter().any(|r| r.id == id),
            Pick::Patch(id) => p.site.patches.iter().any(|r| r.id == id),
            Pick::Barrier(id) => p.site.barriers.iter().any(|r| r.id == id),
            Pick::Plant(id) => p.site.plants.iter().any(|r| r.id == id),
        }
    }

    /// The level a pick lives on.
    pub fn level_of(&self, pick: Pick) -> Option<Id> {
        self.project.levels.iter().find(|l| match pick {
            Pick::Wall(id) | Pick::Opening(id, _) => l.walls.iter().any(|w| w.id == id),
            Pick::Room(id) => l.rooms.iter().any(|r| r.id == id),
            Pick::Stair(id) => l.stairs.iter().any(|r| r.id == id),
            Pick::Fixture(id) => l.fixtures.iter().any(|r| r.id == id),
            Pick::Furniture(id) => l.furniture.iter().any(|r| r.id == id),
            Pick::Roof(_) | Pick::Patch(_) | Pick::Barrier(_) | Pick::Plant(_) => false,
        }).map(|l| l.id)
    }

    pub fn select(&mut self, pick: Option<Pick>) {
        if self.selected != pick {
            self.commit_editing();
            self.picker = None;
            self.linking = None;
        }
        self.selected = pick;
        self.settle();
        if let Some(level) = pick.and_then(|p| self.level_of(p)) {
            self.level = level;
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.commit_editing();
        self.tool = tool;
        self.drag = DragOp::Idle;
        if self.tool != Tool::Select {
            self.selected = None;
        }
    }

    // -----------------------------------------------------------------------------------------
    // Typed values

    pub fn begin_editing(&mut self, key: String, text: String, apply: Rc<dyn Fn(&mut State, &str)>) {
        self.commit_editing();
        let mut edit = TextEdit::new(text.clone());
        edit.set_selection(0, text.len());
        self.editing = Some(Editing { key, edit, apply });
    }

    pub fn commit_editing(&mut self) {
        if let Some(e) = self.editing.take() {
            let text = e.edit.text.clone();
            (e.apply)(self, &text);
            self.settle();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Edits on the selection

    pub fn delete_selected(&mut self) {
        let Some(pick) = self.selected.take() else { return };
        self.edit("", |p| {
            for l in &mut p.levels {
                match pick {
                    Pick::Wall(id) => {
                        l.walls.retain(|w| w.id != id);
                        l.fixtures.retain(|f| !matches!(f.mount, Mount::Wall { wall, .. } if wall == id));
                    }
                    Pick::Opening(w, o) => l.walls.iter_mut().filter(|x| x.id == w).for_each(|x| x.openings.retain(|y| y.id != o)),
                    Pick::Room(id) => l.rooms.retain(|r| r.id != id),
                    Pick::Stair(id) => l.stairs.retain(|r| r.id != id),
                    Pick::Fixture(id) => {
                        l.fixtures.retain(|r| r.id != id);
                        l.fixtures.iter_mut().for_each(|f| f.controls.retain(|c| *c != id));
                    }
                    Pick::Furniture(id) => l.furniture.retain(|r| r.id != id),
                    Pick::Roof(_) | Pick::Patch(_) | Pick::Barrier(_) | Pick::Plant(_) => {}
                }
            }
            match pick {
                Pick::Roof(id) => p.roofs.retain(|r| r.id != id),
                Pick::Patch(id) => p.site.patches.retain(|r| r.id != id),
                Pick::Barrier(id) => p.site.barriers.retain(|r| r.id != id),
                Pick::Plant(id) => p.site.plants.retain(|r| r.id != id),
                _ => {}
            }
        });
    }

    /// A copy of the selected furniture, fixture, opening or room, offset a little.
    pub fn duplicate_selected(&mut self) {
        let Some(pick) = self.selected else { return };
        let mut made = None;
        self.edit("", |p| {
            let id = p.id();
            for l in &mut p.levels {
                match pick {
                    Pick::Furniture(src) => {
                        if let Some(f) = l.furniture.iter().find(|f| f.id == src).cloned() {
                            l.furniture.push(Furniture { id, at: geom::add(f.at, [0.4, -0.4]), ..f });
                            made = Some(Pick::Furniture(id));
                        }
                    }
                    Pick::Fixture(src) => {
                        if let Some(f) = l.fixtures.iter().find(|f| f.id == src).cloned() {
                            let mount = match f.mount {
                                Mount::Wall { wall, side, at, height } => Mount::Wall { wall, side, at: at + 0.4, height },
                                Mount::Ceiling { at, turn } => Mount::Ceiling { at: geom::add(at, [0.5, 0.0]), turn },
                                Mount::Floor { at, turn } => Mount::Floor { at: geom::add(at, [0.5, 0.0]), turn },
                            };
                            l.fixtures.push(Fixture { id, mount, controls: vec![], ..f });
                            made = Some(Pick::Fixture(id));
                        }
                    }
                    Pick::Opening(w, src) => {
                        if let Some(wall) = l.walls.iter_mut().find(|x| x.id == w) {
                            if let Some(o) = wall.openings.iter().find(|o| o.id == src).cloned() {
                                let at = (o.at + o.width + 0.3).min(wall.length() - o.width / 2.0);
                                wall.openings.push(Opening { id, at, ..o });
                                made = Some(Pick::Opening(w, id));
                            }
                        }
                    }
                    Pick::Room(src) => {
                        if let Some(r) = l.rooms.iter().find(|r| r.id == src).cloned() {
                            let outline = r.outline.iter().map(|p| geom::add(*p, [1.0, -1.0])).collect();
                            l.rooms.push(Room { id, outline, name: format!("{} copy", r.name), ..r });
                            made = Some(Pick::Room(id));
                        }
                    }
                    _ => {}
                }
            }
        });
        if made.is_some() {
            self.selected = made;
        }
    }

    /// Flicks a switch: its lights go off if any is on, else on.
    pub fn flick(&mut self, switch: Id) {
        let Some(lights) = self.project.fixture(switch).map(|f| f.controls.clone()) else { return };
        let any_on = lights.iter().filter_map(|id| self.project.fixture(*id)).any(|f| f.lamp().is_some_and(|l| l.on));
        self.edit("", |p| {
            for id in &lights {
                if let Some(f) = p.fixture_mut(*id) {
                    if let Some(lamp) = f.lamp() {
                        f.lamp = Some(Lamp { on: !any_on, ..lamp });
                    }
                }
            }
        });
    }

    /// Adds a light to a switch, or takes it off.
    pub fn link(&mut self, switch: Id, light: Id) {
        if self.project.fixture(light).and_then(|f| f.lamp()).is_none() {
            return;
        }
        self.edit("", |p| {
            if let Some(f) = p.fixture_mut(switch) {
                match f.controls.iter().position(|c| *c == light) {
                    Some(i) => {
                        f.controls.remove(i);
                    }
                    None => f.controls.push(light),
                }
            }
        });
    }

    /// Saves the selected piece, at its size and finishes, as a product to place again.
    pub fn save_product(&mut self) {
        let Some(Pick::Furniture(id)) = self.selected else { return };
        let Some(piece) = self.project.furniture(id).cloned() else { return };
        let product = Product::of(&piece);
        let name = product.name.clone();
        self.edit("", |p| match p.products.iter_mut().find(|x| x.name == product.name) {
            Some(existing) => *existing = product,
            None => p.products.push(product),
        });
        self.status = format!("“{name}” is saved under Your products in the tool panel.");
    }

    /// Reads a library file of products into the project, replacing ones of the same name.
    pub fn import_products(&mut self, path: &std::path::Path) {
        let read = std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Vec<Product>>(&t).map_err(|e| e.to_string()));
        match read {
            Ok(list) => {
                let n = list.len();
                self.edit("", |p| {
                    for product in list {
                        match p.products.iter_mut().find(|x| x.name == product.name) {
                            Some(existing) => *existing = product,
                            None => p.products.push(product),
                        }
                    }
                });
                self.status = format!("{n} products read from {}.", path.display());
            }
            Err(e) => self.status = format!("Couldn't read products from {}: {e}", path.display()),
        }
    }

    pub fn export_products(&mut self, path: &std::path::Path) {
        let json = serde_json::to_string_pretty(&self.project.products).unwrap_or_default();
        self.status = match std::fs::write(path, json) {
            Ok(()) => format!("{} products written to {}. Import that file into another project to use them there.", self.project.products.len(), path.display()),
            Err(e) => format!("Couldn't write {}: {e}", path.display()),
        };
    }

    /// Turn the selected furniture, stair or ceiling fixture by `by` radians.
    pub fn turn_selected(&mut self, by: f32) {
        let Some(pick) = self.selected else { return };
        self.edit("turn", |p| {
            for l in &mut p.levels {
                match pick {
                    Pick::Furniture(id) => l.furniture.iter_mut().filter(|f| f.id == id).for_each(|f| f.turn = wrap(f.turn + by)),
                    Pick::Stair(id) => l.stairs.iter_mut().filter(|f| f.id == id).for_each(|f| f.direction = wrap(f.direction + by)),
                    Pick::Fixture(id) => l.fixtures.iter_mut().filter(|f| f.id == id).for_each(|f| {
                        if let Mount::Ceiling { turn, .. } | Mount::Floor { turn, .. } = &mut f.mount {
                            *turn = wrap(*turn + by);
                        }
                    }),
                    _ => {}
                }
            }
            if let Pick::Roof(id) = pick {
                p.roofs.iter_mut().filter(|r| r.id == id).for_each(|r| r.turn = wrap(r.turn + by));
            }
        });
    }

    /// Move the selection by `d` metres.
    pub fn nudge_selected(&mut self, d: P2) {
        let Some(pick) = self.selected else { return };
        self.checkpoint("nudge");
        let before = self.project.clone();
        translate(&mut self.project, &before, pick, d);
        self.touch();
    }

    // -----------------------------------------------------------------------------------------
    // Options: versions of the house kept side by side in one project

    /// After the house under the editor changed wholesale: keep the same storey in view.
    fn after_option_change(&mut self, storey: usize) {
        self.selected = None;
        self.editing = None;
        self.linking = None;
        self.level = self.project.levels.get(storey).or(self.project.levels.last()).map(|l| l.id).unwrap_or(0);
        if self.compare.as_deref() == Some(self.project.option.as_str()) {
            self.set_compare(None);
        }
        self.lux.levels = None;
        self.load_underlays();
    }

    /// Chooses the option to compare against: drawn in orange under the plan, and standing
    /// beside this one in 3D.
    pub fn set_compare(&mut self, name: Option<String>) {
        self.compare = name;
        if self.view != ViewMode::Plan {
            self.rebuild_scene();
            if self.view == ViewMode::Model {
                self.camera.frame(self.scene.bounds);
            }
        }
    }

    fn storey(&self) -> usize {
        self.project.levels.iter().position(|l| l.id == self.level).unwrap_or(0)
    }

    /// Works on another option.
    pub fn switch_option(&mut self, index: usize) {
        let storey = self.storey();
        self.edit("", |p| p.switch_option(index));
        self.after_option_change(storey);
    }

    /// Copies the house as it stands into a new option and works on that.
    pub fn branch_option(&mut self) {
        let storey = self.storey();
        let taken: Vec<String> = self.project.option_names().iter().map(|n| n.to_string()).collect();
        let name = (1..).map(|n| format!("Option {n}")).find(|n| !taken.contains(n)).unwrap_or_default();
        // Show what it was branched from underneath, so changes stand out.
        let from = self.project.option.clone();
        self.edit("", |p| p.branch_option(&name));
        self.after_option_change(storey);
        self.set_compare(Some(from));
        self.status = format!("“{name}” starts as a copy. Change it freely: the other options are untouched.");
    }

    pub fn remove_option(&mut self) {
        let storey = self.storey();
        self.edit("", |p| p.remove_option());
        self.after_option_change(storey);
    }

    // -----------------------------------------------------------------------------------------
    // Levels

    /// Adds a level above the top one (or below the bottom one) and makes it active.
    pub fn add_level(&mut self, above: bool) {
        let mut made = 0;
        self.edit("", |p| {
            let id = p.id();
            made = id;
            let n = p.levels.len();
            if above {
                let (elevation, height) = p.levels.last().map(|l| (l.elevation + l.height + 0.3, l.height)).unwrap_or((0.0, 2.7));
                p.levels.push(Level::new(id, &format!("Level {}", n + 1), elevation, height));
            } else {
                let elevation = p.levels.first().map(|l| l.elevation - l.slab - 2.5).unwrap_or(0.0);
                p.levels.insert(0, Level::new(id, "Lower level", elevation, 2.5));
            }
        });
        self.level = made;
        self.selected = None;
    }

    pub fn remove_level(&mut self, id: Id) {
        if self.project.levels.len() < 2 {
            return;
        }
        self.edit("", |p| p.levels.retain(|l| l.id != id));
        self.selected = None;
        if self.project.level(self.level).is_none() {
            self.level = self.project.levels.last().map(|l| l.id).unwrap_or(0);
        }
    }

    // -----------------------------------------------------------------------------------------
    // Files

    pub fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "unsaved".into())
    }

    pub fn save(&mut self) {
        let path = self.path.clone().unwrap_or_else(|| {
            let slug: String = self.project.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
            PathBuf::from(format!("{}.lintel.json", slug.trim_matches('-')))
        });
        self.save_to(path);
    }

    pub fn save_to(&mut self, path: PathBuf) {
        match crate::io::save(&self.project, &path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.status = format!("Couldn't save {}: {e}", path.display()),
        }
    }

    pub fn open(&mut self, path: PathBuf) {
        match crate::io::load(&path) {
            Ok(project) => self.replace_project(project, Some(path)),
            Err(e) => self.status = format!("Couldn't open {}: {e}", path.display()),
        }
    }

    /// Swap in another project, keeping window state (theme, view).
    pub fn replace_project(&mut self, project: Project, path: Option<PathBuf>) {
        let (view, dark, canvas) = (self.view, self.dark, self.canvas);
        let status = path.as_ref().map(|p| format!("Opened {}", p.display())).unwrap_or_default();
        let mut next = State::new(project, path);
        next.images = self.images.take();
        next.lux.request = std::mem::take(&mut self.lux.request);
        next.geo.request = std::mem::take(&mut self.geo.request);
        next.load_underlays();
        next.dark = dark;
        next.canvas = canvas;
        next.fit_plan();
        next.set_view(view);
        next.aim_elevation(true);
        next.status = status;
        *self = next;
    }
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let a = a % t;
    if a > std::f32::consts::PI {
        a - t
    } else if a < -std::f32::consts::PI {
        a + t
    } else {
        a
    }
}

/// Sets `pick`'s position in `project` to where it was in `before`, moved by `d`.
pub fn translate(project: &mut Project, before: &Project, pick: Pick, d: P2) {
    let mv = |p: P2| geom::add(p, d);
    match pick {
        Pick::Roof(id) => {
            if let (Some(r), Some(o)) = (project.roofs.iter_mut().find(|r| r.id == id), before.roofs.iter().find(|r| r.id == id)) {
                r.center = mv(o.center);
            }
        }
        Pick::Patch(id) => {
            if let (Some(r), Some(o)) = (project.site.patches.iter_mut().find(|r| r.id == id), before.site.patches.iter().find(|r| r.id == id)) {
                r.outline = o.outline.iter().map(|p| mv(*p)).collect();
            }
        }
        Pick::Barrier(id) => {
            if let (Some(r), Some(o)) = (project.barrier_mut(id), before.barrier(id)) {
                r.path = o.path.iter().map(|p| mv(*p)).collect();
            }
        }
        Pick::Plant(id) => {
            if let (Some(r), Some(o)) = (project.plant_mut(id), before.plant(id)) {
                r.at = mv(o.at);
            }
        }
        _ => {}
    }
    for (l, lo) in project.levels.iter_mut().zip(&before.levels) {
        match pick {
            Pick::Wall(id) => {
                let Some(o) = lo.walls.iter().find(|w| w.id == id) else { continue };
                // Walls that met this one's ends follow, so corners stay closed.
                let ends = [o.a, o.b];
                for (w, wo) in l.walls.iter_mut().zip(&lo.walls) {
                    if wo.id == id {
                        w.a = mv(wo.a);
                        w.b = mv(wo.b);
                        continue;
                    }
                    if ends.iter().any(|e| geom::dist(*e, wo.a) < 0.02) {
                        w.a = mv(wo.a);
                    }
                    if ends.iter().any(|e| geom::dist(*e, wo.b) < 0.02) {
                        w.b = mv(wo.b);
                    }
                }
            }
            Pick::Room(id) => {
                if let (Some(r), Some(o)) = (l.rooms.iter_mut().find(|r| r.id == id), lo.rooms.iter().find(|r| r.id == id)) {
                    r.outline = o.outline.iter().map(|p| mv(*p)).collect();
                }
            }
            Pick::Stair(id) => {
                if let (Some(r), Some(o)) = (l.stairs.iter_mut().find(|r| r.id == id), lo.stairs.iter().find(|r| r.id == id)) {
                    r.at = mv(o.at);
                }
            }
            Pick::Furniture(id) => {
                if let (Some(r), Some(o)) = (l.furniture.iter_mut().find(|r| r.id == id), lo.furniture.iter().find(|r| r.id == id)) {
                    r.at = mv(o.at);
                }
            }
            Pick::Fixture(id) => {
                if let (Some(r), Some(o)) = (l.fixtures.iter_mut().find(|r| r.id == id), lo.fixtures.iter().find(|r| r.id == id)) {
                    r.mount = match o.mount {
                        Mount::Ceiling { at, turn } => Mount::Ceiling { at: mv(at), turn },
                        Mount::Floor { at, turn } => Mount::Floor { at: mv(at), turn },
                        wall => wall,
                    };
                }
            }
            _ => {}
        }
    }
}

impl Host for State {
    fn capture_wanted(&self) -> bool {
        self.capture.is_some()
    }

    fn captured(&mut self, image: Result<(u32, u32, Vec<u8>), String>) {
        let Some(path) = self.capture.take() else { return };
        self.status = match image.and_then(|(w, h, rgba)| crate::export::png(&path, w, h, &rgba).map(|()| (w, h))) {
            Ok((w, h)) => format!("Saved {} ({w} × {h} pixels).", path.display()),
            Err(e) => format!("Couldn't save the picture: {e}"),
        };
    }

    fn viewport(&self) -> Option<Viewport> {
        let camera = match self.view {
            ViewMode::Plan => return None,
            ViewMode::Elevation => self.elevation,
            ViewMode::Model => self.camera,
        };
        Some(Viewport { scene: self.scene.clone(), revision: self.scene_revision, camera, sun: self.sun(), selected: self.selected, hovered: self.hovered })
    }
}
