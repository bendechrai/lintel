//! Wall-clock timing that also works in a browser.

pub type Instant = rune::time::Instant;

pub fn now() -> Instant {
    Instant::now()
}

/// Seconds since `start`.
pub fn since(start: Instant) -> f32 {
    start.elapsed().as_secs_f32()
}
