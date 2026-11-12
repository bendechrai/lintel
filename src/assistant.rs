//! The measuring assistant: a conversation with Claude that builds the house as you read
//! measurements out. It asks for one thing at a time, and each thing it does is a tool call
//! applied to the project as an ordinary, undoable edit.
//!
//! It uses your own Anthropic API key, which stays in memory for the session (or comes from
//! `ANTHROPIC_API_KEY`). Requests go straight from this computer to api.anthropic.com; what is
//! sent is the conversation and, when the assistant asks for it, a summary of the house.
//!
//! Rust has no official Anthropic SDK, so this speaks the Messages API over HTTP.

use serde_json::{Value, json};

use crate::geom;
use crate::model::*;

pub const MODEL: &str = "claude-opus-5-5";
#[cfg(not(target_arch = "wasm32"))]
const API_VERSION: &str = "2026-10-21";
/// Lets the API hand a request another model declines to one that will take it.
#[cfg(not(target_arch = "wasm32"))]
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

const SYSTEM: &str = "You are the measuring assistant inside Lintel, an app for modelling a home. The person is \
walking round their house with a tape measure, reading measurements out, often by voice, so what you get may be \
terse, out of order, or mis-transcribed.\n\n\
Your job is to turn what they say into the model, using the tools. Work on one room or wall at a time. Ask for one \
measurement at a time, in plain words, saying exactly which wall or opening you mean and from which corner to \
measure. When a figure is ambiguous or implausible (a 30 m bedroom, a door wider than its wall), ask before using \
it. After you change the model, say in one short sentence what you did, then ask for the next measurement.\n\n\
The model: coordinates are metres, X to the east (right on the plan) and Y to the north (up). A wall runs from one \
point to another along its centre line and has a thickness. Openings (doors, windows) sit in a wall, placed by the \
distance from the wall's first end to the opening's centre. Rooms are floor outlines, listed corner by corner. \
Outlets and switches sit on a wall face at a height above the floor; lights sit on the ceiling. \
Convert whatever units the person uses into metres yourself. Call get_house before your first change and whenever \
you are unsure what exists; ids come from there and from the results of your own tool calls.\n\n\
Keep replies short enough to be listened to: one or two sentences, no lists, no markdown.";

/// A line of the conversation as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub from_you: bool,
    pub text: String,
}

