# Lintel

Model your home in plan, elevation and 3D, so you can plan changes before you make them.
Single-family houses, any number of levels, on flat or sloping ground.
Built in Rust with [Rune](https://github.com/anomalyco/rune).

![The main level cut open in 3D](docs/dollhouse.png)

| | |
| --- | --- |
| ![Plan](docs/plan.png) | ![Section through the house](docs/section.png) |
| ![The walk-out basement from the garden](docs/back.png) | ![Picking a floor material and stain](docs/room.png) |

## What it models

- **Levels** with their own floor height, ceiling height and floor thickness. A walk-out
  basement is a level with a negative floor height on ground that falls away.
- **Walls** with thickness, height (full or partial) and a finish on each face.
- **Doors** (hinged, double, sliding, pocket, bifold) with frames, architraves and glazing;
  **garage doors** (sectional, roller, tilt-up) at any width, drawn at any amount open;
  **windows** (fixed, casement, awning, double-hung, sliding, louvre); cased **openings**;
  **recesses**.
- **Rooms**: floor outlines of any shape with their own floor finish, ceiling finish, ceiling
  height and floor offset. Where there is no room there is no floor, which is how a stairwell
  stays open.
- **Stairs**, **roofs** (gable, hip, shed, flat).
- **Fixtures**: power and data outlets, light switches, thermostats, smoke detectors,
  downlights, recessed tube lights, surface and pendant lights, air vents.
- **Furniture and appliances**: a starter catalogue of 43 resizable pieces.
- **Ground**: a height grid you can shape as a plane, or fetch from a public elevation
  service by latitude and longitude, plus draped surfaces (driveway, paths, patio).
- **Materials**: about 55 procedural surfaces. Paints take any colour, timbers keep their own
  colour and take a stain, and stone, brick and concrete are the colour they are.

## Run it

```bash
cargo run --release
```

```bash
cargo run --release -- my-house.lintel.json
```

Linux needs `libxkbcommon-dev libwayland-dev` to build and `libxkbcommon-x11-0` to open a
window. It opens on a sample house; **New** starts empty. Projects are pretty-printed JSON.

| | |
| --- | --- |
| `1` `2` `3` | Plan, elevation, 3D |
| `V` `W` `M` `D` `N` | Select, wall, room, door, window |
| Drag | Move things; pan on empty space (plan); orbit (3D, Shift to pan) |
| Scroll | Zoom |
| Double-click an edge | Add a corner to the selected room or ground surface |
| `R`, arrows, Delete | Turn, nudge, delete the selection |
| `Ctrl/⌘ Z`, `Shift Z`, `S`, `O`, `D` | Undo, redo, save, open, duplicate |

Numbers in the inspector can be typed (`2.4`, `2400mm`, `7' 10 1/2"`), stepped, or scrubbed
by dragging sideways.

## Ground from an elevation service

In the project panel, enter a latitude and longitude and press **Fetch ground heights**.
Lintel sends a grid of coordinates around that point to
[Open Topo Data](https://www.opentopodata.org) and, failing that,
[Open-Meteo](https://open-meteo.com/en/docs/elevation-api). Nothing else is sent. These
models are 10 to 90 m coarse: they give a lot's overall fall, not its grading.

## Develop

```bash
cargo test --release
```

`tests/editing.rs` drives the real view with presses, drags and keys. Any view renders
headless (on Linux without a GPU, install `mesa-vulkan-drivers` and set
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`):

```bash
target/release/lintel --snap out.png --scene dollhouse --size 1440x900
```

| File | Holds |
| --- | --- |
| `model.rs`, `materials.rs` | The project file's data, and the material library |
| `mesh/`, `catalog.rs` | Triangles for a project; furniture pieces |
| `gfx.rs`, `shader.wgsl`, `camera.rs` | The 3D renderer, a Rune compositor on wgpu |
| `plan.rs`, `edit.rs` | The plan drawing, and pointer editing on it |
| `ui.rs`, `inspector.rs`, `widgets.rs` | The window |
| `app.rs` | Editor state, undo, files |
| `geo.rs` | Elevation services |

## Not yet

L- and U-shaped stairs, dormers and valleys, wall editing in 3D, dimension lines, textures
from photos, printing and export. Furniture is boxes.

MIT.
