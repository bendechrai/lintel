# Open questions

Things I'd like your answer on. Each has the default I'm working to until you say otherwise.
Answered ones move to the bottom with the decision.

## Direction

1. **Where does it run?** Native desktop only, or in a browser too?
   Default: native desktop. This decides how much Rune's experimental web target matters.
2. **How real should the render be?** A live view that looks good, or also a slow,
   high-quality still (bounced light, soft shadows)?
   Default: live view only.
3. **How do measurements get in?** Typed from a tape measure, traced over a floor plan image,
   or imported from a phone room scan?
   Default: typed and drawn by hand.
4. **What is the first real job?** Which change to your own home do you want to plan first?
   It decides what gets depth first (lighting, layout, finishes, electrical).

## Lighting

5. **Sun.** Built: by hand, or from latitude, longitude, date and time, with shortcuts for
   the solstices and equinoxes. The clock is a UTC offset you set; should it know time zones
   and daylight saving from the location instead?
6. **Lamps.** Which sources matter: bulbs by wattage or lumens, colour temperature, dimming,
   LED strip by lumens per metre in a diffuser channel? Do you want real photometric numbers
   (lux on a bench) or only a believable picture?

## Model

8. **Units.** Metric or feet and inches as your default?
9. **Walls.** Is one finish per face enough, or do you need layers (stud, insulation, plaster),
   skirting, cornices, half-height tiling?
10. **Ceilings.** Flat per room is in. Do you need sloped or vaulted ceilings, bulkheads, beams?
11. **Stairs.** Which shapes does your house have: straight, L, U, winders?
12. **Roof.** Does yours have valleys, dormers or several ridges?
13. **Outside.** Fences, retaining walls, steps, decks with posts, trees?
14. **Services.** Plumbing points, HVAC duct runs?

## Use

15. **Before and after.** Do you want to keep the existing house and a proposed change side by
    side (options, phases), or is one model with undo enough?
16. **Output.** Dimensioned drawings to print or hand to a tradesperson? Material and fixture
    schedules? Costs?
17. **Furniture.** Generic resizable pieces, or specific products by real dimensions
    (your LG fridge and wash tower)? Imported 3D models?
18. **Licence and name.** MIT and "Lintel" are my placeholders.

## New, from your answers

19. **Shared link.** Built so the house travels inside the link (no upload, no account) and
    a phone gets a viewer. Anyone with the link sees the whole model and can't change your
    copy. Is that right, or do you want links that stay current as you edit (which needs
    somewhere to host the project), comments, or measuring in the viewer?
20. **Voice agent.** Whose model and key (your own Claude API key in settings)? Should it only
    fill in the model as you dictate measurements, or also answer questions about the house
    ("how much skirting do I need")? Push-to-talk or always listening?
21. **Phone capture.** A companion iOS app (RoomPlan gives walls, doors and windows from
    LiDAR) is a separate project. Is importing a scan file made by an existing app enough to
    start with?
22. **Light accuracy.** Built with what you type per fitting (lumens, colour temperature,
    source) and a clear-sky sun; it gives a lux map and a figure per room. Do you need
    manufacturer photometric files (IES) for real beam shapes, overcast skies, or targets per
    room ("the kitchen bench should reach 500 lux") that the app checks?

## Answered

- **1. Where does it run?** Native desktop app, plus a URL anyone can open to view the model
  on any device, mobile included.
- **2. How real?** Accurate light levels, not only a believable picture.
- **3. Measurements.** All three (typed, traced plan, phone depth scan), plus a built-in voice
  agent that asks for measurements and takes them as you speak them.
- **4. First job.** Layout, finishes and electrical. Electrical means outlet and switch
  positions on regular walls, nothing more for now.
- **7. Switching.** Yes: a switch knows which lights it controls, so lights can be turned
  on and off in the view.
