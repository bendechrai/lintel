//! The surface material library.
//!
//! A material is a procedural pattern (drawn by `shader.wgsl`) with a base colour and a rule
//! for how a `Finish`'s colour applies to it:
//!
//! - `Tint::Paint`: the surface is whatever colour you give it (paint, laminate, fabric).
//! - `Tint::Stain`: the material has its own colour (a timber species) and a colour laid over
//!   it works like a stain: it darkens and shifts the timber while the grain shows through.
//! - `Tint::Fixed`: the material is the colour it is (marble, concrete, brick, glass).

use serde::{Deserialize, Serialize};

use crate::model::{Finish, Project, rgb};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tint {
    Paint,
    Stain,
    Fixed,
}

/// The procedural pattern the shader draws. The discriminants are the shader's pattern ids.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Pattern {
    /// Flat colour with a faint roller texture.
    Plain = 0,
    /// Floorboards: staggered planks with grain. `scale` is the board width.
    Planks = 1,
    /// Continuous timber grain (a tabletop, a door leaf, joinery).
    Grain = 2,
    /// Square tiles with grout lines. `scale` is the tile size.
    Tile = 3,
    /// Carpet or rug pile.
    Carpet = 4,
    /// Poured concrete: mottled, with fine speckle.
    Concrete = 5,
    /// Running-bond brickwork. `scale` is the brick length.
    Brick = 6,
    /// Veined stone.
    Marble = 7,
    Grass = 8,
    /// Clear glazing: translucent, with a sky reflection.
    Glass = 9,
    /// Brushed metal.
    Metal = 10,
    /// Asphalt or gravel: coarse dark aggregate.
    Asphalt = 11,
    /// Roof shingles or tiles in courses.
    Shingle = 12,
    /// Horizontal lap siding / weatherboard. `scale` is the board exposure.
    Siding = 13,
    /// Woven upholstery fabric.
    Fabric = 14,
    /// A light source: drawn at full brightness, unlit.
    Emissive = 15,
    /// Speckled stone benchtop (granite, quartz, terrazzo).
    Speckle = 16,
    /// Bare soil or mulch.
    Soil = 17,
    /// Horizontal ribs: a roller door curtain, corrugated sheet.
    Ribbed = 18,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub group: String,
    pub pattern: Pattern,
    pub tint: Tint,
    /// sRGB base colour: the default paint colour, the unstained timber, or the fixed colour.
    pub color: [f32; 3],
    /// Pattern size in metres (board width, tile size, brick length …).
    #[serde(default)]
    pub scale: f32,
    /// 0 mirror-smooth … 1 matte.
    pub roughness: f32,
    /// 1 opaque. Less draws in the translucent pass.
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
}

fn m(id: &str, name: &str, group: &str, pattern: Pattern, tint: Tint, color: u32, scale: f32, roughness: f32) -> Material {
    Material { id: id.into(), name: name.into(), group: group.into(), pattern, tint, color: rgb(color), scale, roughness, opacity: 1.0 }
}

