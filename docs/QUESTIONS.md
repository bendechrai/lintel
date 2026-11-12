# Questions and decisions

Open questions first, each with the default I'm working to until you say otherwise. Decisions
are below, with what they turn into.

## Open

1. **Time zones.** The sun's clock is a UTC offset you set. Should it work out the time zone
   and daylight saving from the location instead? Default: leave it as an offset.
2. **Roof.** Does your roof have valleys, dormers or several ridges? Today a roof is one
   rectangle (gable, hip, shed or flat). Default: add roofs made of several rectangles later.
3. **Services.** Do you want plumbing points and HVAC duct runs? Default: no.
4. **Voice agent scope.** It will use each person's own API key. Should it only fill in the
   model as you dictate measurements, or also answer questions about the house ("how much
   skirting do I need")? Push-to-talk or always listening? Default: dictation only,
   push-to-talk.
5. **Named options.** When you keep "Existing" and a proposal in one project, do they share
   the site and the unchanged levels, or is each a full copy of the house? Default: each is a
   full copy you can diverge freely, with a side-by-side view.
6. **Real products.** Where does the product library live: in the project file, in a library
   file you keep and share between projects, or a public list people contribute to?
   Default: a library file of your own, with a few examples included.
7. **Drawings.** For dimensioned drawings, which paper sizes and scales do you print at
   (A3 at 1:100, Letter at 1/4" = 1')? Default: A3 and A4 at 1:50 and 1:100, as PDF.
8. **Licence and name.** MIT and "Lintel" are my placeholders.

## Decided

| | Decision |
| --- | --- |
| Platform | Native desktop app, plus a link anyone can open to view a house on any device. |
| Shared link | A frozen copy inside the link: no hosting, no account. Send a new link after a change. |
| Realism | Calculated light levels, not only a believable picture. |
| Getting measurements in | Typed, and traced over a plan image. Phone scans are parked. |
| Voice agent | Wanted; uses each person's own API key. |
| Electrical | Outlet and switch positions on regular walls. A switch knows the lights it controls. |
| Units | Metric with neutral sizes by default; feet and inches per project. |
| Stairs | Straight flights are enough. |
| Before and after | Named options inside one project. |
| Light levels | Add targets per room, and an overcast sky. |
| Output | Dimensioned drawings, images, and 3D model export. |
| Walls | Skirting and cornices, part-height finishes, and the build-up through the wall. |
| Ceilings | Sloped or vaulted, bulkheads and drops, exposed beams. |
| Outside | Fences and gates, retaining walls and steps, trees and planting, deck structure. |
| Furniture | Real products by their true dimensions. |

## Built from these

Sun by place and time; lamps with lumens and colour temperature; LED strips; switches linked
to lights; light levels under a clear or overcast sky, with a target per room; the shared link
and phone viewer; tracing a plan image; pictures and glTF export; named options; skirting,
cornices, part-height finishes and wall build-up; sloped and vaulted ceilings, bulkheads and
beams; fences, gates, retaining walls, trees and deck structure; dimensioned plans as PDF; a
product library; the measuring assistant on your own API key.

## Left open

- The assistant takes typed or OS-dictated speech and can read replies aloud; it does not
  capture the microphone itself (question 4 stands for whether it should also answer
  questions about the house).
- Options compare on the plan, not side by side in 3D.
- Drawings are plans; elevations go out as pictures.
- Questions 1 to 3 and 5 onward above keep their defaults until answered.
