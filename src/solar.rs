//! Where the sun is, from a place and a time (NOAA's low-accuracy formulas, good to a
//! fraction of a degree).

/// Compass bearing (degrees clockwise from true north) and height above the horizon (degrees)
/// of the sun at `lat`, `lon` on `month`/`day` at `hour` on a clock `utc_offset` hours ahead
/// of UTC.
pub fn position(lat: f64, lon: f64, month: u32, day: u32, hour: f32, utc_offset: f32) -> (f32, f32) {
    const BEFORE: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let n = (BEFORE[(month.clamp(1, 12) - 1) as usize] + day.clamp(1, 31)) as f64;
    let hour = hour as f64;
    let g = std::f64::consts::TAU / 365.0 * (n - 1.0 + (hour - 12.0) / 24.0);
    // Equation of time in minutes, and the sun's declination in radians.
    let eqtime = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin() - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
    // True solar time in minutes, then the hour angle: 0 at solar noon, positive after.
    let solar = hour * 60.0 + eqtime + 4.0 * lon - 60.0 * utc_offset as f64;
    let ha = (solar / 4.0 - 180.0).to_radians();
    let lat = lat.to_radians();
    let altitude = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * ha.cos()).clamp(-1.0, 1.0).asin();
    let azimuth = ha.sin().atan2(ha.cos() * lat.sin() - decl.tan() * lat.cos()) + std::f64::consts::PI;
    (azimuth.to_degrees().rem_euclid(360.0) as f32, altitude.to_degrees() as f32)
}

/// The clock offset a longitude would have with no time zone politics: 15° an hour.
pub fn natural_offset(lon: f64) -> f32 {
    (lon / 15.0).round() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_sun_is_where_almanacs_put_it() {
        // Overhead at the equator on the March equinox.
        let (_, alt) = position(0.0, 0.0, 3, 20, 12.1, 0.0);
        assert!(alt > 88.0, "{alt}");
        // 40° N on the June solstice: due south, 73.4° up. Solar noon is near 12:02.
        let (az, alt) = position(40.0, 0.0, 6, 21, 12.03, 0.0);
        assert!((az - 180.0).abs() < 2.0 && (alt - 73.4).abs() < 0.5, "{az} {alt}");
        // The same place in December: 26.6° up.
        let (_, alt) = position(40.0, 0.0, 12, 21, 11.97, 0.0);
        assert!((alt - 26.6).abs() < 0.5, "{alt}");
        // Sydney in December: the noon sun is to the north, and high.
        let (az, alt) = position(-33.87, 151.21, 12, 21, 12.9, 11.0);
        assert!((az < 20.0 || az > 340.0) && (alt - 79.6).abs() < 1.0, "{az} {alt}");
    }

    #[test]
    fn it_rises_in_the_east_sets_in_the_west_and_is_down_at_night() {
        let (az, alt) = position(40.0, -105.0, 6, 21, 7.0, -7.0);
        assert!(az > 50.0 && az < 100.0 && alt > 0.0, "{az} {alt}");
        let (az, alt) = position(40.0, -105.0, 6, 21, 18.0, -7.0);
        assert!(az > 260.0 && az < 300.0 && alt > 0.0, "{az} {alt}");
        let (_, alt) = position(40.0, -105.0, 6, 21, 1.0, -7.0);
        assert!(alt < -15.0, "{alt}");
        assert_eq!(natural_offset(-105.0), -7.0);
    }
}