/// The tools the assistant has, as the API takes them.
pub fn tools() -> Value {
    let point = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] in metres" });
    json!([
        {
            "name": "get_house",
            "description": "Describes the house as it stands: the levels, and on the level being worked on every wall (id, ends, length, thickness, openings), room (id, name, outline, area) and fixture (id, kind, where). Call this before making changes and whenever you need an id.",
            "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "add_wall",
            "description": "Adds a straight wall between two points on the level being worked on. Returns its id.",
            "input_schema": { "type": "object", "properties": { "from": point, "to": point, "thickness": { "type": "number", "description": "metres; 0.115 for an interior partition, 0.2 for an outside wall" } }, "required": ["from", "to"], "additionalProperties": false }
        },
        {
            "name": "update_wall",
            "description": "Changes a wall: either end, its length (the first end stays put), its thickness, or its height.",
            "input_schema": { "type": "object", "properties": { "wall_id": { "type": "integer" }, "from": point, "to": point, "length": { "type": "number" }, "thickness": { "type": "number" }, "height": { "type": "number" } }, "required": ["wall_id"], "additionalProperties": false }
        },
        {
            "name": "add_opening",
            "description": "Puts a door, window, plain opening or garage door in a wall. `at` is the distance in metres from the wall's first end to the centre of the opening. Returns its id.",
            "input_schema": { "type": "object", "properties": {
                "wall_id": { "type": "integer" },
                "kind": { "type": "string", "enum": ["door", "window", "opening", "garage_door"] },
                "at": { "type": "number" },
                "width": { "type": "number" },
                "height": { "type": "number" },
                "sill": { "type": "number", "description": "height of the bottom of the opening above the floor; 0 for a door" }
            }, "required": ["wall_id", "kind", "at", "width"], "additionalProperties": false }
        },
        {
            "name": "update_opening",
            "description": "Moves or resizes an opening in a wall.",
            "input_schema": { "type": "object", "properties": { "wall_id": { "type": "integer" }, "opening_id": { "type": "integer" }, "at": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" }, "sill": { "type": "number" } }, "required": ["wall_id", "opening_id"], "additionalProperties": false }
        },
        {
            "name": "add_room",
            "description": "Adds a room's floor: its outline corner by corner (at least three), and its name. Returns its id.",
            "input_schema": { "type": "object", "properties": { "name": { "type": "string" }, "outline": { "type": "array", "items": point, "minItems": 3 }, "ceiling_height": { "type": "number" } }, "required": ["name", "outline"], "additionalProperties": false }
        },
        {
            "name": "update_room",
            "description": "Renames a room, changes its outline, or sets its ceiling height.",
            "input_schema": { "type": "object", "properties": { "room_id": { "type": "integer" }, "name": { "type": "string" }, "outline": { "type": "array", "items": point, "minItems": 3 }, "ceiling_height": { "type": "number" } }, "required": ["room_id"], "additionalProperties": false }
        },
        {
            "name": "add_fixture",
            "description": "Places an outlet, switch, light or other fixture. Outlets, data outlets, switches and thermostats go on a wall: give a point about 0.1 m off the wall face, on the side of the room they are in, and it snaps to that face. Lights, smoke detectors and ceiling vents go on the ceiling at the point given. Returns its id and where it landed.",
            "input_schema": { "type": "object", "properties": {
                "kind": { "type": "string", "enum": ["outlet", "data_outlet", "switch", "thermostat", "smoke_detector", "downlight", "pendant", "surface_light", "tube_light", "vent"] },
                "position": point,
                "height": { "type": "number", "description": "for wall fixtures: metres from the floor to its centre. Defaults: outlets 0.3, switches 1.1, thermostats 1.5" },
                "label": { "type": "string", "description": "what it is for, such as 'fridge' or 'hall lights'" }
            }, "required": ["kind", "position"], "additionalProperties": false }
        },
        {
            "name": "remove",
            "description": "Removes a wall, an opening, a room or a fixture. For an opening give its wall's id as well.",
            "input_schema": { "type": "object", "properties": { "what": { "type": "string", "enum": ["wall", "opening", "room", "fixture"] }, "id": { "type": "integer" }, "wall_id": { "type": "integer" } }, "required": ["what", "id"], "additionalProperties": false }
        },
        {
            "name": "set_level",
            "description": "Chooses the level to work on by name, or sets the height of the one being worked on. Levels are listed by get_house.",
            "input_schema": { "type": "object", "properties": { "name": { "type": "string" }, "ceiling_height": { "type": "number" }, "floor_elevation": { "type": "number" } }, "additionalProperties": false }
        }
    ])
}

/// The request for the next turn of the conversation.
pub fn request(messages: &[Value]) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": 16000,
        // Reading a tape measure back is not deep work; keep the turns quick.
        "output_config": { "effort": "low" },
        "fallbacks": "default",
        "system": [{ "type": "text", "text": SYSTEM, "cache_control": { "type": "ephemeral" } }],
        "tools": tools(),
        "messages": messages,
    })
}

fn round(v: f32) -> f64 {
    (v as f64 * 1000.0).round() / 1000.0
}

fn pt(p: P2) -> Value {
    json!([round(p[0]), round(p[1])])
}

