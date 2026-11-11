//! Small plan-geometry helpers.

use crate::model::P2;

pub fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn scale(a: P2, k: f32) -> P2 {
    [a[0] * k, a[1] * k]
}

pub fn dot(a: P2, b: P2) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

pub fn dist(a: P2, b: P2) -> f32 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

pub fn rotate(p: P2, turn: f32) -> P2 {
    let (s, c) = turn.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// Where `p` projects onto the segment `a → b`: the distance along it (clamped to the segment)
/// and the signed distance to its left.
pub fn project(p: P2, a: P2, b: P2) -> (f32, f32) {
    let ab = sub(b, a);
    let len = dot(ab, ab).sqrt().max(1e-6);
    let d = scale(ab, 1.0 / len);
    let ap = sub(p, a);
    (dot(ap, d).clamp(0.0, len), ap[0] * -d[1] + ap[1] * d[0])
}

/// Distance from `p` to the segment `a → b`.
pub fn segment_distance(p: P2, a: P2, b: P2) -> f32 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab).max(1e-12);
    let t = (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0);
    dist(p, add(a, scale(ab, t)))
}

pub fn inside(p: P2, poly: &[P2]) -> bool {
    let mut hit = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            hit = !hit;
        }
    }
    hit
}

/// Signed area: positive for a counter-clockwise outline.
pub fn area(poly: &[P2]) -> f32 {
    let n = poly.len();
    (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum::<f32>() / 2.0
}

pub fn centroid(poly: &[P2]) -> P2 {
    let a = area(poly);
    if a.abs() < 1e-6 {
        let n = poly.len().max(1) as f32;
        return scale(poly.iter().fold([0.0, 0.0], |s, p| add(s, *p)), 1.0 / n);
    }
    let n = poly.len();
    let mut c = [0.0, 0.0];
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let k = p[0] * q[1] - q[0] * p[1];
        c[0] += (p[0] + q[0]) * k;
        c[1] += (p[1] + q[1]) * k;
    }
    scale(c, 1.0 / (6.0 * a))
}

pub fn snap(v: f32, step: f32) -> f32 {
    if step > 0.0 { (v / step).round() * step } else { v }
}

pub fn snap2(p: P2, step: f32) -> P2 {
    [snap(p[0], step), snap(p[1], step)]
}

/// The four corners of a `w × d` rectangle centred on `at`, turned by `turn`.
pub fn oriented_rect(at: P2, w: f32, d: f32, turn: f32) -> [P2; 4] {
    [[-w / 2.0, -d / 2.0], [w / 2.0, -d / 2.0], [w / 2.0, d / 2.0], [-w / 2.0, d / 2.0]].map(|c| add(at, rotate(c, turn)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_and_containment() {
        let (along, left) = project([1.0, 1.0], [0.0, 0.0], [4.0, 0.0]);
        assert!((along - 1.0).abs() < 1e-6 && (left - 1.0).abs() < 1e-6);
        let l = [[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 3.0], [0.0, 3.0]];
        assert!(inside([0.5, 2.0], &l) && !inside([2.0, 2.0], &l));
        assert!((area(&l) - 6.0).abs() < 1e-5);
    }
}
