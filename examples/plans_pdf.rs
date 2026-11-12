//! Writes the sample house's plans and elevations as a PDF: `cargo run --release --example plans_pdf -- out.pdf [a4|a3|letter|tabloid]`.

use lintel::drawing::{Paper, Sheet, drawings};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a path to write");
    let paper = match args.next().as_deref() {
        Some("a4") => Paper::A4,
        Some("letter") => Paper::Letter,
        Some("tabloid") => Paper::Tabloid,
        _ => Paper::A3,
    };
    std::fs::write(&path, drawings(&lintel::sample::house(), Sheet { paper, scale: None })).expect("written");
}