/// What `get_house` tells the assistant.
pub fn describe(project: &Project, level: Id) -> Value {
    let Some(l) = project.level(level) else { return json!({ "error": "there is no level to work on" }) };
    json!({
        "units_the_person_reads": if project.units == Units::Metric { "metric" } else { "feet and inches" },
        "levels": project.levels.iter().map(|x| json!({ "name": x.name, "floor_elevation": round(x.elevation), "ceiling_height": round(x.height), "working_on": x.id == level })).collect::<Vec<_>>(),
        "walls": l.walls.iter().map(|w| json!({
            "id": w.id, "from": pt(w.a), "to": pt(w.b), "length": round(w.length()), "thickness": round(w.thickness),
            "openings": w.openings.iter().map(|o| json!({
                "id": o.id,
                "kind": match o.kind { OpeningKind::Door(_) => "door", OpeningKind::Window(_) => "window", OpeningKind::Garage(_) => "garage_door", OpeningKind::Passage { .. } => "opening", OpeningKind::Recess { .. } => "recess" },
                "at": round(o.at), "width": round(o.width), "height": round(o.height), "sill": round(o.sill),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "fixtures": l.fixtures.iter().map(|f| {
            let mut v = json!({ "id": f.id, "kind": f.kind.name() });
            match f.mount {
                Mount::Wall { wall, at, height, .. } => {
                    v["wall_id"] = json!(wall);
                    v["at"] = json!(round(at));
                    v["height"] = json!(round(height));
                }
                Mount::Ceiling { at, .. } => v["on_ceiling_at"] = pt(at),
                Mount::Floor { at, .. } => v["on_floor_at"] = pt(at),
            }
            if !f.label.is_empty() {
                v["label"] = json!(f.label);
            }
            v
        }).collect::<Vec<_>>(),
        "rooms": l.rooms.iter().map(|r| json!({ "id": r.id, "name": r.name, "outline": r.outline.iter().map(|p| pt(*p)).collect::<Vec<_>>(), "area": round(geom::area(&r.outline).abs()) })).collect::<Vec<_>>(),
    })
}

fn number(input: &Value, key: &str) -> Option<f32> {
    input.get(key).and_then(Value::as_f64).map(|v| v as f32).filter(|v| v.is_finite())
}

fn point(input: &Value, key: &str) -> Option<P2> {
    let a = input.get(key)?.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32]).filter(|p| p[0].is_finite() && p[1].is_finite() && p[0].abs() < 1e4 && p[1].abs() < 1e4)
}

fn id(input: &Value, key: &str) -> Option<Id> {
    input.get(key).and_then(Value::as_u64)
}

fn outline(input: &Value) -> Option<Vec<P2>> {
    let pts: Option<Vec<P2>> = input.get("outline")?.as_array()?.iter().map(|p| {
        let a = p.as_array()?;
        Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32])
    }).collect();
    pts.filter(|p| p.len() >= 3 && p.iter().all(|q| q[0].is_finite() && q[1].is_finite()))
}

/// Carries out one tool call on the project. `level` is the level being worked on, which
/// `set_level` can change. Returns what to tell the assistant, or what went wrong.
pub fn apply(project: &mut Project, level: &mut Id, name: &str, input: &Value) -> Result<Value, String> {
    let missing = |what: &str| format!("`{what}` is missing or not a usable value");
    match name {
        "get_house" => Ok(describe(project, *level)),
        "add_wall" => {
            let (a, b) = (point(input, "from").ok_or(missing("from"))?, point(input, "to").ok_or(missing("to"))?);
            if geom::dist(a, b) < 0.05 {
                return Err("the two ends are the same point".into());
            }
            let thickness = number(input, "thickness").unwrap_or(0.115).clamp(0.02, 1.5);
            let new = project.id();
            let paint = Finish::paint(0xf2efe8);
            let l = project.level_mut(*level).ok_or("there is no level to work on")?;
            l.walls.push(Wall { id: new, a, b, thickness, height: None, base: 0.0, left: paint.clone(), right: paint, openings: vec![], bands: vec![], layers: vec![] });
            Ok(json!({ "wall_id": new, "length": round(geom::dist(a, b)) }))
        }
        "update_wall" => {
            let wall = id(input, "wall_id").ok_or(missing("wall_id"))?;
            let w = project.wall_mut(wall).ok_or(format!("there is no wall {wall}"))?;
            if let Some(p) = point(input, "from") {
                w.a = p;
            }
            if let Some(p) = point(input, "to") {
                w.b = p;
            }
            if let Some(len) = number(input, "length") {
                if len < 0.05 {
                    return Err("a wall needs some length".into());
                }
                let (d, _) = w.frame();
                w.b = geom::add(w.a, geom::scale(d, len));
            }
            if let Some(t) = number(input, "thickness") {
                w.layers.clear();
                w.thickness = t.clamp(0.02, 1.5);
            }
            if let Some(h) = number(input, "height") {
                w.height = Some(h.clamp(0.05, 20.0));
            }
            Ok(json!({ "wall_id": wall, "from": pt(w.a), "to": pt(w.b), "length": round(w.length()), "thickness": round(w.thickness) }))
        }
        "add_opening" => {
            let wall = id(input, "wall_id").ok_or(missing("wall_id"))?;
            let kind = input.get("kind").and_then(Value::as_str).ok_or(missing("kind"))?;
            let (at, width) = (number(input, "at").ok_or(missing("at"))?, number(input, "width").ok_or(missing("width"))?);
            let new = project.id();
            let w = project.wall_mut(wall).ok_or(format!("there is no wall {wall}"))?;
            let len = w.length();
            if width <= 0.05 || width > len {
                return Err(format!("the opening is {width} m wide and the wall is {:.3} m long", len));
            }
            if at - width / 2.0 < -0.001 || at + width / 2.0 > len + 0.001 {
                return Err(format!("an opening {width} m wide centred {at} m along doesn't fit in a wall {:.3} m long", len));
            }
            let trim = Finish::paint(0xf7f5f0);
            let (height, sill, kind) = match kind {
                "door" => (2.04, 0.0, OpeningKind::Door(Door { style: DoorStyle::Hinged, hinge: Hinge::Start, swing: WallSide::Left, open: 0.0, leaf: trim, frame: Some(Frame::default()), glazing: 0.0 })),
                "window" => (1.2, 0.9, OpeningKind::Window(Window { casing: Some(Frame::default()), ..Window::new(WindowStyle::Casement) })),
                "opening" => (2.1, 0.0, OpeningKind::Passage { frame: Some(Frame::default()) }),
                "garage_door" => (2.13, 0.0, OpeningKind::Garage(GarageDoor { style: GarageStyle::Sectional, inside: WallSide::Left, open: 0.0, panels: 4, finish: Finish::paint(0xeceae4) })),
                other => return Err(format!("`{other}` isn't a kind of opening: use door, window, opening or garage_door")),
            };
            let height = number(input, "height").unwrap_or(height).clamp(0.1, 10.0);
            let sill = number(input, "sill").unwrap_or(sill).clamp(0.0, 10.0);
            w.openings.push(Opening { id: new, at, width, height, sill, kind });
            Ok(json!({ "opening_id": new, "wall_id": wall }))
        }
        "update_opening" => {
            let (wall, opening) = (id(input, "wall_id").ok_or(missing("wall_id"))?, id(input, "opening_id").ok_or(missing("opening_id"))?);
            let len = project.wall(wall).ok_or(format!("there is no wall {wall}"))?.length();
            let o = project.opening_mut(wall, opening).ok_or(format!("wall {wall} has no opening {opening}"))?;
            let width = number(input, "width").unwrap_or(o.width);
            let at = number(input, "at").unwrap_or(o.at);
            if width <= 0.05 || at - width / 2.0 < -0.001 || at + width / 2.0 > len + 0.001 {
                return Err(format!("an opening {width} m wide centred {at} m along doesn't fit in a wall {len:.3} m long"));
            }
            o.width = width;
            o.at = at;
            if let Some(h) = number(input, "height") {
                o.height = h.clamp(0.1, 10.0);
            }
            if let Some(s) = number(input, "sill") {
                o.sill = s.clamp(0.0, 10.0);
            }
            Ok(json!({ "opening_id": opening, "at": round(o.at), "width": round(o.width), "height": round(o.height), "sill": round(o.sill) }))
        }
        "add_room" => {
            let outline = outline(input).ok_or(missing("outline"))?;
            let name = input.get("name").and_then(Value::as_str).unwrap_or("Room").to_string();
            let new = project.id();
            let area = geom::area(&outline).abs();
            if area < 0.05 {
                return Err("those corners enclose no area".into());
            }
            let l = project.level_mut(*level).ok_or("there is no level to work on")?;
            l.rooms.push(Room {
                id: new,
                name,
                outline,
                floor: Finish::of("oak_boards"),
                ceiling: Finish::paint(0xfafaf7),
                ceiling_height: number(input, "ceiling_height").map(|h| h.clamp(0.5, 20.0)),
                floor_offset: 0.0,
                has_ceiling: true,
                balustrade: None,
                ceiling_shape: CeilingShape::Flat,
                skirting: None,
                cornice: None,
                target_lux: None,
            });
            Ok(json!({ "room_id": new, "area": round(area) }))
        }
        "update_room" => {
            let room = id(input, "room_id").ok_or(missing("room_id"))?;
            let new_outline = if input.get("outline").is_some() { Some(outline(input).ok_or(missing("outline"))?) } else { None };
            let r = project.room_mut(room).ok_or(format!("there is no room {room}"))?;
            if let Some(n) = input.get("name").and_then(Value::as_str) {
                r.name = n.to_string();
            }
            if let Some(o) = new_outline {
                r.outline = o;
            }
            if let Some(h) = number(input, "ceiling_height") {
                r.ceiling_height = Some(h.clamp(0.5, 20.0));
            }
            Ok(json!({ "room_id": room, "name": r.name, "area": round(geom::area(&r.outline).abs()) }))
        }
        "add_fixture" => {
            let kind = match input.get("kind").and_then(Value::as_str).ok_or(missing("kind"))? {
                "outlet" => FixtureKind::Outlet { gangs: 2 },
                "data_outlet" => FixtureKind::Data,
                "switch" => FixtureKind::Switch { gangs: 1 },
                "thermostat" => FixtureKind::Thermostat,
                "smoke_detector" => FixtureKind::SmokeDetector,
                "downlight" => FixtureKind::Downlight { diameter: 0.1 },
                "pendant" => FixtureKind::Pendant { diameter: 0.3, drop: 0.8 },
                "surface_light" => FixtureKind::Surface { diameter: 0.3 },
                "tube_light" => FixtureKind::TubeLight { length: 1.2, width: 0.09 },
                "vent" => FixtureKind::Vent { width: 0.6, height: 0.4 },
                other => return Err(format!("`{other}` isn't a kind of fixture")),
            };
            let at = point(input, "position").ok_or(missing("position"))?;
            let new = project.id();
            let l = project.level_mut(*level).ok_or("there is no level to work on")?;
            let mut mount = crate::edit::default_mount(l, kind, at).ok_or("that goes on a wall, and there is no wall within 0.8 m of that point")?;
            if let (Mount::Wall { height, .. }, Some(h)) = (&mut mount, number(input, "height")) {
                *height = h.clamp(0.0, 10.0);
            }
            let mut fixture = Fixture::new(new, kind, mount);
            fixture.label = input.get("label").and_then(Value::as_str).unwrap_or_default().to_string();
            l.fixtures.push(fixture);
            Ok(match mount {
                Mount::Wall { wall, at, height, .. } => json!({ "fixture_id": new, "wall_id": wall, "at": round(at), "height": round(height) }),
                Mount::Ceiling { at, .. } | Mount::Floor { at, .. } => json!({ "fixture_id": new, "at": pt(at) }),
            })
        }
        "remove" => {
            let what = input.get("what").and_then(Value::as_str).ok_or(missing("what"))?;
            let target = id(input, "id").ok_or(missing("id"))?;
            let found = match what {
                "wall" => project.levels.iter_mut().any(|l| {
                    let n = l.walls.len();
                    l.walls.retain(|w| w.id != target);
                    l.fixtures.retain(|f| !matches!(f.mount, Mount::Wall { wall, .. } if wall == target));
                    l.walls.len() < n
                }),
                "room" => project.levels.iter_mut().any(|l| {
                    let n = l.rooms.len();
                    l.rooms.retain(|r| r.id != target);
                    l.rooms.len() < n
                }),
                "fixture" => project.levels.iter_mut().any(|l| {
                    let n = l.fixtures.len();
                    l.fixtures.retain(|f| f.id != target);
                    // A switch no longer controls a light that is gone.
                    l.fixtures.iter_mut().for_each(|f| f.controls.retain(|c| *c != target));
                    l.fixtures.len() < n
                }),
                "opening" => {
                    let wall = id(input, "wall_id").ok_or(missing("wall_id"))?;
                    let w = project.wall_mut(wall).ok_or(format!("there is no wall {wall}"))?;
                    let n = w.openings.len();
                    w.openings.retain(|o| o.id != target);
                    w.openings.len() < n
                }
                other => return Err(format!("`{other}` can't be removed here: use wall, opening, room or fixture")),
            };
            if found { Ok(json!({ "removed": what, "id": target })) } else { Err(format!("there is no {what} {target}")) }
        }
        "set_level" => {
            if let Some(name) = input.get("name").and_then(Value::as_str) {
                let found = project.levels.iter().find(|l| l.name.eq_ignore_ascii_case(name)).map(|l| l.id);
                *level = found.ok_or_else(|| format!("there is no level called {name}: the levels are {}", project.levels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")))?;
            }
            let l = project.level_mut(*level).ok_or("there is no level to work on")?;
            if let Some(h) = number(input, "ceiling_height") {
                l.height = h.clamp(0.5, 20.0);
            }
            if let Some(e) = number(input, "floor_elevation") {
                l.elevation = e;
            }
            Ok(json!({ "working_on": l.name, "ceiling_height": round(l.height), "floor_elevation": round(l.elevation) }))
        }
        other => Err(format!("there is no tool called {other}")),
    }
}

/// What a response from the API asks for.
#[derive(Debug, PartialEq)]
pub enum Turn {
    /// The assistant has finished speaking: the text to show.
    Reply(String),
    /// It wants tools run: the text so far, and each call as (id, name, input).
    Tools(String, Vec<(String, String, Value)>),
    /// The request was declined, or cut short.
    Stopped(String),
}

/// Reads a Messages API response.
pub fn read(response: &Value) -> Turn {
    let blocks = response["content"].as_array().cloned().unwrap_or_default();
    let text: String = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ");
    match response["stop_reason"].as_str() {
        Some("tool_use") => {
            let calls = blocks.iter().filter(|b| b["type"] == "tool_use").map(|b| (b["id"].as_str().unwrap_or_default().to_string(), b["name"].as_str().unwrap_or_default().to_string(), b["input"].clone())).collect();
            Turn::Tools(text, calls)
        }
        Some("refusal") => Turn::Stopped("The assistant declined that request.".into()),
        Some("max_tokens") => Turn::Stopped(if text.is_empty() { "The reply was cut off before it said anything.".into() } else { text }),
        _ => Turn::Reply(text),
    }
}

/// Sends one request and returns the response body. Blocks; call it off the UI thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn send(key: &str, body: &Value) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(180)).build();
    let sent = agent
        .post("https://api.anthropic.com/v1/messages")
        .set("content-type", "application/json")
        .set("x-api-key", key.trim())
        .set("anthropic-version", API_VERSION)
        .set("anthropic-beta", FALLBACK_BETA)
        .send_string(&body.to_string());
    match sent {
        Ok(response) => serde_json::from_str(&response.into_string().map_err(|e| e.to_string())?).map_err(|e| format!("the reply wasn't readable: {e}")),
        Err(ureq::Error::Status(code, response)) => {
            let detail = response.into_string().ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["error"]["message"].as_str().map(str::to_string)).unwrap_or_default();
            Err(match code {
                401 | 403 => format!("That API key wasn't accepted. {detail}"),
                429 => format!("The API is rate-limiting this key; wait a moment and send again. {detail}"),
                400 | 404 | 413 | 422 => format!("The API rejected the request. {detail}"),
                _ => format!("The API had a problem ({code}); try again shortly. {detail}"),
            })
        }
        Err(e) => Err(format!("Couldn't reach api.anthropic.com: {e}")),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn send(_: &str, _: &Value) -> Result<Value, String> {
    Err("The assistant runs in the desktop app, not in the browser.".into())
}

/// Reads a reply aloud with the computer's own voice, if it has one. Returns false when no
/// speech command could be started.
#[cfg(not(target_arch = "wasm32"))]
pub fn speak(text: &str) -> bool {
    use std::process::{Command, Stdio};
    let quiet = |c: &mut Command| c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok();
    if cfg!(target_os = "macos") {
        quiet(Command::new("say").arg(text))
    } else if cfg!(target_os = "windows") {
        // The text goes in through the environment, never into the command line.
        quiet(Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak($env:LINTEL_SAY)"]).env("LINTEL_SAY", text))
    } else {
        quiet(Command::new("spd-say").arg("--").arg(text)) || quiet(Command::new("espeak").arg("--").arg(text))
    }
}

#[cfg(target_arch = "wasm32")]
pub fn speak(_: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> (Project, Id) {
        let p = crate::sample::blank();
        let level = p.levels[0].id;
        (p, level)
    }

    #[test]
    fn the_request_is_the_messages_api_shape_with_every_tool_closed_to_extra_fields() {
        let body = request(&[json!({ "role": "user", "content": "The living room is 5 by 4 metres." })]);
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        assert!(body.get("thinking").is_none(), "thinking is left to the model");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        let tools = body["tools"].as_array().unwrap();
        assert!(tools.len() >= 8);
        for t in tools {
            assert_eq!(t["input_schema"]["type"], "object", "{}", t["name"]);
            assert_eq!(t["input_schema"]["additionalProperties"], false, "{}", t["name"]);
            assert!(t["description"].as_str().unwrap().len() > 30);
        }
    }

    #[test]
    fn tool_calls_build_a_room_wall_by_wall_and_refuse_what_cannot_be() {
        let (mut p, mut level) = blank();
        let wall = apply(&mut p, &mut level, "add_wall", &json!({ "from": [0, 0], "to": [5, 0], "thickness": 0.2 })).unwrap();
        let id = wall["wall_id"].as_u64().unwrap();
        assert_eq!(wall["length"], 5.0);
        // A door 0.82 wide, centred 1.2 m along.
        let door = apply(&mut p, &mut level, "add_opening", &json!({ "wall_id": id, "kind": "door", "at": 1.2, "width": 0.82 })).unwrap();
        let o = p.opening(id, door["opening_id"].as_u64().unwrap()).unwrap();
        assert!(matches!(o.kind, OpeningKind::Door(_)) && o.sill == 0.0 && (o.height - 2.04).abs() < 1e-6);
        // One that doesn't fit is refused, with the reason, and nothing changes.
        let err = apply(&mut p, &mut level, "add_opening", &json!({ "wall_id": id, "kind": "window", "at": 4.8, "width": 1.2 })).unwrap_err();
        assert!(err.contains("doesn't fit"), "{err}");
        assert_eq!(p.wall(id).unwrap().openings.len(), 1);
        // The tape said 5.2, not 5: the first end stays put.
        apply(&mut p, &mut level, "update_wall", &json!({ "wall_id": id, "length": 5.2 })).unwrap();
        assert_eq!((p.wall(id).unwrap().a, p.wall(id).unwrap().b), ([0.0, 0.0], [5.2, 0.0]));
        let room = apply(&mut p, &mut level, "add_room", &json!({ "name": "Living room", "outline": [[0, 0], [5.2, 0], [5.2, 4], [0, 4]] })).unwrap();
        assert_eq!(room["area"], 20.8);
        // The description lists what was built, with ids to refer to.
        let house = apply(&mut p, &mut level, "get_house", &json!({})).unwrap();
        assert_eq!(house["walls"][0]["openings"][0]["kind"], "door");
        assert_eq!(house["rooms"][0]["name"], "Living room");
        // An outlet 1 m along the south wall, on the room side, at the usual height; a light overhead.
        let outlet = apply(&mut p, &mut level, "add_fixture", &json!({ "kind": "outlet", "position": [1.0, 0.2], "label": "TV" })).unwrap();
        assert_eq!((outlet["wall_id"].as_u64(), outlet["at"].as_f64(), outlet["height"].as_f64()), (Some(id), Some(1.0), Some(0.3)));
        let light = apply(&mut p, &mut level, "add_fixture", &json!({ "kind": "downlight", "position": [2.6, 2.0] })).unwrap();
        assert!(apply(&mut p, &mut level, "add_fixture", &json!({ "kind": "switch", "position": [2.6, 2.0] })).unwrap_err().contains("no wall"));
        let house = apply(&mut p, &mut level, "get_house", &json!({})).unwrap();
        assert_eq!((house["fixtures"].as_array().unwrap().len(), house["fixtures"][0]["label"].as_str()), (2, Some("TV")));
        apply(&mut p, &mut level, "remove", &json!({ "what": "fixture", "id": light["fixture_id"] })).unwrap();
        assert_eq!(p.levels[0].fixtures.len(), 1);
        // Bad input is an error to hand back, not a panic.
        assert!(apply(&mut p, &mut level, "add_wall", &json!({ "from": [0, 0] })).is_err());
        assert!(apply(&mut p, &mut level, "update_wall", &json!({ "wall_id": 9999, "length": 3 })).is_err());
        assert!(apply(&mut p, &mut level, "launch", &json!({})).is_err());
        apply(&mut p, &mut level, "remove", &json!({ "what": "wall", "id": id })).unwrap();
        assert!(p.wall(id).is_none());
    }

    #[test]
    fn responses_are_read_as_a_reply_a_set_of_tool_calls_or_a_stop() {
        let reply = json!({ "stop_reason": "end_turn", "content": [{ "type": "thinking", "thinking": "" }, { "type": "text", "text": "How long is the north wall?" }] });
        assert_eq!(read(&reply), Turn::Reply("How long is the north wall?".into()));
        let tools = json!({ "stop_reason": "tool_use", "content": [
            { "type": "text", "text": "Adding that wall." },
            { "type": "tool_use", "id": "toolu_1", "name": "add_wall", "input": { "from": [0, 0], "to": [3, 0] } },
            { "type": "tool_use", "id": "toolu_2", "name": "get_house", "input": {} }
        ] });
        let Turn::Tools(text, calls) = read(&tools) else { panic!("tool calls") };
        assert_eq!((text.as_str(), calls.len(), calls[0].1.as_str(), calls[1].0.as_str()), ("Adding that wall.", 2, "add_wall", "toolu_2"));
        assert!(matches!(read(&json!({ "stop_reason": "refusal", "content": [] })), Turn::Stopped(_)));
    }
}