/// The built-in library.
pub fn library() -> Vec<Material> {
    use Pattern::*;
    use Tint::*;
    let mut lib = vec![
        // Coatings: any colour you like.
        m("paint", "Paint, matte", "Paint & plaster", Plain, Paint, 0xf2efe8, 0.0, 0.9),
        m("paint_satin", "Paint, satin", "Paint & plaster", Plain, Paint, 0xf2efe8, 0.0, 0.45),
        m("paint_gloss", "Paint, gloss", "Paint & plaster", Plain, Paint, 0xf7f6f2, 0.0, 0.15),
        m("render", "Render / stucco", "Paint & plaster", Concrete, Paint, 0xe6e1d6, 0.6, 0.95),
        m("laminate", "Laminate", "Paint & plaster", Plain, Paint, 0xeeeeea, 0.0, 0.35),
        m("plastic", "Plastic", "Paint & plaster", Plain, Paint, 0xfafafa, 0.0, 0.4),
        // Timbers: their own colour, with an optional stain over it.
        m("pine", "Pine", "Timber", Grain, Stain, 0xe3c896, 0.14, 0.6),
        m("birch", "Birch", "Timber", Grain, Stain, 0xecd9b4, 0.12, 0.55),
        m("oak", "Oak", "Timber", Grain, Stain, 0xc9a26b, 0.14, 0.55),
        m("walnut", "Walnut", "Timber", Grain, Stain, 0x6f4a32, 0.14, 0.5),
        m("maple", "Maple", "Timber", Grain, Stain, 0xe8cfa2, 0.12, 0.5),
        m("cedar", "Cedar", "Timber", Grain, Stain, 0xb5704a, 0.14, 0.65),
        m("oak_boards", "Oak floorboards", "Timber floors", Planks, Stain, 0xc49a63, 0.13, 0.45),
        m("pine_boards", "Pine floorboards", "Timber floors", Planks, Stain, 0xdcbd86, 0.13, 0.5),
        m("walnut_boards", "Walnut floorboards", "Timber floors", Planks, Stain, 0x6a4630, 0.13, 0.45),
        m("vinyl_plank", "Vinyl plank (timber look)", "Timber floors", Planks, Stain, 0xb08d62, 0.18, 0.4),
        m("laminate_boards", "Laminate floorboards", "Timber floors", Planks, Stain, 0xbf9c70, 0.19, 0.35),
        m("decking", "Timber decking", "Timber floors", Planks, Stain, 0x9a6b47, 0.09, 0.7),
        // Hard floors and stone.
        m("tile", "Ceramic tile", "Tile & stone", Tile, Paint, 0xe9e6df, 0.6, 0.3),
        m("tile_small", "Small tile", "Tile & stone", Tile, Paint, 0xf1f1ee, 0.1, 0.25),
        m("subway_tile", "Wall tile", "Tile & stone", Tile, Paint, 0xf6f6f3, 0.15, 0.2),
        m("slate", "Slate tile", "Tile & stone", Tile, Fixed, 0x4a4f55, 0.4, 0.6),
        m("marble", "Marble, white", "Tile & stone", Marble, Fixed, 0xefece6, 1.0, 0.15),
        m("marble_dark", "Marble, dark", "Tile & stone", Marble, Fixed, 0x2c2c30, 1.0, 0.15),
        m("granite", "Granite", "Tile & stone", Speckle, Fixed, 0x6e6a66, 1.0, 0.25),
        m("quartz", "Quartz benchtop", "Tile & stone", Speckle, Paint, 0xeceae6, 1.0, 0.2),
        m("concrete", "Concrete", "Tile & stone", Concrete, Fixed, 0xa9a8a3, 1.0, 0.85),
        m("polished_concrete", "Polished concrete", "Tile & stone", Concrete, Fixed, 0x9c9b97, 1.0, 0.25),
        // Soft floors and fabric.
        m("carpet", "Carpet", "Carpet & fabric", Carpet, Paint, 0xb9b2a6, 1.0, 1.0),
        m("rug", "Rug", "Carpet & fabric", Carpet, Paint, 0x8c5a4a, 1.0, 1.0),
        m("fabric", "Upholstery fabric", "Carpet & fabric", Fabric, Paint, 0x8f9499, 1.0, 0.95),
        m("leather", "Leather", "Carpet & fabric", Plain, Paint, 0x5b3a29, 0.0, 0.5),
        // Masonry and cladding.
        m("brick", "Brick, red", "Masonry & cladding", Brick, Fixed, 0xa5553f, 0.23, 0.9),
        m("brick_painted", "Brick, painted", "Masonry & cladding", Brick, Paint, 0xf0ede6, 0.23, 0.85),
        m("blockwork", "Concrete block", "Masonry & cladding", Brick, Fixed, 0xa6a39c, 0.4, 0.9),
        m("siding", "Lap siding / weatherboard", "Masonry & cladding", Siding, Paint, 0xdfe3e6, 0.15, 0.7),
        m("cedar_siding", "Cedar cladding", "Masonry & cladding", Siding, Stain, 0xb5704a, 0.12, 0.7),
        // Roofing.
        m("shingle", "Asphalt shingle", "Roofing", Shingle, Paint, 0x4b4d52, 0.3, 0.9),
        m("roof_tile", "Roof tile", "Roofing", Shingle, Paint, 0x9a5a44, 0.33, 0.8),
        m("metal_roof", "Metal roofing", "Roofing", Ribbed, Paint, 0x5d666e, 0.2, 0.4),
        // Metal and glass.
        m("steel", "Stainless steel", "Metal & glass", Metal, Fixed, 0xc3c6c9, 1.0, 0.3),
        m("black_steel", "Black stainless", "Metal & glass", Metal, Fixed, 0x3a3c40, 1.0, 0.3),
        m("aluminium", "Aluminium, powder-coated", "Metal & glass", Plain, Paint, 0x2f3134, 0.0, 0.5),
        m("brass", "Brass", "Metal & glass", Metal, Fixed, 0xb89b5e, 1.0, 0.35),
        m("roller_door", "Ribbed steel (roller door)", "Metal & glass", Ribbed, Paint, 0xe6e6e3, 0.075, 0.5),
        m("mirror", "Mirror", "Metal & glass", Metal, Fixed, 0xe4e9ec, 1.0, 0.02),
        // Outside.
        m("grass", "Lawn", "Landscape", Grass, Fixed, 0x5f8a45, 1.0, 1.0),
        m("soil", "Garden bed / mulch", "Landscape", Soil, Fixed, 0x5a4636, 1.0, 1.0),
        m("asphalt", "Asphalt", "Landscape", Asphalt, Fixed, 0x3f4144, 1.0, 0.95),
        m("gravel", "Gravel", "Landscape", Asphalt, Fixed, 0xa59c8e, 1.0, 1.0),
        m("pavers", "Pavers", "Landscape", Brick, Paint, 0xb3a999, 0.2, 0.9),
        // Lights.
        m("light", "Light (emissive)", "Lighting", Emissive, Paint, 0xfff4d6, 0.0, 1.0),
    ];
    lib.push(Material { opacity: 0.25, ..m("glass", "Clear glass", "Metal & glass", Glass, Fixed, 0xd7e6ea, 1.0, 0.03) });
    lib.push(Material { opacity: 0.45, ..m("glass_tinted", "Tinted glass", "Metal & glass", Glass, Fixed, 0x6f8087, 1.0, 0.03) });
    lib.push(Material { opacity: 0.7, ..m("glass_frosted", "Frosted glass", "Metal & glass", Glass, Fixed, 0xe9eff0, 1.0, 0.6) });
    lib
}

