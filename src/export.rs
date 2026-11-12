//! Getting the house out: the 3D model as glTF, and pictures as PNG.

use std::path::Path;

use base64::Engine;

use crate::mesh::{self, BuildOptions};
use crate::model::Project;
use crate::scene::Scene;

/// The whole house as a glTF 2.0 file with its data embedded, so it is one file that opens in
/// Blender, three.js viewers, and most 3D software. Colours ride on the vertices (the
/// procedural grain and tile patterns are Lintel's own and don't travel). glTF is Y-up, so
/// Lintel's north (+Y) becomes -Z.
pub fn gltf(project: &Project) -> String {
    scene_gltf(&mesh::build(project, &BuildOptions::default()), &project.name)
}

pub fn scene_gltf(scene: &Scene, name: &str) -> String {
    let n = scene.vertices.len();
    let mut bin: Vec<u8> = Vec::with_capacity(n * 40 + (scene.opaque.len() + scene.translucent.len()) * 4);
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &scene.vertices {
        let p = [v.pos[0], v.pos[2], -v.pos[1]];
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
        bin.extend(p.iter().flat_map(|f| f.to_le_bytes()));
    }
    for v in &scene.vertices {
        bin.extend([v.normal[0], v.normal[2], -v.normal[1]].iter().flat_map(|f| f.to_le_bytes()));
    }
    for v in &scene.vertices {
        bin.extend(v.color.iter().flat_map(|f| f.to_le_bytes()));
    }
    let index_start = bin.len();
    bin.extend(scene.opaque.iter().flat_map(|i| i.to_le_bytes()));
    let blend_start = bin.len();
    bin.extend(scene.translucent.iter().flat_map(|i| i.to_le_bytes()));
    if n == 0 {
        (lo, hi) = ([0.0; 3], [0.0; 3]);
    }

    let view = |offset: usize, length: usize, target: u32| serde_json::json!({ "buffer": 0, "byteOffset": offset, "byteLength": length, "target": target });
    let mut views = vec![view(0, n * 12, 34962), view(n * 12, n * 12, 34962), view(n * 24, n * 16, 34962)];
    let mut accessors = vec![
        serde_json::json!({ "bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi }),
        serde_json::json!({ "bufferView": 1, "componentType": 5126, "count": n, "type": "VEC3" }),
        serde_json::json!({ "bufferView": 2, "componentType": 5126, "count": n, "type": "VEC4" }),
    ];
    let mut primitives = Vec::new();
    for (start, list, material) in [(index_start, &scene.opaque, 0), (blend_start, &scene.translucent, 1)] {
        if list.is_empty() || n == 0 {
            continue;
        }
        views.push(view(start, list.len() * 4, 34963));
        accessors.push(serde_json::json!({ "bufferView": views.len() - 1, "componentType": 5125, "count": list.len(), "type": "SCALAR" }));
        primitives.push(serde_json::json!({ "attributes": { "POSITION": 0, "NORMAL": 1, "COLOR_0": 2 }, "indices": accessors.len() - 1, "material": material }));
    }
    let mut doc = serde_json::json!({
        "asset": { "version": "2.0", "generator": "Lintel" },
        "scene": 0,
        "scenes": [{ "name": name, "nodes": [] }],
        "nodes": [],
        "materials": [
            { "name": "Surfaces", "doubleSided": true, "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 0.8 } },
            { "name": "Glass", "doubleSided": true, "alphaMode": "BLEND", "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 0.1 } }
        ],
        "buffers": [{ "byteLength": bin.len(), "uri": format!("data:application/octet-stream;base64,{}", base64::engine::general_purpose::STANDARD.encode(&bin)) }],
        "bufferViews": views,
        "accessors": accessors,
    });
    if !primitives.is_empty() {
        doc["scenes"][0]["nodes"] = serde_json::json!([0]);
        doc["nodes"] = serde_json::json!([{ "name": name, "mesh": 0 }]);
        doc["meshes"] = serde_json::json!([{ "name": name, "primitives": primitives }]);
    }
    doc.to_string()
}

/// Writes sRGB RGBA bytes as a PNG.
pub fn png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_house_exports_as_a_self_contained_gltf_that_stands_upright() {
        let project = crate::sample::house();
        let scene = mesh::build(&project, &BuildOptions::default());
        let doc: serde_json::Value = serde_json::from_str(&gltf(&project)).unwrap();
        assert_eq!(doc["asset"]["version"], "2.0");
        assert_eq!(doc["accessors"][0]["count"], scene.vertices.len());
        let prims = doc["meshes"][0]["primitives"].as_array().unwrap();
        assert_eq!(prims.len(), 2, "solid surfaces and glass");
        assert_eq!(doc["accessors"][prims[0]["indices"].as_u64().unwrap() as usize]["count"], scene.opaque.len());
        // The data is inside the file and is as long as it says.
        let uri = doc["buffers"][0]["uri"].as_str().unwrap();
        let data = base64::engine::general_purpose::STANDARD.decode(uri.split_once("base64,").unwrap().1).unwrap();
        assert_eq!(data.len() as u64, doc["buffers"][0]["byteLength"].as_u64().unwrap());
        // Up is +Y in glTF: the ridge of the roof is the highest thing, about 5 m up.
        let top = doc["accessors"][0]["max"][1].as_f64().unwrap();
        assert!(top > 4.5 && top < 6.5, "{top}");
        // An empty project still makes a valid file.
        let empty: serde_json::Value = serde_json::from_str(&scene_gltf(&Scene::default(), "Empty")).unwrap();
        assert!(empty.get("meshes").is_none());
    }
}
