//! Showing and reading lengths. The model is always metres; this is only the text.

use crate::model::Units;

const FOOT: f32 = 0.3048;
const INCH: f32 = 0.0254;

/// A length as text: `2.43 m` or `7' 11½"`.
pub fn length(m: f32, units: Units) -> String {
    match units {
        Units::Metric => {
            if m.abs() < 1.0 && m != 0.0 {
                format!("{} mm", (m * 1000.0).round() as i32)
            } else {
                format!("{} m", trim(format!("{m:.3}")))
            }
        }
        Units::Imperial => {
            let sign = if m < 0.0 { "-" } else { "" };
            // Quarter inches.
            let quarters = (m.abs() / INCH * 4.0).round() as i64;
            let (feet, rest) = (quarters / 48, quarters % 48);
            let (inches, frac) = (rest / 4, rest % 4);
            let frac = ["", "¼", "½", "¾"][frac as usize];
            if feet == 0 {
                format!("{sign}{inches}{frac}\"")
            } else if inches == 0 && frac.is_empty() {
                format!("{sign}{feet}'")
            } else {
                format!("{sign}{feet}' {inches}{frac}\"")
            }
        }
    }
}

/// An area as text: `18.4 m²` or `198 sq ft`.
pub fn area(m2: f32, units: Units) -> String {
    match units {
        Units::Metric => format!("{m2:.1} m²"),
        Units::Imperial => format!("{:.0} sq ft", m2 / (FOOT * FOOT)),
    }
}

fn trim(s: String) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// Reads a length someone typed. A bare number is in the project's main unit (metres or
/// feet); `mm`, `cm`, `m`, `ft`, `in`, `'` and `"` are understood in either system, so
/// `2400mm`, `2.4`, `7' 10 1/2"`, `94in` and `7.5ft` all work.
pub fn parse_length(text: &str, units: Units) -> Option<f32> {
    let t = text.trim().to_lowercase().replace('′', "'").replace(['″', '”', '“'], "\"");
    let t = t.replace('¼', " 1/4").replace('½', " 1/2").replace('¾', " 3/4");
    if t.is_empty() {
        return None;
    }
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim().to_string()),
        None => (false, t),
    };
    let sign = if neg { -1.0 } else { 1.0 };
    for (suffix, k) in [("mm", 0.001), ("cm", 0.01), ("ft", FOOT), ("in", INCH), ("m", 1.0)] {
        if let Some(n) = t.strip_suffix(suffix) {
            return number(n).map(|v| sign * v * k);
        }
    }
    if t.contains('\'') || t.contains('"') {
        let (feet, inches) = match t.split_once('\'') {
            Some((f, i)) => (number(f)?, i),
            None => (0.0, t.as_str()),
        };
        let inches = inches.trim().trim_end_matches('"').trim();
        let inches = if inches.is_empty() { 0.0 } else { number(inches)? };
        return Some(sign * (feet * FOOT + inches * INCH));
    }
    let v = number(&t)?;
    Some(sign * v * if units == Units::Imperial { FOOT } else { 1.0 })
}

/// A number, optionally with a fraction: `10`, `10.5`, `10 1/2`, `1/2`.
fn number(s: &str) -> Option<f32> {
    let mut total = 0.0;
    let mut any = false;
    for part in s.split_whitespace() {
        total += match part.split_once('/') {
            Some((a, b)) => {
                let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
                if b == 0.0 {
                    return None;
                }
                a / b
            }
            None => part.parse::<f32>().ok()?,
        };
        any = true;
    }
    any.then_some(total)
}

/// The grid step lengths snap to while drawing: 50 mm, or one inch.
pub fn snap_step(units: Units) -> f32 {
    match units {
        Units::Metric => 0.05,
        Units::Imperial => INCH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_reads_and_writes() {
        assert_eq!(length(2.43, Units::Metric), "2.43 m");
        assert_eq!(length(0.09, Units::Metric), "90 mm");
        assert_eq!(length(3.0, Units::Metric), "3 m");
        assert_eq!(parse_length("2.4", Units::Metric), Some(2.4));
        assert_eq!(parse_length("2400mm", Units::Metric), Some(2.4));
        assert_eq!(parse_length("90 cm", Units::Metric), Some(0.9));
        assert_eq!(parse_length("-0.3 m", Units::Metric), Some(-0.3));
        assert_eq!(parse_length("wide", Units::Metric), None);
    }

    #[test]
    fn imperial_reads_and_writes() {
        assert_eq!(length(2.4384, Units::Imperial), "8'");
        assert_eq!(length(2.4257, Units::Imperial), "7' 11½\"");
        assert_eq!(length(0.3, Units::Imperial), "11¾\"");
        let close = |a: Option<f32>, b: f32| (a.unwrap() - b).abs() < 1e-4;
        assert!(close(parse_length("8", Units::Imperial), 2.4384));
        assert!(close(parse_length("7' 10 1/2\"", Units::Imperial), 7.0 * FOOT + 10.5 * INCH));
        assert!(close(parse_length("94in", Units::Imperial), 94.0 * INCH));
        assert!(close(parse_length("7' 11½\"", Units::Imperial), 7.0 * FOOT + 11.5 * INCH));
        assert!(close(parse_length("36\"", Units::Metric), 0.9144));
    }
}