/// A surface ready for the renderer: the pattern and the colour it draws with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub pattern: Pattern,
    /// Linear RGB.
    pub color: [f32; 3],
    pub scale: f32,
    pub roughness: f32,
    pub opacity: f32,
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn linear(c: [f32; 3]) -> [f32; 3] {
    c.map(to_linear)
}

/// Looks materials up by id: the project's own first, then the built-in library.
pub struct Library {
    all: Vec<Material>,
}

impl Library {
    pub fn new(project: &Project) -> Library {
        let mut all = project.materials.clone();
        all.extend(library());
        Library { all }
    }

    pub fn builtin() -> Library {
        Library { all: library() }
    }

    pub fn all(&self) -> &[Material] {
        &self.all
    }

    pub fn get(&self, id: &str) -> Option<&Material> {
        self.all.iter().find(|m| m.id == id)
    }

    /// The colour a finish shows, in sRGB: what a swatch draws.
    pub fn swatch(&self, finish: &Finish) -> [f32; 3] {
        let Some(m) = self.get(&finish.material) else { return [1.0, 0.0, 1.0] };
        match (m.tint, finish.color) {
            (Tint::Paint, Some(c)) => c,
            // A stain multiplies the timber, as a translucent coat does, then leans toward the
            // stain's own colour so a dark stain on pale pine still reads as that stain.
            (Tint::Stain, Some(c)) => std::array::from_fn(|i| {
                let through = m.color[i] * c[i];
                through * 0.65 + c[i] * 0.35 * (0.4 + 0.6 * m.color[i])
            }),
            _ => m.color,
        }
    }

    pub fn surface(&self, finish: &Finish) -> Surface {
        match self.get(&finish.material) {
            Some(m) => Surface { pattern: m.pattern, color: linear(self.swatch(finish)), scale: m.scale, roughness: m.roughness, opacity: m.opacity },
            None => Surface { pattern: Pattern::Plain, color: [1.0, 0.0, 1.0], scale: 0.0, roughness: 1.0, opacity: 1.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let lib = library();
        for (i, a) in lib.iter().enumerate() {
            assert!(lib[i + 1..].iter().all(|b| b.id != a.id), "duplicate material id {}", a.id);
        }
    }

    #[test]
    fn paint_takes_its_colour_timber_keeps_its_own_and_a_stain_darkens_it() {
        let lib = Library::builtin();
        assert_eq!(lib.swatch(&Finish::paint(0x336699)), rgb(0x336699));
        let pine = lib.swatch(&Finish::of("pine"));
        assert_eq!(pine, rgb(0xe3c896));
        let stained = lib.swatch(&Finish::colored("pine", rgb(0x5a3a22)));
        assert!(stained.iter().zip(pine).all(|(s, p)| *s < p), "{stained:?} vs {pine:?}");
        // Marble ignores a colour.
        assert_eq!(lib.swatch(&Finish::colored("marble", rgb(0xff0000))), rgb(0xefece6));
    }
}
