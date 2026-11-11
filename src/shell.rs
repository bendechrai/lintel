//! The app's entry point, shared by the desktop binary and the browser build.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use crate::app::{State, Tool, ViewMode};
use crate::gfx::Gfx;
use crate::model::Pick;
use crate::{sample, ui};
use rune::{AppSpec, Tx};

/// The page's address and fetching, in the browser.
#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;

    use crate::model::Project;

    thread_local! {
        static FETCHED: RefCell<Option<Result<Project, String>>> = const { RefCell::new(None) };
    }

    /// The part of the address after `#`, and the part after `?`.
    pub fn address() -> (String, String) {
        let location = web_sys::window().map(|w| w.location());
        let part = |f: fn(&web_sys::Location) -> Result<String, wasm_bindgen::JsValue>| location.as_ref().and_then(|l| f(l).ok()).unwrap_or_default();
        (part(web_sys::Location::hash), part(web_sys::Location::search))
    }

    async fn get(url: &str) -> Result<Project, String> {
        let window = web_sys::window().ok_or("no window")?;
        let failed = |_| format!("couldn't fetch {url}");
        let response: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url)).await.map_err(failed)?.dyn_into().map_err(failed)?;
        if !response.ok() {
            return Err(format!("{url} answered {}", response.status()));
        }
        let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(failed)?).await.map_err(failed)?;
        serde_json::from_str(&text.as_string().unwrap_or_default()).map_err(|e| format!("{url} isn't a Lintel project: {e}"))
    }

    /// Fetches a project file, then wakes the app to take it.
    pub fn fetch(url: String, waker: rune::runtime::Waker) {
        wasm_bindgen_futures::spawn_local(async move {
            let result = get(&url).await;
            FETCHED.with(|slot| *slot.borrow_mut() = Some(result));
            waker();
        });
    }

    pub fn take() -> Option<Result<Project, String>> {
        FETCHED.with(|slot| slot.borrow_mut().take())
    }

    pub fn set_title(title: &str) {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.set_title(title);
        }
    }
}

