//! Ground heights from public elevation services.
//!
//! Lintel asks [Open Topo Data](https://www.opentopodata.org) first (USGS 10 m data in the US,
//! EU-DEM 25 m in Europe, Mapzen 30 m elsewhere) and falls back to
//! [Open-Meteo](https://open-meteo.com/en/docs/elevation-api) (Copernicus 90 m). Both are free
//! and need no key. The only thing sent is a grid of coordinates around the point you give.
//!
//! These models are coarse next to a house lot: they get the overall fall of the land right
//! and know nothing about retaining walls or a levelled pad. Treat the result as a starting
//! shape.

use std::time::Duration;

use crate::model::{Finish, Geo, Terrain};

#[derive(Clone, Debug)]
pub struct Fetched {
    pub terrain: Terrain,
    pub geo: Geo,
}

const EARTH_M_PER_DEG: f64 = 111_320.0;

/// The grid of coordinates for a square `span` metres across centred on `geo`, row by row
/// from the south-west corner, with `n` posts a side.
pub fn grid(geo: Geo, span: f32, n: usize) -> Vec<(f64, f64)> {
    let step = span as f64 / (n - 1) as f64;
    let (dlat, dlon) = (1.0 / EARTH_M_PER_DEG, 1.0 / (EARTH_M_PER_DEG * geo.lat.to_radians().cos().max(0.01)));
    let mut out = Vec::with_capacity(n * n);
    for r in 0..n {
        for c in 0..n {
            let (x, y) = (c as f64 * step - span as f64 / 2.0, r as f64 * step - span as f64 / 2.0);
            out.push((geo.lat + y * dlat, geo.lon + x * dlon));
        }
    }
    out
}

/// Terrain from absolute elevations on `grid`'s posts: heights are relative to the ground at
/// the centre, which becomes the plan origin at z = 0.
pub fn terrain(elevations: &[f64], span: f32, n: usize, source: &str) -> Terrain {
    let spacing = span / (n - 1) as f32;
    let datum = elevations[(n / 2) * n + n / 2];
    Terrain {
        origin: [-span / 2.0, -span / 2.0],
        spacing,
        cols: n,
        rows: n,
        heights: elevations.iter().map(|e| (e - datum) as f32).collect(),
        finish: Finish::of("grass"),
        source: source.into(),
    }
}

/// Elevations out of an Open Topo Data response: `{"results": [{"elevation": 12.3, ..}, ..]}`.
pub fn parse_opentopodata(body: &str) -> Result<Vec<f64>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if v["status"] != "OK" {
        return Err(v["error"].as_str().unwrap_or("the service refused the request").to_string());
    }
    let results = v["results"].as_array().ok_or("no results")?;
    results.iter().map(|r| r["elevation"].as_f64().ok_or_else(|| "no elevation data for this place".to_string())).collect()
}

/// Elevations out of an Open-Meteo response: `{"elevation": [12.0, ..]}`.
pub fn parse_open_meteo(body: &str) -> Result<Vec<f64>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = v["elevation"].as_array().ok_or_else(|| v["reason"].as_str().unwrap_or("no elevation in the response").to_string())?;
    list.iter().map(|e| e.as_f64().ok_or_else(|| "no elevation data for this place".to_string())).collect()
}

#[cfg(target_arch = "wasm32")]
fn get(_url: &str) -> Result<String, String> {
    Err("fetching ground heights isn't available in the browser yet".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn get(url: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent("lintel").build();
    agent.get(url).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

fn opentopodata(points: &[(f64, f64)]) -> Result<Vec<f64>, String> {
    let mut out = Vec::with_capacity(points.len());
    // 100 locations a request, one request a second.
    for (i, chunk) in points.chunks(100).enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(1100));
        }
        let locations: Vec<String> = chunk.iter().map(|(lat, lon)| format!("{lat:.6},{lon:.6}")).collect();
        let url = format!("https://api.opentopodata.org/v1/ned10m,eudem25m,mapzen?locations={}&interpolation=cubic", locations.join("|"));
        out.extend(parse_opentopodata(&get(&url)?)?);
    }
    Ok(out)
}

fn open_meteo(points: &[(f64, f64)]) -> Result<Vec<f64>, String> {
    let mut out = Vec::with_capacity(points.len());
    for chunk in points.chunks(100) {
        let lat: Vec<String> = chunk.iter().map(|p| format!("{:.6}", p.0)).collect();
        let lon: Vec<String> = chunk.iter().map(|p| format!("{:.6}", p.1)).collect();
        let url = format!("https://api.open-meteo.com/v1/elevation?latitude={}&longitude={}", lat.join(","), lon.join(","));
        out.extend(parse_open_meteo(&get(&url)?)?);
    }
    Ok(out)
}

/// Fetches the ground around `geo`: a square `span` metres across. Blocks; call it off the UI
/// thread.
pub fn fetch(geo: Geo, span: f32) -> Result<Fetched, String> {
    if !(-90.0..=90.0).contains(&geo.lat) || !(-180.0..=180.0).contains(&geo.lon) {
        return Err("Latitude is -90 to 90 and longitude -180 to 180.".into());
    }
    let n = 13;
    let points = grid(geo, span, n);
    let (elevations, source) = match opentopodata(&points) {
        Ok(e) => (e, "Open Topo Data (10–30 m model)"),
        Err(first) => match open_meteo(&points) {
            Ok(e) => (e, "Open-Meteo (90 m model)"),
            Err(second) => return Err(format!("Open Topo Data: {first}. Open-Meteo: {second}.")),
        },
    };
    if elevations.len() != n * n {
        return Err("The service returned a different number of points than asked for.".into());
    }
    Ok(Fetched { terrain: terrain(&elevations, span, n, &format!("{source} at {:.5}, {:.5}", geo.lat, geo.lon)), geo })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_is_centred_and_spaced_in_metres() {
        let geo = Geo { lat: 40.0, lon: -105.0 };
        let g = grid(geo, 40.0, 5);
        assert_eq!(g.len(), 25);
        assert!((g[12].0 - 40.0).abs() < 1e-9 && (g[12].1 + 105.0).abs() < 1e-9);
        // One row up is 10 m north.
        assert!(((g[5].0 - g[0].0) * EARTH_M_PER_DEG - 10.0).abs() < 1e-6);
        // One column over is 10 m east: more degrees of longitude than of latitude at 40° N.
        assert!(((g[1].1 - g[0].1) * EARTH_M_PER_DEG * 40f64.to_radians().cos() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn responses_parse_and_heights_are_relative_to_the_centre() {
        let body = r#"{"results":[{"dataset":"ned10m","elevation":1600.0,"location":{"lat":1,"lng":2}},{"elevation":1601.5},{"elevation":1603.0},{"elevation":1600.5},{"elevation":1602.0},{"elevation":1603.5},{"elevation":1601.0},{"elevation":1602.5},{"elevation":1604.0}],"status":"OK"}"#;
        let e = parse_opentopodata(body).unwrap();
        let t = terrain(&e, 20.0, 3, "test");
        assert_eq!(t.height([0.0, 0.0]), 0.0);
        assert_eq!(t.height([-10.0, -10.0]), -2.0);
        assert_eq!(t.height([10.0, 10.0]), 2.0);
        assert!(parse_opentopodata(r#"{"status":"INVALID_REQUEST","error":"Too many locations"}"#).is_err());
        assert!(parse_opentopodata(r#"{"status":"OK","results":[{"elevation":null}]}"#).is_err());
        assert_eq!(parse_open_meteo(r#"{"elevation":[38.0,41.5]}"#).unwrap(), vec![38.0, 41.5]);
    }
}