/// A project opened from a link starts in 3D: most people following one want to look round.
#[cfg(target_arch = "wasm32")]
fn init() -> State {
    let (fragment, _) = web::address();
    match crate::share::from_fragment(&fragment) {
        Some(Ok(project)) => {
            web::set_title(&format!("{} · Lintel", project.name));
            let mut s = State::new(project, None);
            s.set_view(ViewMode::Model);
            s
        }
        Some(Err(e)) => {
            let mut s = State::new(sample::house(), None);
            s.status = format!("Couldn't open the linked project: {e}. Showing the sample house.");
            s
        }
        None => State::new(sample::house(), None),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn init() -> State {
    // The first argument that isn't one of Rune's tooling flags (or a flag's value) is a project.
    let file = std::env::args().skip(1).find(|a| a.ends_with(".json") && !a.starts_with("--"));
    match file {
        Some(path) => {
            let path = PathBuf::from(path);
            match crate::io::load(&path) {
                Ok(project) => State::new(project, Some(path)),
                Err(e) => {
                    let mut s = State::new(sample::house(), None);
                    s.status = format!("Couldn't open {}: {e}. Showing the sample house.", path.display());
                    s
                }
            }
        }
        None => State::new(sample::house(), None),
    }
}

fn setup(s: &mut State, waker: rune::runtime::Waker) {
    s.geo.request.set_waker(waker.clone());
    s.lux.request.set_waker(waker.clone());
    #[cfg(target_arch = "wasm32")]
    if let Some(url) = crate::share::from_query(&web::address().1) {
        s.status = format!("Fetching {url}…");
        web::fetch(url, waker);
    }
    let _ = waker;
}

/// Lands a fetched terrain.
fn poll(s: &mut State, _: f64, _: &mut Tx) -> bool {
    #[cfg(target_arch = "wasm32")]
    let lit = match web::take() {
        Some(Ok(project)) => {
            web::set_title(&format!("{} · Lintel", project.name));
            s.replace_project(project, None);
            s.set_view(ViewMode::Model);
            true
        }
        Some(Err(e)) => {
            s.status = format!("{e}. Showing the sample house.");
            true
        }
        None => s.land_light(),
    };
    #[cfg(not(target_arch = "wasm32"))]
    let lit = s.land_light();
    let Some(answer) = s.geo.request.take() else { return lit };
    match answer {
        Ok(fetched) => {
            let note = fetched.terrain.source.clone();
            s.edit("", |p| {
                let finish = p.site.terrain.finish.clone();
                p.site.terrain = crate::model::Terrain { finish, ..fetched.terrain };
                p.site.geo = Some(fetched.geo);
            });
            s.geo.status = format!("Ground replaced: {note}.");
        }
        Err(e) => s.geo.status = format!("Couldn't fetch the ground: {e}"),
    }
    true
}

fn first_of(s: &State, f: impl Fn(&crate::model::Level) -> Option<Pick>) -> Option<Pick> {
    s.project.levels.iter().rev().find_map(f)
}

/// Named states for headless snapshots and the docs.
const SCENES: &[(&str, fn(&mut State))] = &[
    ("plan", |_| {}),
    ("basement", |s| s.level = s.project.levels[0].id),
    ("wall", |s| s.selected = first_of(s, |l| l.walls.get(1).map(|w| Pick::Wall(w.id)))),
    ("door", |s| s.selected = first_of(s, |l| l.walls.first().and_then(|w| w.openings.first().map(|o| Pick::Opening(w.id, o.id))))),
    ("room", |s| {
        s.selected = first_of(s, |l| l.rooms.get(2).map(|r| Pick::Room(r.id)));
        s.picker = Some("room-floor".into());
    }),
    ("furniture", |s| s.selected = first_of(s, |l| l.furniture.iter().find(|f| f.kind == "sofa").map(|f| Pick::Furniture(f.id)))),
    ("draw", |s| s.set_tool(Tool::Wall)),
    ("dark", |s| s.dark = true),
    ("elevation", |s| s.set_view(ViewMode::Elevation)),
    ("east", |s| {
        s.side = crate::app::Side::East;
        s.set_view(ViewMode::Elevation);
        s.aim_elevation(true);
    }),
    ("section", |s| {
        s.side = crate::app::Side::East;
        s.section = Some(5.0);
        s.set_view(ViewMode::Elevation);
        s.aim_elevation(true);
    }),
    ("model", |s| s.set_view(ViewMode::Model)),
    ("back", |s| {
        s.set_view(ViewMode::Model);
        s.camera.yaw = 1.9;
        s.camera.pitch = 0.32;
    }),
    ("dollhouse", |s| {
        s.set_view(ViewMode::Model);
        let main = s.project.levels.last().map(|l| l.id);
        s.set_options(|o| o.top_level = main);
        s.camera.pitch = 0.95;
        s.camera.distance *= 0.8;
    }),
    ("garage", |s| {
        s.set_view(ViewMode::Model);
        s.edit("", |p| {
            for o in p.levels.iter_mut().flat_map(|l| &mut l.walls).flat_map(|w| &mut w.openings) {
                if let crate::model::OpeningKind::Garage(g) = &mut o.kind {
                    g.open = 0.55;
                }
            }
        });
        s.camera.target = [3.1, 0.0, 1.2];
        s.camera.yaw = -1.75;
        s.camera.pitch = 0.16;
        s.camera.distance = 13.0;
    }),
    ("night", |s| {
        s.set_view(ViewMode::Model);
        s.edit("", |p| {
            p.site.geo = Some(crate::model::Geo { lat: 40.0, lon: -105.0 });
            p.site.sun = crate::model::SunSetting { by_clock: true, month: 12, day: 21, hour: 21.0, utc_offset: -7.0, ..p.site.sun };
        });
        let main = s.project.levels.last().map(|l| l.id);
        s.set_options(|o| o.top_level = main);
        s.camera.pitch = 0.95;
        s.camera.distance *= 0.8;
    }),
    ("evening", |s| {
        s.set_view(ViewMode::Model);
        s.edit("", |p| {
            p.site.geo = Some(crate::model::Geo { lat: 40.0, lon: -105.0 });
            p.site.sun = crate::model::SunSetting { by_clock: true, month: 6, day: 21, hour: 19.25, utc_offset: -7.0, ..p.site.sun };
        });
        s.camera.yaw = -2.6;
        s.camera.pitch = 0.3;
    }),
    ("switch", |s| {
        s.selected = first_of(s, |l| l.fixtures.iter().find(|f| f.label == "Living room lights").map(|f| Pick::Fixture(f.id)));
    }),
    ("lux", |s| {
        s.lux.levels = Some(crate::lux::compute(&s.project, &s.lux.settings));
        s.lux.show = true;
    }),
    ("lux-night", |s| {
        s.edit("", |p| p.site.sun = crate::model::SunSetting { altitude: -10.0, ..p.site.sun });
        s.lux.levels = Some(crate::lux::compute(&s.project, &s.lux.settings));
        s.lux.show = true;
        s.selected = first_of(s, |l| l.rooms.iter().find(|r| r.name == "Living room").map(|r| Pick::Room(r.id)));
    }),
    ("lux-3d", |s| {
        s.lux.levels = Some(crate::lux::compute(&s.project, &s.lux.settings));
        s.lux.show = true;
        s.set_view(ViewMode::Model);
        let main = s.project.levels.last().map(|l| l.id);
        s.set_options(|o| o.top_level = main);
        s.camera.pitch = 0.95;
        s.camera.distance *= 0.8;
    }),
    ("blank", |s| s.replace_project(sample::blank(), None)),
];

/// Opens the window (or, in a browser, the canvas).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    let spec = AppSpec::<State> { scenes: SCENES, min_size: Some((1280.0, 720.0)), setup: Some(setup), poll: Some(poll), on_key: Some(ui::on_key), ..AppSpec::new("Lintel", (1440.0, 900.0), init, ui::view) };
    rune::App::new(spec).compositor(Gfx::default()).run();
}
